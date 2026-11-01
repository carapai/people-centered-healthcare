//! Self-registration (patient-identity.md § Self-registration, §59; ADR 0029).
//!
//! A patient signed in with their phone (`level1`) submits the self-registration
//! QuestionnaireResponse. [`SelfRegistration::submit`] stores it with its Registration Request
//! (a Task) and routes it after `$match`:
//! - **no candidate:** a new self-registered source Patient, provisional and patient-reported,
//!   and the account's Person linked to it at `level1`, so the patient sees only what they
//!   submitted;
//! - **any candidate:** staff review in the registry review queue. A phone account is never
//!   linked to an existing Patient automatically: [`SelfRegistration::review`] links it only
//!   after the reviewer checks the holder's ID in person (`level3`), or creates a new Patient.
//!
//! The account has one Registration Request and one Person, with ids derived from the account,
//! so a retried submission writes nothing new. The client registry is reached through the
//! [`PatientRegistry`] port (contexts compose in the runtime, ADR 0006).

mod fhir;

use std::future::Future;

use health_client_registry::person::{
    Assurance as LinkAssurance, Evidence, Person, PortalAccess, Verification,
};
use health_identifiers::{DerivationName, OpId, ResourceId, derive_id};
use health_patient_engagement::registration::{
    Assurance, Matched, Outcome, RegistrationRequest, Reviewed, Route, Status,
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
    /// The account has no registration, or it is not waiting for review.
    #[error("no registration of this account is waiting for review")]
    NotInReview,
    /// Another reviewer decided it first.
    #[error("this registration has already been reviewed")]
    AlreadyDecided,
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

/// The registry review queue, for registrations that need a person (it is the client
/// registry's; the runtime implements this over it, in the use case's transaction).
pub trait RegistrationReviews<T: Transaction>: Send + Sync {
    /// Queues `request` of `account` for review against `candidates`.
    fn open(
        &self,
        tx: &mut T,
        request: &str,
        account: &str,
        candidates: &[String],
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// Records the reviewer's decision on `request` (`link`, or create); false when another
    /// reviewer decided it first.
    fn close(
        &self,
        tx: &mut T,
        request: &str,
        link: bool,
        reviewer: &str,
    ) -> impl Future<Output = Result<bool, WriteError>> + Send;
}

/// A reviewer's decision on a registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaffDecision {
    /// The account holder is candidate `patient`: the reviewer checked `document` (e.g.
    /// `national-id`) in person, which verifies them at `level3`.
    Link {
        /// The candidate source Patient.
        patient: String,
        /// The ID document checked.
        document: String,
    },
    /// None of the candidates: a new self-registered Patient.
    Create,
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
pub struct SelfRegistration<S, R, A, Q> {
    store: S,
    registry: R,
    audit: A,
    reviews: Q,
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

impl<S, R, A, Q> SelfRegistration<S, R, A, Q>
where
    S: Store,
    R: PatientRegistry,
    A: AuditTrail<S::Tx>,
    Q: RegistrationReviews<S::Tx>,
{
    /// The use cases over `store`, reaching the client registry through `registry` and its
    /// review queue through `reviews`.
    pub fn new(store: S, registry: R, audit: A, reviews: Q) -> Self {
        Self {
            store,
            registry,
            audit,
            reviews,
        }
    }

    /// The account's Registration Request id (its Task).
    pub fn request_id(account: &str) -> ResourceId {
        derived(account, TASK_RULE)
    }

    /// The account's submitted QuestionnaireResponse id.
    pub fn response_id(account: &str) -> ResourceId {
        derived(account, RESPONSE_RULE)
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
            // Certain only when it is the one candidate: a reviewer sees every candidate.
            Some(candidates) => match candidates.as_slice() {
                [only] if only.certain => Matched::Certain(only.patient.clone()),
                _ => Matched::Possible(candidates.into_iter().map(|c| c.patient).collect()),
            },
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
        if let Some(Route::Review(candidates)) = &request.route {
            self.reviews
                .open(&mut tx, &task_id.to_string(), account, candidates)
                .await?;
        }
        tx.commit().await?;
        Ok(view(&request, &person))
    }

    /// A reviewer's decision on `account`'s registration: link it to a candidate after an
    /// in-person ID check, or create a new Patient. The Task, the Person and the queue item
    /// change in one transaction.
    pub async fn review(
        &self,
        account: &str,
        decision: &StaffDecision,
        reviewer: &str,
    ) -> Result<RegistrationView, EngagementError> {
        let task_id = Self::request_id(account);
        let person_id = Self::person_id(account);
        let (mut request, mut person, phone) = {
            let mut tx = self.store.begin().await?;
            let Some(task) = Self::read(&mut tx, "Task", task_id).await? else {
                return Err(EngagementError::NotInReview);
            };
            let request = fhir::request_of(&task, account)?;
            let person_json = Self::read(&mut tx, "Person", person_id)
                .await?
                .ok_or(EngagementError::NotInReview)?;
            let phone = person_json["telecom"][0]["value"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            (request, fhir::person_of(&person_json, account)?, phone)
        };
        let Some(Route::Review(candidates)) = request.route.clone() else {
            return Err(EngagementError::NotInReview);
        };
        if request.status != Status::Review {
            return Err(EngagementError::NotInReview);
        }
        let invalid = |e: &dyn std::fmt::Display| EngagementError::Invalid(e.to_string());
        let mut verification = None;
        match decision {
            StaffDecision::Link { patient, document } => {
                let document = document.trim();
                if !candidates.contains(patient) {
                    return Err(EngagementError::Invalid(
                        "the patient is not one of the request's candidates".into(),
                    ));
                }
                if document.is_empty() {
                    return Err(EngagementError::Invalid(
                        "linking needs the ID document checked in person".into(),
                    ));
                }
                let checked = Verification {
                    evidence: Evidence::InPerson {
                        staff: reviewer.to_owned(),
                        document: document.to_owned(),
                    },
                    at: health_time::Timestamp::now(),
                };
                request
                    .verified(Assurance::Level3)
                    .map_err(|e| invalid(&e))?;
                request
                    .review(Reviewed::Link(patient.clone()), reviewer)
                    .map_err(|e| invalid(&e))?;
                person
                    .link_existing(patient, &checked)
                    .map_err(|e| invalid(&e))?;
                request
                    .complete(Outcome::Linked(patient.clone()))
                    .map_err(|e| invalid(&e))?;
                verification = Some(document.to_owned());
            }
            StaffDecision::Create => {
                request
                    .review(Reviewed::Create, reviewer)
                    .map_err(|e| invalid(&e))?;
                let response = {
                    let mut tx = self.store.begin().await?;
                    Self::read(
                        &mut tx,
                        "QuestionnaireResponse",
                        derived(account, RESPONSE_RULE),
                    )
                    .await?
                    .ok_or(EngagementError::NotInReview)?
                };
                let patient = patient_of(&response, &phone)?;
                let patient_id = derived(account, PATIENT_RULE);
                self.registry
                    .create_self_registered(patient_id, &patient, reviewer)
                    .await?;
                person
                    .link_self_registered(&patient_id.to_string(), LinkAssurance::Level1)
                    .map_err(|e| invalid(&e))?;
                request
                    .complete(Outcome::Created(patient_id.to_string()))
                    .map_err(|e| invalid(&e))?;
            }
        }
        let mut task = fhir::task_json(&request, person_id);
        if let (Some(document), Some(inputs)) = (verification, task["input"].as_array_mut()) {
            inputs.push(serde_json::json!({
                "type": { "text": "verification" },
                "valueString": format!("in-person: {document}"),
            }));
        }
        let mut tx = self.store.begin().await?;
        let link = matches!(decision, StaffDecision::Link { .. });
        if !self
            .reviews
            .close(&mut tx, &task_id.to_string(), link, reviewer)
            .await?
        {
            return Err(EngagementError::AlreadyDecided);
        }
        let person_json = fhir::person_json(&person, &phone);
        self.write(
            &mut tx,
            reviewer,
            "registration-review",
            "Person",
            person_id,
            &person_json,
        )
        .await?;
        self.write(
            &mut tx,
            reviewer,
            "registration-review",
            "Task",
            task_id,
            &task,
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
