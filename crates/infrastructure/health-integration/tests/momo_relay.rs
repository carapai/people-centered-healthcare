//! MoMo for an edge's facility, relayed by central (#124): the edge records the payment in its
//! own ledger and runs the same flow, central sends the request once with the facility's keys and
//! reads MTN's status, keeping no copy of the payment, and answers only the facility that made it.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Extensions, HeaderMap, Method, Request, Uri};
use health_financial_ledger::{ChartTemplate, Currency, IN_TRANSIT};
use health_identifiers::{NodeId, NumberFormat};
use health_integration::mobile_money::{
    CentralLink, Collections, MomoAccounts, MomoError, MomoService, MtnConfig, MtnMomo,
    RelayTransport, Settled,
};
use health_payments::{Method as PaymentMethod, NewPayment, Purpose, Status};
use health_payments_app::{Accounts, Payments, Settings};
use health_surrealdb::{
    Connection, MIGRATIONS, Settings as Db, SurrealStore, SyncScope, Target, migrate,
};
use health_sync_server::{AuthenticatedPeer, PeerAuth, PeerRefused};
use health_time::{Clock, SignedDuration, SystemClock};
use http_body_util::BodyExt as _;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use tower::ServiceExt as _;

mod common;

use common::Mock;

const MERCHANT: &str = "cash/merchant-mtn";

/// The edges, as central's sync authenticates them: here, by the facility they name.
struct ByFacility;

impl PeerAuth for ByFacility {
    fn authenticate<'a>(
        &'a self,
        _: &'a Method,
        _: &'a Uri,
        headers: &'a HeaderMap,
        _: &'a Extensions,
        _: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<AuthenticatedPeer, PeerRefused>> + Send + 'a>> {
        let peer = headers
            .get("x-test-facility")
            .and_then(|v| v.to_str().ok())
            .map(|facility| AuthenticatedPeer {
                id: NodeId::new(),
                scope: SyncScope::Facility(facility.to_owned()),
            })
            .ok_or_else(|| PeerRefused::Unauthorized("not signed".into()));
        Box::pin(std::future::ready(peer))
    }
}

/// An edge's signed link to central, in process.
#[derive(Clone)]
struct Link {
    central: Router,
    facility: Option<&'static str>,
}

impl CentralLink for Link {
    async fn post(&self, path: &str, body: Value) -> Result<(u16, Value), MomoError> {
        let mut request = Request::builder().method("POST").uri(path);
        if let Some(facility) = self.facility {
            request = request.header("x-test-facility", facility);
        }
        let response = self
            .central
            .clone()
            .oneshot(request.body(Body::from(body.to_string())).expect("request"))
            .await
            .map_err(|_| MomoError::Unreachable)?;
        let status = response.status().as_u16();
        let bytes = response
            .into_body()
            .collect()
            .await
            .map_err(|_| MomoError::Unreachable)?
            .to_bytes();
        Ok((
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        ))
    }
}

fn settings(facility: &str) -> Result<Settings, String> {
    Ok(Settings {
        facility: facility.parse().map_err(|_| "facility".to_owned())?,
        node: "E1".parse().expect("node"),
        currency: Currency::from_code("UGX").expect("currency"),
        chart: ChartTemplate {
            funding_sources: vec!["patient".into()],
            tills: vec!["merchant-mtn".into()],
            ..ChartTemplate::default()
        },
        accounts: Accounts {
            receivable: "receivable/patient".into(),
            merchants: vec![(PaymentMethod::MtnMomo, MERCHANT.into())],
        },
        receipts: NumberFormat {
            padding: 6,
            ..NumberFormat::default()
        },
    })
}

