//! Self-registration (patient-identity.md § Self-registration, §59; ADR 0029).
//!
//! A patient signed in with their phone (`level1`) submits the self-registration
//! QuestionnaireResponse. [`SelfRegistration::submit`] stores it with its Registration Request
//! (a Task) and routes it after `$match`:
//! - **no candidate:** a new self-registered source Patient, provisional and patient-reported,
//!   and the account's Person linked to it at `level1`, so the patient sees only what they
//!   submitted;
//! - **any candidate:** staff review. A phone account is never linked to an existing Patient
//!   automatically.
//!
//! The account has one Registration Request and one Person, with ids derived from the account,
//! so a retried submission writes nothing new. The client registry is reached through the
//! [`PatientRegistry`] port (contexts compose in the runtime, ADR 0006).

mod fhir;

use std::future::Future;

use health_client_registry::person::{Assurance as LinkAssurance, Person, PortalAccess};
use health_identifiers::{DerivationName, OpId, ResourceId, derive_id};
use health_patient_engagement::registration::{
    Assurance, Matched, Outcome, RegistrationRequest, Route, Status,
};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, StoreError, Transaction, WriteError, WriteRequest,
};
use serde_json::Value;

pub use fhir::{QUESTIONNAIRE, patient_of};

/// A self-registration failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngagementError {
    /// The QuestionnaireResponse is not a self-registration, or lacks a required answer.
    #[error("{0}")]
    Invalid(String),
    /// The account has already submitted its registration.
    #[error("this account has already submitted its registration")]
    AlreadySubmitted,
    /// The client registry failed.
    #[error("client registry: {0}")]
    Registry(String),
    /// The store failed.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for EngagementError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for EngagementError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

/// A `$match` candidate, as the client registry graded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The source Patient's id.
    pub patient: String,
    /// Whether the grade is `certain`.
    pub certain: bool,
}

/// The client registry, as self-registration needs it.
pub trait PatientRegistry: Send + Sync {
    /// The `$match` candidates for `patient` (graded possible or better); `None` when there is
    /// too little to match on.
    fn find(
        &self,
        patient: &Value,
    ) -> impl Future<Output = Result<Option<Vec<Candidate>>, EngagementError>> + Send;
    /// Creates the self-registered source Patient `id` from `patient` (provisional,
    /// patient-reported, never auto-linked); a no-op when it exists.
    fn create_self_registered(
        &self,
        id: ResourceId,
        patient: &Value,
        actor: &str,
    ) -> impl Future<Output = Result<(), EngagementError>> + Send;
}

/// Records each version a use case writes, in its transaction.
pub trait AuditTrail<T: Transaction>: Send + Sync {
    /// `json` was written in `tx` by `actor`, for `reason`.
    fn written(
        &self,
        tx: &mut T,
        actor: &str,
        reason: &str,
        json: &RawJson,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
}

/// What the patient sees of their registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationView {
    /// Where the request is.
    pub status: Status,
    /// The Patient the account is linked to, once there is one.
    pub patient: Option<String>,
    /// What the account may see of it.
    pub access: PortalAccess,
}

/// The self-registration use cases over `store`.
pub struct SelfRegistration<S, R, A> {
    store: S,
    registry: R,
    audit: A,
}

const TASK_RULE: &str = "urn:health:rule/registration-request";
const RESPONSE_RULE: &str = "urn:health:rule/registration-response";
const PERSON_RULE: &str = "urn:health:rule/account-person";
const PATIENT_RULE: &str = "urn:health:rule/self-registered-patient";

fn derived(account: &str, rule: &str) -> ResourceId {
    derive_id(&DerivationName::rule(account, rule))
}

fn kind(name: &str) -> ResourceType {
    name.parse().expect("a FHIR resource type")
}

