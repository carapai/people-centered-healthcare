# Research: FHIR R5 workflow, SDC extraction and WHO SMART Guidelines vs a generic engine

Ticket: #7 (wayfinder:research). Feeds #23, #24, #25. Date: 2026-10-27.
Spec context: `MASTER_PROMPT.md` Parts 5–7 (§§34–52).

## Summary (answer to the question)

FHIR R5 gives us a **vocabulary for definitions, intents and outcomes** of workflow, but explicitly **not an execution
model**. PlanDefinition/ActivityDefinition are the *definitions*; `$apply` turns them into a transient
RequestOrchestration (+ child requests) for one subject; CarePlan groups a patient's plan; Task tracks fulfilment of one
step with a fixed 12-code status set plus a free `businessStatus`. Nothing in R5 models queues, timers, escalation,
compensation, parallel join semantics, or "where is this patient in OPD right now" as durable engine state. So the
generic Rust engine should **own its own execution model (state machine instances + history)** and **project** to FHIR:
Task for each human/system step, RequestOrchestration/CarePlan for the plan, EpisodeOfCare for program enrolment, and
clinical requests/events as the real outputs. This matches MASTER_PROMPT §47 ("workflow state is different from clinical
resource status") and §48.

For forms, SDC defines four extraction approaches (observation-, definition-, template-, StructureMap-based) and three
population approaches (observation-, expression-, StructureMap-based). For a Rust engine with no Java StructureMap
runtime, **template-based extraction (FHIRPath + contained template resources -> transaction Bundle) plus
observation-based extraction for simple coded items** is the pragmatic default; definition-based is a later add; StructureMap
should be avoided initially.

WHO SMART Guidelines are layered L1 (narrative) -> L2 (DAK, software-neutral requirements: personas, BPMN business
processes, data dictionary, DMN-style decision tables, indicators) -> L3 (FHIR IG: profiles, Questionnaires, CQL
Libraries, PlanDefinitions, Measures) -> L4 (executable reference software) -> L5 (dynamic/precision). L2 business
processes are what our engine's workflow definitions should be authored from; L3 artefacts are importable as
knowledge packages. **All SMART, SDC, CRMI and CPG IGs are FHIR R4-based**; this is the single largest risk for an
R5-native platform.

Packaging: FHIR NPM packages (tarball + `package.json`, semver, dependencies) for distribution; CRMI for lifecycle
(draft -> active -> retired, no back-transitions), canonical `url|version` identity, semver, and manifest Libraries that
pin dependency versions (including ValueSet expansions). This maps almost 1:1 onto MASTER_PROMPT §45's
Draft -> ... -> Publication -> Supersession pipeline.

UgandaEMR (OpenMRS) models ART/TB as **OpenMRS Programs** (HIV Program, TB Program, MCH, Nutrition, DSDM sub-programs)
with patient-program enrolment, outcome concepts, and scheduled Java tasks that exit patients as lost-to-follow-up after N
days without an encounter. That is program enrolment + state, which maps to EpisodeOfCare (+ statusHistory) and a
timer-driven engine transition, not to a separate per-program engine.

## Cited findings

### 1. FHIR R5 workflow module

- Workflow resources fall into **definitions** ("activities that could be performed in a time and subject-independent
  manner"), **requests** and **events**; Task "takes on characteristics of both 'requests' and 'events'".
  "FHIR does not define a 'one size fits all' solution for workflow architecture" and "FHIR does not need to be used
  for the execution of workflow." — https://hl7.org/fhir/R5/workflow.html
- PlanDefinition "defines the interrelationships of steps and the rules around their execution"; ActivityDefinition
  "defines an activity to be performed as a single step." — https://hl7.org/fhir/R5/workflow.html
- **`PlanDefinition/$apply`** (R5, Trial Use): inputs `subject` (1..*), `encounter`, `practitioner`, `organization`,
  `setting`, `userTaskContext`, etc.; returns per subject "a Bundle ... [whose] first entry [is] a RequestOrchestration";
  applicable actions are applied via ActivityDefinition `$apply`; dynamicValue expressions see inputs as `%`-variables.
  "Note that result of this operation is **transient**" — nothing is persisted; the caller decides.
  (Note: in R4 `$apply` returned a CarePlan/RequestGroup; the R5 RequestOrchestration return is a change — R4 detail is
  from prior knowledge, *unverified* in this session.) — https://hl7.org/fhir/R5/plandefinition-operation-apply.html
- **RequestOrchestration** "represent[s] a set of optional and related activities that may be performed for a specific
  patient or context"; hierarchical actions with `condition` (kind applicability|start|stop, boolean expression),
  `relatedAction` (before/after with offset), `timing[x]`, `groupingBehavior`, `selectionBehavior`
  (any|all|exactly-one|at-most-one|one-or-more...), `requiredBehavior` (must|could|must-unless-documented). It is "often,
  but not always, the result of applying a specific PlanDefinition"; its `action` mirrors PlanDefinition.action.
  — https://hl7.org/fhir/R5/requestorchestration.html
- **CarePlan** is "a specific plan instance for a particular patient or group"; "Protocols and order sets are supported
  through PlanDefinition"; it is "an intermediate approach to complexity" (dependencies/timing via extensions). R5
  `activity` = `plannedActivityReference` (Appointment, MedicationRequest, ServiceRequest, Task,
  RequestOrchestration, ...) + `performedActivity` (CodeableReference to events) + `progress`. — https://hl7.org/fhir/R5/careplan.html
- **Task** "describes an activity that can be performed and tracks the state of completion of that activity";
  `focus` = request being fulfilled, `for` = beneficiary, `basedOn` = higher-level authorization (e.g. CarePlan);
  `businessStatus` (CodeableConcept, example binding) for domain detail like "Specimen collected".
  Suspending a task suspends its children. — https://hl7.org/fhir/R5/task.html
- Task.status (required binding) = draft | requested | received | accepted | rejected | ready | cancelled | in-progress |
  on-hold | failed | completed | entered-in-error. — https://hl7.org/fhir/R5/codesystem-task-status.html
- Workflow communication patterns: Task on placer's system (Option F), POST Task to fulfiller (Option G), Task to a
  workflow broker (Option H), with polling/subscriptions for status. — https://hl7.org/fhir/R5/workflow-management.html
- **EpisodeOfCare** "An association between a patient and an organization / healthcare provider(s) during which time
  encounters may occur"; status planned|waitlist|active|onhold|finished|cancelled|entered-in-error with
  `statusHistory`; "a tracking resource, rather than a planning resource", "usually exists before the CarePlan", "You
  don't need a CarePlan to use an EpisodeOfCare"; linked via `Encounter.episodeOfCare`. No HIV/TB example on the page.
  — https://hl7.org/fhir/R5/episodeofcare.html

### 2. SDC extraction and population

SDC current published version is **v4.0.0 (STU 4), based on FHIR R4** — https://hl7.org/fhir/uv/sdc/extraction.html

- Extraction "only makes sense to occur once a QuestionnaireResponse is completed"; result may be a single resource, a
  Bundle, or "a transaction intended to create some resources and update others"; QR is usually retained "to keep a
  record of the original source-of-truth and for traceability". Extraction can be done via `$extract` on a Form
  Response Manager or locally by the Form Filler. — https://hl7.org/fhir/uv/sdc/extraction.html
- Four approaches (same page):
  - **Observation-based**: items with `item.code` + `sdc-questionnaire-observationExtract` -> Observations;
    `observationExtractEntry` lets other extracted resources (e.g. ServiceRequest.supportingInfo) reference them in the
    same transaction Bundle. Simplest; Observation only.
  - **Definition-based**: `definitionExtract` / `definitionExtractValue` + `item.definition` pointing at
    StructureDefinition paths; any resource type, profile-aware; `extractAllocateId` for intra-bundle fullUrl
    references; `itemExtractionContext` is **deprecated**. Validators SHALL check definition paths match the
    StructureDefinition.
  - **Template-based**: resource templates contained in the Questionnaire, annotated with `templateExtractContext` /
    `templateExtractValue` FHIRPath; conditionals = empty result removes property, loops = multiple results clone it;
    engine SHALL build a transaction Bundle; id present -> PUT, else POST; errors logged and processing continues,
    OperationOutcome for errors. "supports the same level of capability [as definition-based], however is unable to
    leverage any of the information inside a profile". `templateExtractBundle` is incompatible with modular forms.
  - **StructureMap-based**: "the most sophisticated approach of the four - and the most powerful", including code
    translation.
  - `$extract` FHIRPath "do not have access to the launchContext, initialExpression, or variable extensions" — only QR,
    Questionnaire and extract-specific extensions.
- Population: **observation-based**, **expression-based** (`launchContext`, `variable`, `initialExpression`,
  `candidateExpression`, FHIRPath/CQL/FHIR query; "significantly more powerful" but more expertise) and
  **StructureMap-based** (takes precedence over all others). `$populate` returns a QR; `$populate-html` and
  `$populate-link` are deprecated. — https://hl7.org/fhir/uv/sdc/populate.html

### 3. CRMI (knowledge artefact lifecycle and packaging)

CRMI current published version **v2.0.0 (STU 2), FHIR R4** — https://hl7.org/fhir/uv/crmi/artifact-lifecycle.html

- Status draft -> active -> retired; "An active artifact SHALL NOT transition back to draft. A new version of the
  artifact is required." Same for retired. Canonical `url` is "globally unique, version-independent"; references may be
  `url|version`. Artifacts "SHALL specify a version", SHOULD use `<major>.<minor>.<patch>[-<label>]`. Versioning policy may
  be `package` (version dictated by the containing package). Collections are a CRMIManifestLibrary with `composed-of`
  relatedArtifacts; manifests pin unversioned references incl. "stable value set expansion"; "released content is
  immutable"; operations `$release`, `$package`, `$data-requirements`, `$infer-manifest-parameters`.
  — https://hl7.org/fhir/uv/crmi/artifact-lifecycle.html
- Packaging as transaction (or collection) Bundle with the main artifact first, then dependencies and test cases; e.g.
  "A Questionnaire, together with all the ValueSets referenced" or "A PlanDefinition, together with the Executable
  libraries and Expanded ValueSets required to apply the PlanDefinition in a setting without a terminology service";
  recommended conditional create on `url`+`version`. Alternatively FHIR NPM packages. — https://hl7.org/fhir/uv/crmi/packaging.html
- CPG IG v2.0.0 (STU2, R4) defines CPGRecommendation/Pathway/OrderSet/Strategy/WorkflowDefinition profiles, CQL
  logic via the Using-CQL-with-FHIR IG, lifecycle via CRMI, and a CPGCase episode-of-care. — https://hl7.org/fhir/uv/cpg/

### 4. FHIR NPM packages

- Tarball containing a `package` folder with `package.json`, JSON resources, optional `examples/` and `.index.json`.
  `name` globally unique; `version` "SHALL ... conform to Semantic Versioning"; `dependencies` must include a core
  package (e.g. `"hl7.fhir.r5.core": "5.0.x"`); `canonical` required for IG packages; `fhirVersions` array.
  `.index.json` is derivable from resources. Immutability of a published version is **not stated** on this page
  (unverified here; it is conventional registry practice). — https://hl7.org/fhir/R5/packages.html

### 5. WHO SMART Guidelines (ANC, HIV, TB)

- Catalogue (smart.who.int): ANC DAK Feb 2021 (IG: build.fhir.org/ig/WorldHealthOrganization/smart-anc), HIV DAK Feb
  2024 (IG: smart.who.int/hiv), TB DAK May 2024 (IG "Not available"), also FP, PNC, immunizations, etc. — https://smart.who.int/
- **HIV IG v1.0.0 (Release), FHIR R4**, active as of 2025-07-08; sections: business requirements (personas, user
  scenarios, business processes, data dictionary, decision-support logic, indicators, functional/non-functional),
  data models & exchange, deployment. Uses "CPG ActivityDefinitions", "IPS Patient", and CQL for PlanDefinitions and
  Measures; "for trial implementation and adaptation only". — https://smart.who.int/hiv/ ; business processes page
  https://smart.who.int/hiv/business-processes.html
- **ANC IG**: only a **CI build 0.3.0 (Draft), FHIR 4.0.1**; "draft material that has not yet been tested or validated
  for use in production". — https://build.fhir.org/ig/WorldHealthOrganization/smart-anc/
- **TB**: "SMART DAK TB" v1.0.1 (Release, R4) publishes the L2 DAK, but "The L3 FHIR Implementation Guide for the TB
  SMART Guidelines is yet to be developed." — https://smart.who.int/dak-tb/index.html
- DAK has nine components (health interventions & recommendations, generic personas, user scenarios, business processes
  & workflows, core data elements, decision-support logic, indicators, functional and non-functional requirements), as
  listed on the HIV and ANC IG homes; SMART Base 0.3.0 (CI build, R4) shows the authoring lifecycle as BPMN 2.0.
  — https://smart.who.int/hiv/ , https://build.fhir.org/ig/WorldHealthOrganization/smart-base/
- L1–L5 layer names (L1 narrative, L2 operational/DAK, L3 machine-readable, L4 executable, L5 dynamic): the official
  smart.who.int pages fetched only labelled L2 = DAK and L3 = FHIR IG (https://smart.who.int/ ,
  https://smart.who.int/ig-starter-kit/l2_authoring_overview.html). Full L1/L4/L5 definitions came from secondary
  sources (https://en.wikipedia.org/wiki/WHO_SMART_guidelines) — **unverified against a WHO primary page**.
  The starter kit expands BPMN and DMN in its abbreviations, i.e. L2 processes/decisions are authored in those notations.

### 6. OpenHIE

- Three layers: business-domain and registry services; interoperability layer (IOL) as gateway; point-of-service.
  Components include IOL, Client Registry, Facility Registry, Health Worker Registry, Product Catalogue, Shared Health
  Record, Terminology Service, HMIS, LMIS, Finance & Insurance; terminology workflows include expand value set,
  translate code, validate membership. Each component has requirements (must) and recommendations. Spec PDF v3.0
  https://ohie.org/wp-content/uploads/2020/12/OpenHIE-Specification-Release-3.0.pdf ; online guide
  https://guides.ohie.org/arch-spec/ (latest version reported as 5.2 in search snippets — **unverified**; the fetched
  pages did not render component text).
- Relevance: OpenHIE is about **inter-system data exchange** workflows, not intra-facility clinical workflow; our engine
  is a point-of-service + SHR concern, with registry/terminology lookups as external calls.

### 7. UgandaEMR / OpenMRS ART and TB modelling

- `Programs.java` in openmrs-module-ugandaemr defines OpenMRS Programs: **TB Program**, **HIV Program**, **MCH
  Program**, **Nutrition Program**, and DSDM programs **FBIM, FTR, FBG, CCLAD, CDDP**, each with a CIEL concept, and a TB
  outcomes concept. — https://github.com/METS-Programme/openmrs-module-ugandaemr/blob/master/api/src/main/java/org/openmrs/module/ugandaemr/metadata/core/Programs.java
- `ExitTBProgramASLostToFollowUpTask` is a scheduled task that, for each active TB PatientProgram whose latest
  encounter is older than global property `ugandaemr.lostToFollowUpDaysTB` (default 60), sets `dateCompleted`, outcome =
  concept 5240 (lost to follow-up) and voids the enrolment; an equivalent MCH task exists.
  — https://github.com/METS-Programme/openmrs-module-ugandaemr/blob/master/api/src/main/java/org/openmrs/module/ugandaemr/tasks/ExitTBProgramASLostToFollowUpTask.java
- PatientProgramAttribute types and a "Enroll In DSDM" action exist (`UgandaEMRServiceImpl`, `messages.properties`) —
  same repo.
- OpenMRS model: a Program contains optional Workflows, each with States; one active state per workflow per patient
  (OpenMRS docs, https://openmrs.atlassian.net/wiki/x/u6uEAQ — page did not render in fetch; claim from search
  snippet, **unverified**).
- UgandaEMR runs at "over 1900 ART facilities" (METS newsletter, https://mets.or.ug/wp-content/uploads/2023/09/METS-WATCH_AUG-2023.pdf);
  UgandaEMR+ runs on OpenMRS 3 and "introduces the ability to serve the patients without focusing on a particular
  disease" (https://openmrs.atlassian.net/wiki/spaces/docs/pages/26938864/UgandaEMR+User+Manual). Search-snippet level;
  not re-read in full.
- Not verified: UgandaEMR's HIV program workflow/state lists (e.g. ART regimen lines, transfer in/out states) and
  its FHIR mapping — would need reading the metadata bundle/CIEL concept sets.

### 8. Terminology

- CodeSystem ("declare the existence of and describe a code system or code system supplement"), ValueSet ("a set of codes
  drawn from one or more code systems"), ConceptMap ("A statement of relationships from one set of concepts to one or
  more other concepts"), NamingSystem ("A curated namespace that issues unique symbols"), TerminologyCapabilities;
  operations `$expand`, `$validate-code`, `$lookup`, `$translate`, `$subsumes`, `$closure`.
  — https://hl7.org/fhir/R5/terminology-module.html
- UCUM is documented at HL7 Terminology (THO v7.5.0, FHIR 5.0.0): https://terminology.hl7.org/UCUM.html (R5 `ucum.html`
  redirects there). System URI `http://unitsofmeasure.org` used in Quantity.system/code is standard but the fetched page
  did not render it — **unverified in this session**.

## Implications for the decision

### What FHIR can represent vs what needs an internal execution model

| Concern | Represent in FHIR | Engine-internal |
|---|---|---|
| Program/protocol definition (ART, TB, ANC steps) | PlanDefinition + ActivityDefinition (CPG profiles), versioned per CRMI | Compiled state-machine definition derived from / linked to the PlanDefinition canonical `url|version` |
| Per-patient plan | CarePlan (+ RequestOrchestration from `$apply`) | — |
| Program enrolment and outcome | EpisodeOfCare (status + statusHistory, type = program) | enrolment state + transitions history |
| A step someone must do (triage, review, dispense) | Task (`basedOn` CarePlan/RequestOrchestration, `focus` the request, `for` patient, `businessStatus` = engine state label) | assignment, queue position, claim/lease, SLA timers |
| Clinical outputs | ServiceRequest, MedicationRequest, Observation, Condition, Appointment... | — |
| Timers, escalation, LTFU after N days | — (Task.restriction/period is descriptive only) | timer wheel / scheduled transitions (cf. UgandaEMR LTFU task) |
| Parallel/join, compensation, nested subprocess, simulation | Partially in RequestOrchestration action semantics (selection/required behaviour, relatedAction) but `$apply` is transient and stateless | yes, fully |
| OPD visit flow (Reception -> Triage -> ...) | Encounter (+ Encounter.status / location history), Tasks per station | the queue/flow instance itself |

Recommendation: **engine-native workflow definitions and instances; FHIR is the projection and the interchange
format**. Do not try to execute PlanDefinition directly as the engine's runtime language. Do support *importing*
PlanDefinition/ActivityDefinition (and evaluating `$apply`-style applicability) as an authoring source, and *exporting*
Tasks/CarePlans so external systems (and FHIR Subscriptions) see progress. Keep Task.status within the 12 R5 codes and put
engine state in `businessStatus` — this keeps §47's separation explicit. One engine handles OPD (Encounter-scoped),
ART/TB (EpisodeOfCare-scoped) and patient requests (Task/ServiceRequest-scoped) by varying the context type, as
UgandaEMR's single Program abstraction already does for HIV/TB/MCH/DSDM.

Decision support (CQL in SMART L3 PlanDefinitions/Libraries) is a separate question: a Rust CQL engine does not
obviously exist (unverified); options are a sidecar CQL engine, FHIRPath-only guards, or translating DAK decision tables
(DMN-like) into engine guards. Flag for the rules/CDS decision.

### Extraction options

1. **Template-based** (recommended default): FHIRPath-only, deterministic, produces a transaction Bundle with PUT/POST
   semantics, supports conditionals/loops, no profile engine needed. Fits §37 "deterministic extraction identifiers,
   versioned mappings and provenance" — add `allocateId`/`resourceId` expressions to derive deterministic ids from
   QR id + linkId.
2. **Observation-based** for simple coded vitals (weight, BP components need template or definition-based).
3. **Definition-based** later: needs StructureDefinition snapshot walking in Rust; most profile-aware.
4. **StructureMap**: avoid initially (needs a FML engine; most powerful; most implementation cost).
Run extraction server-side on QR `completed`, inside the same transaction as QR persistence + Provenance; also allow
the frontend form filler to preview. Expression-based population (`launchContext`/`initialExpression`) is the
counterpart for prefill. A Rust FHIRPath implementation is therefore a hard dependency for forms *and* engine guards.

### Packaging

- Program packages = **FHIR NPM packages** (`package.json`, semver, dependencies on `hl7.fhir.r5.core`, national
  terminology packages, SMART-derived content) containing Questionnaires, PlanDefinitions, ActivityDefinitions,
  ValueSets (with pinned expansions for offline edge), and an engine-workflow definition (as a Binary/Library or our
  own logical resource).
- Lifecycle per **CRMI**: draft -> active -> retired, no reverse transitions, canonical `url|version`, Manifest Library
  to pin dependency versions. This directly implements §45 (Draft -> ... -> Supersession) and gives historical
  interpretation (QR.questionnaire is `url|version`).
- Bundle-form artefact packages (CRMI packaging) are useful for pushing config to facility edge nodes over the API.

### Risks

- **R4 vs R5 mismatch (high)**: SDC 4.0.0, CRMI 2.0.0, CPG 2.0.0, SMART HIV 1.0.0, ANC 0.3.0, SMART Base 0.3.0 are all R4.
  Importing them into an R5-native store needs R4->R5 conversion (RequestGroup->RequestOrchestration, CarePlan.activity
  restructuring, Questionnaire/QR differences) and extension-URL checks. Specific R4->R5 Questionnaire diffs were not
  verified here.
- **SMART maturity (high)**: only HIV has a released L3 IG; ANC is a draft CI build; TB has no L3 IG. ART/TB content for
  Uganda will largely be authored locally from L2 DAKs and national guidelines (MASTER_PROMPT §44: no hardcoded regimens
  without approved sources).
- **CQL dependency (medium)**: SMART L3 logic is CQL; no confirmed production Rust CQL engine (unverified).
- **`$apply` is transient and stateless (medium)**: cannot serve as engine runtime; need our own persistence.
- **Task status vocabulary is fixed (low)**: map rich engine states to `businessStatus`, not new status codes.
- **EpisodeOfCare has no HIV/TB example in the spec (low)**: program typing (EpisodeOfCare.type) is a local
  ValueSet decision.
- **UgandaEMR migration (medium)**: existing programs use OpenMRS Program/PatientProgram + CIEL concepts; migration
  needs ConceptMaps from CIEL/OpenMRS concepts and mapping PatientProgram -> EpisodeOfCare, outcomes -> statusHistory +
  Condition/Observation.

## Unverified items

- Full official WHO definitions of L1, L4, L5 (only L2/L3 labels confirmed on smart.who.int).
- OpenHIE current spec version (5.2 per search snippet) and component text.
- OpenMRS program/workflow/state rule (one active state per workflow) — page did not render.
- UgandaEMR HIV/TB workflow state lists and FHIR mappings.
- UCUM system URI on the THO page (did not render); R4 `$apply` return type; specific R4->R5 Questionnaire diffs;
  existence of a production Rust CQL engine; NPM package immutability statement.
