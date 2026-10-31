//! The policy decision point on every FHIR interaction (offline-auth.md § Authorization model;
//! #192).
//!
//! With access control on ([`FhirServer::with_access_control`]), the router's [`scope`]
//! middleware requires the request's [`Subject`] (a request extension the authentication layer
//! inserts) and answers 401 without one. The steps then ask the guard in a task-local:
//!
//! - **Searches** run under [`SubjectSearchPolicy`], confined to a patient when the search is
//!   (`patient=`, or `subject=` naming a Patient); 403 when the subject may not search the type.
//! - **Reads and vreads** go through [`Pdp::decide`] with the record's facility, labels and
//!   patient. A denied read answers 404, so the record's existence is not disclosed.
//! - **History** answers 404 for a record the subject may not read. Type and system history
//!   leave out the versions the subject may not see in a search.
//! - **Creates, updates and deletes** are decided on the resource written and, for an update or
//!   a delete, on the version it replaces: 403 when denied.
//!
//! Without access control, the server's construction-time [`SearchPolicy`] applies and nothing
//! else is checked: only for system jobs and tests.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use health_access_policy::{
    Access, Decision, FACILITY_TAG_SYSTEM, Interaction, Label, Pdp, Subject, is_facility_scope,
};
use health_fhir_search::SearchPolicy;
use health_identity_app::SubjectSearchPolicy;
use health_persistence::RawJson;
use health_time::Timestamp;
use serde_json::Value;

use crate::FhirServer;
use crate::outcome::FhirError;

/// Which facility a facility-scope record belongs to (write-authority.md: authority follows the
/// record's facility). `None` for a record that is not facility-scope.
pub trait RecordFacility: Send + Sync {
    /// The facility of `resource`, if it is facility-scope.
    fn facility(&self, resource: &Value) -> Option<String>;
}

/// The facility tag (ADR 0057): a record of a facility-scope type names its facility in
/// `meta.tag` with system `urn:health:facility`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FacilityTag;

/// The facility code `json` is tagged with.
fn tagged(json: &Value) -> Option<String> {
    json["meta"]["tag"]
        .as_array()?
        .iter()
        .find(|t| t["system"] == FACILITY_TAG_SYSTEM)
        .and_then(|t| t["code"].as_str())
        .map(str::to_owned)
}

impl RecordFacility for FacilityTag {
    fn facility(&self, resource: &Value) -> Option<String> {
        resource["resourceType"]
            .as_str()
            .filter(|t| is_facility_scope(t))
            .and_then(|_| tagged(resource))
    }
}

/// `json` with its facility tag (ADR 0057), for a facility-scope type under access control: the
/// facility of the version it replaces, or else the subject's. A tag naming another facility is
/// 403: authority stays with the record's facility. Other writes pass unchanged.
pub(crate) fn stamp(
    resource_type: &str,
    json: &RawJson,
    replaced: Option<&RawJson>,
) -> Result<RawJson, FhirError> {
    let Some(guard) = current() else {
        return Ok(json.clone());
    };
    if !is_facility_scope(resource_type) {
        return Ok(json.clone());
    }
    let facility = replaced
        .and_then(|r| tagged(&parse(r)))
        .unwrap_or_else(|| guard.subject.facility.clone());
    let mut value = parse(json);
    match tagged(&value) {
        Some(named) if named == facility => return Ok(json.clone()),
        Some(named) => {
            return Err(FhirError::new(
                StatusCode::FORBIDDEN,
                "forbidden",
                format!("the record belongs to facility {facility}, not {named}"),
            ));
        }
        None => {}
    }
    let tag = serde_json::json!({ "system": FACILITY_TAG_SYSTEM, "code": facility });
    match value["meta"]["tag"].as_array_mut() {
        Some(tags) => tags.push(tag),
        None => value["meta"]["tag"] = serde_json::json!([tag]),
    }
    Ok(RawJson::from_validated(value.to_string()))
}

/// No record is facility-scope: the default until records carry their facility (#192).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFacilityScope;

impl RecordFacility for NoFacilityScope {
    fn facility(&self, _resource: &Value) -> Option<String> {
        None
    }
}

/// What access control needs: the PDP and how to find a record's facility.
#[derive(Clone)]
pub(crate) struct Control {
    pub(crate) pdp: Pdp,
    pub(crate) facility: Arc<dyn RecordFacility>,
}

#[derive(Clone)]
struct Guard {
    control: Control,
    subject: Subject,
    now: Timestamp,
}

tokio::task_local! {
    static GUARD: Guard;
}

/// Middleware: runs the request with its subject's guard, or answers 401 without a subject.
pub(crate) async fn scope(
    State(server): State<FhirServer>,
    request: Request,
    next: Next,
) -> Response {
    let Some(control) = server.access.clone() else {
        return next.run(request).await;
    };
    let Some(subject) = request.extensions().get::<Subject>().cloned() else {
        return FhirError::new(
            StatusCode::UNAUTHORIZED,
            "login",
            "this request needs a signed-in user",
        )
        .into_response();
    };
    let guard = Guard {
        control,
        subject,
        now: Timestamp::now(),
    };
    GUARD.scope(guard, next.run(request)).await
}

