# Research: workflow, SDC extraction and WHO SMART Guidelines

A condensed version of the research ticket [How do FHIR R5 workflow, SDC extraction and WHO SMART Guidelines fit a generic engine?](https://github.com/carapai/people-centered-healthcare/issues/7). The full findings, with citations, are on the unmerged branch `research/clinical-workflows`. The research feeds three decisions:
- extraction: [docs/architecture/questionnaire-extraction.md](../architecture/questionnaire-extraction.md) (ADRs 0022, 0023);
- packaging: [How are terminology, metadata and configuration packaged and published?](https://github.com/carapai/people-centered-healthcare/issues/23);
- the workflow engine: [What is the generic workflow engine's definition and execution model?](https://github.com/carapai/people-centered-healthcare/issues/24).

## SDC extraction and population (SDC 4.0.0, FHIR R4)

- **When.** Extraction "only makes sense" once a QuestionnaireResponse is completed. The QuestionnaireResponse is kept as the source of truth.
- **Four extraction approaches:**
  - **Observation-based:** `observationExtract` on coded items. Observation only.
  - **Definition-based:** `definitionExtract` plus `item.definition` paths. Profile-aware, but needs StructureDefinition snapshot walking. `itemExtractionContext` is deprecated.
  - **Template-based:** contained templates with `templateExtractContext` / `templateExtractValue` FHIRPath. An empty result removes a property, and multiple results repeat it. The output is a transaction Bundle (PUT when an id is present). SDC says to log errors and continue. "Unable to leverage any of the information inside a profile".
  - **StructureMap:** the most powerful, but it needs an FML engine.
- **Restricted context.** FHIRPath in `$extract` has no access to `launchContext`, `initialExpression` or variables, only to the QuestionnaireResponse, the Questionnaire and extract extensions.
- **Population:**
  - observation-based;
  - expression-based (`launchContext`, `variable`, `initialExpression`, `candidateExpression`; FHIRPath, CQL or FHIR query);
  - StructureMap, which takes precedence over the others.
- **Our pick:** template-based extraction as the default, observation-based for simple items, definition-based later, and StructureMap avoided.

## Workflow (FHIR R5)

- **Definitions only.** FHIR defines definitions, requests and events, but explicitly no execution model: "FHIR does not need to be used for the execution of workflow".
- **`$apply` is transient:** `PlanDefinition/$apply` returns a RequestOrchestration and persists nothing.
- **CarePlan** groups a patient's plan.
- **Task** has 12 fixed status codes; domain detail goes in `businessStatus`.
- **EpisodeOfCare** is a tracking resource for program enrolment, linked via `Encounter.episodeOfCare`. It has no HIV or TB example.
- **Implication:** the engine owns its own state machine instances and history, and projects them to Task, CarePlan, RequestOrchestration and EpisodeOfCare.

## Lifecycle and packaging

- **CRMI 2.0.0 (R4) lifecycle:**
  - status draft → active → retired, never backwards;
  - canonical `url|version`, with semantic versioning recommended;
  - manifest Libraries pin dependency versions and stable ValueSet expansions;
  - released content is immutable.
- **FHIR NPM packages:** a tarball with `package.json`, semantic versioning, dependencies on `hl7.fhir.r5.core`, and `fhirVersions`. Immutability is conventional, not stated in the spec.

## WHO SMART Guidelines

- **Layers.** L2 is the DAK: personas, BPMN processes, data dictionary, DMN-style decisions, indicators. L3 is the FHIR IG: profiles, Questionnaires, CQL, PlanDefinitions, Measures.
- **HIV:** the L3 IG 1.0.0 is released.
- **ANC:** only a draft CI build exists.
- **TB:** the L2 DAK is published, but there is no L3 IG.
- **Every SMART, SDC, CRMI and CPG IG is R4**, which is the largest risk for an R5-native platform.

## UgandaEMR

- **Programs.** OpenMRS Programs are used for HIV, TB, MCH, Nutrition and the DSDM models, with CIEL concepts.
- **Lost to follow-up.** Scheduled tasks exit patients as lost to follow-up after N days without an encounter (TB defaults to 60). This maps to EpisodeOfCare plus a timer-driven engine transition.

## Risks

- **R4 → R5 conversion** of imported SDC, SMART and CPG content (RequestGroup → RequestOrchestration, Questionnaire differences, extension URLs).
- **CQL.** No confirmed production Rust CQL engine exists (unverified).
- **UgandaEMR migration** needs ConceptMaps from CIEL, and a mapping from PatientProgram to EpisodeOfCare.
