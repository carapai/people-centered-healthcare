//! Price Books and Price Rules over HTTP at central (#120; pricing-and-insurance.md; ADR 0031),
//! behind `subject_layer`. Finance staff with [`MANAGE_PERMISSION`] draft; a finance approver
//! with [`APPROVE_PERMISSION`], who is not the author, approves and retires; anyone with
//! [`QUOTE_PERMISSION`] prices a charge.
//!
//! - `GET /books`, `GET /rules`: every version;
//! - `POST /books`, `POST /rules`: a draft (the next version, or a replacement of the latest
//!   draft);
//! - `POST /books/{id}/{version}/approve`, `POST /rules/{id}/{version}/approve`;
//! - `POST /books/{id}/{version}/retire` `{to}`, `POST /rules/{id}/{version}/retire` `{to}`;
//! - `POST /quote` `{request, context}`: a charge priced through the six stages, with every
//!   version it used. `context` maps each line id to the FHIR resource its rules' applicability
//!   is evaluated against (a Parameters resource, say); a line without one is evaluated against
//!   an empty Parameters. An expression must give a Boolean: an empty result does not apply, and
//!   anything else refuses the quote.
//!
//! Approving or retiring writes the version's ChargeItemDefinitions in the same transaction.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_fhir_validation::FhirPathItem;
use health_pricing::{Applicability, LifecycleError, Line, PriceBook, PriceRule, Request};
use health_pricing_app::{Pricing, PricingAppError};
use health_surrealdb::SurrealStore;
use health_time::{Clock, SystemClock, Timestamp};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::FhirServer;

/// The permission to draft Price Books and rules.
pub const MANAGE_PERMISSION: &str = "pricing.manage";
/// The permission to approve and retire them.
pub const APPROVE_PERMISSION: &str = "pricing.approve";
/// The permission to price a charge.
pub const QUOTE_PERMISSION: &str = "pricing.quote";

/// How this node prices: the ledger's functional currency.
#[derive(Debug, Clone)]
pub struct PricingConfig {
    /// ISO 4217.
    pub currency: String,
}

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn failed(error: &PricingAppError) -> Response {
    let status = match error {
        PricingAppError::NotFound(_) => StatusCode::NOT_FOUND,
        PricingAppError::Lifecycle(LifecycleError::OwnApproval) => StatusCode::FORBIDDEN,
        PricingAppError::Store(_) => return refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    };
    refused(status, &error.to_string())
}

fn staff(
    server: &FhirServer,
    subject: Option<Extension<Subject>>,
    permission: &str,
) -> Result<(Subject, Pricing<SurrealStore>), Box<Response>> {
    let Some(config) = server.pricing.clone() else {
        return Err(Box::new(refused(
            StatusCode::NOT_FOUND,
            "pricing is not enabled",
        )));
    };
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")));
    };
    if !subject.permissions.has(permission) {
        return Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            &format!("this needs the {permission} permission"),
        )));
    }
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    Ok((
        subject,
        Pricing::new(server.store.clone(), clock, &config.currency),
    ))
}

macro_rules! staff_or_return {
    ($server:expr, $subject:expr, $permission:expr) => {
        match staff(&$server, $subject, $permission) {
            Ok(found) => found,
            Err(response) => return *response,
        }
    };
}

fn answer<T: serde::Serialize>(result: Result<T, PricingAppError>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => failed(&error),
    }
}

async fn books(State(server): State<FhirServer>, subject: Option<Extension<Subject>>) -> Response {
    let (_, pricing) = staff_or_return!(server, subject, MANAGE_PERMISSION);
    answer(pricing.catalogue().await.map(|(books, _)| books))
}

async fn rules(State(server): State<FhirServer>, subject: Option<Extension<Subject>>) -> Response {
    let (_, pricing) = staff_or_return!(server, subject, MANAGE_PERMISSION);
    answer(pricing.catalogue().await.map(|(_, rules)| rules))
}

async fn draft_book(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(book): Json<PriceBook>,
) -> Response {
    let (subject, pricing) = staff_or_return!(server, subject, MANAGE_PERMISSION);
    answer(pricing.draft_book(book, &subject.account).await)
}

