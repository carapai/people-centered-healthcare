//! The laboratory workflow at the facility authority (#109; resource-ownership.md).
//!
//! Each step of a [`LabTest`] is one transaction holding the test (`lab_test`), its FHIR
//! projections and its outbox event:
//!
//! - the **Specimen** once collected (`unsatisfactory` once rejected);
//! - one laboratory **Observation** per result: `preliminary` until verified, then `final`
//!   (an Observation a correction leaves out is `entered-in-error`, never deleted);
//! - the **DiagnosticReport** (category `LAB`): `registered` once collected, `preliminary` once
//!   resulted, `final` once verified (the verifier as `resultsInterpreter`), `cancelled` when the
//!   specimen is rejected or the test cancelled.
//!
//! Their ids are derived from the test, so every step versions the same resources. Each is tagged
//! with the facility (ADR 0057), names the ServiceRequest (`basedOn`) and the patient.

use health_access_policy::FACILITY_TAG_SYSTEM;
use health_identifiers::{DerivationName, OpId, ResourceId, derive_id};
use health_laboratory::{
    Done, LabEvent, LabRefused, LabStore, LabTest, Order, ResultEntry, ResultValue, Specimen,
    Stage, order,
};
use health_persistence::{
    OutboxEntry, OutboxPayload, Precondition, RawJson, ResourceType, Store, StoreError,
    Transaction, WriteError, WriteRequest,
};
use health_time::Timestamp;
use serde_json::{Value, json};

const REPORT_RULE: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/rules/lab-report";
const RESULT_RULE: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/rules/lab-result";

/// Why a laboratory step failed. Nothing was kept.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LabError {
    /// The domain refused it.
    #[error(transparent)]
    Refused(#[from] LabRefused),
    /// The test was already ordered.
    #[error("test {0} is already ordered")]
    AlreadyOrdered(String),
    /// No such test.
    #[error("test {0} is not known")]
    NotFound(String),
    /// The store failed, or a concurrent change came first; try again.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for LabError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for LabError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

fn store_error(error: impl ToString) -> LabError {
    LabError::Store(error.to_string())
}

/// The DiagnosticReport id of test `id`.
pub fn report_id(test: &str) -> ResourceId {
    derive_id(&DerivationName::rule(test, REPORT_RULE))
}

/// The Observation id of result `index` of test `id`.
pub fn result_id(test: &str, index: usize) -> ResourceId {
    derive_id(&DerivationName::rule(
        format!("{test}#{index}"),
        RESULT_RULE,
    ))
}

fn concept(coded: &health_laboratory::Coded) -> Value {
    match (&coded.system, &coded.code) {
        (Some(system), Some(code)) => {
            json!({ "coding": [{ "system": system, "code": code, "display": coded.text }], "text": coded.text })
        }
        _ => json!({ "text": coded.text }),
    }
}

fn number(text: &str) -> Value {
    serde_json::from_str::<serde_json::Number>(text.trim())
        .map_or_else(|_| Value::String(text.to_owned()), Value::Number)
}

fn base(resource_type: &str, id: impl std::fmt::Display, test: &LabTest) -> Value {
    let mut resource = json!({
        "resourceType": resource_type,
        "id": id.to_string(),
        "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": test.facility }] },
        "subject": { "reference": test.patient },
    });
    if resource_type != "Specimen" {
        resource["basedOn"] = json!([{ "reference": format!("ServiceRequest/{}", test.id) }]);
    } else {
        resource["request"] = json!([{ "reference": format!("ServiceRequest/{}", test.id) }]);
    }
    if let Some(encounter) = &test.encounter
        && resource_type != "Specimen"
    {
        resource["encounter"] = json!({ "reference": encounter });
    }
    resource
}

/// The FHIR Specimen, once collected.
pub fn specimen(test: &LabTest) -> Option<Value> {
    let collected: &Specimen = test.specimen.as_ref()?;
    let mut specimen = base("Specimen", &collected.id, test);
    specimen["status"] = json!(if test.stage == Stage::Rejected {
        "unsatisfactory"
    } else {
        "available"
    });
    specimen["type"] = concept(&collected.kind);
    specimen["collection"] = json!({
        "collector": { "display": collected.collected.by },
        "collectedDateTime": collected.collected.at.to_string()
    });
    if let (Stage::Rejected, Some(reason)) = (test.stage, &test.reason) {
        specimen["condition"] = json!([{ "text": reason }]);
    }
    Some(specimen)
}

