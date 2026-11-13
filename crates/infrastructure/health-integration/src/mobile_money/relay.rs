//! MoMo for a facility whose ledger an edge keeps (#124; billing-and-payments.md: provider
//! integrations run only at central, and an edge asks central to initiate).
//!
//! Central is a **relay**. It holds the facility's MTN keys, sends the request to pay, and reads
//! MTN's status; it keeps no copy of the edge's payment. The edge runs the same [`Collections`]
//! flow over a [`RelayTransport`], so it records the pending payment and its reference in its own
//! ledger first, polls, and confirms or voids there.
//!
//! - `POST /sync/v1/momo/requests` `{reference, payment, amount, msisdn, note}`: sends the request
//!   to pay for the calling edge's facility, once per reference: a repeat is not sent again.
//! - `POST /sync/v1/momo/status` `{reference}`: MTN's status of a reference this facility made.
//!
//! Both are signed by the edge as its sync requests are, and checked by the same [`PeerAuth`].
//!
//! [`Collections`]: super::Collections

use std::future::Future;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{Extensions, HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use health_surrealdb::SyncScope;
use health_sync_server::PeerAuth;
use health_time::Timestamp;
use rust_decimal::Decimal;
use serde_json::{Value, json};

use super::central::MomoService;
use super::mtn::{Initiated, MomoError, MomoTransport, ProviderStatus, Request};

/// The relay's paths.
const REQUESTS: &str = "/sync/v1/momo/requests";
const STATUS: &str = "/sync/v1/momo/status";

/// How an edge reaches central: a signed POST of JSON, its status and JSON answer.
pub trait CentralLink: Send + Sync {
    /// POSTs `body` to central's `path`, signed as this node.
    fn post(
        &self,
        path: &str,
        body: Value,
    ) -> impl Future<Output = Result<(u16, Value), MomoError>> + Send;
}

/// MTN's API as an edge reaches it: through central's relay.
pub struct RelayTransport<C> {
    central: C,
}

impl<C: CentralLink> RelayTransport<C> {
    /// Through `central`.
    pub fn new(central: C) -> Self {
        Self { central }
    }
}

impl<C: CentralLink> MomoTransport for RelayTransport<C> {
    async fn send(&self, request: Request) -> Result<(u16, Value), MomoError> {
        match (request.method, request.path.as_str()) {
            // Central holds the keys: the edge needs no token of its own.
            ("POST", "/collection/token/") => Ok((
                200,
                json!({ "access_token": "relayed", "expires_in": 86_400 }),
            )),
            ("POST", "/collection/v1_0/requesttopay") => {
                let reference = request
                    .headers
                    .iter()
                    .find(|(name, _)| *name == "X-Reference-Id")
                    .map(|(_, value)| value.clone())
                    .ok_or_else(|| {
                        MomoError::Unexpected("a request without its reference".into())
                    })?;
                let body = request.body.unwrap_or(Value::Null);
                let (status, answer) = self
                    .central
                    .post(
                        REQUESTS,
                        json!({
                            "reference": reference,
                            "payment": body["externalId"],
                            "amount": body["amount"],
                            "msisdn": body["payer"]["partyId"],
                            "note": body["payerMessage"],
                        }),
                    )
                    .await?;
                match (status, answer["initiated"].as_str()) {
                    (200, Some("accepted")) => Ok((202, Value::Null)),
                    (200, Some("unknown")) => Err(MomoError::Unreachable),
                    (200, Some("refused")) => Ok((400, answer["reason"].clone())),
                    (status, _) if status >= 500 => Err(MomoError::Unreachable),
                    (status, _) => Ok((status, answer)),
                }
            }
            ("GET", path) => {
                let reference = path
                    .strip_prefix("/collection/v1_0/requesttopay/")
                    .ok_or_else(|| MomoError::Unexpected(format!("no relay for {path}")))?;
                let (status, answer) = self
                    .central
                    .post(STATUS, json!({ "reference": reference }))
                    .await?;
                if status >= 500 {
                    return Err(MomoError::Unreachable);
                }
                Ok((status, answer))
            }
            (method, path) => Err(MomoError::Unexpected(format!(
                "no relay for {method} {path}"
            ))),
        }
    }
}

impl<T: MomoTransport + Clone + 'static> MomoService<T> {
    /// Sends the request to pay `reference` for `facility`'s `payment`, once: a reference already
    /// relayed is not sent again, and its status says where it is.
    pub async fn relay_request(
        &self,
        facility: &str,
        reference: &str,
        payment: &str,
        amount: Decimal,
        msisdn: &str,
        note: &str,
    ) -> Result<Initiated, String> {
        let collections = self
            .collections(facility)
            .ok_or("this facility has no MTN MoMo merchant account")?;
        if let Some(known) = self
            .store()
            .provider_request_facility(reference)
            .await
            .map_err(|e| e.to_string())?
        {
            return if known == facility {
                // Sent before: never twice. The edge's poll reads where it is.
                Ok(Initiated::Unknown)
            } else {
                Err("the reference belongs to another facility".into())
            };
        }
        self.store()
            .put_provider_request(reference, facility, payment, "mtn-momo-relay")
            .await
            .map_err(|e| e.to_string())?;
        Ok(collections
            .mtn()
            .request_to_pay(reference, amount, msisdn, payment, note, Timestamp::now())
            .await)
    }

    /// MTN's status of `reference`, if `facility` made it.
    pub async fn relay_status(
        &self,
        facility: &str,
        reference: &str,
    ) -> Result<Option<ProviderStatus>, String> {
        let owner = self
            .store()
            .provider_request_facility(reference)
            .await
            .map_err(|e| e.to_string())?;
        if owner.as_deref() != Some(facility) {
            return Ok(None);
        }
        let collections = self
            .collections(facility)
            .ok_or("this facility has no MTN MoMo merchant account")?;
        collections
            .mtn()
            .status(reference, Timestamp::now())
            .await
            .map(Some)
            .map_err(|e| e.to_string())
    }

    /// The relay's routes, for edges, authenticated by `auth` as central's sync is.
    pub fn relay_routes(self: &Arc<Self>, auth: Arc<dyn PeerAuth>) -> Router {
        Router::new()
            .route(REQUESTS, post(relay_request::<T>))
            .route(STATUS, post(relay_status::<T>))
            .with_state((Arc::clone(self), auth))
    }
}

