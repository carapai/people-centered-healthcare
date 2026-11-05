//! Care plans at the facility authority (#110).
//!
//! - **The record** is the FHIR R5 CarePlan ([`fhir`]; [`care_plan`] reads it back): `intent`
//!   `plan`, tagged with its facility (ADR 0057). Its conditions are `addresses`
//!   (CodeableReferences) and its activities `activity.plannedActivityReference`.
//! - **Each change** ([`CarePlans`]) is one transaction holding the CarePlan and
//!   `care-plans.changed.v1`.
//! - **Only the patient's own:** a Condition or request linked to a plan must exist, be about the
//!   plan's patient, and be at its facility.

use health_access_policy::FACILITY_TAG_SYSTEM;
use health_care_plans::{CarePlan, Draft, PlanRefused, PlanStatus, draft};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    OutboxEntry, OutboxPayload, Precondition, RawJson, ResourceType, Store, StoreError,
    Transaction, WriteError, WriteRequest,
};
use health_time::Timestamp;
use serde_json::{Value, json};

const AUTHOR_EXTENSION: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/care-plan-author";

/// Why a care-plan step failed. Nothing was kept.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CarePlanError {
    /// The domain refused it.
    #[error(transparent)]
    Refused(#[from] PlanRefused),
    /// A linked resource that is not the patient's here.
    #[error("{0} is not this patient's record here")]
    NotThePatients(String),
    /// A plan with that id exists.
    #[error("care plan {0} exists")]
    Exists(String),
    /// No such plan.
    #[error("care plan {0} is not known")]
    NotFound(String),
    /// The store failed, or a concurrent change came first; try again.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for CarePlanError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for CarePlanError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

fn store_error(error: impl ToString) -> CarePlanError {
    CarePlanError::Store(error.to_string())
}

fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

/// The FHIR R5 CarePlan of `plan`.
pub fn fhir(plan: &CarePlan) -> Value {
    let mut period = json!({ "start": plan.created.to_string() });
    if let Some(ended) = plan.ended {
        period["end"] = json!(ended.to_string());
    }
    let mut resource = json!({
        "resourceType": "CarePlan",
        "id": plan.id,
        "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": plan.facility }] },
        "extension": [{ "url": AUTHOR_EXTENSION, "valueString": plan.author }],
        "status": plan.status.code(),
        "intent": "plan",
        "title": plan.title,
        "subject": { "reference": plan.patient },
        "period": period,
        "created": plan.created.to_string(),
        "custodian": { "display": plan.author }
    });
    if let Some(description) = &plan.description {
        resource["description"] = json!(description);
    }
    if !plan.addresses.is_empty() {
        resource["addresses"] = json!(
            plan.addresses
                .iter()
                .map(|c| json!({ "reference": { "reference": c } }))
                .collect::<Vec<_>>()
        );
    }
    if !plan.activities.is_empty() {
        resource["activity"] = json!(
            plan.activities
                .iter()
                .map(|a| json!({ "plannedActivityReference": { "reference": a } }))
                .collect::<Vec<_>>()
        );
    }
    resource
}

/// The plan a CarePlan written by [`fhir`] holds.
pub fn care_plan(resource: &Value) -> Option<CarePlan> {
    if resource["resourceType"] != "CarePlan" {
        return None;
    }
    let facility = resource["meta"]["tag"]
        .as_array()?
        .iter()
        .find(|t| t["system"] == FACILITY_TAG_SYSTEM)
        .and_then(|t| text(&t["code"]))?;
    let author = resource["extension"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|e| e["url"] == AUTHOR_EXTENSION)
        .and_then(|e| text(&e["valueString"]))
        .unwrap_or_default();
    Some(CarePlan {
        id: text(&resource["id"])?,
        patient: text(&resource["subject"]["reference"])?,
        facility,
        title: text(&resource["title"])?,
        description: text(&resource["description"]),
        status: PlanStatus::from_code(resource["status"].as_str()?)?,
        created: resource["created"].as_str()?.parse().ok()?,
        ended: resource["period"]["end"]
            .as_str()
            .and_then(|e| e.parse().ok()),
        author,
        addresses: resource["addresses"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| text(&a["reference"]["reference"]))
            .collect(),
        activities: resource["activity"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| text(&a["plannedActivityReference"]["reference"]))
            .collect(),
    })
}

/// The care-plan use cases over `store`.
pub struct CarePlans<S> {
    store: S,
}

fn kind() -> Result<ResourceType, CarePlanError> {
    "CarePlan".parse().map_err(store_error)
}