fn current() -> Option<Guard> {
    GUARD.try_with(Clone::clone).ok()
}

/// The patient a search is confined to: `patient=` or a `subject=` naming a Patient, with one
/// value.
pub(crate) fn search_patient(pairs: &[(String, String)]) -> Option<String> {
    pairs.iter().find_map(|(name, value)| {
        let value = value.trim();
        if value.contains(',') {
            return None;
        }
        match name.as_str() {
            "patient" if value.starts_with("Patient/") => Some(value.to_owned()),
            "patient" if !value.is_empty() && !value.contains('/') => {
                Some(format!("Patient/{value}"))
            }
            "subject" if value.starts_with("Patient/") => Some(value.to_owned()),
            _ => None,
        }
    })
}

/// The policy every search of `resource_type` runs under, or 403 when the subject may not
/// search it.
pub(crate) fn search_policy(
    server: &FhirServer,
    resource_type: &str,
    patient: Option<String>,
) -> Result<Arc<dyn SearchPolicy>, FhirError> {
    let Some(guard) = current() else {
        return Ok(Arc::clone(&server.policy));
    };
    let policy = SubjectSearchPolicy::new(guard.control.pdp, guard.subject, patient, guard.now);
    policy
        .check(resource_type)
        .map_err(|deny| FhirError::new(StatusCode::FORBIDDEN, "forbidden", deny.to_string()))?;
    Ok(Arc::new(policy))
}

/// The `meta.security` labels of a resource.
fn labels(json: &Value) -> Vec<Label> {
    json["meta"]["security"]
        .as_array()
        .map(|codings| {
            codings
                .iter()
                .filter_map(|c| Some(Label::new(c["system"].as_str()?, c["code"].as_str()?)))
                .collect()
        })
        .unwrap_or_default()
}

/// The patient a resource is about: itself, or its `subject` / `patient` when that is a Patient.
pub(crate) fn patient_of(json: &Value) -> Option<String> {
    if json["resourceType"] == "Patient" {
        return json["id"].as_str().map(|id| format!("Patient/{id}"));
    }
    ["subject", "patient"]
        .iter()
        .filter_map(|field| json[*field]["reference"].as_str())
        .find(|r| r.starts_with("Patient/"))
        .map(str::to_owned)
}

fn decide(guard: &Guard, interaction: Interaction, json: &Value) -> Decision {
    let resource_type = json["resourceType"].as_str().unwrap_or_default();
    let facility = guard.control.facility.facility(json);
    let labels = labels(json);
    let patient = patient_of(json);
    guard.control.pdp.decide(
        &guard.subject,
        &Access {
            resource_type,
            interaction,
            facility: facility.as_deref(),
            labels: &labels,
            patient: patient.as_deref(),
        },
        guard.now,
    )
}

fn parse(json: &RawJson) -> Value {
    serde_json::from_str(json.as_str()).unwrap_or(Value::Null)
}

/// Whether the subject may read the stored version `json`; a record it may not read answers
/// `not_found`, so its existence is not disclosed.
pub(crate) fn readable(
    json: &RawJson,
    not_found: impl FnOnce() -> FhirError,
) -> Result<(), FhirError> {
    match current() {
        Some(guard) => match decide(&guard, Interaction::Read, &parse(json)) {
            Decision::Permit(_) => Ok(()),
            Decision::Deny(_) => Err(not_found()),
        },
        None => Ok(()),
    }
}

/// Whether the subject may see the stored version `json` in a listing that spans patients
/// (type and system history): decided as a search, so central's patient-confined shared reads
/// (ADR 0056) do not apply.
pub(crate) fn may_list(json: &RawJson) -> bool {
    current().is_none_or(|guard| {
        matches!(
            decide(&guard, Interaction::Search, &parse(json)),
            Decision::Permit(_)
        )
    })
}

/// Whether the subject may do `interaction` (create, update or delete) to `json`: 403
/// otherwise.
pub(crate) fn permitted(interaction: Interaction, json: &RawJson) -> Result<(), FhirError> {
    match current() {
        Some(guard) => match decide(&guard, interaction, &parse(json)) {
            Decision::Permit(_) => Ok(()),
            Decision::Deny(deny) => Err(FhirError::new(
                StatusCode::FORBIDDEN,
                "forbidden",
                deny.to_string(),
            )),
        },
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn a_search_is_confined_to_one_named_patient() {
        assert_eq!(
            search_patient(&pairs(&[("patient", "p1")])).as_deref(),
            Some("Patient/p1")
        );
        assert_eq!(
            search_patient(&pairs(&[("code", "x"), ("subject", "Patient/p2")])).as_deref(),
            Some("Patient/p2")
        );
        assert_eq!(search_patient(&pairs(&[("patient", "p1,p2")])), None);
        assert_eq!(search_patient(&pairs(&[("subject", "Group/g")])), None);
        assert_eq!(search_patient(&pairs(&[("code", "x")])), None);
    }
}
