//! The form designer's *Import FHIR JSON* (studio.md § The form designer; #497, #853), at
//! `POST /packages/{id}/questionnaires/import`. A pasted Questionnaire is read as R5, or as R4
//! converted to R5 by `health-fhir-mapping` (ADR 0015, 0050): R4 conversion lives only there, on
//! the server. The answer is the R5 Questionnaire and the conversion report; a finding that was
//! not converted blocks the import in the designer. Nothing is saved: the designer replaces its
//! form, and the person saves it as any edit.

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use health_access_policy::Subject;
use health_fhir_mapping::questionnaire::{self, Finding, Refusal, Version};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::{Studio, StudioProblem, author, failure};

/// The FHIR version a paste is read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
pub(crate) enum StudioFhirVersion {
    /// Read as it is.
    R5,
    /// Converted to R5.
    R4,
}

/// A pasted Questionnaire to read.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
pub(crate) struct StudioQuestionnaireImportInput {
    /// The text as pasted.
    pub(crate) text: String,
    /// The FHIR version to read it as.
    pub(crate) version: StudioFhirVersion,
}

/// Why a paste was not read at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioImportRefusal {
    NotJson,
    NotQuestionnaire,
    /// Read as R5, it holds R4's choice item types.
    LooksR4,
}

/// Whether a finding was converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioConversionOutcome {
    Converted,
    /// Not converted: it blocks the import.
    Unconverted,
}

/// What a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioConversionCode {
    /// An R4 `choice` item became `coding`, `answerConstraint` `optionsOnly`.
    ChoiceToCoding,
    /// An R4 `open-choice` item became `coding`, `answerConstraint` `optionsOrString`.
    OpenChoiceToCoding,
    /// An element neither version knows (`element`).
    UnknownElement,
    /// Contained resources.
    Contained,
    /// A modifier extension.
    ModifierExtension,
    /// An item type R5 does not have (`type`).
    UnknownType,
}

/// One line of the conversion report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct StudioConversionFinding {
    pub(crate) outcome: StudioConversionOutcome,
    pub(crate) code: StudioConversionCode,
    /// `Questionnaire`, or the item's `linkId` (`?` when it has none).
    pub(crate) at: String,
    /// The element, for `unknown-element`.
    pub(crate) element: Option<String>,
    /// The item type, for `unknown-type`.
    #[serde(rename = "type")]
    pub(crate) item_type: Option<String>,
}

impl From<&Finding> for StudioConversionFinding {
    fn from(finding: &Finding) -> Self {
        let (code, element, item_type) = match finding {
            Finding::ChoiceToCoding { .. } => (StudioConversionCode::ChoiceToCoding, None, None),
            Finding::OpenChoiceToCoding { .. } => {
                (StudioConversionCode::OpenChoiceToCoding, None, None)
            }
            Finding::UnknownElement { element, .. } => (
                StudioConversionCode::UnknownElement,
                Some(element.clone()),
                None,
            ),
            Finding::Contained { .. } => (StudioConversionCode::Contained, None, None),
            Finding::ModifierExtension { .. } => {
                (StudioConversionCode::ModifierExtension, None, None)
            }
            Finding::UnknownType { item_type, .. } => (
                StudioConversionCode::UnknownType,
                None,
                Some(item_type.clone()),
            ),
        };
        Self {
            outcome: if finding.converted() {
                StudioConversionOutcome::Converted
            } else {
                StudioConversionOutcome::Unconverted
            },
            code,
            at: finding.at().to_owned(),
            element,
            item_type,
        }
    }
}

/// A JSON object of any members, or null: a resource, kept whole by the client's schema.
fn any_object() -> utoipa::openapi::schema::Object {
    use utoipa::openapi::schema::{AdditionalProperties, ObjectBuilder, SchemaType, Type};
    ObjectBuilder::new()
        .schema_type(SchemaType::from_iter([Type::Object, Type::Null]))
        .additional_properties(Some(AdditionalProperties::FreeForm(true)))
        .build()
}

/// A paste read: the R5 Questionnaire and its report, or why it was not read.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub(crate) struct StudioQuestionnaireImport {
    /// Why it was not read; the other fields are then empty.
    pub(crate) refusal: Option<StudioImportRefusal>,
    /// The R5 Questionnaire.
    #[schema(schema_with = any_object)]
    pub(crate) questionnaire: Option<Value>,
    /// What was converted, and what could not be.
    pub(crate) report: Vec<StudioConversionFinding>,
    /// Whether something could not be converted, so the import must not go on.
    pub(crate) blocked: bool,
}

/// Documents and serves *Import FHIR JSON*.
#[utoipa::path(post, path = "/api/studio/packages/{id}/questionnaires/import", tag = "studio", operation_id = "importStudioQuestionnaire",
    description = "The form designer's Import FHIR JSON: reads the pasted text as an R5 Questionnaire, or as an R4 one converted to R5 by `health-fhir-mapping`, and answers it with its conversion report. A finding that was not converted blocks the import (`blocked`). Text that is not JSON, not a Questionnaire, or read as R5 but holding R4's choice item types is refused (`refusal`). Nothing is saved.",
    params(("id" = String, Path, description = "`name@version`")),
    request_body = StudioQuestionnaireImportInput,
    responses(
        (status = 200, description = "The paste read, or why it was not", body = StudioQuestionnaireImport),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn import_questionnaire(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(input): Json<StudioQuestionnaireImportInput>,
) -> Response {
    if let Err(response) = author(subject) {
        return *response;
    }
    if let Err(error) = studio.publications.draft_files(&id).await {
        return failure(&error);
    }
    let version = match input.version {
        StudioFhirVersion::R4 => Version::R4,
        StudioFhirVersion::R5 => Version::R5,
    };
    let answer = match questionnaire::read(&input.text, version) {
        Ok(read) => StudioQuestionnaireImport {
            refusal: None,
            blocked: questionnaire::blocks(&read.report),
            report: read.report.iter().map(Into::into).collect(),
            questionnaire: Some(read.questionnaire),
        },
        Err(refusal) => StudioQuestionnaireImport {
            refusal: Some(match refusal {
                Refusal::NotJson => StudioImportRefusal::NotJson,
                Refusal::NotQuestionnaire => StudioImportRefusal::NotQuestionnaire,
                Refusal::LooksR4 => StudioImportRefusal::LooksR4,
            }),
            questionnaire: None,
            report: Vec::new(),
            blocked: true,
        },
    };
    Json(answer).into_response()
}