async fn draft_rule(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(rule): Json<PriceRule>,
) -> Response {
    let (subject, pricing) = staff_or_return!(server, subject, MANAGE_PERMISSION);
    answer(pricing.draft_rule(rule, &subject.account).await)
}

#[derive(Deserialize)]
struct RetireBody {
    to: Timestamp,
}

async fn book_step(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path((id, version, step)): Path<(String, u32, String)>,
    body: Option<Json<RetireBody>>,
) -> Response {
    let (subject, pricing) = staff_or_return!(server, subject, APPROVE_PERMISSION);
    match (step.as_str(), body) {
        ("approve", _) => answer(pricing.approve_book(&id, version, &subject.account).await),
        ("retire", Some(Json(body))) => answer(pricing.retire_book(&id, version, body.to).await),
        ("retire", None) => refused(StatusCode::BAD_REQUEST, "retire needs `to`"),
        _ => refused(StatusCode::NOT_FOUND, "no such step"),
    }
}

async fn rule_step(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path((id, version, step)): Path<(String, u32, String)>,
    body: Option<Json<RetireBody>>,
) -> Response {
    let (subject, pricing) = staff_or_return!(server, subject, APPROVE_PERMISSION);
    match (step.as_str(), body) {
        ("approve", _) => answer(pricing.approve_rule(&id, version, &subject.account).await),
        ("retire", Some(Json(body))) => answer(pricing.retire_rule(&id, version, body.to).await),
        ("retire", None) => refused(StatusCode::BAD_REQUEST, "retire needs `to`"),
        _ => refused(StatusCode::NOT_FOUND, "no such step"),
    }
}

/// Applicability evaluated before pricing, since the FHIRPath engine is asynchronous: each
/// line's outcome for each expression.
struct Evaluated(BTreeMap<(String, String), Result<bool, String>>);

impl Applicability for Evaluated {
    fn applies(&self, expression: &str, line: &Line) -> Result<bool, String> {
        self.0
            .get(&(line.id.clone(), expression.to_owned()))
            .cloned()
            .unwrap_or_else(|| Err(format!("{expression} was not evaluated for {}", line.id)))
    }
}

#[derive(Deserialize)]
struct QuoteBody {
    request: Request,
    #[serde(default)]
    context: BTreeMap<String, Value>,
}

async fn quote(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<QuoteBody>,
) -> Response {
    let (_, pricing) = staff_or_return!(server, subject, QUOTE_PERMISSION);
    let rules = match pricing.catalogue().await {
        Ok((_, rules)) => rules,
        Err(error) => return failed(&error),
    };
    let expressions: Vec<String> = rules
        .iter()
        .filter_map(|r| r.applicability.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut evaluated = BTreeMap::new();
    let empty = json!({ "resourceType": "Parameters" });
    for line in &body.request.lines {
        let resource = body.context.get(&line.id).unwrap_or(&empty);
        let results = server.fhirpath.evaluate_many(resource, &expressions).await;
        for (expression, result) in expressions.iter().zip(results) {
            let outcome = match result {
                Ok(items) => match items.as_slice() {
                    [] => Ok(false),
                    [FhirPathItem::Boolean(value)] => Ok(*value),
                    _ => Err(format!("{expression} does not give a Boolean")),
                },
                Err(error) => Err(error.0),
            };
            evaluated.insert((line.id.clone(), expression.clone()), outcome);
        }
    }
    answer(pricing.price(&body.request, &Evaluated(evaluated)).await)
}

impl FhirServer {
    /// This server, keeping Price Books and rules and pricing charges
    /// ([`Self::pricing_router`]).
    #[must_use]
    pub fn with_pricing(mut self, config: PricingConfig) -> Self {
        self.pricing = Some(Arc::new(config));
        self
    }

    /// The pricing routes, to mount behind `subject_layer`.
    pub fn pricing_router(&self) -> Router {
        Router::new()
            .route("/books", get(books).post(draft_book))
            .route("/books/{id}/{version}/{step}", post(book_step))
            .route("/rules", get(rules).post(draft_rule))
            .route("/rules/{id}/{version}/{step}", post(rule_step))
            .route("/quote", post(quote))
            .with_state(self.clone())
    }
}
