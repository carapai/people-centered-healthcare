//! Inventory over HTTP (#130): goods received with their cost, stock on hand, reorder, GS1 and
//! requisitions, for staff with `stock.manage`.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::Unrestricted;
use health_fhir_server::{FhirServer, STOCK_PERMISSION};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn app(permissions: &[&str]) -> Router {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "stock-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    FhirServer::new(store, "http://test/fhir", Arc::new(Unrestricted))
        .inventory_router()
        .layer(Extension(Subject {
            account: "storekeeper-1".into(),
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

fn note() -> Value {
    json!({
        "store": "Location/pharmacy",
        "supplier": "NMS",
        "delivery": "INV-42",
        "lines": [{ "product": "amoxicillin-500", "batch": "B42", "expiry": "2027-06-30", "quantity": 1000, "pack_size": 100, "unit_cost": "1.50" }],
    })
}

#[tokio::test]
async fn a_storekeeper_receives_goods_and_sees_them_on_hand_with_their_cost() {
    let app = app(&[STOCK_PERMISSION]).await;
    let (status, created) = send(&app, "POST", "/goods-received", Some(note())).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let (status, on_hand) = send(&app, "GET", "/on-hand?store=Location%2Fpharmacy", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(on_hand[0]["on_hand"], 1000);
    assert_eq!(on_hand[0]["unit_cost"], "1.50");
    assert_eq!(on_hand[0]["status"], "usable");
    let (status, reorder) = send(
        &app,
        "GET",
        "/reorder?store=Location%2Fpharmacy&days=30",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reorder[0]["order"], 0, "nothing consumed yet");

    let mut bad = note();
    bad["lines"][0]["unit_cost"] = json!("free");
    assert_eq!(
        send(&app, "POST", "/goods-received", Some(bad)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut empty = note();
    empty["lines"] = json!([]);
    assert_eq!(
        send(&app, "POST", "/goods-received", Some(empty)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn gs1_requisitions_and_permissions() {
    let app = app(&[STOCK_PERMISSION]).await;
    let (status, read) = send(
        &app,
        "POST",
        "/gs1",
        Some(json!({ "data": "(01)09501101530003(17)270630(10)B42" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (read["batch"].as_str(), read["expiry"].as_str()),
        (Some("B42"), Some("2027-06-30"))
    );
    assert_eq!(
        send(&app, "POST", "/gs1", Some(json!({ "data": "nonsense" })))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let (status, created) = send(
        &app,
        "POST",
        "/requisitions",
        Some(json!({ "requesting_store": "Location/ward", "supplying_store": "Location/pharmacy", "lines": [{ "product": "amoxicillin-500", "quantity": 200 }] })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["supplyRequests"][0]["status"], "active");
    let id = created["id"].as_str().expect("id");
    let (status, decided) = send(
        &app,
        "POST",
        &format!("/requisitions/{id}/decision"),
        Some(json!({ "approve": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(decided["supplyRequests"][0]["status"], "cancelled");

    let clerk = self::app(&["Patient.crs"]).await;
    assert_eq!(
        send(&clerk, "POST", "/goods-received", Some(note()))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}