fn observation_status(test: &LabTest) -> &'static str {
    match test.stage {
        Stage::Verified | Stage::Reviewed => "final",
        Stage::Rejected | Stage::Cancelled => "cancelled",
        _ => "preliminary",
    }
}

fn observation(test: &LabTest, index: usize, entry: &ResultEntry, flag: &str) -> Value {
    let mut observation = base("Observation", result_id(&test.id, index), test);
    observation["status"] = json!(observation_status(test));
    observation["category"] = json!([{ "coding": [{
        "system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "laboratory"
    }] }]);
    observation["code"] = concept(&entry.code);
    match &entry.value {
        ResultValue::Quantity { value, unit } => {
            observation["valueQuantity"] = json!({ "value": number(value), "unit": unit });
            if let Some(range) = &entry.reference {
                let mut fhir_range = json!({});
                if let Some(low) = &range.low {
                    fhir_range["low"] = json!({ "value": number(low), "unit": unit });
                }
                if let Some(high) = &range.high {
                    fhir_range["high"] = json!({ "value": number(high), "unit": unit });
                }
                if fhir_range.as_object().is_some_and(|o| !o.is_empty()) {
                    observation["referenceRange"] = json!([fhir_range]);
                }
            }
        }
        ResultValue::Coded { concept: found } => {
            observation["valueCodeableConcept"] = concept(found);
        }
        ResultValue::Text { text } => observation["valueString"] = json!(text),
    }
    observation["interpretation"] = json!([{ "coding": [{
        "system": "http://terminology.hl7.org/CodeSystem/v3-ObservationInterpretation", "code": flag
    }] }]);
    if let Some(collected) = &test.specimen {
        observation["specimen"] = json!({ "reference": format!("Specimen/{}", collected.id) });
        observation["effectiveDateTime"] = json!(collected.collected.at.to_string());
    }
    if let Some(resulted) = &test.resulted {
        observation["performer"] = json!([{ "display": resulted.by }]);
    }
    observation
}

/// The laboratory Observations, one per result.
pub fn observations(test: &LabTest) -> Vec<Value> {
    test.results
        .iter()
        .enumerate()
        .map(|(i, r)| observation(test, i, &r.entry, r.flag.code()))
        .collect()
}

/// The DiagnosticReport, once collected.
pub fn report(test: &LabTest) -> Option<Value> {
    let collected = test.specimen.as_ref()?;
    let mut report = base("DiagnosticReport", report_id(&test.id), test);
    report["status"] = json!(match test.stage {
        Stage::Ordered | Stage::Collected => "registered",
        Stage::Resulted => "preliminary",
        Stage::Verified | Stage::Reviewed => "final",
        Stage::Rejected | Stage::Cancelled => "cancelled",
    });
    report["category"] = json!([{ "coding": [{
        "system": "http://terminology.hl7.org/CodeSystem/v2-0074", "code": "LAB"
    }] }]);
    report["code"] = concept(&test.test);
    report["specimen"] = json!([{ "reference": format!("Specimen/{}", collected.id) }]);
    report["effectiveDateTime"] = json!(collected.collected.at.to_string());
    if !test.results.is_empty() {
        report["result"] = json!(
            (0..test.results.len())
                .map(|i| json!({ "reference": format!("Observation/{}", result_id(&test.id, i)) }))
                .collect::<Vec<_>>()
        );
    }
    if let Some(resulted) = &test.resulted {
        report["performer"] = json!([{ "display": resulted.by }]);
    }
    if let Some(verified) = &test.verified {
        report["resultsInterpreter"] = json!([{ "display": verified.by }]);
        report["issued"] = json!(verified.at.to_string());
    }
    Some(report)
}

/// The laboratory use cases over `store`.
pub struct Laboratory<S> {
    store: S,
}

