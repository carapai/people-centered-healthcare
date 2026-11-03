//! Provisioning an edge from central (#268; ADRs 0048, 0058), over SurrealDB in memory with a real
//! root and issuing CA: a code redeems once, before it expires, for an encrypted edge unless
//! the facility is exempt; the node gets its id and a short code never reused; and the chain it
//! gets serves `<node-id>.local` under the national root.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use health_identity::{CodeRefused, Encryption, NodeStatus};
use health_pki::{Authority, IssuingCa, Trust, new_key, request};
use health_provisioning::{Error, Issuer, NodeRegistry, router};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, SignedDuration, Timestamp};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const FACILITY: &str = "ug-kla-mulago";
const NAME: &str = "mulago.facilities.health.go.ug";

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

struct Central {
    registry: NodeRegistry,
    clock: Arc<FixedClock>,
    issuer: Arc<Authority>,
    trust: Trust,
}

async fn central() -> Central {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock = Arc::new(FixedClock::new(at("2026-11-03T09:00:00Z")));
    let root = Authority::root("Uganda Health Root CA", at("2026-01-01T00:00:00Z")).expect("root");
    let key = new_key().expect("key");
    let issued = root
        .issue_issuing_ca(
            &request(&key, "issuing").expect("request"),
            &IssuingCa {
                common_name: "Uganda Facility Issuing CA 1".into(),
                dns_suffixes: vec!["facilities.health.go.ug".into()],
            },
            at("2026-01-01T00:00:00Z"),
        )
        .expect("issuing CA");
    Central {
        registry: NodeRegistry::new(
            SurrealStore::new(connection, clock.clone()),
            clock.clone(),
            "central",
        ),
        clock,
        issuer: Arc::new(
            Authority::from_pem(&issued.pem(), &key.serialize_pem()).expect("issuing CA"),
        ),
        trust: Trust::new(&root.certificate_pem()).expect("trust"),
    }
}

/// An edge's request: the intermediate's key stays with it.
fn edge() -> (health_pki::KeyPair, String) {
    let key = new_key().expect("key");
    let csr = request(&key, "edge").expect("request");
    (key, csr)
}

#[tokio::test]
async fn an_edge_is_provisioned_from_a_one_time_code() {
    let central = central().await;
    let (text, code) = central
        .registry
        .issue_code(FACILITY, vec![NAME.into()], None, "ops:auma")
        .await
        .expect("code");
    assert!(text.len() == 9 && text.as_bytes()[4] == b'-', "{text}");
    assert_eq!(code.expires, at("2026-11-06T09:00:00Z"));

    let (key, csr) = edge();
    // Typed by hand: lower case, a space for the dash.
    let typed = text.to_lowercase().replace('-', " ");
    let provisioned = central
        .registry
        .redeem(&typed, &csr, true, central.issuer.as_ref())
        .await
        .expect("provisioned");
    let node = &provisioned.node;
    assert_eq!(
        (
            node.facility.as_str(),
            node.short_code.as_str(),
            node.status
        ),
        (FACILITY, "E1", NodeStatus::Active)
    );
    assert_eq!(node.encryption, Encryption::Compliant);
    assert_eq!(node.code, code.id);

    // The chain: an intermediate for this node, under the issuing CA, under the root. Its leaf
    // serves `<node-id>.local` and the facility's name.
    let intermediate = Authority::from_pem(&provisioned.chain_pem, &key.serialize_pem())
        .expect("the intermediate matches the edge's key");
    let leaf_key = new_key().expect("key");
    let node_name = format!("{}.local", node.id);
    let leaf = intermediate
        .issue_server(
            &leaf_key,
            &[node_name.clone(), NAME.into()],
            &[],
            at("2026-11-03T09:00:00Z"),
        )
        .expect("leaf");
    let served = format!("{}{}", leaf.pem(), provisioned.chain_pem);
    central
        .trust
        .verify_server(&served, &node_name, at("2026-11-04T09:00:00Z"))
        .expect("the chain serves the node's name");

    // Once only.
    let (_, csr) = edge();
    assert_eq!(
        central
            .registry
            .redeem(&text, &csr, true, central.issuer.as_ref())
            .await,
        Err(Error::Refused(CodeRefused::Used))
    );
    assert_eq!(
        central
            .registry
            .nodes_at(FACILITY)
            .await
            .expect("nodes")
            .len(),
        1
    );
}

