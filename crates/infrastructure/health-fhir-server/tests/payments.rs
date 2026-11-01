//! Payments and cashier shifts over HTTP (#254): a cashier's shift, a provisional mobile-money
//! receipt confirmed by a supervisor's statement, a supervised shortage, and the permissions.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::Unrestricted;
use health_fhir_server::{
    CASH_PERMISSION, FhirServer, PaymentConfig, REFUND_PERMISSION, SUPERVISE_PERMISSION,
};
use health_financial_ledger::Currency;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SignedDuration, SystemClock};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn server() -> FhirServer {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "payments-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    FhirServer::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        "http://test/fhir",
        Arc::new(Unrestricted),
    )
    .with_payments(PaymentConfig {
        currency: Currency::from_code("UGX").expect("currency"),
        tills: vec!["till-1".into()],
        node: "C1".into(),
        pending_for: SignedDuration::from_hours(24),
    })
}

fn as_(server: &FhirServer, account: &str, permissions: &[&str]) -> Router {
    server.payments_router().layer(Extension(Subject {
        account: account.into(),
        facility: "KAM013".into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    }))
}

async fn send(app: &Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(body.map_or(Body::empty(), |b| Body::from(b.to_string())))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn a_cashiers_shift_with_a_provisional_receipt_confirmed_by_the_statement() {
    let server = server().await;
    let cashier = as_(&server, "cashier-1", &[CASH_PERMISSION]);
    let supervisor = as_(
        &server,
        "supervisor-1",
        &[CASH_PERMISSION, SUPERVISE_PERMISSION],
    );

    let (status, shift) = send(
        &cashier,
        "POST",
        "/shifts",
        Some(json!({ "till": "till-1", "float": "0" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{shift}");
    let id = shift["id"].as_str().expect("id").to_owned();
    let (_, open) = send(&cashier, "GET", "/shifts/open?till=till-1", None).await;
    assert_eq!(open["id"], id.as_str());
    assert_eq!(
        send(
            &cashier,
            "POST",
            "/shifts",
            Some(json!({ "till": "till-1" }))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );

    let (status, cash) = send(
        &cashier,
        "POST",
        "/cash",
        Some(json!({ "shift": id, "amount": "30000", "payer": "Account/acc-1" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{cash}");
    assert_eq!(cash["receipt"]["status"], "confirmed");
    assert!(
        cash["receipt"]["number"]
            .as_str()
            .expect("number")
            .starts_with("RCT-KAM013-C1-")
    );

    let mobile = json!({ "shift": id, "method": "mtn-momo", "amount": "20000", "payer": "Account/acc-1", "reference": "x-ref-1" });
    let (status, momo) = send(&cashier, "POST", "/pending", Some(mobile.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{momo}");
    assert_eq!(momo["receipt"]["status"], "provisional");
    assert_eq!(
        send(&cashier, "POST", "/pending", Some(mobile)).await.0,
        StatusCode::CONFLICT,
        "reference used once"
    );

    // Only a supervisor confirms by statement.
    let line = json!({ "lines": [{ "reference": "x-ref-1", "amount": "20000" }] });
    assert_eq!(
        send(&cashier, "POST", "/statements", Some(line.clone()))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, result) = send(&supervisor, "POST", "/statements", Some(line)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["confirmed"][0], momo["id"]);
    let (_, listed) = send(&cashier, "GET", &format!("/shifts/{id}"), None).await;
    let statuses: Vec<&str> = listed["payments"]
        .as_array()
        .expect("payments")
        .iter()
        .filter_map(|p| p["receipt"]["status"].as_str())
        .collect();
    assert_eq!(statuses, ["confirmed", "confirmed"]);

    // Short by 1,000: the cashier cannot approve it; the supervisor can.
    let (_, closed) = send(
        &cashier,
        "POST",
        &format!("/shifts/{id}/close"),
        Some(json!({ "counted": "29000" })),
    )
    .await;
    assert_eq!(
        (closed["status"].as_str(), closed["variance"].as_str()),
        (Some("awaiting-approval"), Some("-1000"))
    );
    assert_eq!(
        send(&cashier, "POST", &format!("/shifts/{id}/approve"), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, settled) = send(&supervisor, "POST", &format!("/shifts/{id}/approve"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(settled["status"], "settled");
    let (_, balances) = send(&supervisor, "GET", "/balances", None).await;
    assert_eq!(balances["cash/till-1"], "29000");
    assert_eq!(balances["cash/merchant-mtn"], "20000");
    assert_eq!(balances["clearing/in-transit"], "0");
    assert_eq!(balances["variance/till"], "1000");
}

#[tokio::test]
async fn refunds_and_voids_need_their_permissions() {
    let server = server().await;
    let cashier = as_(&server, "cashier-1", &[CASH_PERMISSION]);
    let (_, shift) = send(
        &cashier,
        "POST",
        "/shifts",
        Some(json!({ "till": "till-1" })),
    )
    .await;
    let id = shift["id"].as_str().expect("id");
    let (_, cash) = send(
        &cashier,
        "POST",
        "/cash",
        Some(json!({ "shift": id, "amount": "5000", "payer": "Account/acc-1" })),
    )
    .await;
    let pay = cash["id"].as_str().expect("payment");
    assert_eq!(
        send(
            &cashier,
            "POST",
            &format!("/payments/{pay}/refund"),
            Some(json!({ "amount": "1000" }))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let refunder = as_(&server, "cashier-1", &[CASH_PERMISSION, REFUND_PERMISSION]);
    let (status, refunded) = send(
        &refunder,
        "POST",
        &format!("/payments/{pay}/refund"),
        Some(json!({ "amount": "1000" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{refunded}");
    assert_eq!(refunded["refunded"], "1000");
    let (_, pending) = send(&cashier, "POST", "/pending", Some(json!({ "method": "card", "amount": "700", "payer": "Account/acc-1", "reference": "c-1" }))).await;
    let card = pending["id"].as_str().expect("card");
    assert_eq!(
        send(
            &cashier,
            "POST",
            &format!("/payments/{card}/void"),
            Some(json!({ "reason": "declined" }))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &cashier,
            "POST",
            "/cash",
            Some(json!({ "shift": id, "amount": "free", "payer": "x" }))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(
            &cashier,
            "POST",
            "/pending",
            Some(json!({ "method": "cheque", "amount": "1", "payer": "x", "reference": "r" }))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let none = FhirServer::new(
        server_store().await,
        "http://test/fhir",
        Arc::new(Unrestricted),
    );
    assert_eq!(
        send(
            &as_(&none, "c", &[CASH_PERMISSION]),
            "GET",
            "/balances",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

async fn server_store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "payments-none".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}
