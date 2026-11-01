//! Caregiver delegation over HTTP (#113): staff record a caregiver by phone, verified against
//! the document checked, revoke the grant or record an adolescent's consent; the caregiver sees
//! their dependents in the portal.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{PatientSubject, Permissions, Subject};
use health_auth::{AuthStore, CAREGIVER_PERMISSION, CaregiverRoutes, SurrealAuthStore};
use health_consent::Confidentiality;
use health_identifiers::OpId;
use health_identity::{AccountId, AccountStatus, PhoneAccount, PhoneNumber, SessionPolicy};
use health_persistence::{Precondition, RawJson, Store, Transaction, WriteRequest};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const CHILD: &str = "018f0000-0000-7000-8000-0000000000bb";

async fn setup() -> (CaregiverRoutes<SurrealAuthStore>, String) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "caregivers".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    // The dependent, as the registry holds her.
    let mut tx = store.begin().await.expect("tx");
    tx.write(WriteRequest {
        resource_type: "Patient".parse().expect("type"),
        id: CHILD.parse().expect("id"),
        json: RawJson::from_validated(
            json!({ "resourceType": "Patient", "id": CHILD, "name": [{ "family": "Nakato", "given": ["Amina"] }], "birthDate": "2013-02-02" })
                .to_string(),
        ),
        op_id: OpId::new(),
        precondition: Precondition::MustNotExist,
    })
    .await
    .expect("patient");
    tx.commit().await.expect("commit");
    // The caregiver signed in to the portal once, so her phone has an account.
    let accounts = SurrealAuthStore::new(store.clone(), SessionPolicy::default().hard_cap);
    let account = accounts
        .create_phone_account(PhoneAccount {
            id: AccountId(uuid::Uuid::now_v7()),
            phone: PhoneNumber::parse("0772100009", "256").expect("phone"),
            status: AccountStatus::Active,
            verified_at: Timestamp::now(),
        })
        .await
        .expect("account");
    let routes = CaregiverRoutes::new(
        store,
        accounts,
        None,
        Confidentiality {
            sensitive: vec![("urn:test".into(), "HIV".into())],
            ..Confidentiality::default()
        },
        "256",
    );
    (routes, account.id.to_string())
}

fn staff(permissions: &[&str]) -> Subject {
    Subject {
        account: "clerk-1".into(),
        facility: "KAM013".into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    }
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

fn grant_body() -> Value {
    json!({
        "patient": CHILD,
        "phone": "0772 100009",
        "relationship": "MTH",
        "authority": "guardian",
        "scopes": ["view-records", "book-appointments"],
        "expires": "2031-02-01",
        "document": "birth-certificate",
    })
}

#[tokio::test]
async fn staff_record_revoke_and_consent_and_the_caregiver_sees_her_dependent() {
    let (routes, account) = setup().await;
    let staff_app = routes
        .caregiver_router()
        .layer(Extension(staff(&[CAREGIVER_PERMISSION])));
    let (status, grant) = send(&staff_app, "POST", "/", Some(grant_body())).await;
    assert_eq!(status, StatusCode::CREATED, "{grant}");
    assert_eq!(grant["caregiver"], account.as_str());
    assert_eq!(grant["status"], "active");
    assert_eq!(grant["verified"], "birth-certificate");
    let (status, _) = send(&staff_app, "POST", "/", Some(grant_body())).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, listed) = send(&staff_app, "GET", &format!("/?patient={CHILD}"), None).await;
    assert_eq!(listed.as_array().map(Vec::len), Some(1));

    // The caregiver, in the portal: her daughter, with her sensitive records withheld.
    let portal = routes.dependents_router().layer(Extension(PatientSubject {
        account: account.clone(),
        phone: "+256772100009".into(),
    }));
    let (status, mine) = send(&portal, "GET", "/dependents", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(mine[0]["name"], "Amina Nakato");
    assert_eq!(mine[0]["confidential_withheld"], true);

    let id = grant["id"].as_str().expect("id");
    let (status, consented) = send(
        &staff_app,
        "POST",
        &format!("/{id}/adolescent-consent"),
        Some(json!({ "consents": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(consented["adolescent_consents"], true);
    let (_, mine) = send(&portal, "GET", "/dependents", None).await;
    assert_eq!(mine[0]["confidential_withheld"], false);

    let (status, revoked) = send(&staff_app, "POST", &format!("/{id}/revoke"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(revoked["status"], "revoked");
    let (_, mine) = send(&portal, "GET", "/dependents", None).await;
    assert_eq!(mine, json!([]), "a revoked grant is gone from the portal");
}

#[tokio::test]
async fn caregivers_are_managed_only_by_permitted_staff_with_complete_details() {
    let (routes, _) = setup().await;
    let (status, _) = send(
        &routes.caregiver_router(),
        "GET",
        &format!("/?patient={CHILD}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let clerk = routes
        .caregiver_router()
        .layer(Extension(staff(&["Patient.crs"])));
    let (status, _) = send(&clerk, "POST", "/", Some(grant_body())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = routes
        .caregiver_router()
        .layer(Extension(staff(&[CAREGIVER_PERMISSION])));
    for (field, value) in [
        ("phone", json!("0772 999999")),
        ("authority", json!("aunt")),
        ("scopes", json!(["everything"])),
        ("expires", json!("next year")),
        ("document", json!("")),
    ] {
        let mut body = grant_body();
        body[field] = value;
        let (status, outcome) = send(&app, "POST", "/", Some(body)).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{field}: {outcome}"
        );
    }
    let (status, _) = send(&app, "POST", "/no-such/revoke", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(&routes.dependents_router(), "GET", "/dependents", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
