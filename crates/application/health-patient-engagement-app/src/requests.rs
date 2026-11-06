//! Patient requests at the facility authority (#114; patient-identity.md § Patient-initiated
//! workflows): proposals a patient makes from the portal, which staff accept or decline. Nothing
//! here writes an order or a booking: accepting is staff's step, which writes the order and then
//! [`PatientRequests::accepted`] closes the proposal.
//!
//! - **Submitting** ([`PatientRequests::submit`]): only for a Patient the account is linked to.
//!   A refill names one of that patient's prescriptions and goes to its facility; a care or
//!   appointment request names the facility. Each proposal is tagged `patient-reported`.
//! - **Withdrawing** ([`PatientRequests::withdraw`]) by the patient, **declining**
//!   ([`PatientRequests::decline`]) by staff with a reason: only while pending.

use health_access_policy::FACILITY_TAG_SYSTEM;
use health_client_registry::person::PortalAccess;
use health_identifiers::{OpId, ResourceId};
use health_patient_engagement::requests::{
    RequestKind, RequestRefused, RequestState, decline_reason, may_close,
};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, Transaction, WriteError, WriteRequest,
};
use serde_json::{Value, json};

use crate::fhir::{ORIGIN_TAG, person_of};
use crate::{EngagementError, account_person};

/// What a patient asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// A refill of `prior`, one of their prescriptions (`MedicationRequest/{id}`).
    Refill {
        /// The prescription.
        prior: String,
    },
    /// A test or other care, at `facility`.
    Care {
        /// The facility asked.
        facility: String,
        /// What.
        what: String,
    },
    /// An appointment at `facility`.
    Appointment {
        /// The facility asked.
        facility: String,
        /// What for.
        reason: String,
        /// The day they would like, if any (`YYYY-MM-DD`).
        preferred: Option<String>,
    },
}

/// A request as submitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    /// The proposal's id (a client UUIDv7).
    pub id: String,
    /// The patient, `Patient/{id}`.
    pub patient: String,
    /// What.
    pub asked: Asked,
    /// Anything the patient adds.
    pub note: Option<String>,
    /// When, as an instant.
    pub at: String,
}

/// Why a request step failed. Nothing was kept.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    /// The domain refused it.
    #[error(transparent)]
    Refused(#[from] RequestRefused),
    /// The account is not linked to that patient.
    #[error("the account may not ask for that patient")]
    NotLinked,
    /// The refill names no prescription of the patient's.
    #[error("{0} is not one of the patient's prescriptions")]
    NotAPrescription(String),
    /// A request with that id exists.
    #[error("request {0} exists")]
    Exists(String),
    /// No such request (here).
    #[error("request {0} is not known")]
    NotFound(String),
    /// The store failed, or a concurrent change came first; try again.
    #[error("{0}")]
    Store(String),
}

impl From<WriteError> for RequestError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<health_persistence::StoreError> for RequestError {
    fn from(error: health_persistence::StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<EngagementError> for RequestError {
    fn from(error: EngagementError) -> Self {
        Self::Store(error.to_string())
    }
}

fn facility_of(resource: &Value) -> Option<String> {
    resource["meta"]["tag"]
        .as_array()?
        .iter()
        .find(|t| t["system"] == FACILITY_TAG_SYSTEM)
        .and_then(|t| t["code"].as_str().map(str::to_owned))
}

/// A request as the patient and staff see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestView {
    /// The proposal, `Type/id`.
    pub reference: String,
    /// What kind.
    pub kind: RequestKind,
    /// Where it is.
    pub state: RequestState,
    /// What is asked, as a person reads it.
    pub title: String,
    /// The patient.
    pub patient: String,
    /// The facility asked.
    pub facility: String,
}

/// The request a proposal holds, if it is a patient's proposal.
pub fn view(proposal: &Value) -> Option<RequestView> {
    let resource_type = proposal["resourceType"].as_str()?;
    let kind = RequestKind::of(resource_type)?;
    let patient_reported = proposal["meta"]["tag"]
        .as_array()?
        .iter()
        .any(|t| t["system"] == ORIGIN_TAG && t["code"] == "patient-reported");
    if !patient_reported || (kind != RequestKind::Appointment && proposal["intent"] != "proposal") {
        return None;
    }
    let title = match kind {
        RequestKind::Refill => proposal["medication"]["concept"]["text"].as_str(),
        RequestKind::Care => proposal["code"]["concept"]["text"].as_str(),
        RequestKind::Appointment => proposal["description"].as_str(),
    }?;
    let patient = proposal["subject"]["reference"].as_str()?;
    Some(RequestView {
        reference: format!("{resource_type}/{}", proposal["id"].as_str()?),
        kind,
        state: RequestState::of(kind, proposal["status"].as_str()?)?,
        title: title.to_owned(),
        patient: patient.to_owned(),
        facility: facility_of(proposal)?,
    })
}

fn tags(facility: &str) -> Value {
    json!({ "tag": [
        { "system": FACILITY_TAG_SYSTEM, "code": facility },
        { "system": ORIGIN_TAG, "code": "patient-reported" }
    ] })
}

/// The patient-request use cases over `store`.
pub struct PatientRequests<S> {
    store: S,
}

impl<S: Store> PatientRequests<S> {
    /// The use cases over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    async fn read(
        tx: &mut S::Tx,
        reference: &str,
    ) -> Result<Option<(Value, Precondition)>, RequestError> {
        let Some((type_name, id)) = reference.split_once('/') else {
            return Ok(None);
        };
        let (Ok(kind), Ok(id)) = (type_name.parse::<ResourceType>(), id.parse::<ResourceId>())
        else {
            return Ok(None);
        };
        let Some(head) = tx.read(&kind, id).await?.filter(|h| !h.version.deleted) else {
            return Ok(None);
        };
        let value: Value = serde_json::from_str(head.json.as_str())
            .map_err(|e| RequestError::Store(e.to_string()))?;
        Ok(Some((
            value,
            Precondition::IfMatch(head.version.version_id),
        )))
    }