impl<S> Laboratory<S>
where
    S: Store,
    S::Tx: LabStore,
{
    /// The use cases over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    async fn put(tx: &mut S::Tx, resource: &Value) -> Result<(), LabError> {
        let kind: ResourceType = resource["resourceType"]
            .as_str()
            .unwrap_or_default()
            .parse()
            .map_err(store_error)?;
        let id: ResourceId = resource["id"]
            .as_str()
            .unwrap_or_default()
            .parse()
            .map_err(store_error)?;
        let precondition = match tx.read(&kind, id).await? {
            Some(head) => Precondition::IfMatch(head.version.version_id),
            None => Precondition::MustNotExist,
        };
        tx.write(WriteRequest {
            resource_type: kind,
            id,
            json: RawJson::from_validated(resource.to_string()),
            op_id: OpId::new(),
            precondition,
        })
        .await?;
        Ok(())
    }

    /// Writes the test, its projections and its event.
    async fn save(
        tx: &mut S::Tx,
        before: Option<&LabTest>,
        test: &LabTest,
        event: Option<&LabEvent>,
    ) -> Result<(), LabError> {
        tx.put_test(test).await?;
        if let Some(specimen) = specimen(test) {
            Self::put(tx, &specimen).await?;
        }
        for observation in observations(test) {
            Self::put(tx, &observation).await?;
        }
        // Results a correction left out: entered in error, never deleted.
        let had = before.map_or(0, |b| b.results.len());
        for index in test.results.len()..had {
            let kind: ResourceType = "Observation".parse().map_err(store_error)?;
            if let Some(head) = tx.read(&kind, result_id(&test.id, index)).await? {
                let mut gone: Value =
                    serde_json::from_str(head.json.as_str()).map_err(store_error)?;
                gone["status"] = json!("entered-in-error");
                Self::put(tx, &gone).await?;
            }
        }
        if let Some(report) = report(test) {
            Self::put(tx, &report).await?;
        }
        if let Some(event) = event {
            tx.append_outbox(OutboxEntry {
                op_id: OpId::new(),
                payload: OutboxPayload::DomainEvent {
                    name: event.name().to_owned(),
                    body: RawJson::from_validated(
                        serde_json::to_string(event).map_err(store_error)?,
                    ),
                },
            })
            .await?;
        }
        Ok(())
    }

    /// Follows a laboratory order: a test awaiting its specimen.
    pub async fn order(&self, request: Order) -> Result<LabTest, LabError> {
        let test = order(request)?;
        let mut tx = self.store.begin().await?;
        if tx.test(&test.id).await?.is_some() {
            return Err(LabError::AlreadyOrdered(test.id.clone()));
        }
        Self::save(&mut tx, None, &test, None).await?;
        tx.commit().await?;
        Ok(test)
    }

    async fn step(
        &self,
        id: &str,
        change: impl FnOnce(&LabTest) -> Result<(LabTest, LabEvent), LabRefused>,
    ) -> Result<(LabTest, LabEvent), LabError> {
        let mut tx = self.store.begin().await?;
        let current = tx
            .test(id)
            .await?
            .ok_or_else(|| LabError::NotFound(id.to_owned()))?;
        let (next, event) = change(&current)?;
        Self::save(&mut tx, Some(&current), &next, Some(&event)).await?;
        tx.commit().await?;
        Ok((next, event))
    }

    /// The specimen collected.
    pub async fn collect(
        &self,
        id: &str,
        specimen: Specimen,
    ) -> Result<(LabTest, LabEvent), LabError> {
        self.step(id, |t| t.collect(specimen)).await
    }

    /// Results entered or corrected.
    pub async fn result(
        &self,
        id: &str,
        entries: Vec<ResultEntry>,
        done: Done,
    ) -> Result<(LabTest, LabEvent), LabError> {
        self.step(id, |t| t.result(entries, done)).await
    }

    /// Results verified by a second person.
    pub async fn verify(&self, id: &str, done: Done) -> Result<(LabTest, LabEvent), LabError> {
        self.step(id, |t| t.verify(done)).await
    }

    /// Results reviewed by the clinician.
    pub async fn review(&self, id: &str, done: Done) -> Result<(LabTest, LabEvent), LabError> {
        self.step(id, |t| t.review(done)).await
    }

    /// The specimen rejected.
    pub async fn reject(
        &self,
        id: &str,
        reason: &str,
        at: Timestamp,
    ) -> Result<(LabTest, LabEvent), LabError> {
        self.step(id, |t| t.reject(reason, at)).await
    }

    /// The test cancelled.
    pub async fn cancel(
        &self,
        id: &str,
        reason: &str,
        at: Timestamp,
    ) -> Result<(LabTest, LabEvent), LabError> {
        self.step(id, |t| t.cancel(reason, at)).await
    }

    /// One test.
    pub async fn test(&self, id: &str) -> Result<LabTest, LabError> {
        let mut tx = self.store.begin().await?;
        tx.test(id)
            .await?
            .ok_or_else(|| LabError::NotFound(id.to_owned()))
    }
}
