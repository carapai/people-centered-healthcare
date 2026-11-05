//! Programs at the facility authority (program-designer.md; #116).
//!
//! - **Definitions** ([`program`]): a published ProgramPlanDefinition (ADR 0049):
//!   - the EpisodeOfCare type (`program-episode-type`) and eligibility (`program-eligibility`);
//!   - its `relatedArtifact`s: `citation`, `documentation` and `justification` entries are the
//!     approved protocol sources, and `composed-of` entries name its regimens
//!     (`ActivityDefinition`s), Workflow Definitions (`Library`s) and forms (`Questionnaire`s).
//! - **Eligibility** ([`eligible`]): the FHIRPath over the patient, `%patient` being the
//!   Patient; true only for a single `true`.
//! - **Episodes** ([`Programs`]): enrolment, status and transfer, each one transaction holding
//!   the episode's guard (one open episode per patient per program), its FHIR EpisodeOfCare
//!   (tagged with its facility, ADR 0057) and its outbox event. The EpisodeOfCare is the
//!   module's record: [`fhir`] writes it and [`episode`] reads it back.
//!
//! Starting the program's workflows with the episode as their context is the node's: workflow
//! use cases are another context's.

use health_access_policy::FACILITY_TAG_SYSTEM;
use health_fhir_validation::{FhirPathEngine, FhirPathItem};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    GuardKey, OutboxEntry, OutboxPayload, Precondition, RawJson, ResourceType, Store, StoreError,
    Transaction, WriteError, WriteRequest,
};
use health_programs::{
    Coded, Enrolment, Episode, EpisodeStatus, Program, ProgramEvent, ProgramRefused, enrol,
    open_guard,
};
use health_time::Timestamp;
use serde_json::{Value, json};

const BASE: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition";
/// The ProgramPlanDefinition's EpisodeOfCare type.
pub const EPISODE_TYPE: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/program-episode-type";
/// The ProgramPlanDefinition's eligibility.
pub const ELIGIBILITY: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/program-eligibility";

fn extension_url(name: &str) -> String {
    format!("{BASE}/{name}")
}

/// Why a program step failed. Nothing was kept.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProgramError {
    /// The domain refused it.
    #[error(transparent)]
    Refused(#[from] ProgramRefused),
    /// Not a usable ProgramPlanDefinition.
    #[error("not a program definition: {0}")]
    NotAProgram(String),
    /// The eligibility could not be decided.
    #[error("the eligibility `{expression}` does not give true or false: {reason}")]
    Eligibility {
        /// The expression.
        expression: String,
        /// Why.
        reason: String,
    },
    /// The patient is already in the program.
    #[error("the patient already has an open episode of {0}")]
    AlreadyEnrolled(String),
    /// No such episode.
    #[error("episode {0} is not known")]
    NotFound(String),
    /// The store failed, or a concurrent change came first; try again.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for ProgramError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for ProgramError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

fn coded(concept: &Value) -> Coded {
    let coding = &concept["coding"][0];
    Coded {
        text: text(&coding["display"])
            .or_else(|| text(&concept["text"]))
            .or_else(|| text(&coding["code"]))
            .unwrap_or_default(),
        system: text(&coding["system"]),
        code: text(&coding["code"]),
    }
}

fn concept(coded: &Coded) -> Value {
    match (&coded.system, &coded.code) {
        (Some(system), Some(code)) => {
            json!({ "coding": [{ "system": system, "code": code, "display": coded.text }], "text": coded.text })
        }
        _ => json!({ "text": coded.text }),
    }
}

fn extension<'a>(resource: &'a Value, url: &str) -> Option<&'a Value> {
    resource["extension"]
        .as_array()?
        .iter()
        .find(|e| e["url"] == url)
}

