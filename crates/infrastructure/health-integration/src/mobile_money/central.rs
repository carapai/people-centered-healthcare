//! MTN MoMo at central (#124): one merchant account per facility, the routes the till uses, the
//! public route MTN calls back, and the poller.
//!
//! - `POST /momo` `{amount, payer, msisdn, deposit?, shift?, note?}` (with the cash permission,
//!   for the session's facility): asks the payer's phone to pay into the facility's account.
//!   The answer is the payment, pending until MTN confirms it.
//! - `GET /exceptions` and `POST /exceptions/{id}/resolve` `{note}` (with the supervise
//!   permission): the payment exceptions queue.
//! - `POST /momo/callback?reference=` (public, for MTN): only a hint. The status is read back
//!   from MTN by the reference, for the facility that reference was made for. It always answers
//!   200 with nothing in the body, so it tells a caller nothing.
//! - [`MomoService::spawn_poller`]: every minute, polls every facility's pending MoMo payments
//!   (what a dropped callback leaves) and expires those whose time has passed.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::http::Uri;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_payments::{Method, NewPayment, Purpose};
use health_payments_app::Settings;
use health_surrealdb::SurrealStore;
use health_time::{Clock, SignedDuration};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;

use super::collections::{Collections, Settled};
use super::mtn::{MomoTransport, MtnConfig, MtnMomo};

/// The permission to take payments (`health_fhir_server`'s `payments.cash`).
const CASH_PERMISSION: &str = "payments.cash";
/// The permission to work the exceptions queue (`health_fhir_server`'s `payments.supervise`).
const SUPERVISE_PERMISSION: &str = "payments.supervise";
/// How often pending payments are polled.
const POLL_EVERY: Duration = Duration::from_secs(60);

/// The facilities' MTN merchant accounts, from central's secrets file
/// (`HEALTH_MOMO_ACCOUNTS_FILE`):
///
/// ```json
/// { "base_url": "https://sandbox.momodeveloper.mtn.com",
///   "callback_url": "https://central.example.org/api/payments/momo/callback",
///   "pending_minutes": 60,
///   "facilities": { "KAM013": { "subscription_key": "…", "api_user": "…", "api_key": "…",
///                               "environment": "sandbox", "currency": "UGX" } } }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MomoAccounts {
    /// The API's base URL.
    pub base_url: String,
    /// Where MTN calls back.
    pub callback_url: Option<String>,
    /// How long a payment waits for the payer.
    pub pending_for: SignedDuration,
    /// Each facility's account, by its short code.
    pub facilities: BTreeMap<String, MtnConfig>,
}

#[derive(Deserialize)]
struct AccountsFile {
    base_url: String,
    callback_url: Option<String>,
    #[serde(default = "default_minutes")]
    pending_minutes: i64,
    facilities: BTreeMap<String, AccountFile>,
}

fn default_minutes() -> i64 {
    60
}

#[derive(Deserialize)]
struct AccountFile {
    subscription_key: String,
    api_user: String,
    api_key: String,
    environment: String,
    currency: String,
}

impl MomoAccounts {
    /// The accounts in `text`, the secrets file's JSON.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: AccountsFile = serde_json::from_str(text)
            .map_err(|e| format!("the MoMo accounts file is not valid: {e}"))?;
        if !file.base_url.starts_with("https://") {
            return Err("the MoMo base_url must be https".into());
        }
        if file.pending_minutes <= 0 {
            return Err("pending_minutes must be positive".into());
        }
        Ok(Self {
            base_url: file.base_url,
            pending_for: SignedDuration::from_mins(file.pending_minutes),
            facilities: file
                .facilities
                .into_iter()
                .map(|(facility, account)| {
                    (
                        facility,
                        MtnConfig {
                            subscription_key: account.subscription_key,
                            api_user: account.api_user,
                            api_key: account.api_key,
                            environment: account.environment,
                            currency: account.currency,
                            callback_url: file.callback_url.clone(),
                        },
                    )
                })
                .collect(),
            callback_url: file.callback_url,
        })
    }

    /// The accounts in the secrets file at `path`.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Self::parse(&text)
    }
}

