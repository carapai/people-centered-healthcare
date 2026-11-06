//! Prescribing at the facility authority (#108; inventory-and-procurement.md § Dispensing).
//!
//! - **The record** is the FHIR R5 MedicationRequest ([`fhir`]; [`prescription`] reads it back),
//!   tagged with its facility (ADR 0057). The course in days is
//!   `dispenseRequest.expectedSupplyDuration`.
//! - **Prescribing** ([`Medications::prescribe`]) runs the clinical safety checks first. With a
//!   warning the prescriber has not acknowledged, nothing is written and the warnings come back;
//!   once acknowledged, the MedicationRequest and `medications.prescribed.v1` are written in one
//!   transaction. Nothing is reserved or moved: a prescription has no stock effect.
//! - **What the checks know** ([`patient`]): the patient's open prescriptions and current
//!   allergies, from their MedicationRequests and AllergyIntolerances.
//! - **The pharmacy worklist** ([`worklist`]): the facility's open prescriptions, oldest first.

use health_access_policy::FACILITY_TAG_SYSTEM;
use health_identifiers::{OpId, ResourceId};
use health_medications::{
    MedicationEvent, Medicine, Patient, Prescribe, Prescription, PrescriptionRefused,
    PrescriptionStatus, SafetyChecks, Warning, prescribe,
};
use health_persistence::{
    OutboxEntry, OutboxPayload, Precondition, RawJson, ResourceType, Store, StoreError,
    Transaction, WriteError, WriteRequest,
};
use health_time::Timestamp;
use serde_json::{Value, json};

/// Why a prescribing step failed. Nothing was kept.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MedicationError {
    /// The domain refused it.
    #[error(transparent)]
    Refused(#[from] PrescriptionRefused),
    /// Safety warnings the prescriber has not acknowledged.
    #[error("{} safety warning(s) to acknowledge", .0.len())]
    Unacknowledged(Vec<Warning>),
    /// A prescription with that id was already written.
    #[error("prescription {0} was already written")]
    AlreadyWritten(String),
    /// No such prescription.
    #[error("prescription {0} is not known")]
    NotFound(String),
    /// The store failed, or a concurrent change came first; try again.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for MedicationError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for MedicationError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

/// The FHIR R5 MedicationRequest of `prescription`.
pub fn fhir(prescription: &Prescription) -> Value {
    let medicine = &prescription.medicine;
    let concept = match (&medicine.system, &medicine.code) {
        (Some(system), Some(code)) => {
            json!({ "coding": [{ "system": system, "code": code, "display": medicine.text }], "text": medicine.text })
        }
        _ => json!({ "text": medicine.text }),
    };
    let mut request = json!({
        "resourceType": "MedicationRequest",
        "id": prescription.id,
        "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": prescription.facility }] },
        "status": prescription.status.code(),
        "intent": "order",
        "medication": { "concept": concept },
        "subject": { "reference": prescription.patient },
        "authoredOn": prescription.authored.to_string(),
        "requester": { "display": prescription.prescriber },
        "dosageInstruction": [{ "text": prescription.dosage }]
    });
    if let Some(encounter) = &prescription.encounter {
        request["encounter"] = json!({ "reference": encounter });
    }
    if let Some(based_on) = &prescription.based_on {
        request["basedOn"] = json!([{ "reference": based_on }]);
    }
    if let Some(days) = prescription.days {
        request["dispenseRequest"] = json!({ "expectedSupplyDuration": {
            "value": days, "unit": "days", "system": "http://unitsofmeasure.org", "code": "d"
        } });
    }
    request
}

/// The prescription a MedicationRequest holds.
pub fn prescription(resource: &Value) -> Option<Prescription> {
    if resource["resourceType"] != "MedicationRequest" {
        return None;
    }
    let concept = &resource["medication"]["concept"];
    let coding = &concept["coding"][0];
    Some(Prescription {
        id: text(&resource["id"])?,
        patient: text(&resource["subject"]["reference"])?,
        encounter: text(&resource["encounter"]["reference"]),
        facility: resource["meta"]["tag"]
            .as_array()?
            .iter()
            .find(|t| t["system"] == FACILITY_TAG_SYSTEM)
            .and_then(|t| text(&t["code"]))?,
        medicine: Medicine {
            text: text(&concept["text"]).or_else(|| text(&coding["display"]))?,
            system: text(&coding["system"]),
            code: text(&coding["code"]),
        },
        dosage: text(&resource["dosageInstruction"][0]["text"]).unwrap_or_default(),
        days: resource["dispenseRequest"]["expectedSupplyDuration"]["value"]
            .as_u64()
            .and_then(|d| u32::try_from(d).ok()),
        status: PrescriptionStatus::from_code(resource["status"].as_str()?)?,
        prescriber: text(&resource["requester"]["display"]).unwrap_or_default(),
        authored: resource["authoredOn"].as_str()?.parse().ok()?,
        based_on: text(&resource["basedOn"][0]["reference"]),
    })
}