/// The program a ProgramPlanDefinition defines.
pub fn program(plan: &Value) -> Result<Program, ProgramError> {
    let refused = |why: &str| ProgramError::NotAProgram(why.to_owned());
    if plan["resourceType"] != "PlanDefinition" {
        return Err(refused("not a PlanDefinition"));
    }
    let is_protocol = plan["type"]["coding"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|c| c["code"] == "clinical-protocol");
    if !is_protocol {
        return Err(refused("its type is not clinical-protocol"));
    }
    let url = text(&plan["url"]).ok_or_else(|| refused("no url"))?;
    let version = text(&plan["version"]).ok_or_else(|| refused("no version"))?;
    let episode_type = extension(plan, EPISODE_TYPE)
        .map(|e| coded(&e["valueCodeableConcept"]))
        .filter(|c| !c.text.is_empty())
        .ok_or_else(|| refused("no program-episode-type"))?;
    let eligibility = extension(plan, ELIGIBILITY)
        .and_then(|e| text(&e["valueExpression"]["expression"]))
        .filter(|e| !e.trim().is_empty());
    let mut program = Program {
        url,
        version,
        episode_type,
        eligibility,
        workflows: Vec::new(),
        regimens: Vec::new(),
        questionnaires: Vec::new(),
        sources: Vec::new(),
    };
    for artifact in plan["relatedArtifact"].as_array().into_iter().flatten() {
        let named = text(&artifact["resource"]);
        match artifact["type"].as_str() {
            Some("citation" | "documentation" | "justification") => {
                if let Some(source) = named
                    .or_else(|| text(&artifact["document"]["url"]))
                    .or_else(|| text(&artifact["citation"]))
                {
                    program.sources.push(source);
                }
            }
            Some("composed-of") => match named {
                Some(r) if r.contains("/ActivityDefinition/") => program.regimens.push(r),
                Some(r) if r.contains("/Library/") => program.workflows.push(r),
                Some(r) if r.contains("/Questionnaire/") => program.questionnaires.push(r),
                _ => {}
            },
            _ => {}
        }
    }
    Ok(program)
}

/// Whether `patient` meets `program`'s eligibility; a program without one takes everyone.
pub async fn eligible(
    engine: &dyn FhirPathEngine,
    program: &Program,
    patient: &Value,
) -> Result<bool, ProgramError> {
    let Some(expression) = &program.eligibility else {
        return Ok(true);
    };
    // The patient is the evaluation root, so `%patient` is `%resource`.
    let rooted = vec![expression.replace("%patient", "%resource")];
    let failed = |reason: String| ProgramError::Eligibility {
        expression: expression.clone(),
        reason,
    };
    let mut results = engine.evaluate_many(patient, &rooted).await;
    match results.pop() {
        Some(Ok(items)) => match items.as_slice() {
            [] => Ok(false),
            [FhirPathItem::Boolean(b)] => Ok(*b),
            other => Err(failed(format!("it gave {other:?}"))),
        },
        Some(Err(failure)) => Err(failed(failure.0)),
        None => Err(failed("no result".into())),
    }
}

/// The FHIR R5 EpisodeOfCare of `episode`.
pub fn fhir(episode: &Episode) -> Value {
    let mut period = json!({ "start": episode.start.to_string() });
    if let Some(end) = episode.end {
        period["end"] = json!(end.to_string());
    }
    let mut extensions = vec![
        json!({ "url": extension_url("episode-program"), "valueCanonical": episode.program }),
        json!({ "url": extension_url("episode-enrolled-by"), "valueString": episode.by }),
    ];
    if let Some(from) = &episode.transferred_from {
        extensions.push(json!({ "url": extension_url("episode-transferred-from"), "valueReference": { "reference": from } }));
    }
    if let Some(to) = &episode.transferred_to {
        extensions
            .push(json!({ "url": extension_url("episode-transferred-to"), "valueString": to }));
    }
    json!({
        "resourceType": "EpisodeOfCare",
        "id": episode.id,
        "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": episode.facility }] },
        "extension": extensions,
        "status": episode.status.code(),
        "type": [concept(&episode.episode_type)],
        "patient": { "reference": episode.patient },
        "period": period
    })
}

/// The episode an EpisodeOfCare written by [`fhir`] holds.
pub fn episode(resource: &Value) -> Option<Episode> {
    let facility = resource["meta"]["tag"]
        .as_array()?
        .iter()
        .find(|t| t["system"] == FACILITY_TAG_SYSTEM)
        .and_then(|t| text(&t["code"]))?;
    let ext = |name: &str| extension(resource, &extension_url(name));
    Some(Episode {
        id: text(&resource["id"])?,
        patient: text(&resource["patient"]["reference"])?,
        program: ext("episode-program").and_then(|e| text(&e["valueCanonical"]))?,
        episode_type: coded(&resource["type"][0]),
        facility,
        status: EpisodeStatus::from_code(resource["status"].as_str()?)?,
        start: resource["period"]["start"].as_str()?.parse().ok()?,
        end: resource["period"]["end"]
            .as_str()
            .and_then(|e| e.parse().ok()),
        by: ext("episode-enrolled-by")
            .and_then(|e| text(&e["valueString"]))
            .unwrap_or_default(),
        transferred_from: ext("episode-transferred-from")
            .and_then(|e| text(&e["valueReference"]["reference"])),
        transferred_to: ext("episode-transferred-to").and_then(|e| text(&e["valueString"])),
    })
}