impl<S: Store> CarePlans<S> {
    /// The use cases over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    async fn save(
        tx: &mut S::Tx,
        plan: &CarePlan,
        precondition: Precondition,
    ) -> Result<(), CarePlanError> {
        let id: ResourceId = plan
            .id
            .parse()
            .map_err(|_| CarePlanError::NotFound(plan.id.clone()))?;
        match tx
            .write(WriteRequest {
                resource_type: kind()?,
                id,
                json: RawJson::from_validated(fhir(plan).to_string()),
                op_id: OpId::new(),
                precondition,
            })
            .await
        {
            Ok(_) => {}
            Err(WriteError::PreconditionFailed { .. })
                if precondition == Precondition::MustNotExist =>
            {
                return Err(CarePlanError::Exists(plan.id.clone()));
            }
            Err(error) => return Err(error.into()),
        }
        let body = json!({ "carePlan": plan.id, "status": plan.status.code() });
        tx.append_outbox(OutboxEntry {
            op_id: OpId::new(),
            payload: OutboxPayload::DomainEvent {
                name: "care-plans.changed.v1".into(),
                body: RawJson::from_validated(body.to_string()),
            },
        })
        .await?;
        Ok(())
    }

    /// A draft plan.
    pub async fn draft(&self, request: Draft) -> Result<CarePlan, CarePlanError> {
        let plan = draft(request)?;
        let mut tx = self.store.begin().await?;
        Self::save(&mut tx, &plan, Precondition::MustNotExist).await?;
        tx.commit().await?;
        Ok(plan)
    }

    async fn load(tx: &mut S::Tx, id: &str) -> Result<(CarePlan, Precondition), CarePlanError> {
        let resource_id: ResourceId = id
            .parse()
            .map_err(|_| CarePlanError::NotFound(id.to_owned()))?;
        let head = tx
            .read(&kind()?, resource_id)
            .await?
            .filter(|h| !h.version.deleted)
            .ok_or_else(|| CarePlanError::NotFound(id.to_owned()))?;
        let value: Value = serde_json::from_str(head.json.as_str()).map_err(store_error)?;
        let plan = care_plan(&value).ok_or_else(|| CarePlanError::NotFound(id.to_owned()))?;
        Ok((plan, Precondition::IfMatch(head.version.version_id)))
    }

    /// Whether `reference` is the patient's record at the plan's facility.
    async fn theirs(tx: &mut S::Tx, plan: &CarePlan, reference: &str) -> Result<(), CarePlanError> {
        let not_theirs = || CarePlanError::NotThePatients(reference.to_owned());
        let (type_name, id) = reference.split_once('/').ok_or_else(not_theirs)?;
        let (Ok(resource_type), Ok(id)) =
            (type_name.parse::<ResourceType>(), id.parse::<ResourceId>())
        else {
            return Err(not_theirs());
        };
        let head = tx
            .read(&resource_type, id)
            .await?
            .filter(|h| !h.version.deleted)
            .ok_or_else(not_theirs)?;
        let value: Value = serde_json::from_str(head.json.as_str()).map_err(store_error)?;
        let about = ["subject", "patient"]
            .iter()
            .filter_map(|f| value[*f]["reference"].as_str())
            .any(|r| r == plan.patient);
        let here = value["meta"]["tag"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|t| t["system"] == FACILITY_TAG_SYSTEM && t["code"] == plan.facility.as_str());
        if about && here {
            Ok(())
        } else {
            Err(not_theirs())
        }
    }

    async fn change(
        &self,
        id: &str,
        link: Option<&str>,
        step: impl FnOnce(&CarePlan) -> Result<CarePlan, PlanRefused>,
    ) -> Result<CarePlan, CarePlanError> {
        let mut tx = self.store.begin().await?;
        let (current, precondition) = Self::load(&mut tx, id).await?;
        let next = step(&current)?;
        if let Some(reference) = link {
            Self::theirs(&mut tx, &current, reference).await?;
        }
        Self::save(&mut tx, &next, precondition).await?;
        tx.commit().await?;
        Ok(next)
    }

    /// Plan `id` with status `to`.
    pub async fn transition(
        &self,
        id: &str,
        to: PlanStatus,
        at: Timestamp,
    ) -> Result<CarePlan, CarePlanError> {
        self.change(id, None, |p| p.transition(to, at)).await
    }

    /// Plan `id` addressing the patient's Condition `condition`.
    pub async fn address(&self, id: &str, condition: &str) -> Result<CarePlan, CarePlanError> {
        self.change(id, Some(condition), |p| p.address(condition))
            .await
    }

    /// Plan `id` with the patient's request `request` among its activities.
    pub async fn plan(&self, id: &str, request: &str) -> Result<CarePlan, CarePlanError> {
        self.change(id, Some(request), |p| p.plan(request)).await
    }

    /// One plan.
    pub async fn care_plan(&self, id: &str) -> Result<CarePlan, CarePlanError> {
        let mut tx = self.store.begin().await?;
        Ok(Self::load(&mut tx, id).await?.0)
    }
}
