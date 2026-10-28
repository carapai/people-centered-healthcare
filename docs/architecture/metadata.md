# Metadata layer

Status: **accepted** (ADRs 0003, 0024, 0025). Produced while resolving the Wayfinder ticket [How are terminology, metadata and configuration packaged and published?](https://github.com/carapai/people-centered-healthcare/issues/23). Related docs:
- packaging, lifecycle and activation: [configuration.md](configuration.md);
- the terminology layer: [terminology.md](terminology.md);
- extraction: [questionnaire-extraction.md](questionnaire-extraction.md).

Background: [docs/research/metadata-terminology.md](../research/metadata-terminology.md).

## Scope

Metadata defines **clinical structure** (§41). It is owned by the `metadata` Module (Terminology & Metadata).

| Artefact | Notes |
|---|---|
| StructureDefinition / profiles | National core and program profiles; validation tier 3 |
| SearchParameter | Custom parameters; activation starts a reindex (persistence.md) |
| ImplementationGuide, CapabilityStatement | Package and server descriptions |
| Questionnaire | Clinical structure only, plus intrinsic SDC rendering such as `questionnaire-itemControl`. Layout lives in configuration |
| Extraction Mapping | questionnaire-extraction.md |
| PlanDefinition, ActivityDefinition, Library, Measure | Protocol and indicator definitions |
| Program definitions, workflow definitions | Their models are decided in [What is the generic workflow engine's definition and execution model?](https://github.com/carapai/people-centered-healthcare/issues/24) |

## Versioning

- **Own semver.** Every artefact has its own semver and canonical `url|version`. Every package has its own semver too.
- **Immutable once published.** A published `url|version` never changes content, and nodes verify the package hash.
- **Bump rules,** enforced by a diff at validation:

  | Change | Bump |
  |---|---|
  | Display text, help text, typo | patch |
  | New optional item, new code in a binding, relaxed constraint | minor |
  | Removed item or code, changed answer type, tightened profile, changed extraction target | major |

## Historical interpretation

Every record pins the definitions it was created under:

| Record field | Pins |
|---|---|
| `QuestionnaireResponse.questionnaire` | Questionnaire `url\|version` |
| QuestionnaireResponse extension | Extraction Mapping `url\|version` |
| `meta.profile` | Profile `url\|version` |
| Workflow instance | Workflow definition `url\|version` |
| `Coding.version` | Local versioned CodeSystems |

- **Rendering and validation** of an old record always use its pinned versions.
- **Retention.** Nodes keep every package version ever activated in their scope for as long as any data references it.
- **In-flight workflows** stay on their pinned version. Migration rules belong to the workflow-engine ticket.

## Importing R4 content

SMART Guidelines, SDC, CRMI and CPG content is R4.
- **Conversion at import.** Imports into a draft are converted to R5, for example RequestGroup → RequestOrchestration, CarePlan activity restructuring, Questionnaire differences and extension-URL checks.
- **Report.** Each import produces a conversion report, and any unconverted element blocks validation.
- **Source link.** An imported artefact keeps its source canonical in `relatedArtifact` (type `derived-from`).
- **Splitting.** An SDC Questionnaire with contained templates is split into a Questionnaire and an Extraction Mapping.