impl<S, R, A> SelfRegistration<S, R, A>
where
    S: Store,
    R: PatientRegistry,
    A: AuditTrail<S::Tx>,
{
    /// The use cases over `store`, reaching the client registry through `registry`.
    pub fn new(store: S, registry: R, audit: A) -> Self {
        Self {
            store,
            registry,
            audit,
        }
    }

    /// The account's Registration Request id (its Task).
    pub fn request_id(account: &str) -> ResourceId {
        derived(account, TASK_RULE)
    }

    /// The account's Person id.
    pub fn person_id(account: &str) -> ResourceId {
        derived(account, PERSON_RULE)
    }

    async fn write(
        &self,
        tx: &mut S::Tx,
        actor: &str,
        reason: &str,
        resource: &str,
        id: ResourceId,
        json: &Value,
    ) -> Result<(), EngagementError> {
        let resource_type = kind(resource);
        let precondition = match tx.read(&resource_type, id).await? {
            Some(stored) => Precondition::IfMatch(stored.version.version_id),
            None => Precondition::MustNotExist,
        };
        tx.write(WriteRequest {
            resource_type: resource_type.clone(),
            id,
            json: RawJson::from_validated(json.to_string()),
            op_id: OpId::new(),
            precondition,
        })
        .await?;
        let stored = tx
            .read(&resource_type, id)
            .await?
            .ok_or_else(|| EngagementError::Store(format!("written {resource} is missing")))?;
        self.audit.written(tx, actor, reason, &stored.json).await?;
        Ok(())
    }

    async fn read(
        tx: &mut S::Tx,
        resource: &str,
        id: ResourceId,
    ) -> Result<Option<Value>, EngagementError> {
        let Some(stored) = tx.read(&kind(resource), id).await? else {
            return Ok(None);
        };
        if stored.version.deleted {
            return Ok(None);
        }
        serde_json::from_str(stored.json.as_str())
            .map(Some)
            .map_err(|e| EngagementError::Store(e.to_string()))
    }

    /// Submits `response`, the self-registration QuestionnaireResponse of the patient signed in
    /// as `account` with the verified `phone`, and routes it.
    pub async fn submit(
        &self,
        account: &str,
        phone: &str,
        response: &Value,
    ) -> Result<RegistrationView, EngagementError> {
        let patient = patient_of(response, phone)?;
        let task_id = Self::request_id(account);
        let person_id = Self::person_id(account);
        {
            let mut tx = self.store.begin().await?;
            if Self::read(&mut tx, "Task", task_id).await?.is_some() {
                return Err(EngagementError::AlreadySubmitted);
            }
        }
        let response_id = derived(account, RESPONSE_RULE);
        let mut request = RegistrationRequest::new(&task_id.to_string(), account);
        let refused = |e: health_patient_engagement::registration::RequestRefused| {
            EngagementError::Invalid(e.to_string())
        };
        request
            .submit(&format!("QuestionnaireResponse/{response_id}"))
            .map_err(refused)?;
        // The phone's code, checked at sign-in.
        request.verified(Assurance::Level1).map_err(refused)?;
        let matched = match self.registry.find(&patient).await? {
            None => Matched::Possible(Vec::new()),
            Some(candidates) if candidates.is_empty() => Matched::None,
            Some(candidates) => {
                let certain: Vec<&Candidate> = candidates.iter().filter(|c| c.certain).collect();
                match certain.as_slice() {
                    [only] => Matched::Certain(only.patient.clone()),
                    _ => Matched::Possible(candidates.into_iter().map(|c| c.patient).collect()),
                }
            }
        };
        let route = request.matched(&matched).map_err(refused)?;
        let actor = format!("account {account}");
        let mut person = Person::new(&person_id.to_string(), account);
        if route == Route::Create {
            let patient_id = derived(account, PATIENT_RULE);
            self.registry
                .create_self_registered(patient_id, &patient, &actor)
                .await?;
            person
                .link_self_registered(&patient_id.to_string(), LinkAssurance::Level1)
                .map_err(|e| EngagementError::Invalid(e.to_string()))?;
            request
                .complete(Outcome::Created(patient_id.to_string()))
                .map_err(refused)?;
        }
        let mut tx = self.store.begin().await?;
        let response_json = fhir::response_json(response, response_id, person_id);
        self.write(
            &mut tx,
            &actor,
            "self-register",
            "QuestionnaireResponse",
            response_id,
            &response_json,
        )
        .await?;
        self.write(
            &mut tx,
            &actor,
            "self-register",
            "Person",
            person_id,
            &fhir::person_json(&person, phone),
        )
        .await?;
        self.write(
            &mut tx,
            &actor,
            "self-register",
            "Task",
            task_id,
            &fhir::task_json(&request, person_id),
        )
        .await?;
        tx.commit().await?;
        Ok(view(&request, &person))
    }

    /// The account's registration, if it has submitted one.
    pub async fn status(&self, account: &str) -> Result<Option<RegistrationView>, EngagementError> {
        let mut tx = self.store.begin().await?;
        let Some(task) = Self::read(&mut tx, "Task", Self::request_id(account)).await? else {
            return Ok(None);
        };
        let request = fhir::request_of(&task, account)?;
        let person = match Self::read(&mut tx, "Person", Self::person_id(account)).await? {
            Some(json) => fhir::person_of(&json, account)?,
            None => Person::new(&Self::person_id(account).to_string(), account),
        };
        Ok(Some(view(&request, &person)))
    }
}

fn view(request: &RegistrationRequest, person: &Person) -> RegistrationView {
    let patient = match &request.outcome {
        Some(Outcome::Created(id) | Outcome::Linked(id)) => Some(id.clone()),
        None => None,
    };
    let access = patient
        .as_deref()
        .map_or(PortalAccess::None, |id| person.access(id));
    RegistrationView {
        status: request.status,
        patient,
        access,
    }
}