fn store_error(error: impl ToString) -> ProgramError {
    ProgramError::Store(error.to_string())
}

/// The programs use cases over `store`.
pub struct Programs<S> {
    store: S,
}

impl<S: Store> Programs<S> {
    /// The use cases over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    fn kind() -> Result<ResourceType, ProgramError> {
        "EpisodeOfCare".parse().map_err(store_error)
    }

    fn id(text: &str) -> Result<ResourceId, ProgramError> {
        text.parse()
            .map_err(|_| ProgramError::NotFound(text.to_owned()))
    }

    /// Writes the EpisodeOfCare and the outbox event.
    async fn save(
        tx: &mut S::Tx,
        episode: &Episode,
        event: &ProgramEvent,
        precondition: Precondition,
    ) -> Result<(), ProgramError> {
        tx.write(WriteRequest {
            resource_type: Self::kind()?,
            id: Self::id(&episode.id)?,
            json: RawJson::from_validated(fhir(episode).to_string()),
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

    /// Enrols a patient, whose eligibility the caller decided with [`eligible`].
    pub async fn enrol(
        &self,
        request: &Enrolment<'_>,
    ) -> Result<(Episode, ProgramEvent), ProgramError> {
        let (episode, event) = enrol(request)?;
        let mut tx = self.store.begin().await?;
        match tx
            .claim_guard(GuardKey(open_guard(&episode.patient, &episode.program)))
            .await
        {
            Ok(()) => {}
            Err(WriteError::GuardHeld(_)) => {
                return Err(ProgramError::AlreadyEnrolled(episode.program.clone()));
            }
            Err(error) => return Err(error.into()),
        }
        Self::save(&mut tx, &episode, &event, Precondition::MustNotExist).await?;
        tx.commit().await?;
        Ok((episode, event))
    }

    async fn load(tx: &mut S::Tx, id: &str) -> Result<(Episode, Precondition), ProgramError> {
        let head = tx
            .read(&Self::kind()?, Self::id(id)?)
            .await?
            .filter(|h| !h.version.deleted)
            .ok_or_else(|| ProgramError::NotFound(id.to_owned()))?;
        let value: Value = serde_json::from_str(head.json.as_str()).map_err(store_error)?;
        let found = episode(&value).ok_or_else(|| ProgramError::NotFound(id.to_owned()))?;
        Ok((found, Precondition::IfMatch(head.version.version_id)))
    }

    async fn step(
        &self,
        id: &str,
        change: impl FnOnce(&Episode) -> Result<(Episode, ProgramEvent), ProgramRefused>,
    ) -> Result<(Episode, ProgramEvent), ProgramError> {
        let mut tx = self.store.begin().await?;
        let (current, precondition) = Self::load(&mut tx, id).await?;
        let (next, event) = change(&current)?;
        if current.status.is_open() && !next.status.is_open() {
            tx.release_guard(GuardKey(open_guard(&next.patient, &next.program)))
                .await?;
        }
        Self::save(&mut tx, &next, &event, precondition).await?;
        tx.commit().await?;
        Ok((next, event))
    }

    /// Moves episode `id` to status `to`; one that ends releases its guard.
    pub async fn transition(
        &self,
        id: &str,
        to: EpisodeStatus,
        at: Timestamp,
    ) -> Result<(Episode, ProgramEvent), ProgramError> {
        self.step(id, |e| e.transition(to, at)).await
    }

    /// Finishes episode `id` here: the patient moved to `facility`.
    pub async fn transfer_out(
        &self,
        id: &str,
        facility: &str,
        at: Timestamp,
    ) -> Result<(Episode, ProgramEvent), ProgramError> {
        self.step(id, |e| e.transfer_out(facility, at)).await
    }

    /// One episode.
    pub async fn episode(&self, id: &str) -> Result<Episode, ProgramError> {
        let mut tx = self.store.begin().await?;
        Ok(Self::load(&mut tx, id).await?.0)
    }
}