type Relay<T> = (Arc<MomoService<T>>, Arc<dyn PeerAuth>);

fn problem(status: StatusCode, detail: &str) -> Response {
    (status, Json(json!({ "detail": detail }))).into_response()
}

/// The calling edge's facility, or the refusal.
async fn facility_of(
    auth: &dyn PeerAuth,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    extensions: &Extensions,
    body: &[u8],
) -> Result<String, Box<Response>> {
    let refusal = |status, detail: &str| Box::new(problem(status, detail));
    match auth
        .authenticate(method, uri, headers, extensions, body)
        .await
    {
        Ok(peer) => match peer.scope {
            SyncScope::Facility(facility) => Ok(facility),
            _ => Err(refusal(
                StatusCode::FORBIDDEN,
                "only a facility's node relays MoMo",
            )),
        },
        Err(refused) => Err(refusal(StatusCode::UNAUTHORIZED, &format!("{refused:?}"))),
    }
}

async fn relay_request<T: MomoTransport + Clone + 'static>(
    State((service, auth)): State<Relay<T>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    extensions: Extensions,
    body: Bytes,
) -> Response {
    let facility =
        match facility_of(auth.as_ref(), &method, &uri, &headers, &extensions, &body).await {
            Ok(facility) => facility,
            Err(response) => return *response,
        };
    let Ok(request) = serde_json::from_slice::<Value>(&body) else {
        return problem(StatusCode::BAD_REQUEST, "the body is not JSON");
    };
    let text = |name: &str| {
        request[name]
            .as_str()
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    let (Some(reference), Some(payment), Some(msisdn), Some(amount)) = (
        text("reference"),
        text("payment"),
        text("msisdn"),
        text("amount").and_then(|a| a.parse::<Decimal>().ok()),
    ) else {
        return problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "reference, payment, msisdn and amount are required",
        );
    };
    match service
        .relay_request(
            &facility,
            reference,
            payment,
            amount,
            msisdn,
            text("note").unwrap_or("Payment"),
        )
        .await
    {
        Ok(Initiated::Accepted) => Json(json!({ "initiated": "accepted" })).into_response(),
        Ok(Initiated::Unknown) => Json(json!({ "initiated": "unknown" })).into_response(),
        Ok(Initiated::Refused(reason)) => {
            Json(json!({ "initiated": "refused", "reason": reason })).into_response()
        }
        Err(why) => problem(StatusCode::UNPROCESSABLE_ENTITY, &why),
    }
}

async fn relay_status<T: MomoTransport + Clone + 'static>(
    State((service, auth)): State<Relay<T>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    extensions: Extensions,
    body: Bytes,
) -> Response {
    let facility =
        match facility_of(auth.as_ref(), &method, &uri, &headers, &extensions, &body).await {
            Ok(facility) => facility,
            Err(response) => return *response,
        };
    let Some(reference) = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|b| b["reference"].as_str().map(str::to_owned))
    else {
        return problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "the reference is required",
        );
    };
    match service.relay_status(&facility, &reference).await {
        // In MTN's own shape, so the edge reads it as it would MTN's.
        Ok(Some(ProviderStatus::Pending)) => Json(json!({ "status": "PENDING" })).into_response(),
        Ok(Some(ProviderStatus::Successful {
            amount,
            transaction,
        })) => Json(json!({
            "status": "SUCCESSFUL",
            "amount": amount.normalize().to_string(),
            "financialTransactionId": transaction,
        }))
        .into_response(),
        Ok(Some(ProviderStatus::Failed { reason })) => {
            Json(json!({ "status": "FAILED", "reason": reason })).into_response()
        }
        Ok(Some(ProviderStatus::NotFound) | None) => {
            problem(StatusCode::NOT_FOUND, "no such request for this facility")
        }
        Err(why) => problem(StatusCode::SERVICE_UNAVAILABLE, &why),
    }
}
