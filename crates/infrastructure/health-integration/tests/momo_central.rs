//! MTN MoMo at central (#124): each facility collects into its own merchant account, a till
//! initiates for its session's facility, MTN's public callback only triggers a status read for
//! the facility the reference was made for, and the poller settles every facility's payments.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use health_access_policy::{Permissions, Subject};
use health_financial_ledger::{ChartTemplate, Currency, IN_TRANSIT};
use health_identifiers::NumberFormat;
use health_integration::mobile_money::{MomoAccounts, MomoService, Settled};
use health_payments::{Evidence, Method, NewPayment, Purpose, Status};
use health_payments_app::{Accounts, Payments, Settings};
use health_surrealdb::{Connection, MIGRATIONS, Settings as Db, SurrealStore, Target, migrate};
use health_time::{Clock, SystemClock};
use http_body_util::BodyExt as _;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use tower::ServiceExt as _;

mod common;

use common::Mock;

const MERCHANT: &str = "cash/merchant-mtn";

fn accounts() -> MomoAccounts {
    let account = |key: &str| {
        json!({ "subscription_key": key, "api_user": "user", "api_key": "secret-api-key",
                "environment": "sandbox", "currency": "UGX" })
    };
    MomoAccounts::parse(
        &json!({
            "base_url": "https://sandbox.momodeveloper.mtn.com",
            "callback_url": "https://central.example/api/payments/momo/callback",
            "pending_minutes": 30,
            "facilities": { "KAM013": account("kam"), "GUL002": account("gul") }
        })
        .to_string(),
    )
    .expect("accounts")
}

fn settings(facility: &str) -> Result<Settings, String> {
    Ok(Settings {
        facility: facility.parse().map_err(|_| "facility".to_owned())?,
        node: "C1".parse().expect("node"),
        currency: Currency::from_code("UGX").expect("currency"),
        chart: ChartTemplate {
            funding_sources: vec!["patient".into()],
            tills: vec!["merchant-mtn".into()],
            ..ChartTemplate::default()
        },
        accounts: Accounts {
            receivable: "receivable/patient".into(),
            merchants: vec![(Method::MtnMomo, MERCHANT.into())],
        },
        receipts: NumberFormat {
            padding: 6,
            ..NumberFormat::default()
        },
    })
}

