//! Encounters at the facility authority (#102; clinical-workspaces.md; resource-ownership.md).
//! The Encounter is this module's: each step is one transaction holding the Encounter, its guard,
//! its FHIR projection and its outbox event.
//!
//! - **Check-in** ([`Encounters::check_in`]) claims the patient's open-encounter guard at the
//!   service point (a second check-in there is refused while the first is open), starts the
//!   Encounter, writes its FHIR Encounter (tagged with its facility, ADR 0057) and puts
//!   `encounters.started.v1` on the outbox.
//! - **Status** ([`Encounters::transition`]) follows the R5 lifecycle; an Encounter that ends
//!   releases its guard.
//! - **Moving** ([`Encounters::move_to`]) releases the guard at the service point left and claims
//!   the one at the service point reached.
//!
//! Patient-flow follows the events into its queues (`health-patient-flow-app`). Who may check
//! in, end or move an Encounter is the caller's permission check.

use health_access_policy::FACILITY_TAG_SYSTEM;
use health_encounters::{
    CheckIn, Encounter, EncounterEvent, EncounterRefused, EncounterStatus, EncounterStore,
    check_in, open_guard,
};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    GuardKey, OutboxEntry, OutboxPayload, Precondition, RawJson, ResourceType, Store, StoreError,
    Transaction, WriteError, WriteRequest,
};
use health_time::Timestamp;
use serde_json::{Value, json};

/// Why an Encounter step failed. Nothing was kept.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EncountersError {
    /// The domain refused it.
    #[error(transparent)]
    Refused(#[from] EncounterRefused),
    /// The patient already has an open Encounter at that service point.
    #[error("the patient already has an open encounter at {0}")]
    AlreadyOpen(String),
    /// No such Encounter.
    #[error("encounter {0} is not known")]
    NotFound(String),
    /// The store failed, or a concurrent change came first; try again.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for EncountersError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for EncountersError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

/// The FHIR R5 Encounter of `encounter`.
pub fn fhir(encounter: &Encounter) -> Value {
    let mut period = json!({ "start": encounter.started.to_string() });
    if let Some(ended) = encounter.ended {
        period["end"] = json!(ended.to_string());
    }
    let location_status = if encounter.status.is_final() {
        "completed"
    } else {
        "active"
    };
    json!({
        "resourceType": "Encounter",
        "id": encounter.id,
        "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": encounter.facility }] },
        "status": encounter.status.code(),
        "class": [{ "coding": [{
            "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
            "code": encounter.class.code()
        }] }],
        "subject": { "reference": encounter.patient },
        "actualPeriod": period,
        "location": [{
            "location": { "display": encounter.service_point },
            "status": location_status
        }]
    })
}

/// The Encounter use cases over `store`.
pub struct Encounters<S> {
    store: S,
}

fn store_error(error: impl ToString) -> EncountersError {
    EncountersError::Store(error.to_string())
}

impl<S> Encounters<S>
where
    S: Store,
    S::Tx: EncounterStore,
{
    /// The use cases over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Writes the FHIR Encounter and the outbox event of `encounter`.
    async fn project(
        &self,
        tx: &mut S::Tx,
        encounter: &Encounter,
        event: &EncounterEvent,
    ) -> Result<(), EncountersError> {
        let kind: ResourceType = "Encounter".parse().map_err(store_error)?;
        let id = ResourceId::from_uuid(encounter.id.parse().map_err(store_error)?)
            .map_err(store_error)?;
        let precondition = match tx.read(&kind, id).await? {
            Some(head) => Precondition::IfMatch(head.version.version_id),
            None => Precondition::MustNotExist,
        };
        tx.write(WriteRequest {
            resource_type: kind,
            id,
            json: RawJson::from_validated(fhir(encounter).to_string()),
            op_id: OpId::new(),
            precondition,
        })
        .await?;
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

    async fn claim(
        &self,
        tx: &mut S::Tx,
        patient: &str,
        service_point: &str,
    ) -> Result<(), EncountersError> {
        match tx
            .claim_guard(GuardKey(open_guard(patient, service_point)))
            .await
        {
            Ok(()) => Ok(()),
            Err(WriteError::GuardHeld(_)) => {
                Err(EncountersError::AlreadyOpen(service_point.to_owned()))
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Checks a patient in: the Encounter, and the event patient-flow follows.
    pub async fn check_in(
        &self,
        request: &CheckIn,
    ) -> Result<(Encounter, EncounterEvent), EncountersError> {
        let (encounter, event) = check_in(request)?;
        let mut tx = self.store.begin().await?;
        self.claim(&mut tx, &encounter.patient, &encounter.service_point)
            .await?;
        tx.put_encounter(&encounter).await?;
        self.project(&mut tx, &encounter, &event).await?;
        tx.commit().await?;
        Ok((encounter, event))
    }

    async fn load(&self, tx: &mut S::Tx, id: &str) -> Result<Encounter, EncountersError> {
        tx.encounter(id)
            .await?
            .ok_or_else(|| EncountersError::NotFound(id.to_owned()))
    }

    /// Moves Encounter `id` to status `to`; one that ends releases its guard.
    pub async fn transition(
        &self,
        id: &str,
        to: EncounterStatus,
        at: Timestamp,
    ) -> Result<(Encounter, EncounterEvent), EncountersError> {
        let mut tx = self.store.begin().await?;
        let encounter = self.load(&mut tx, id).await?;
        let (next, event) = encounter.transition(to, at)?;
        if next.status.is_final() {
            tx.release_guard(GuardKey(open_guard(&next.patient, &next.service_point)))
                .await?;
        }
        tx.put_encounter(&next).await?;
        self.project(&mut tx, &next, &event).await?;
        tx.commit().await?;
        Ok((next, event))
    }

    /// Sends the patient of Encounter `id` to `service_point`.
    pub async fn move_to(
        &self,
        id: &str,
        service_point: &str,
        at: Timestamp,
    ) -> Result<(Encounter, EncounterEvent), EncountersError> {
        let mut tx = self.store.begin().await?;
        let encounter = self.load(&mut tx, id).await?;
        let (next, event) = encounter.move_to(service_point, at)?;
        tx.release_guard(GuardKey(open_guard(
            &encounter.patient,
            &encounter.service_point,
        )))
        .await?;
        self.claim(&mut tx, &next.patient, &next.service_point)
            .await?;
        tx.put_encounter(&next).await?;
        self.project(&mut tx, &next, &event).await?;
        tx.commit().await?;
        Ok((next, event))
    }

    /// One Encounter.
    pub async fn encounter(&self, id: &str) -> Result<Encounter, EncountersError> {
        let mut tx = self.store.begin().await?;
        self.load(&mut tx, id).await
    }
}