#[tokio::test]
async fn codes_are_refused_unknown_expired_or_for_an_unencrypted_edge() {
    let central = central().await;
    let (_, csr) = edge();
    for wrong in ["0000-0000", "not a code"] {
        assert_eq!(
            central
                .registry
                .redeem(wrong, &csr, true, central.issuer.as_ref())
                .await,
            Err(Error::Refused(CodeRefused::Unknown)),
            "{wrong}"
        );
    }
    let (text, _) = central
        .registry
        .issue_code(FACILITY, vec![NAME.into()], None, "ops:auma")
        .await
        .expect("code");
    assert_eq!(
        central
            .registry
            .redeem(&text, &csr, false, central.issuer.as_ref())
            .await,
        Err(Error::Refused(CodeRefused::NotEncrypted))
    );
    // A request the issuing CA refuses leaves the code redeemable.
    assert!(matches!(
        central
            .registry
            .redeem(&text, "not a request", true, central.issuer.as_ref())
            .await,
        Err(Error::Request(_))
    ));
    central.clock.advance(SignedDuration::from_hours(72));
    assert_eq!(
        central
            .registry
            .redeem(&text, &csr, true, central.issuer.as_ref())
            .await,
        Err(Error::Refused(CodeRefused::Expired))
    );
}

#[tokio::test]
async fn an_exempt_facility_provisions_unencrypted_and_stands_as_an_alert() {
    let central = central().await;
    let (text, _) = central
        .registry
        .issue_code(
            FACILITY,
            Vec::new(),
            Some("TPM-less PC pending replacement".into()),
            "ops:auma",
        )
        .await
        .expect("code");
    let (_, csr) = edge();
    let provisioned = central
        .registry
        .redeem(&text, &csr, false, central.issuer.as_ref())
        .await
        .expect("provisioned");
    assert_eq!(
        provisioned.node.encryption,
        Encryption::Exempt {
            reason: "TPM-less PC pending replacement".into()
        }
    );
    let exempt = central.registry.exempt().await.expect("exempt");
    assert_eq!(
        exempt.iter().map(|n| n.id).collect::<Vec<_>>(),
        [provisioned.node.id]
    );
}

#[tokio::test]
async fn short_codes_count_up_per_facility() {
    let central = central().await;
    let mut codes = Vec::new();
    for facility in [FACILITY, FACILITY, "ug-kla-kiruddu", FACILITY] {
        let (text, _) = central
            .registry
            .issue_code(facility, Vec::new(), None, "ops:auma")
            .await
            .expect("code");
        let (_, csr) = edge();
        let provisioned = central
            .registry
            .redeem(&text, &csr, true, central.issuer.as_ref())
            .await
            .expect("provisioned");
        codes.push((facility, provisioned.node.short_code));
    }
    assert_eq!(
        codes,
        [
            (FACILITY, "E1".to_owned()),
            (FACILITY, "E2".to_owned()),
            ("ug-kla-kiruddu", "E1".to_owned()),
            (FACILITY, "E3".to_owned()),
        ]
    );
}

async fn post(app: &Router, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/provisioning/nodes")
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

#[tokio::test]
async fn the_route_answers_an_edge() {
    let central = central().await;
    let issuer: Arc<dyn Issuer> = central.issuer.clone();
    let app = router(central.registry.clone(), issuer);
    let (text, _) = central
        .registry
        .issue_code(FACILITY, vec![NAME.into()], None, "ops:auma")
        .await
        .expect("code");
    let (_, csr) = edge();

    let (status, body) = post(
        &app,
        json!({ "code": text, "request": csr, "encrypted": false }),
    )
    .await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("not_encrypted"))
    );
    let (status, body) = post(
        &app,
        json!({ "code": text, "request": "garbage", "encrypted": true }),
    )
    .await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::BAD_REQUEST, json!("invalid_request"))
    );

    let (status, body) = post(
        &app,
        json!({ "code": text, "request": csr, "encrypted": true }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["short_code"], "E1");
    assert_eq!(body["facility"], FACILITY);
    assert!(
        body["chain"]
            .as_str()
            .is_some_and(|c| c.matches("BEGIN CERTIFICATE").count() == 2)
    );

    let (status, body) = post(
        &app,
        json!({ "code": text, "request": csr, "encrypted": true }),
    )
    .await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::FORBIDDEN, json!("code_refused"))
    );
}