/// MoMo collections for every facility with an account.
pub struct MomoService<T> {
    store: SurrealStore,
    collections: BTreeMap<String, Collections<T>>,
}

impl<T: MomoTransport + Clone + 'static> MomoService<T> {
    /// The service over `store`, each facility's collections through `transport`, with the
    /// payment settings `settings_for` gives each facility.
    pub fn new(
        store: SurrealStore,
        accounts: &MomoAccounts,
        transport: T,
        settings_for: impl Fn(&str) -> Result<Settings, String>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, String> {
        let mut collections = BTreeMap::new();
        for (facility, config) in &accounts.facilities {
            collections.insert(
                facility.clone(),
                Collections::new(
                    store.clone(),
                    settings_for(facility)?,
                    MtnMomo::new(transport.clone(), config.clone()),
                    Arc::clone(&clock),
                    accounts.pending_for,
                ),
            );
        }
        Ok(Self { store, collections })
    }

    /// The facility's collections, if it has an account.
    pub fn collections(&self, facility: &str) -> Option<&Collections<T>> {
        self.collections.get(facility)
    }

    /// MTN's callback for `reference`: settled for the facility it was made for.
    pub async fn callback(&self, reference: &str) -> Option<Settled> {
        let facility = self
            .store
            .provider_request_facility(reference)
            .await
            .ok()
            .flatten()?;
        match self.collections.get(&facility)?.callback(reference).await {
            Ok(settled) => Some(settled),
            Err(error) => {
                tracing::warn!(%error, reference, "a MoMo callback could not be settled now");
                None
            }
        }
    }

    /// One round for every facility: polls its pending MoMo payments and expires the overdue.
    pub async fn poll(&self) -> Vec<(String, Settled)> {
        let mut settled = Vec::new();
        for (facility, collections) in &self.collections {
            match collections.poll().await {
                Ok(done) => settled.extend(done),
                Err(error) => {
                    tracing::warn!(%error, facility, "MoMo polling failed; it runs again")
                }
            }
            if let Err(error) = collections.expire_due().await {
                tracing::warn!(%error, facility, "pending payments could not be expired");
            }
        }
        settled
    }

    /// Polls every minute until the process ends.
    pub fn spawn_poller(self: &Arc<Self>) {
        let service = Arc::clone(self);
        tokio::spawn(async move {
            let mut every = tokio::time::interval(POLL_EVERY);
            loop {
                every.tick().await;
                service.poll().await;
            }
        });
    }

    /// The till's routes, to mount under `/api/payments` behind the subject layer.
    pub fn routes(self: &Arc<Self>) -> Router {
        Router::new()
            .route("/momo", post(initiate::<T>))
            .route("/exceptions", get(exceptions::<T>))
            .route("/exceptions/{id}/resolve", post(resolve::<T>))
            .with_state(Arc::clone(self))
    }

    /// MTN's public callback route, `POST /momo/callback?reference=`, outside any sign-in.
    pub fn callback_route(self: &Arc<Self>) -> Router {
        Router::new()
            .route("/momo/callback", post(callback::<T>))
            .with_state(Arc::clone(self))
    }
}

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn subject_with(
    subject: Option<Extension<Subject>>,
    permission: &str,
) -> Result<Subject, Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")));
    };
    if !subject.permissions.has(permission) {
        return Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            &format!("this needs the {permission} permission"),
        )));
    }
    Ok(subject)
}

#[derive(Deserialize)]
struct Initiate {
    amount: String,
    payer: String,
    msisdn: String,
    #[serde(default)]
    deposit: bool,
    shift: Option<String>,
    note: Option<String>,
}