    /// The Patients `account` is linked to (`Patient/{id}`).
    pub async fn linked(&self, account: &str) -> Result<Vec<String>, RequestError> {
        let mut tx = self.store.begin().await?;
        let person = format!("Person/{}", account_person(account));
        let Some((json, _)) = Self::read(&mut tx, &person).await? else {
            return Ok(Vec::new());
        };
        Ok(person_of(&json, account)?
            .links
            .iter()
            .map(|l| format!("Patient/{}", l.patient))
            .collect())
    }

    /// What `account` may see of `patient`'s records (`Patient/{id}`): records only at `level3`.
    pub async fn access(&self, account: &str, patient: &str) -> Result<PortalAccess, RequestError> {
        let mut tx = self.store.begin().await?;
        let person = format!("Person/{}", account_person(account));
        let Some((json, _)) = Self::read(&mut tx, &person).await? else {
            return Ok(PortalAccess::None);
        };
        let id = patient.strip_prefix("Patient/").unwrap_or(patient);
        Ok(person_of(&json, account)?.access(id))
    }

    async fn write(
        tx: &mut S::Tx,
        resource: &Value,
        precondition: Precondition,
    ) -> Result<(), RequestError> {
        let kind: ResourceType = resource["resourceType"]
            .as_str()
            .unwrap_or_default()
            .parse()
            .map_err(|_| RequestError::Store("resource type".into()))?;
        let id: ResourceId = resource["id"]
            .as_str()
            .unwrap_or_default()
            .parse()
            .map_err(|_| RequestError::NotFound(resource["id"].to_string()))?;
        match tx
            .write(WriteRequest {
                resource_type: kind,
                id,
                json: RawJson::from_validated(resource.to_string()),
                op_id: OpId::new(),
                precondition,
            })
            .await
        {
            Ok(_) => Ok(()),
            Err(WriteError::PreconditionFailed { .. })
                if precondition == Precondition::MustNotExist =>
            {
                Err(RequestError::Exists(
                    resource["id"].as_str().unwrap_or_default().to_owned(),
                ))
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Submits `request` for a Patient `account` is linked to: the proposal, pending.
    pub async fn submit(&self, account: &str, request: Submission) -> Result<Value, RequestError> {
        if !self.linked(account).await?.contains(&request.patient) {
            return Err(RequestError::NotLinked);
        }
        let mut tx = self.store.begin().await?;
        let note = request
            .note
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(|n| json!([{ "text": n }]));
        let mut proposal = match &request.asked {
            Asked::Refill { prior } => {
                let not_theirs = || RequestError::NotAPrescription(prior.clone());
                let (prescription, _) = Self::read(&mut tx, prior).await?.ok_or_else(not_theirs)?;
                if prescription["resourceType"] != "MedicationRequest"
                    || prescription["intent"] != "order"
                    || prescription["subject"]["reference"] != request.patient.as_str()
                {
                    return Err(not_theirs());
                }
                let facility = facility_of(&prescription).ok_or_else(not_theirs)?;
                json!({
                    "resourceType": "MedicationRequest", "id": request.id, "meta": tags(&facility),
                    "status": "active", "intent": "proposal",
                    "medication": prescription["medication"],
                    "dosageInstruction": prescription["dosageInstruction"],
                    "subject": { "reference": request.patient },
                    "requester": { "reference": request.patient },
                    "priorPrescription": { "reference": prior },
                    "authoredOn": request.at
                })
            }
            Asked::Care { facility, what } => {
                if what.trim().is_empty() || facility.trim().is_empty() {
                    return Err(RequestRefused::Missing("what is asked and where").into());
                }
                json!({
                    "resourceType": "ServiceRequest", "id": request.id, "meta": tags(facility),
                    "status": "active", "intent": "proposal",
                    "code": { "concept": { "text": what.trim() } },
                    "subject": { "reference": request.patient },
                    "requester": { "reference": request.patient },
                    "authoredOn": request.at
                })
            }
            Asked::Appointment {
                facility,
                reason,
                preferred,
            } => {
                if reason.trim().is_empty() || facility.trim().is_empty() {
                    return Err(RequestRefused::Missing("what it is for and where").into());
                }
                let mut appointment = json!({
                    "resourceType": "Appointment", "id": request.id, "meta": tags(facility),
                    "status": "proposed",
                    "description": reason.trim(),
                    "subject": { "reference": request.patient },
                    "participant": [{ "actor": { "reference": request.patient }, "status": "accepted" }],
                    "created": request.at
                });
                if let Some(day) = preferred.as_deref().filter(|d| !d.trim().is_empty()) {
                    appointment["requestedPeriod"] = json!([{ "start": day.trim() }]);
                }
                appointment
            }
        };
        if let Some(note) = note {
            proposal["note"] = note;
        }
        Self::write(&mut tx, &proposal, Precondition::MustNotExist).await?;
        tx.commit().await?;
        Ok(proposal)
    }

    /// Closes the pending proposal `reference` with its closed status, after `check` passes.
    async fn close(
        &self,
        reference: &str,
        check: impl FnOnce(&RequestView) -> Result<(), RequestError>,
        to: impl FnOnce(RequestKind) -> &'static str,
        reason: Option<&str>,
    ) -> Result<Value, RequestError> {
        let mut tx = self.store.begin().await?;
        let (mut proposal, precondition) = Self::read(&mut tx, reference)
            .await?
            .ok_or_else(|| RequestError::NotFound(reference.to_owned()))?;
        let request =
            view(&proposal).ok_or_else(|| RequestError::NotFound(reference.to_owned()))?;
        check(&request)?;
        may_close(request.state)?;
        proposal["status"] = json!(to(request.kind));
        if let Some(reason) = reason {
            let field = match request.kind {
                RequestKind::Appointment => "cancellationReason",
                _ => "statusReason",
            };
            proposal[field] = json!({ "text": reason });
        }
        Self::write(&mut tx, &proposal, precondition).await?;
        tx.commit().await?;
        Ok(proposal)
    }

    /// The patient withdraws their pending request.
    pub async fn withdraw(&self, account: &str, reference: &str) -> Result<Value, RequestError> {
        let linked = self.linked(account).await?;
        self.close(
            reference,
            |r| {
                if linked.contains(&r.patient) {
                    Ok(())
                } else {
                    Err(RequestError::NotFound(reference.to_owned()))
                }
            },
            RequestKind::closed,
            Some("withdrawn by the patient"),
        )
        .await
    }

    /// Staff at `facility` decline a pending request, saying why.
    pub async fn decline(
        &self,
        reference: &str,
        facility: &str,
        reason: &str,
    ) -> Result<Value, RequestError> {
        let reason = decline_reason(reason)?.to_owned();
        self.close(
            reference,
            |r| here(r, facility, reference),
            RequestKind::closed,
            Some(&reason),
        )
        .await
    }

    /// The pending request at `facility`, for staff to accept.
    pub async fn pending(
        &self,
        reference: &str,
        facility: &str,
    ) -> Result<(RequestView, Value), RequestError> {
        let mut tx = self.store.begin().await?;
        let (proposal, _) = Self::read(&mut tx, reference)
            .await?
            .ok_or_else(|| RequestError::NotFound(reference.to_owned()))?;
        let request =
            view(&proposal).ok_or_else(|| RequestError::NotFound(reference.to_owned()))?;
        here(&request, facility, reference)?;
        may_close(request.state)?;
        Ok((request, proposal))
    }

    /// Closes a request staff accepted, once its order or booking is written; `change` sets
    /// anything else (an appointment's time).
    pub async fn accepted(
        &self,
        reference: &str,
        facility: &str,
        change: impl FnOnce(&mut Value),
    ) -> Result<Value, RequestError> {
        let mut tx = self.store.begin().await?;
        let (mut proposal, precondition) = Self::read(&mut tx, reference)
            .await?
            .ok_or_else(|| RequestError::NotFound(reference.to_owned()))?;
        let request =
            view(&proposal).ok_or_else(|| RequestError::NotFound(reference.to_owned()))?;
        here(&request, facility, reference)?;
        may_close(request.state)?;
        proposal["status"] = json!(request.kind.accepted());
        change(&mut proposal);
        Self::write(&mut tx, &proposal, precondition).await?;
        tx.commit().await?;
        Ok(proposal)
    }
}

fn here(request: &RequestView, facility: &str, reference: &str) -> Result<(), RequestError> {
    if request.facility == facility {
        Ok(())
    } else {
        Err(RequestError::NotFound(reference.to_owned()))
    }
}
