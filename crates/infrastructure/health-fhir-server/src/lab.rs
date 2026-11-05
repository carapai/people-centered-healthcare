//! The laboratory over HTTP (#109), behind `subject_layer`, for the session's facility. Tests are
//! laboratory's (`health-laboratory-app`); the lab Workflow Definition ([`LAB_WORKFLOW`], from
//! the `ug.moh.lab` reference package) puts each step on a worklist: `lab-collect`, `lab-result`
//! and `lab-verify`, then the clinician's `clinician-results`, at `stat` priority for a critical
//! result. Each step is delivered to the test's workflow instance as its event.
//!
//! - A laboratory order from the consultation ([`follow`]) starts the test and its workflow.
//! - `GET /tests/{id}` needs [`WORK_PERMISSION`] or [`REVIEW_PERMISSION`]. A clinician without
//!   [`WORK_PERMISSION`] sees no results until they are verified.
//! - `POST /tests/{id}/collect` `{specimen, kind}`, `/result` `{results}`, `/reject` `{reason}`
//!   and `/cancel` `{reason}` need [`WORK_PERMISSION`]; `/verify` needs [`VERIFY_PERMISSION`] (and
//!   someone other than who entered the results); `/review` needs [`REVIEW_PERMISSION`].

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_clinical_documentation_app::consultation::{Concept, Context};
use health_laboratory::{Coded, Done, LabEvent, LabTest, Order, ResultEntry, Specimen};
use health_laboratory_app::{LabError, Laboratory};
use health_time::Timestamp;
use serde::Deserialize;
use serde_json::json;

use crate::FhirServer;

/// The lab Workflow Definition (the `ug.moh.lab` reference package).
pub const LAB_WORKFLOW: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/Library/lab-test";
/// The permission to collect, result, reject and cancel tests.
pub const WORK_PERMISSION: &str = "laboratory.work";
/// The permission to verify results.
pub const VERIFY_PERMISSION: &str = "laboratory.verify";
/// The permission to read and review released results.
pub const REVIEW_PERMISSION: &str = "results.review";

fn start_op(test: &str) -> String {
    format!("lab:{test}")
}

/// Starts the laboratory test of a laboratory order written in the consultation, and its
/// workflow. A failure is logged: the order stands, and the laboratory can be told again.
pub(crate) async fn follow(server: &FhirServer, context: &Context, id: &str, concept: &Concept) {
    let Ok(at) = context.at.parse::<Timestamp>() else {
        return;
    };
    let order = Order {
        id: id.to_owned(),
        patient: context.patient.clone(),
        encounter: Some(context.encounter.clone()),
        facility: context.facility.clone(),
        test: Coded {
            text: concept.display.trim().to_owned(),
            system: concept.system.clone(),
            code: concept.code.clone(),
        },
        ordered: Done {
            by: context.author.clone(),
            at,
        },
    };
    let order_author = order.ordered.by.clone();
    if let Err(error) = Laboratory::new(server.store.clone()).order(order).await {
        tracing::warn!(error.kind = "lab-order-failed", %error, "a laboratory order was not followed");
        return;
    }
    let context = format!("ServiceRequest/{id}");
    if let Err(reason) =
        crate::workflow::start_for(server, LAB_WORKFLOW, &context, &order_author, &start_op(id))
            .await
    {
        tracing::warn!(error.kind = "lab-workflow-failed", %reason, "the lab workflow did not start");
    }
}

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn failed(error: &LabError) -> Response {
    let status = match error {
        LabError::Refused(_) => StatusCode::UNPROCESSABLE_ENTITY,
        LabError::AlreadyOrdered(_) => StatusCode::CONFLICT,
        LabError::NotFound(_) => StatusCode::NOT_FOUND,
        LabError::Store(_) => return refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
    };
    refused(status, &error.to_string())
}

fn signed_in(subject: Option<Extension<Subject>>) -> Result<Subject, Box<Response>> {
    subject
        .map(|Extension(s)| s)
        .ok_or_else(|| Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")))
}

fn needs(subject: &Subject, permission: &str) -> Result<(), Box<Response>> {
    if subject.permissions.has(permission) {
        Ok(())
    } else {
        Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            &format!("this needs the {permission} permission"),
        )))
    }
}

macro_rules! or_return {
    ($result:expr) => {
        match $result {
            Ok(found) => found,
            Err(response) => return *response,
        }
    };
}

/// Test `id`, if it is this facility's.
async fn ours(server: &FhirServer, subject: &Subject, id: &str) -> Result<LabTest, Box<Response>> {
    match Laboratory::new(server.store.clone()).test(id).await {
        Ok(test) if test.facility == subject.facility => Ok(test),
        Ok(_) => Err(Box::new(refused(
            StatusCode::NOT_FOUND,
            "no such test here",
        ))),
        Err(error) => Err(Box::new(failed(&error))),
    }
}

