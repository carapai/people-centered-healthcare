# QuestionnaireResponse extraction and population

Status: **accepted** (ADRs 0022, 0023). Produced while resolving the Wayfinder ticket [How are QuestionnaireResponses extracted into clinical resources?](https://github.com/carapai/people-centered-healthcare/issues/25). Background: [docs/research/clinical-workflows.md](../research/clinical-workflows.md). Spec: MASTER_PROMPT §36–38.

Related decisions:
- deterministic ids: [identifiers.md](identifiers.md) (ADR 0013);
- Owners: [resource-ownership.md](resource-ownership.md);
- validation tiers: [fhir-r5-model.md](fhir-r5-model.md) (ADR 0016);
- sync groups and rejection codes: [synchronization.md](synchronization.md).

Packaging and publication of Questionnaires and mappings is decided in [How are terminology, metadata and configuration packaged and published?](https://github.com/carapai/people-centered-healthcare/issues/23).

## Definitions

| Artefact | Owner | Notes |
|---|---|---|
| Questionnaire | metadata | Canonical `url\|version`; the form |
| **Extraction Mapping** | metadata | Its own canonical `url\|version`. Declares the Questionnaire `url` and version range it applies to. Its body is SDC template-extraction content: contained resource templates annotated with `templateExtractContext` / `templateExtractValue` FHIRPath |
| Fixture set | metadata, inside the mapping's package | Sample QuestionnaireResponses and their expected output Bundles; a publication gate |

- **Template-based extraction only, at runtime** (ADR 0022):
  - An empty FHIRPath result removes a property, and multiple results repeat it.
  - SDC `observationExtract` items are converted into templates at publish time.
  - Definition-based extraction and StructureMap are deferred.
- **Default: no extraction.** An item produces a resource only if a mapping template targets it. Narrative answers stay in the QuestionnaireResponse (§37).
- **SDC imports.** An imported SDC Questionnaire with contained templates is split into a Questionnaire and a generated Extraction Mapping. Converting R4 SDC content belongs to the packaging ticket.

Each target template in a mapping declares:

| Property | Values |
|---|---|
| `mode` | `managed` or `seed` (ADR 0023) |
| `retraction` | The status set when a managed target disappears from the output, e.g. `entered-in-error` |
| `owner_use_case` | The Owner's use case that accepts it (checked against the Owner's opt-in list at publish) |

## Runtime flow

```mermaid
sequenceDiagram
  participant UI as Form filler
  participant UC as CompleteQuestionnaireResponse (clinical-documentation, application)
  participant X as health-fhir-extraction (pure)
  participant O as Owner use cases (clinical-data, laboratory, medications, …)
  participant UoW as Unit of work
  UI->>UC: complete(QR, If-Match)
  UC->>X: extract(QR, Questionnaire, mapping url|version, context)
  X-->>UC: plan: [target, deterministic id, resource, mode]
  UC->>O: for each target: create / version / retract (seed: create once)
  O-->>UC: ok | validation error
  alt any error (except authorization-only)
    UC-->>UI: errors; QR stays in-progress
  else
    UC->>UoW: QR (completed, mapping recorded) + resources + Provenance + outbox, one transaction
    UoW-->>UI: committed
  end
```

- **When it runs:** on QuestionnaireResponse `completed` and `amended`, on the writing node (edge, desktop or central). There is no extraction for `in-progress` drafts.
- **What the mapping can see.** Extraction is a **pure function** of:
  - the QuestionnaireResponse;
  - the Questionnaire;
  - the mapping;
  - a context taken from the response itself: subject, encounter, author, `authored`, and the program episode (below).

  It has no access to `now()`, queries or `launchContext`, as in SDC `$extract`.
- **Which mapping version.** The response records the mapping version used, in an `extension` on `QuestionnaireResponse` (mapping `url|version`). Later amendments reuse that version. If that version is retired for a defect, they use the replacement named on the retirement.
- **All or nothing.** Any extraction or validation failure fails completion as a whole. Authorization-only failures are accepted and flagged (ADR 0011).
- **Preview.** A dry-run `$extract` on the user's own node shows what would be created (the edge for LAN browsers, the local runtime for Tauri). Nothing is persisted.
- **Implementation.** The extractor exists only in Rust: `health-fhir-extraction`, a pure crate in the contracts layer that uses the FHIRPath engine behind its trait.

## Ownership of extracted resources (ADR 0023)

- **Plan, then Owners.** The extractor only produces a plan. `CompleteQuestionnaireResponse` hands each entry to the use case of the Owner the ownership table gives it. This is one transaction inside one Deployable (ADR 0004).

  | Extracted resource | Owner |
  |---|---|
  | Observation: vital signs, exam, social history, survey | clinical-data |
  | Observation, category laboratory | laboratory: **point-of-care results only** (mRDT, glucometer, HB meter). Analyser and lab-workflow results never come from forms |
  | Condition | its clinical Owner |
  | MedicationRequest | medications |
  | ServiceRequest | orders |
  | Appointment | scheduling |
  | Provenance | the Owner of its target (as decided) |

- **Opting in.** Each Owner declares which types and profiles it accepts from extraction. A mapping that targets anything else fails publication.
- **Mode `managed`:**
  - The response is the source of truth.
  - The resource is read-only elsewhere, and the UI routes edits to "amend form".
  - Re-extraction writes new versions; targets that disappear get their `retraction` status.
- **Mode `seed`:**
  - The resource is created on first extraction only.
  - Later amendments raise a "source form amended" review item for the responsible clinician and never rewrite it.

## Deterministic ids

- **Id name.** Ids follow identifiers.md: `UUIDv5(platform_namespace, "{QR.id}|{mapping canonical, no version}|{linkId path}#{repeat instance key}|{type}")`.
- **Repeat instance key.** It is the stable `QuestionnaireResponse.item.id` (a UUIDv7) that our form filler stamps on every repeat instance. The repeat index is used only for imported responses that have no item ids. Removing one repeat therefore never shifts another repeat's identity.
- **Same mapping canonical.** A newer mapping version produces new **versions** of the same resources, never duplicates.

## Re-derivation at authority (ADR 0022)

- **Check.** When a sync group containing a completed or amended QuestionnaireResponse reaches the Authority Node, the node re-runs extraction with the recorded mapping version and compares content hashes with the extracted resources in the group.
- **Mismatch.** The group is rejected with `extraction_mismatch` and sent to the originating node's review queue.
- **Missing mapping.** If the mapping version is unknown or not approved at the authority, the group is rejected with `metadata_incompatible` (synchronization.md).

## Re-extraction with a newer mapping

- **No automatic re-extraction.** Publishing a mapping never re-extracts existing responses.
- **Bulk re-extraction** is an explicit admin job, for example to correct a wrong code:
  - audited and resumable;
  - scoped by facility, date range and mapping;
  - touches **managed** targets only;
  - writes new versions with Provenance.

## Provenance

Each extraction run writes, in the same transaction:
- **One Provenance:**
  - `target`: every resource version produced or retracted;
  - `entity` (role `source`): the QuestionnaireResponse version;
  - `entity`: the mapping `url|version`;
  - `agent`: the QuestionnaireResponse author, and the node as the software `Device`;
  - `activity`: the platform code `extraction`.
- **`derivedFrom`** set to the QuestionnaireResponse, where the target has that element (Observation, DocumentReference).
- **`meta.tag`** `extracted` plus the mode (`managed` or `seed`), on every extracted resource.

## Program context (§38)

- **Encounter.** It comes from `QuestionnaireResponse.encounter`.
- **When the form belongs to a program** (its program definition lists it):
  - **One active EpisodeOfCare** of that program type for the patient at this facility: the response and every extracted resource get the R5 core extension `workflow-episodeOfCare` referencing it, and a custom SearchParameter indexes it.
  - **Several:** the clinician chooses one at completion.
  - **None:** resources are extracted without the extension.
- **Reporting** falls back to `Encounter.episodeOfCare` when the extension is absent.

## Population (prefill)

- **v1 supports:**
  - SDC **expression-based** population: `launchContext`, `variable`, `initialExpression` and `candidateExpression` in FHIRPath;
  - **observation-based** population.
- **Where it runs:** `$populate`, evaluated in `health-fhir-extraction` on the node.
- **Data access.** Population may query local data. It is not pure, which is why it is kept apart from extraction.
- **Not in v1:** CQL and StructureMap population. CQL stays in the map's rules-language fog.

## Publication gates

A mapping can be published only if:
1. every FHIRPath expression parses and type-checks against the Questionnaire's items;
2. every target is accepted by its Owner's opt-in list;
3. every fixture response produces exactly its expected Bundle;
4. every output validates against its declared profile (ADR 0016 tiers).
