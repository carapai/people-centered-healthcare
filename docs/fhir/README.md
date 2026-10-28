# FHIR profiles, extensions and implementation-guide notes

This folder will hold the platform's FHIR conformance artefacts and notes: profiles, extensions, custom SearchParameters, CodeSystems for platform codes, NamingSystems, and implementation-guide pages. No artefact has been written yet. The artefacts themselves ship in signed configuration packages ([configuration.md](../architecture/configuration.md)); this folder documents them.

- **Model, validation tiers and search:** [fhir-r5-model.md](../architecture/fhir-r5-model.md) (ADRs 0015, 0016). R5 (5.0.0) only; R4 only at integration boundaries.
- **Storage of resources and history:** [persistence.md](../architecture/persistence.md).
- **Packaging, versioning and publication:** [configuration.md](../architecture/configuration.md), [metadata.md](../architecture/metadata.md), [terminology.md](../architecture/terminology.md).
- **Research:** [docs/research/fhir-r5.md](../research/fhir-r5.md).

## Profile and extension needs already decided

Canonical URLs and the exact artefact form are not fixed yet for any of these; they are to be fixed when each artefact is authored.

### Extensions

| Need | Kind | Decided in |
|---|---|---|
| `workflow-episodeOfCare` on a program QuestionnaireResponse and every resource extracted from it, plus a custom SearchParameter that indexes it | R5 core extension; custom SearchParameter | [questionnaire-extraction.md:131](../architecture/questionnaire-extraction.md) |
| Extraction Mapping `url\|version` recorded on QuestionnaireResponse | Platform extension | [questionnaire-extraction.md:66](../architecture/questionnaire-extraction.md), [metadata.md:43](../architecture/metadata.md) |
| Price Rule priority on a ChargeItemDefinition `propertyGroup` | Platform extension | [pricing-and-insurance.md:11](../architecture/pricing-and-insurance.md) |
| `match-grade` on `Patient/$match` results, with `search.score` | R5 core extension | [patient-identity.md:92](../architecture/patient-identity.md) |

### Profiles

| Need | Decided in |
|---|---|
| National core and program profiles, validated in tier 3 (including slicing) | [metadata.md:16](../architecture/metadata.md), [fhir-r5-model.md:45](../architecture/fhir-r5-model.md), [configuration.md:35](../architecture/configuration.md) |
| Price Book entries as profiled ChargeItemDefinition; Price Rules as profiled ChargeItemDefinition `propertyGroup` with `applicability` FHIRPath | [pricing-and-insurance.md:10-11](../architecture/pricing-and-insurance.md) |
| **Locally pinned financial profiles:** ChargeItem, Account, Invoice, Coverage, CoverageEligibility\*, Claim, ClaimResponse, ExplanationOfBenefit, PaymentReconciliation, PaymentNotice (low-maturity R5 resources) | [ADR 0030](../adr/0030-double-entry-ledger-is-the-financial-source-of-truth.md), [pricing-and-insurance.md:23](../architecture/pricing-and-insurance.md), [billing-and-payments.md:108](../architecture/billing-and-payments.md) |
| **Locally pinned inventory façade profiles** for "all FHIR inventory resources" (FMM 0–1); the façades named there are InventoryItem, SupplyDelivery, SupplyRequest and InventoryReport | [inventory-and-procurement.md:22](../architecture/inventory-and-procurement.md) |
| CRMI-style `Library` manifest in each package, with pinned ValueSet expansions | [configuration.md:25](../architecture/configuration.md), [terminology.md:12](../architecture/terminology.md) |

### Representation of engine-native Published Definitions

Decided in [ADR 0049](../adr/0049-engine-native-definitions-as-profiled-library-and-plandefinition.md):

| Definition | Representation | Profile |
|---|---|---|
| Extraction Mapping | `Library` (`type` = `extraction-mapping`) with SDC templates as contained resources | `ExtractionMappingLibrary` |
| Matching Rules | `Library` (`type` = `matching-rules`), content `application/vnd.health.matching-rules+json` | `MatchingRulesLibrary` |
| Workflow Definition | `Library` (`type` = `workflow-definition`), content `application/vnd.health.workflow+json` | `WorkflowDefinitionLibrary` |
| Program definition | `PlanDefinition` (`type` = `clinical-protocol`) linking its artefacts in `relatedArtifact` | `ProgramPlanDefinition` |
| Form Layouts, charge triggers, Financial Gate configs, override points, settings, human-readable identifier format rules | Non-FHIR JSON in `configuration/` with `url` and `version`, pinned by the manifest Library (`composed-of`) | JSON Schema per kind |

Identifier *systems* remain NamingSystems ([identifiers.md](../architecture/identifiers.md)); only their formatting rules are configuration JSON.

### Codes, tags and identifier systems

| Need | Decided in |
|---|---|
| Platform activity code `extraction` on extraction Provenance | [questionnaire-extraction.md:123](../architecture/questionnaire-extraction.md) |
| `meta.tag` `extracted` plus `managed` / `seed` | [questionnaire-extraction.md:125](../architecture/questionnaire-extraction.md) |
| `meta.tag = provisional` and the provisional identifier system `urn:…/provisional/{node_id}` | [identifiers.md:47](../architecture/identifiers.md) |
| NamingSystems for NIN, ECID, MRN and program numbers in the country package | [identifiers.md:41](../architecture/identifiers.md) |
| Task `businessStatus` = engine state label (status stays within the 12 R5 codes) | [workflow-engine.md:15](../architecture/workflow-engine.md) |
| Break-the-glass: `v3-ActReason#BTG` and break-the-glass security labels | [offline-auth.md:69-71](../architecture/offline-auth.md) |
| `relatedArtifact` type `derived-from` on R4 imports | [metadata.md:57](../architecture/metadata.md) |

## Planned contents

- One page per profile and extension: purpose, canonical, the package that ships it, binding strengths, examples.
- Notes on R4 → R5 conversions applied at import and in integration adapters (`health-fhir-mapping`).
- The CapabilityStatement summary: supported resources, interactions and SearchParameters.
