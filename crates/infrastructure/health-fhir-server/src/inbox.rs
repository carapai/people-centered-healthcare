//! The clinical inbox (#110; clinical-workspaces.md § Clinical inbox), behind `subject_layer`:
//! `GET /` gives the signed-in clinician's items at their facility, the most urgent first, then
//! the newest. Each names its **authoritative resource** (`reference`), which the client opens.
//!
//! - **Results** (with [`crate::RESULTS_REVIEW_PERMISSION`]): verified laboratory results on the
//!   `clinician-results` worklist, linking their DiagnosticReport; `stat` when critical.
//! - **Outstanding documentation:** the clinician's own unsigned notes (preliminary Compositions).
//! - **Patient requests** (with `medications.prescribe` or `orders.request`): proposals from the
//!   portal (`intent=proposal` MedicationRequests and ServiceRequests).
//!
//! Every item passes the policy decision point: one whose resource the clinician may not read is
//! left out, as a read would be refused. Referrals and approvals join as their modules arrive.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json, Router};
use health_access_policy::{FACILITY_TAG_SYSTEM, Subject};
use health_identifiers::ResourceId;
use health_laboratory_app::{Laboratory, report_id};
use health_persistence::{ResourceType, Store, Transaction};
use serde::Serialize;
use serde_json::{Value, json};

use crate::FhirServer;
use crate::programs::found;

/// One inbox item.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Item {
    /// `result`, `documentation` or `request`.
    pub(crate) kind: &'static str,
    /// What a person reads.
    pub(crate) title: String,
    /// The patient, `Patient/{id}`.
    pub(crate) patient: String,
    /// The authoritative resource, `Type/id`.
    pub(crate) reference: String,
    /// `routine` or `stat`.
    pub(crate) priority: &'static str,
    /// Since when, as an instant.
    pub(crate) since: String,
}

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

/// The current version of `reference`, unless deleted.
async fn resource(server: &FhirServer, reference: &str) -> Option<Value> {
    let (kind, id) = reference.split_once('/')?;
    let kind: ResourceType = kind.parse().ok()?;
    let id: ResourceId = id.parse().ok()?;
    let mut tx = server.store.begin().await.ok()?;
    let head = tx.read(&kind, id).await.ok()??;
    (!head.version.deleted)
        .then(|| serde_json::from_str(head.json.as_str()).ok())
        .flatten()
}

async fn results(server: &FhirServer, subject: &Subject) -> Vec<Item> {
    let lab = Laboratory::new(server.store.clone());
    let mut items = Vec::new();
    for task in crate::workflow::worklist_for(server, "clinician-results").await {
        let Some(test_id) = task.context.strip_prefix("ServiceRequest/") else {
            continue;
        };
        let Ok(test) = lab.test(test_id).await else {
            continue;
        };
        if test.facility != subject.facility || !test.released() {
            continue;
        }
        items.push(Item {
            kind: "result",
            title: test.test.text.clone(),
            patient: test.patient.clone(),
            reference: format!("DiagnosticReport/{}", report_id(&test.id)),
            priority: if test.is_critical() {
                "stat"
            } else {
                "routine"
            },
            since: test
                .verified
                .as_ref()
                .map_or_else(|| task.authored_on.to_string(), |v| v.at.to_string()),
        });
    }
    items
}

async fn documentation(server: &FhirServer, subject: &Subject, tag: &str) -> Vec<Item> {
    found(server, "Composition", &[("_tag", tag)])
        .await
        .iter()
        .filter(|c| {
            c["status"] == "preliminary" && c["author"][0]["display"] == subject.account.as_str()
        })
        .filter_map(|c| {
            Some(Item {
                kind: "documentation",
                title: c["title"].as_str().unwrap_or("Note").to_owned(),
                patient: c["subject"][0]["reference"].as_str()?.to_owned(),
                reference: format!("Composition/{}", c["id"].as_str()?),
                priority: "routine",
                since: c["date"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

async fn requests(server: &FhirServer, tag: &str) -> Vec<Item> {
    let mut items = Vec::new();
    for resource_type in ["MedicationRequest", "ServiceRequest"] {
        for proposal in found(server, resource_type, &[("_tag", tag)])
            .await
            .iter()
            .filter(|r| {
                r["intent"] == "proposal"
                    && matches!(r["status"].as_str(), Some("active" | "draft"))
            })
        {
            let concept = if resource_type == "MedicationRequest" {
                &proposal["medication"]["concept"]
            } else {
                &proposal["code"]["concept"]
            };
            let (Some(patient), Some(id)) = (
                proposal["subject"]["reference"].as_str(),
                proposal["id"].as_str(),
            ) else {
                continue;
            };
            items.push(Item {
                kind: "request",
                title: concept["text"]
                    .as_str()
                    .or_else(|| concept["coding"][0]["display"].as_str())
                    .unwrap_or(resource_type)
                    .to_owned(),
                patient: patient.to_owned(),
                reference: format!("{resource_type}/{id}"),
                priority: "routine",
                since: proposal["authoredOn"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            });
        }
    }
    items
}

async fn inbox(State(server): State<FhirServer>, subject: Option<Extension<Subject>>) -> Response {
    let Some(Extension(subject)) = subject else {
        return refused(StatusCode::UNAUTHORIZED, "sign in");
    };
    let tag = format!("{FACILITY_TAG_SYSTEM}|{}", subject.facility);
    let mut items = Vec::new();
    if subject.permissions.has(crate::lab::REVIEW_PERMISSION) {
        items.extend(results(&server, &subject).await);
    }
    items.extend(documentation(&server, &subject, &tag).await);
    if subject
        .permissions
        .has(crate::consultation::PRESCRIBE_PERMISSION)
        || subject
            .permissions
            .has(crate::consultation::ORDER_PERMISSION)
    {
        items.extend(requests(&server, &tag).await);
    }
    // The policy decision point decides each item as it would a read of its resource.
    let mut readable = Vec::new();
    for item in items {
        if let Some(json) = resource(&server, &item.reference).await
            && crate::access::may_read(&server, &subject, &json)
        {
            readable.push(item);
        }
    }
    readable
        .sort_by(|a, b| (b.priority == "stat", &b.since).cmp(&(a.priority == "stat", &a.since)));
    Json(readable).into_response()
}

impl FhirServer {
    /// The clinical inbox route, to mount behind `subject_layer`.
    pub fn inbox_router(&self) -> Router {
        Router::new()
            .route("/", get(inbox))
            .with_state(self.clone())
    }
}