async fn initiate<T: MomoTransport + Clone + 'static>(
    State(service): State<Arc<MomoService<T>>>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<Initiate>,
) -> Response {
    let subject = match subject_with(subject, CASH_PERMISSION) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let Some(collections) = service.collections(&subject.facility) else {
        return refused(
            StatusCode::NOT_FOUND,
            "this facility has no MTN MoMo merchant account",
        );
    };
    let Ok(amount) = body.amount.trim().parse::<Decimal>() else {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "the amount is not a number",
        );
    };
    let msisdn: String = body.msisdn.chars().filter(char::is_ascii_digit).collect();
    if msisdn.len() < 9 {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "the phone number is not valid",
        );
    }
    let id = format!("pay-{}", uuid::Uuid::now_v7());
    let new = NewPayment {
        id: &id,
        method: Method::MtnMomo,
        purpose: if body.deposit {
            Purpose::Deposit
        } else {
            Purpose::Settlement
        },
        amount,
        payer: body.payer.trim(),
    };
    let note = body.note.as_deref().unwrap_or("Payment");
    match collections
        .initiate(&new, &msisdn, note, body.shift.as_deref())
        .await
    {
        Ok(payment) => (
            StatusCode::CREATED,
            Json(json!({
                "id": payment.id,
                "status": format!("{:?}", payment.status).to_lowercase(),
                "reference": payment.reference,
                "receipt": payment.receipt,
                "reason": payment.reason,
            })),
        )
            .into_response(),
        Err(error) => refused(StatusCode::UNPROCESSABLE_ENTITY, &error.to_string()),
    }
}

async fn exceptions<T: MomoTransport + Clone + 'static>(
    State(service): State<Arc<MomoService<T>>>,
    subject: Option<Extension<Subject>>,
) -> Response {
    if let Err(response) = subject_with(subject, SUPERVISE_PERMISSION) {
        return *response;
    }
    match service.store.open_payment_exceptions().await {
        Ok(open) => Json(
            open.into_iter()
                .map(|e| {
                    json!({
                        "id": e.id, "kind": e.kind, "reference": e.reference,
                        "payment": e.payment, "recorded": e.recorded, "provider": e.provider,
                        "opened_at": e.opened_at,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => refused(StatusCode::SERVICE_UNAVAILABLE, &error.to_string()),
    }
}

#[derive(Deserialize)]
struct Resolution {
    note: String,
}

async fn resolve<T: MomoTransport + Clone + 'static>(
    State(service): State<Arc<MomoService<T>>>,
    subject: Option<Extension<Subject>>,
    UrlPath(params): UrlPath<BTreeMap<String, String>>,
    Json(body): Json<Resolution>,
) -> Response {
    let subject = match subject_with(subject, SUPERVISE_PERMISSION) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    if body.note.trim().is_empty() {
        return refused(StatusCode::UNPROCESSABLE_ENTITY, "say how it was resolved");
    }
    let id = params.get("id").cloned().unwrap_or_default();
    match service
        .store
        .resolve_payment_exception(&id, &subject.account, body.note.trim())
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => refused(StatusCode::NOT_FOUND, "no such open exception"),
        Err(error) => refused(StatusCode::SERVICE_UNAVAILABLE, &error.to_string()),
    }
}

/// The `reference` query parameter: a provider reference is a UUID, so nothing in it is escaped.
fn reference_of(uri: &Uri) -> Option<&str> {
    uri.query()?
        .split('&')
        .find_map(|pair| pair.strip_prefix("reference="))
        .filter(|reference| {
            !reference.is_empty()
                && reference.len() <= 64
                && reference.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        })
}

async fn callback<T: MomoTransport + Clone + 'static>(
    State(service): State<Arc<MomoService<T>>>,
    uri: Uri,
) -> Response {
    // The body is never read: the status comes from MTN itself.
    if let Some(reference) = reference_of(&uri) {
        service.callback(reference).await;
    }
    StatusCode::OK.into_response()
}
