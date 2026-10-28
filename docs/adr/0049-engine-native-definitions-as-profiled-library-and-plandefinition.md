# Engine-native Published Definitions are profiled Library resources, programs are profiled PlanDefinitions, and configuration stays JSON pinned by the manifest

| Artefact | Representation |
|---|---|
| **Extraction Mapping** | Profiled `Library`, `type` = platform code `extraction-mapping`. The SDC template resources are **contained**, carrying the standard `templateExtractContext` / `templateExtractValue` extensions. Extensions record the target Questionnaire canonical and version range, plus each template's `mode`, `retraction` and `owner_use_case`. An export operation produces the SDC-native Questionnaire with the templates contained |
| **Matching Rules** | Profiled `Library`, `type` = `matching-rules`. The `content` attachment is `application/vnd.health.matching-rules+json` |
| **Workflow Definition** | Profiled `Library`, `type` = `workflow-definition`. The `content` attachment is `application/vnd.health.workflow+json`. The source PlanDefinition is cited in `relatedArtifact` (`derived-from`) |
| **Program definition** | Profiled `PlanDefinition`, `type` = `clinical-protocol`. Extensions carry the EpisodeOfCare type and FHIRPath eligibility. `relatedArtifact` links its Workflow Definitions, Questionnaires, Extraction Mappings, Measures, ActivityDefinitions and approved protocol sources |
| **Configuration artefacts** (Form Layouts, charge triggers, Financial Gate configurations, override points, settings) | Non-FHIR JSON in `configuration/`, each with `url` and `version`, validated by JSON Schema. The manifest Library pins each one as a `composed-of` `relatedArtifact` with a `document` attachment URL |

**Content and schemas.** Attachment content is inline up to 1 MiB, and goes in the blob store beyond that. JSON Schemas ship with the platform release and are versioned by `formatVersion` (ADR 0024).

**Profiles.** Each kind has its own profile: `ExtractionMappingLibrary`, `MatchingRulesLibrary`, `WorkflowDefinitionLibrary` and `ProgramPlanDefinition`.

**Validation** of a package runs in three steps:
1. FHIR profile validation;
2. JSON Schema validation of the content;
3. the kind-specific publication gates.

**Canonical URLs** are `{canonicalBase}/{ResourceType or config kind}/{name}`, where `canonicalBase` is the publishing package's `canonical` in `package.json`.
- Platform core artefacts and platform CodeSystems use a platform-owned base.
- Country artefacts use the country's base. Who controls that domain is part of the map's national-governance fog.
- References always pin `url|version`.

We chose this because `Library` gives CRMI identity, lifecycle, `relatedArtifact` and contained resources without bending FHIR to express statecharts or scoring rules. PlanDefinition keeps programs readable by SMART- and CPG-aligned tools. Configuration has no clinical meaning, so it needs no FHIR form; it only needs pinning.

## Considered Options

- **Workflow Definitions as PlanDefinition with extensions.** Rejected: parallel regions, timers, gates and compensation would be expressed almost entirely in extensions, so the result is neither interoperable nor readable.
- **`Basic` resources with extensions.** Rejected: there is no lifecycle or canonical semantics, and no tooling.
- **Extraction Mappings as Questionnaires (the SDC-native form).** Rejected as the stored form, because ADR 0022 separates mapping versions from Questionnaire versions. The SDC-native form is offered as an export.
- **Configuration as FHIR resources.** Rejected: it would add profile maintenance with no interoperability benefit.

## Consequences

- The platform publishes a small CodeSystem (Library types, mode codes) and the four profiles in its core package.
- External systems see executable definitions as opaque Libraries. Interoperability comes from the program PlanDefinition and the SDC export.