/// The event the lab workflow takes for a step.
fn workflow_event(event: &LabEvent) -> &'static str {
    match event {
        LabEvent::Collected { .. } => "laboratory.collected",
        LabEvent::Resulted { .. } => "laboratory.resulted",
        LabEvent::Verified { critical: true, .. } => "laboratory.verified-critical",
        LabEvent::Verified { .. } => "laboratory.verified",
        LabEvent::Reviewed { .. } => "laboratory.reviewed",
        LabEvent::Closed { .. } => "laboratory.closed",
    }
}

/// The step's answer, after its event is delivered to the test's workflow.
async fn stepped(server: &FhirServer, result: Result<(LabTest, LabEvent), LabError>) -> Response {
    match result {
        Ok((test, event)) => {
            let name = workflow_event(&event);
            let event_id = format!("{}#{name}#{}", test.id, Timestamp::now().as_millisecond());
            if let Err(reason) =
                crate::workflow::deliver_for(server, &start_op(&test.id), name, &event_id).await
            {
                tracing::warn!(error.kind = "lab-workflow-failed", %reason, "a lab step was not delivered to its workflow");
            }
            Json(test).into_response()
        }
        Err(error) => failed(&error),
    }
}

async fn read_test(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    let subject = or_return!(signed_in(subject));
    let works = subject.permissions.has(WORK_PERMISSION);
    if !works {
        or_return!(needs(&subject, REVIEW_PERMISSION));
    }
    let mut test = or_return!(ours(&server, &subject, &id).await);
    // Unverified results are the laboratory's alone.
    if !works && !test.released() {
        test.results.clear();
    }
    Json(test).into_response()
}

#[derive(Deserialize)]
struct CollectBody {
    specimen: String,
    kind: String,
}

async fn collect(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<CollectBody>,
) -> Response {
    let subject = or_return!(signed_in(subject));
    or_return!(needs(&subject, WORK_PERMISSION));
    or_return!(ours(&server, &subject, &id).await);
    if body
        .specimen
        .parse::<health_identifiers::ResourceId>()
        .is_err()
    {
        return refused(
            StatusCode::BAD_REQUEST,
            "the specimen id is a client-generated UUIDv7 (ADR 0013)",
        );
    }
    let specimen = Specimen {
        id: body.specimen,
        kind: Coded {
            text: body.kind.trim().to_owned(),
            system: None,
            code: None,
        },
        collected: Done {
            by: subject.account.clone(),
            at: Timestamp::now(),
        },
    };
    let lab = Laboratory::new(server.store.clone());
    stepped(&server, lab.collect(&id, specimen).await).await
}

#[derive(Deserialize)]
struct ResultBody {
    results: Vec<ResultEntry>,
}

async fn result(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<ResultBody>,
) -> Response {
    let subject = or_return!(signed_in(subject));
    or_return!(needs(&subject, WORK_PERMISSION));
    or_return!(ours(&server, &subject, &id).await);
    let done = Done {
        by: subject.account.clone(),
        at: Timestamp::now(),
    };
    let lab = Laboratory::new(server.store.clone());
    stepped(&server, lab.result(&id, body.results, done).await).await
}

async fn verify(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    let subject = or_return!(signed_in(subject));
    or_return!(needs(&subject, VERIFY_PERMISSION));
    or_return!(ours(&server, &subject, &id).await);
    let done = Done {
        by: subject.account.clone(),
        at: Timestamp::now(),
    };
    let lab = Laboratory::new(server.store.clone());
    stepped(&server, lab.verify(&id, done).await).await
}

async fn review(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    let subject = or_return!(signed_in(subject));
    or_return!(needs(&subject, REVIEW_PERMISSION));
    or_return!(ours(&server, &subject, &id).await);
    let done = Done {
        by: subject.account.clone(),
        at: Timestamp::now(),
    };
    let lab = Laboratory::new(server.store.clone());
    stepped(&server, lab.review(&id, done).await).await
}

#[derive(Deserialize)]
struct ReasonBody {
    reason: String,
}

async fn reject(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<ReasonBody>,
) -> Response {
    let subject = or_return!(signed_in(subject));
    or_return!(needs(&subject, WORK_PERMISSION));
    or_return!(ours(&server, &subject, &id).await);
    let lab = Laboratory::new(server.store.clone());
    stepped(
        &server,
        lab.reject(&id, &body.reason, Timestamp::now()).await,
    )
    .await
}

async fn cancel(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<ReasonBody>,
) -> Response {
    let subject = or_return!(signed_in(subject));
    or_return!(needs(&subject, WORK_PERMISSION));
    or_return!(ours(&server, &subject, &id).await);
    let lab = Laboratory::new(server.store.clone());
    stepped(
        &server,
        lab.cancel(&id, &body.reason, Timestamp::now()).await,
    )
    .await
}

impl FhirServer {
    /// The laboratory routes, to mount behind `subject_layer`.
    pub fn laboratory_router(&self) -> Router {
        Router::new()
            .route("/tests/{id}", get(read_test))
            .route("/tests/{id}/collect", post(collect))
            .route("/tests/{id}/result", post(result))
            .route("/tests/{id}/verify", post(verify))
            .route("/tests/{id}/review", post(review))
            .route("/tests/{id}/reject", post(reject))
            .route("/tests/{id}/cancel", post(cancel))
            .with_state(self.clone())
    }
}