async fn service() -> (Arc<MomoService<Mock>>, Mock, SurrealStore) {
    let connection = Connection::open(&Db {
        target: Target::Memory,
        namespace: "momo-central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    let mock = Mock::new();
    let service = MomoService::new(
        store.clone(),
        &accounts(),
        mock.clone(),
        settings,
        Arc::new(SystemClock) as Arc<dyn Clock>,
    )
    .expect("service");
    (Arc::new(service), mock, store)
}

fn as_(service: &Arc<MomoService<Mock>>, facility: &str, permissions: &[&str]) -> Router {
    service.routes().layer(axum::Extension(Subject {
        account: "cashier-1".into(),
        facility: facility.into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: Some("PAY".into()),
        break_glass: None,
    }))
}

async fn send(app: &Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
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

async fn balances(store: &SurrealStore, facility: &str) -> (Decimal, Decimal) {
    let found = Payments::new(store.clone(), settings(facility).expect("settings"))
        .balances(&[IN_TRANSIT, MERCHANT])
        .await
        .expect("balances");
    (found[0].1, found[1].1)
}

#[test]
fn the_accounts_file_is_checked_and_its_keys_are_never_printed() {
    let accounts = accounts();
    assert_eq!(accounts.facilities.len(), 2);
    let printed = format!("{accounts:?}");
    assert!(!printed.contains("secret-api-key") && !printed.contains("\"kam\""));
    assert!(
        MomoAccounts::parse(
            &json!({ "base_url": "http://plain.example", "facilities": {} }).to_string()
        )
        .is_err(),
        "https only"
    );
}

#[tokio::test]
async fn a_till_collects_into_its_own_facility_and_the_callback_settles_it_there() {
    let (service, mock, store) = service().await;
    let cashier = as_(&service, "KAM013", &["payments.cash"]);
    let (status, created) = send(
        &cashier,
        "POST",
        "/momo",
        json!({ "amount": "15000", "payer": "Account/acc-1", "msisdn": "+256 772 000001" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["status"], "pending");
    let reference = created["reference"].as_str().expect("reference").to_owned();
    assert_eq!(
        balances(&store, "KAM013").await,
        (Decimal::from(15_000), Decimal::ZERO)
    );

    // MTN calls back. The body is ignored: the status is read from MTN.
    mock.answer(&reference, "SUCCESSFUL", "15000");
    let public = service.callback_route();
    let (status, _) = send(
        &public,
        "POST",
        &format!("/momo/callback?reference={reference}"),
        json!({ "status": "FAILED" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        balances(&store, "KAM013").await,
        (Decimal::ZERO, Decimal::from(15_000))
    );
    assert_eq!(
        balances(&store, "GUL002").await,
        (Decimal::ZERO, Decimal::ZERO)
    );
    let payment = Payments::new(store.clone(), settings("KAM013").expect("settings"))
        .payment(created["id"].as_str().expect("id"))
        .await
        .expect("payment");
    assert_eq!(payment.status, Status::Posted);
    assert_eq!(payment.evidence, Some(Evidence::Callback));

    // A callback for a reference nobody made, or a malformed one, tells the caller nothing.
    for path in [
        "/momo/callback?reference=00000000-0000-4000-8000-000000000000",
        "/momo/callback?reference=%3Cscript%3E",
        "/momo/callback",
    ] {
        let (status, body) = send(&public, "POST", path, Value::Null).await;
        assert_eq!((status, body), (StatusCode::OK, Value::Null), "{path}");
    }
}

#[tokio::test]
async fn the_poller_settles_each_facility_in_its_own_ledger() {
    let (service, mock, store) = service().await;
    let new = |id: &'static str| NewPayment {
        id,
        method: Method::MtnMomo,
        purpose: Purpose::Settlement,
        amount: Decimal::from(5_000),
        payer: "Account/acc-1",
    };
    let kam = service
        .collections("KAM013")
        .expect("KAM013")
        .initiate(&new("pay-kam"), "256772000001", "OPD", None)
        .await
        .expect("kam");
    let gul = service
        .collections("GUL002")
        .expect("GUL002")
        .initiate(&new("pay-gul"), "256772000002", "OPD", None)
        .await
        .expect("gul");
    mock.answer(
        kam.reference.as_deref().expect("reference"),
        "SUCCESSFUL",
        "5000",
    );
    mock.answer(
        gul.reference.as_deref().expect("reference"),
        "FAILED",
        "5000",
    );
    let settled = service.poll().await;
    assert!(
        settled.contains(&("pay-kam".to_owned(), Settled::Confirmed)),
        "{settled:?}"
    );
    assert!(
        settled
            .iter()
            .any(|(id, s)| id == "pay-gul" && matches!(s, Settled::Voided(_))),
        "{settled:?}"
    );
    assert_eq!(settled.len(), 2, "each polled once, by its own facility");
    assert_eq!(
        balances(&store, "KAM013").await,
        (Decimal::ZERO, Decimal::from(5_000))
    );
    assert_eq!(
        balances(&store, "GUL002").await,
        (Decimal::ZERO, Decimal::ZERO)
    );
}

#[tokio::test]
async fn the_routes_need_the_permissions_and_an_account() {
    let (service, _, store) = service().await;
    let body = json!({ "amount": "100", "payer": "Account/a", "msisdn": "256772000001" });
    let (status, _) = send(&as_(&service, "KAM013", &[]), "POST", "/momo", body.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &as_(&service, "MBR001", &["payments.cash"]),
        "POST",
        "/momo",
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no merchant account there");
    let (status, _) = send(
        &as_(&service, "KAM013", &["payments.cash"]),
        "POST",
        "/momo",
        json!({ "amount": "100", "payer": "Account/a", "msisdn": "12" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "not a phone number"
    );

    // The exceptions queue is a supervisor's.
    store
        .open_payment_exception("unknown", "ref-1", None, None, Some("500"))
        .await
        .expect("exception");
    let (status, _) = send(
        &as_(&service, "KAM013", &["payments.cash"]),
        "GET",
        "/exceptions",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let supervisor = as_(&service, "KAM013", &["payments.supervise"]);
    let (status, listed) = send(&supervisor, "GET", "/exceptions", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let id = listed[0]["id"].as_str().expect("id").to_owned();
    let (status, _) = send(
        &supervisor,
        "POST",
        &format!("/exceptions/{id}/resolve"),
        json!({ "note": "a walk-in deposit, matched by hand" }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, listed) = send(&supervisor, "GET", "/exceptions", Value::Null).await;
    assert_eq!(listed, json!([]));
}
