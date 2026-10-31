//! Registering a patient at reception (patient-identity.md § "Did you mean" at registration,
//! § Provisional registration): look for the person first, then create a provisional source
//! Patient with a facility MRN (ADR 0014) and let registration matching link it, queue it or give
//! it a new identity.
//!
//! **No duplicate without confirmation.** Until the clerk confirms the patient is not one of the
//! candidates, [`RegistryReview::register_new`] only answers with them and writes nothing. When
//! there is too little to match on, it says so, and the clerk searches the registry by hand
//! first.

use health_client_registry::ReviewQueue;
use health_identifiers::{NumberFormat, ResourceId, ShortCode};
use health_persistence::{BlockingIndex, CounterKey, GuardKey, Precondition, Store, Transaction};
use serde_json::{Value, json};

use crate::fhir::REGISTRY_TAG;
use crate::matching::Matched;
use crate::review::{Registered, RegistryReview};
use crate::{AuditTrail, RegistryError};

/// Where and how this node mints MRNs (ADR 0014; the format is a Published Definition).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MrnSettings {
    /// This node's short code.
    pub node: ShortCode,
    /// The MRN format.
    pub format: NumberFormat,
    /// The identifier system MRNs carry, followed by `/{facility}` (e.g. `urn:health:mrn`).
    pub system: String,
}

/// What a registration did.
#[derive(Debug, Clone, PartialEq)]
pub enum Registration {
    /// Possible matches (or too little to match on): nothing was written; the clerk picks one,
    /// searches by hand, or confirms a new patient.
    Candidates(Matched),
    /// A new provisional source Patient.
    Registered {
        /// Its id.
        id: ResourceId,
        /// Its MRN.
        mrn: String,
        /// What registration matching did with it.
        outcome: Registered,
    },
}

impl<S, A> RegistryReview<S, A>
where
    S: Store + Clone,
    S::Tx: BlockingIndex + ReviewQueue,
    A: AuditTrail<S::Tx>,
{
    /// Registers `patient` (a Patient as the clerk entered it) at `facility`. Unless
    /// `confirmed_new`, existing candidates come back instead and nothing is written.
    pub async fn register_new(
        &self,
        patient: &Value,
        confirmed_new: bool,
        facility: &ShortCode,
        mrn: &MrnSettings,
        actor: &str,
        year: i32,
    ) -> Result<Registration, RegistryError> {
        if patient["resourceType"] != "Patient" {
            return Err(RegistryError::WrongKind(
                "a registration is a Patient".into(),
            ));
        }
        if !confirmed_new {
            let matched = self.matcher().find(patient, 10, false).await?;
            if matched.insufficient || !matched.candidates.is_empty() {
                return Ok(Registration::Candidates(matched));
            }
        }
        let store = self.registry().store();
        let mut tx = store.begin().await?;
        let counter = CounterKey(format!("mrn|{facility}|{}", mrn.node));
        let sequence = tx.next_sequence(&counter, &mrn.format, year).await?;
        let number = mrn.format.format(facility, &mrn.node, year, sequence);
        tx.claim_guard(GuardKey(format!("mrn|{facility}|{number}")))
            .await?;
        let id = ResourceId::new();
        let mut json = patient.clone();
        json["id"] = json!(id.to_string());
        let identifier = json!({
            "use": "usual",
            "type": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v2-0203", "code": "MR" }] },
            "system": format!("{}/{facility}", mrn.system),
            "value": number,
        });
        match json["identifier"].as_array_mut() {
            Some(identifiers) => identifiers.push(identifier),
            None => json["identifier"] = json!([identifier]),
        }
        let provisional = json!({ "system": REGISTRY_TAG, "code": "provisional" });
        match json["meta"]["tag"].as_array_mut() {
            Some(tags) => tags.push(provisional),
            None => json["meta"]["tag"] = json!([provisional]),
        }
        self.registry()
            .write(
                &mut tx,
                actor,
                "register",
                id,
                &json,
                Precondition::MustNotExist,
            )
            .await?;
        tx.commit().await?;
        let outcome = self.register(id, actor).await?;
        Ok(Registration::Registered {
            id,
            mrn: number,
            outcome,
        })
    }
}
