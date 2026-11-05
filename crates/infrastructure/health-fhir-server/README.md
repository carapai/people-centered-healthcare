# health-fhir-server

Layer: **infrastructure**. FHIR R5 REST handlers, the CapabilityStatement and Bundle assembly over `health-surrealdb` (persistence.md § REST semantics implemented here, § In code).

- **`FhirServer::new(store, base_url, policy).router()`** returns the routes relative to the FHIR base, to nest under e.g. `/fhir`. They cover `metadata`, read, vread, create, update, delete, conditional create/update/delete, transaction and batch Bundles (`POST /`), instance/type/system `_history` and `search-type` over GET and POST. The write steps live in `ops` and run inside a caller's transaction, so single requests and Bundles share them.
- **Responses:** FHIR JSON bodies with weak ETags (`W/"<versionId>"`) and `Last-Modified`. Errors are OperationOutcomes. Stored bytes are embedded as they are stored (ADR 0051).
- **Validation:** request bodies pass tier 1 (structure) and tier 2 (core invariants) before anything else reads them. `with_fhirpath_engine` replaces the default engine.
- **Not mounted in `central-server` yet:** there is no authentication or authorization until #63–#65.
- **Audit (#66):** `with_audit(Auditor)` records an AuditEvent in the same transaction as every write (create, update, delete, conditional writes and each Bundle entry), and every read, vread, search, history or Bundle GET that returns a sensitive record or relies on break-glass.
  - A write that fails rolls its event back with it.
  - The agent is the `health_audit::Agent` request extension, which authentication inserts (#192). Without one the request is recorded as unauthenticated.
  - AuditEvent is read-only over REST. The CapabilityStatement declares only reads and searches for it, and writes are 405.
  - AuditEvents do not count as referrers, so audited records can still be deleted.
- **Reference integrity:** local references must resolve (422), and a referenced resource cannot be deleted (409). `with_reference_scope` lets a node allow out-of-scope targets (persistence.md § Reference integrity).

- **Access control (#192):** `with_access_control(Pdp, RecordFacility)` decides every interaction for the request's `health_access_policy::Subject` request extension, which authentication inserts ([#209](https://github.com/carapai/people-centered-healthcare/issues/209)).
  - Without a subject the answer is 401; `/metadata` stays open.
  - Searches run under a per-request `SubjectSearchPolicy`, confined to a patient for `patient=` or a `subject=` naming a Patient; 403 when the type may not be searched.
  - Reads, vreads and instance history are 404 when denied. Type and system history are decided as searches, so central's patient-confined shared reads (ADR 0056) never list another facility's records.
  - Creates, updates and deletes are 403 when denied, decided on both the new resource and the version replaced.
  - `RecordFacility` finds a facility-scope record's facility. `FacilityTag` (ADR 0057) reads the `urn:health:facility` tag on the facility-scope types; `NoFacilityScope` treats no record as facility-scope (tests).
  - **Stamping (ADR 0057):** on create, a facility-scope record gets the session's facility tag. A tag naming another facility is 403, and an update keeps the facility of the version it replaces.

- **Client registry (#197, #216):** `with_client_registry(RegistryConfig, MatchingRules)` serves `POST /Patient/$match` to anyone who may search Patients. Candidates they may not see are left out, and so is their count. With `with_registration(MrnSettings)` it serves reception's `POST /Patient/$register` (`resource`, `confirmedNew`) to anyone who may create Patients. Until the clerk confirms, it answers with the candidates the subject may see and writes nothing. Then it creates the provisional source Patient with a facility MRN and runs registration matching. `registry_review_router()` is the review queue for the review screen, a JSON router that central nests at `/api/registry/reviews`. `GET /` lists pending items, oldest first, with `overdue`: potential matches carry both Patients, and survivorship items carry both claims. `POST /{id}` carries out and audits a decision. It also serves the reviewer operations `POST /Patient/$merge` (preview, then the reviewer's approval), `POST /Patient/{id}/$link` and `POST /Patient/{id}/$unlink` (patient-identity.md).
  - The reviewer is the request's `Subject`: without one the answer is 401, and without `registry.review` and `Patient` update it is 403.
  - A merge is for records of the reviewer's facility only.
  - Each version written is audited in its transaction, through `AuditTrail` over the server's `Auditor`, naming the reviewer.
  - The operations are not yet declared in the CapabilityStatement.
- **Self-registration (#111):** `portal_router()` serves patients signed in with their phone, behind `health_auth::patient_layer` (a `PatientSubject`; without one, 401).
  - `GET /registration` is the account's Registration Request: `status`, `patient`, and `access` (`none`, `self-submitted` or `records`).
  - `POST /registration` submits the self-registration QuestionnaireResponse (422 when invalid, 409 when already submitted).
  - **Registration requests in the review queue** (#111 part 2c): a request that resembles existing Patients is a `registration-request` item. The queue lists it with the patient's QuestionnaireResponse and the candidate Patients. `POST /{id}` takes `{"decision": "link-patient", "patient", "document"}` (the holder's ID checked in person; 422 without one) or `{"decision": "create-patient"}`, and carries it out through `SelfRegistration::review`.
  - It implements `health_patient_engagement_app::PatientRegistry` over the client registry: `$match` for the answers, and `create_self_registered` for a new Patient. Each version written is audited, naming the patient account.
- **Stock (#130):** `inventory_router()` is for staff with `stock.manage`, behind `subject_layer`; central nests it at `/api/stock`.
  - `POST /goods-received`: each line is a receipt at its unit cost.
  - `GET /on-hand?store=`: Stock Units, balances and last unit costs.
  - `GET /reorder?store=&days=`.
  - `POST /gs1`: reads a scanned or printed DataMatrix.
  - `GET /requisitions?store=` (newest first), `POST /requisitions`, `/requisitions/{id}/decision` and `/requisitions/{id}/fulfil` (the picked batches, dispatched as a transfer).
  - Over `health-inventory-app`.
- **Questionnaire submissions (#92):** with `with_questionnaire_checks()`, a QuestionnaireResponse written `completed` or `amended` is re-validated against its Questionnaire (`health_fhir_validation::check_response`).
  - **Finding the Questionnaire:** by `Questionnaire?url=&version=`. If the node does not have it, the submission is refused (422).
  - **What is stored:** the re-validated response, with the server's calculated values.
  - **Where it applies:** plain writes, and each entry of transaction and batch Bundles. Drafts are stored as they are.
  - **Central** turns it on, and supports those searches.
- **Pricing (#120):** `with_pricing(PricingConfig)` and `pricing_router()`, behind `subject_layer`. Central nests it at `/api/pricing`.
  - **Permissions:**
    - `pricing.manage` drafts and lists Price Books and rules;
    - `pricing.approve` approves and retires them, and the author may not approve their own draft;
    - `pricing.quote` prices a charge.
  - **Routes:** `GET|POST /books`, `GET|POST /rules`, `POST /books/{id}/{version}/{approve|retire}`, `POST /rules/{id}/{version}/{approve|retire}`, `POST /quote`.
  - **Quotes:**
    - each line's applicability is evaluated with the server's FHIRPath engine, before pricing, against the FHIR resource `context` gives for that line (an empty Parameters otherwise);
    - an expression must give a Boolean, an empty result does not apply, and anything else refuses the quote.
- **Workflows (#99):** `with_workflows(WorkflowConfig)` and `workflow_router()`, behind `subject_layer`. Central nests it at `/api/workflow`.
  - **Permissions:** `workflow.work` to start instances, read worklists and Tasks, claim, release and complete steps; `workflow.supervise` to reassign, escalate and cancel.
  - **Routes:** `POST /instances`; `GET /instances/{id}/tasks`; `POST /instances/{id}/events`; `POST /instances/{id}/cancel`; `POST /instances/{id}/tasks/{state}/{claim|release|reassign|escalate}`; `GET /worklists/{key}`; `GET /definitions`. Each command carries the client's `opId`, and the signed-in account is the actor.
  - **Events offered:** each task lists the events its state's transitions take.
  - **Not wired yet:** guards and typed actions. `WorkflowConfig::new` refuses definitions that use them, and so does any check they fail at publication.
- **Signed notes (#107):** every write that replaces, and every delete of, a Composition or DocumentReference asks `health_clinical_documentation_app::notes`. That covers plain and conditional writes, transactions and batches, and completion.
  - A signed note changed in place, or deleted, answers 409 (`business-rule`): amend it instead.
  - Marking it entered in error, its content unchanged, is allowed.
- **The laboratory (#109):** `laboratory_router()`, behind `subject_layer`, for the session's facility. Central nests it at `/api/laboratory`.
  - **Orders:** a consultation test order (`laboratory`, the default) starts the laboratory test (`health-laboratory-app`) and the lab Workflow Definition (`LAB_WORKFLOW`, the `ug.moh.lab` reference package) about `ServiceRequest/{id}`. Each step is delivered to that instance as its event (`workflow::deliver_for`), so the test moves along the worklists: `lab-collect`, `lab-result`, `lab-verify`, then `clinician-results` (at `stat` for a critical result).
  - **Routes:**
    - `GET /tests/{id}`;
    - `POST /tests/{id}/collect`, `/result`, `/reject` and `/cancel` (`laboratory.work`);
    - `/verify` (`laboratory.verify`, by someone other than who entered the results);
    - `/review` (`results.review`).
  - **Release:** a clinician without `laboratory.work` sees no results until they are verified.
- **Prescribing and the pharmacy worklist (#108):** prescriptions are the medications module's (`health-medications-app`), and none moves stock.
  - **Prescribing:** the consultation's medication orders gather the patient's open prescriptions and allergies for the clinical safety checks.
    - Unacknowledged warnings answer 409 with `warnings`, and nothing is written.
    - The order sent again with `acknowledge: true` is written.
  - **`pharmacy_router()`,** for the session's facility (central nests it at `/api/pharmacy`):
    - `GET /worklist` with `pharmacy.view`: the facility's open prescriptions, oldest first;
    - `POST /prescriptions/{id}/status` with `medications.prescribe`.
- **Clinical notes (#107):** `notes_router()`, behind `subject_layer`, with `consultation.document`, for the session's facility. Central nests it at `/api/notes`.
  - **Routes:**
    - `POST /notes` `{id, kind, patient, encounter?, sections}`: a draft;
    - `POST /notes/{id}/edit` and `/cancel`: drafts only;
    - `POST /notes/{id}/sign`: signed, with a Provenance;
    - `POST /notes/{id}/amend` `{id, relation, sections}`: a new signed note relating to the original, with a Provenance naming the original's version;
    - `GET /patients/{id}/notes`: the patient's notes here, latest first.
  - **The original** is never edited. Changing a signed note answers 409.
- **Completion (#93):** `with_extraction()`, with `with_questionnaire_checks()`. The node's search must support Library `type`, `url` and `version`.
  - **On submission:** a `completed` or `amended` QuestionnaireResponse that records no Extraction Mapping gets the newest one for its Questionnaire version (`Library?type=extraction-mapping`).
  - **After it is written, in its transaction:**
    - extraction with the recorded mapping;
    - each planned resource accepted by its Owner's use case;
    - the plan reconciled with the last run, read from the response's Provenance;
    - each written or retracted resource goes through the write path (tiers 1–2, facility stamp, audit, reference checks);
    - a seed resource an amendment would change is left as it is, and `clinical-documentation.source-form-amended.v1` goes on the outbox;
    - the Provenance gets a new version.
  - **All or nothing:** any failure fails the write, and the response stays as it was.
  - **`POST /QuestionnaireResponse/$extract`:** the dry run (SDC).
    - The body is a response, or Parameters with one as `questionnaire-response`. It needs the QuestionnaireResponse search permission.
    - The answer is the transaction Bundle the response would write: its recorded mapping, else the one chosen.
    - Nothing is kept. A response without an id gets one for the preview, and one with no mapping gives a Bundle without entries.
- **The patient summary (#104):** `GET /Patient/{id}/$summary` answers JSON, the shared record's read model (`health-shared-record-app`): `allergies`, `conditions`, `medications`, `vitals`, `visits` and `programs`.
  - The patient must be readable by the clinician; otherwise the answer is 404, as for a read.
  - The records are the patient's, plus, through the golden record, the same person's other patient records.
  - Each section is a search per patient (`{type}?patient=`) under the clinician's own search policy. A sensitive record the clinician may not see is never found, and a section the clinician may not search is `null`.
  - The records read are audited as a search's are.
- **The consultation's commands (#104):** `consultation_router()`, behind `subject_layer`, for the session's facility. Central nests it at `/api/consultation`.
  - **Commands:** the client sends commands, and the node builds each resource (`health_clinical_documentation_app::consultation`) and writes it with tiers 1–2 and reference checks:
    - `POST /encounters/{id}/notes` `{id, text}`: a Composition;
    - `POST /encounters/{id}/diagnoses` `{id, concept, provisional?}`: a Condition;
    - `POST /encounters/{id}/orders` `{id, kind: "service", concept, note?}`: a ServiceRequest;
    - `POST /encounters/{id}/orders` `{id, kind: "medication", concept, dosage, days?, acknowledge?}`: a prescription, through the medications module (#108).
  - **Permissions:** `consultation.document` for notes and diagnoses, `orders.request` for tests and procedures, `medications.prescribe` for prescriptions.
  - **The visit:** it must be in progress at the session's facility (404 for another facility's, 409 when not in progress).
  - **Retries:** ids are the client's UUIDv7s, so a retried command answers what it wrote (200).
- **Programs (#116):** `programs_router()`, behind `subject_layer`, for the session's facility. Central nests it at `/api/programs`. The definitions are the node's ProgramPlanDefinitions (`PlanDefinition?type=clinical-protocol`).
  - **Permissions:** `programs.view` to list definitions and a patient's enrolments; `programs.enrol` to enrol; `programs.manage` to change an enrolment's status or transfer it.
  - **Routes:** `GET /definitions`; `POST /enrolments` `{id, patient, program, transferredFrom?}`; `GET /patients/{id}/enrolments`; `POST /enrolments/{id}/status` `{status}`; `POST /enrolments/{id}/transfer` `{facility}`.
  - **Enrolment:**
    - the program decides eligibility (422 when not eligible), and the patient may be in several programs at once (409 when already in this one);
    - the program's Workflow Definitions the node runs start with the episode as their context (`EpisodeOfCare/{id}`), once each;
    - another facility's enrolment is not found.
  - **Program context on forms:** a submitted QuestionnaireResponse whose form a program lists (its `composed-of` Questionnaires) records the patient's open episode of that program at the session's facility with `workflow-episodeOfCare` (questionnaire-extraction.md § Program context).
    - With several open episodes, the clinician names one: 422 until the response does.
    - With none, nothing is recorded.
    - Every resource extracted from the response records the same episode.
- **Reception and queues (#102):** `reception_router()`, behind `subject_layer`, for the session's facility. Central nests it at `/api/reception`.
  - **Permissions:** `encounters.checkin` to check patients in; `encounters.manage` to move and end Encounters; `queues.view` to see queues; `queues.work` to call and serve patients.
  - **Routes:** `POST /check-in` `{id, patient, servicePoint, class?}` (the id a client UUIDv7, the patient one that exists); `POST /encounters/{id}/move` `{servicePoint}`; `POST /encounters/{id}/status` `{status}`; `GET /queues/{servicePoint}`; `POST /queue/{entry}/{call|serve|recall|finish|leave}`.
  - **Facilities:** service points are named per facility (`KAM013/triage`), so one facility never sees or steps another's queue.
  - **Patient-flow:** each encounters step is followed into the queues at once, until in-process event dispatch delivers the outbox events. A retried follow-up changes nothing.
- **Payments (#254):** `with_payments(PaymentConfig)` and `payments_router()`, behind `subject_layer`, for the session's facility. Central nests it at `/api/payments`.
  - **Permissions:** `payments.cash` (cashiers), `payments.supervise` (variance approval, statements, voids) and `payments.refund`.
  - **Shifts:** open, look up the open shift of a till, view with its payments, close, approve.
  - **Payments:** cash with its receipt; pending with its provider reference (a reused one is 409), with a provisional receipt until the provider confirms it; void and refund.
  - **Statements and balances:** provider statements confirm what they match. `GET /balances` covers the till, in-transit, variance and merchant accounts.
- **Terminology (#90):** `with_terminology(ActiveTerminology)` serves the terminology operations from the node's own [`health-terminology`](../../domains/health-terminology), with no external terminology server.
  - **Routes** (GET with a query string, or POST with a Parameters body):
    - `ValueSet/$expand`: `url`, `valueSetVersion`, `filter`, `offset`, `count` and `activeOnly`. It returns a ValueSet with its `expansion`: `total`, `offset`, a `pinned` parameter and `contains`.
    - `ValueSet/$validate-code` and `CodeSystem/$validate-code`: `url`, `code` with `system`, or `coding`, and `display`. They return `result`, `display` and `message`.
    - `CodeSystem/$lookup`: `name`, `version`, `display`, `definition`, `designation`, and `property`, with `parent` and `child`.
    - `CodeSystem/$subsumes`: `outcome`.
    - `ConceptMap/$translate`: `sourceCode` and `sourceSystem`, or `sourceCoding`, with `url` and `targetSystem`. It returns `result` and each `match` (`relationship`, `originMap`, `concept`).
  - **Errors:** what the node does not hold is 404. What cannot be answered offline (a filter on a code system held in part, a `regex` filter, a cycle) is 422 `not-supported`. A malformed request is 400.
  - **`ActiveTerminology::replace`** swaps the terminology served, and a request keeps the version it started with. Activation (#89) calls it when a package with terminology becomes active. Until then, no node passes one in, so the routes answer 404.
  - **Access:** the routes sit behind the same sign-in as every FHIR route, with no further permission, because terminology holds no patient data.
- **Tiers 3–4 on writes (#91):** `with_profiles(ActiveProfiles)` validates every write after tiers 1–2: creates, updates, transaction and batch entries, and `$register`.
  - **Which profiles:** each profile the resource claims in `meta.profile`, which must be active on the node (a profile it does not have is a `processing` error), and each profile `Profiles::required` names for its type.
  - **Bindings** are checked through `with_terminology`'s terminology. A bare `code` is matched against the value set's expansion, and a coding with its system through `$validate-code`.
  - **Outcomes:** errors refuse the write with a 400 OperationOutcome that lists the warnings too. Warnings alone let it through.
  - **`ActiveProfiles::replace`** swaps the compiled set. Activation (#89) compiles the active packages' profiles once and calls it. Until then no node has active profiles, and tiers 3–4 do not run.

## Tests

`tests/rest.rs` runs every fixture in `tests/fixtures/rest/` and checks that the CapabilityStatement matches behaviour:
- **Fixtures:** R5 REST conformance scenarios as request/expectation steps.
- **CapabilityStatement:** every declared interaction on all 84 types (AuditEvent: reads only, on events the earlier writes produced, and its writes refused) (conditional ones where `_id` is supported), and every declared search parameter. `concurrent_conditional_creates_make_one_resource` races 8 conditional creates. Without the condition lock it fails with 8 resources.

All of it runs on the SurrealDB memory engine through `oneshot`.

`examples/contract_server.rs` serves this API for the client's contract suite (`apps/client`, `bun run test:contract`). It runs two nodes, "central" and "edge", each on its own memory database and ephemeral local port. It prints their FHIR base URLs as one JSON line. It has no authentication and is only for development and CI.

`tests/registry.rs` covers the registry operations: `$register` (no duplicate without confirmation, sequential MRNs, too little to match on, 403); `$match` ranking, label filtering, 403 and 401; a previewed then approved same-facility merge, a refused cross-facility one, link and unlink audited with survivorship review queued, and reviewers only.

`tests/validation.rs` runs with the core package's profiles active and its terminology served:
- a claimed `ExtractionMappingLibrary` is accepted, and refused with a `code-invalid` error for an unknown template mode;
- a claimed `MatchingRulesLibrary` without content is refused;
- a type's required profile applies without a claim;
- a claimed profile the node does not have is refused;
- without active profiles, nothing changes.

`tests/terminology.rs` covers the terminology operations:
- an expansion by GET, with text filter and paging;
- validation by POST with a coding, a wrong display, a code outside the value set, and validation against a code system;
- a lookup with its parent, subsumption, and translation by POST;
- 404 for unknown resources and codes, and for a node without terminology;
- 422 for a `regex` filter;
- 400 for missing or malformed parameters;
- replacing the terminology served.

`tests/facility.rs` covers the facility tag: stamped on create, refused for another facility, kept through updates, and facility-isolated searches at an edge and at central.

`tests/authorization.rs` runs the authorization matrix through HTTP: no route without a subject, labels and purpose of use, permissions on every interaction (including a transaction Bundle), facility isolation in reads, writes and history, and break-glass confined to one patient.

`tests/audit.rs` checks:
- one event per write, with the agent, the entity version, the patient and the purpose;
- no event when a write fails;
- sensitive reads audited through read, vread, search, history and a Bundle GET, naming only the sensitive record and its label;
- ordinary reads not audited;
- break-glass reads audited with `BTG`;
- unauthenticated requests recorded as such.