async fn store(namespace: &str) -> SurrealStore {
    let connection = Connection::open(&Db {
        target: Target::Memory,
        namespace: namespace.into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

struct World {
    mock: Mock,
    central: Router,
    central_store: SurrealStore,
    edge: Collections<RelayTransport<Link>>,
    edge_store: SurrealStore,
}

async fn world() -> World {
    let central_store = store("central").await;
    let mock = Mock::new();
    let accounts = MomoAccounts::parse(
        &json!({
            "base_url": "https://sandbox.momodeveloper.mtn.com",
            "facilities": { "KAM013": { "subscription_key": "kam", "api_user": "user",
                "api_key": "key", "environment": "sandbox", "currency": "UGX" } }
        })
        .to_string(),
    )
    .expect("accounts");
    let service = Arc::new(
        MomoService::new(
            central_store.clone(),
            &accounts,
            mock.clone(),
            settings,
            Arc::new(SystemClock) as Arc<dyn Clock>,
        )
        .expect("service"),
    );
    let central = service.relay_routes(Arc::new(ByFacility));
    let edge_store = store("edge").await;
    // The edge holds no MTN keys: its account names nothing secret.
    let relayed = MtnMomo::new(
        RelayTransport::new(Link {
            central: central.clone(),
            facility: Some("KAM013"),
        }),
        MtnConfig {
            subscription_key: String::new(),
            api_user: String::new(),
            api_key: String::new(),
            environment: "relay".into(),
            currency: "UGX".into(),
            callback_url: None,
        },
    );
    let edge = Collections::new(
        edge_store.clone(),
        settings("KAM013").expect("settings"),
        relayed,
        Arc::new(SystemClock) as Arc<dyn Clock>,
        SignedDuration::from_hours(1),
    );
    World {
        mock,
        central,
        central_store,
        edge,
        edge_store,
    }
}

fn new(id: &'static str) -> NewPayment<'static> {
    NewPayment {
        id,
        method: PaymentMethod::MtnMomo,
        purpose: Purpose::Settlement,
        amount: Decimal::from(8_000),
        payer: "Account/acc-1",
    }
}

#[tokio::test]
async fn an_edge_collects_through_central_into_its_own_ledger() {
    let w = world().await;
    let payment = w
        .edge
        .initiate(&new("pay-edge"), "256772000001", "OPD", None)
        .await
        .expect("initiate");
    assert_eq!(payment.status, Status::Pending);
    let reference = payment.reference.clone().expect("reference");
    assert_eq!(
        w.mock.count("POST /collection/v1_0/requesttopay"),
        1,
        "central sent it"
    );

    // Central keeps no copy of the edge's payment.
    let central = Payments::new(
        w.central_store.clone(),
        settings("KAM013").expect("settings"),
    );
    assert!(central.pending().await.expect("pending").is_empty());

    // The payer pays; the edge's poll, through central, confirms it in the edge's ledger.
    w.mock.answer(&reference, "SUCCESSFUL", "8000");
    assert_eq!(
        w.edge.poll().await.expect("poll"),
        vec![("pay-edge".to_owned(), Settled::Confirmed)]
    );
    let edge = Payments::new(w.edge_store.clone(), settings("KAM013").expect("settings"));
    assert_eq!(
        edge.balances(&[IN_TRANSIT, MERCHANT])
            .await
            .expect("balances"),
        vec![
            (IN_TRANSIT.to_owned(), Decimal::ZERO),
            (MERCHANT.to_owned(), Decimal::from(8_000))
        ]
    );
}

#[tokio::test]
async fn central_sends_a_reference_once_and_answers_only_its_facility() {
    let w = world().await;
    let payment = w
        .edge
        .initiate(&new("pay-edge"), "256772000001", "OPD", None)
        .await
        .expect("initiate");
    let reference = payment.reference.expect("reference");
    let request = json!({ "reference": reference, "payment": "pay-edge", "amount": "8000",
                          "msisdn": "256772000001", "note": "OPD" });
    let kam = Link {
        central: w.central.clone(),
        facility: Some("KAM013"),
    };
    let gul = Link {
        central: w.central.clone(),
        facility: Some("GUL002"),
    };
    let anonymous = Link {
        central: w.central.clone(),
        facility: None,
    };

    // A repeat is not sent again.
    let (status, answer) = kam
        .post("/sync/v1/momo/requests", request.clone())
        .await
        .expect("repeat");
    assert_eq!(
        (status, answer["initiated"].as_str()),
        (200, Some("unknown"))
    );
    assert_eq!(w.mock.count("POST /collection/v1_0/requesttopay"), 1);

    // Another facility can neither read nor reuse it; an unsigned caller gets nothing.
    let (status, _) = gul
        .post("/sync/v1/momo/status", json!({ "reference": reference }))
        .await
        .expect("status");
    assert_eq!(status, 404);
    let (status, _) = gul
        .post("/sync/v1/momo/requests", request.clone())
        .await
        .expect("reuse");
    assert_eq!(status, 422);
    let (status, _) = anonymous
        .post("/sync/v1/momo/status", json!({ "reference": reference }))
        .await
        .expect("anonymous");
    assert_eq!(status, 401);

    // Its own facility reads MTN's status, in MTN's shape.
    let (status, answer) = kam
        .post("/sync/v1/momo/status", json!({ "reference": reference }))
        .await
        .expect("status");
    assert_eq!((status, answer["status"].as_str()), (200, Some("PENDING")));
}