/// What the safety checks know of a patient: their open prescriptions and current allergies,
/// from their MedicationRequests and AllergyIntolerances.
pub fn patient(resources: &[Value]) -> Patient {
    let prescriptions = resources
        .iter()
        .filter_map(prescription)
        .filter(|p| p.status.is_open())
        .collect();
    let allergies = resources
        .iter()
        .filter(|r| r["resourceType"] == "AllergyIntolerance")
        .filter(|r| {
            !matches!(
                r["clinicalStatus"]["coding"][0]["code"].as_str(),
                Some("inactive" | "resolved")
            ) && !matches!(
                r["verificationStatus"]["coding"][0]["code"].as_str(),
                Some("refuted" | "entered-in-error")
            )
        })
        .filter_map(|r| {
            text(&r["code"]["text"]).or_else(|| text(&r["code"]["coding"][0]["display"]))
        })
        .collect();
    Patient {
        prescriptions,
        allergies,
    }
}

/// The pharmacy worklist of `facility` from MedicationRequests.
pub fn worklist(resources: &[Value], facility: &str) -> Vec<Prescription> {
    let all: Vec<Prescription> = resources.iter().filter_map(prescription).collect();
    health_medications::worklist(&all, facility)
}

fn store_error(error: impl ToString) -> MedicationError {
    MedicationError::Store(error.to_string())
}

/// The prescribing use cases over `store`.
pub struct Medications<S> {
    store: S,
}

impl<S: Store> Medications<S> {
    /// The use cases over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    fn kind() -> Result<ResourceType, MedicationError> {
        "MedicationRequest".parse().map_err(store_error)
    }

    async fn save(
        tx: &mut S::Tx,
        prescription: &Prescription,
        event: &MedicationEvent,
        precondition: Precondition,
    ) -> Result<(), MedicationError> {
        let id: ResourceId = prescription
            .id
            .parse()
            .map_err(|_| MedicationError::NotFound(prescription.id.clone()))?;
        match tx
            .write(WriteRequest {
                resource_type: Self::kind()?,
                id,
                json: RawJson::from_validated(fhir(prescription).to_string()),
                op_id: OpId::new(),
                precondition,
            })
            .await
        {
            Ok(_) => {}
            Err(WriteError::PreconditionFailed { .. })
                if precondition == Precondition::MustNotExist =>
            {
                return Err(MedicationError::AlreadyWritten(prescription.id.clone()));
            }
            Err(error) => return Err(error.into()),
        }
        tx.append_outbox(OutboxEntry {
            op_id: OpId::new(),
            payload: OutboxPayload::DomainEvent {
                name: event.name().to_owned(),
                body: RawJson::from_validated(serde_json::to_string(event).map_err(store_error)?),
            },
        })
        .await?;
        Ok(())
    }

    /// Prescribes, after `checks` over what is known of the `patient`. The warnings come back
    /// unwritten unless `acknowledged`; acknowledged, they are returned with the prescription.
    pub async fn prescribe(
        &self,
        request: &Prescribe,
        patient: &Patient,
        checks: &dyn SafetyChecks,
        acknowledged: bool,
    ) -> Result<(Prescription, Vec<Warning>), MedicationError> {
        let (prescription, event) = prescribe(request)?;
        let warnings = checks.check(&prescription, patient);
        if !warnings.is_empty() && !acknowledged {
            return Err(MedicationError::Unacknowledged(warnings));
        }
        let mut tx = self.store.begin().await?;
        Self::save(&mut tx, &prescription, &event, Precondition::MustNotExist).await?;
        tx.commit().await?;
        Ok((prescription, warnings))
    }

    /// Moves prescription `id` to status `to`.
    pub async fn transition(
        &self,
        id: &str,
        to: PrescriptionStatus,
        at: Timestamp,
    ) -> Result<Prescription, MedicationError> {
        let mut tx = self.store.begin().await?;
        let resource_id: ResourceId = id
            .parse()
            .map_err(|_| MedicationError::NotFound(id.to_owned()))?;
        let head = tx
            .read(&Self::kind()?, resource_id)
            .await?
            .filter(|h| !h.version.deleted)
            .ok_or_else(|| MedicationError::NotFound(id.to_owned()))?;
        let value: Value = serde_json::from_str(head.json.as_str()).map_err(store_error)?;
        let current =
            prescription(&value).ok_or_else(|| MedicationError::NotFound(id.to_owned()))?;
        let (next, event) = current.transition(to, at)?;
        Self::save(
            &mut tx,
            &next,
            &event,
            Precondition::IfMatch(head.version.version_id),
        )
        .await?;
        tx.commit().await?;
        Ok(next)
    }
}
