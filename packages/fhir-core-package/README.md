# health.platform.core

The platform core package (#87; [ADR 0049](../../docs/adr/0049-engine-native-definitions-as-profiled-library-and-plandefinition.md)): the profiles of the engine-native Published Definitions, their extensions, and the platform's own CodeSystem. Its canonical base is `https://carapai.github.io/people-centered-healthcare/fhir` ([docs/fhir](../../docs/fhir/README.md) § Canonical URLs).

| Artefact | What it is |
|---|---|
| `ExtractionMappingLibrary` | An Extraction Mapping: a `Library` of type `extraction-mapping`, naming its target Questionnaire (`extraction-target-questionnaire`) and version range (`extraction-target-versions`). Its SDC templates are contained, each referenced as `composed-of`, and each states how it governs its target (`extraction-template`: `mode`, `retraction`, `ownerUseCase`; invariant `hp-template-1`) |
| `MatchingRulesLibrary` | Matching Rules: a `Library` of type `matching-rules` with `application/vnd.health.matching-rules+json` content |
| `WorkflowDefinitionLibrary` | A Workflow Definition: a `Library` of type `workflow-definition` with `application/vnd.health.workflow+json` content |
| `ProgramPlanDefinition` | A program: a `clinical-protocol` `PlanDefinition` with the EpisodeOfCare type its enrolment opens (`program-episode-type`) and, optionally, its FHIRPath eligibility (`program-eligibility`) |
| `Platform<Resource>`, 15 profiles | Locally pinned profiles of the low-maturity R5 resources the platform serves, each pinning its R5 5.0.0 definition with no constraint of its own, so a change to base FHIR never changes the platform's API. **Financial** (ADR 0030): ChargeItem, Account, Invoice, Coverage, CoverageEligibilityRequest, CoverageEligibilityResponse, Claim, ClaimResponse, ExplanationOfBenefit, PaymentReconciliation, PaymentNotice. **Inventory façades** (ADR 0034): InventoryItem, SupplyRequest, SupplyDelivery, InventoryReport |
| Module extensions, 9 | Extensions the platform's modules write on the resources they own. **QuestionnaireResponse:** `extraction-mapping`, the mapping a completion used (#93). **EpisodeOfCare** (#116): `episode-program`, `episode-enrolled-by`, `episode-transferred-from`, `episode-transferred-to`. **CarePlan:** `care-plan-author`. **Medication**, a stock product: `product-base-unit`, `product-pack-size`, `product-synonym` |
| `CodeSystem/platform` | The Library types (`extraction-mapping`, `matching-rules`, `workflow-definition`), template modes (`managed`, `seed`), with `ValueSet/template-modes`, and the Provenance activity `extraction` of an extraction run |

Every definition needs its `url`, `version` and `name`.

## Layout

- **`input/fsh/`:** the sources, in FSH. `examples.fsh` holds one example of each profile.
- **`sushi-config.yaml`:** SUSHI's configuration: the canonical base, the version and FHIR 5.0.0.
- **`package/`:** the package, **generated and committed**:
  - the StructureDefinitions, with their snapshots;
  - the CodeSystem and ValueSet;
  - `package.json` (`national-core`, so two approvers);
  - the manifest Library. It pins the base definitions as `depends-on` and every artefact as `composed-of`.
- **`examples/`:** the examples, generated, outside the package.

## Building

```sh
tools/fhir/build-core-package.sh   # needs bun, java 17+ and jq
```

1. SUSHI (`fsh-sushi`, pinned in the script) compiles the FSH into differentials.
2. The HL7 validator, the tooling oracle `tools/fhir/fetch-java-validator.sh` pins, generates each snapshot. Packages ship snapshots, and the product never generates them (ADR 0053), so no Java runs in the product.
3. The script writes `package/` and `examples/`.

Re-running it on unchanged sources changes nothing. CI's Java job rebuilds the package and fails if the result differs from what is committed. The validator refuses a file whose name contains "template", so the script hands it neutral names.

## Publishing

The package is published through central's lifecycle like any other (#88): `central-server package draft packages/fhir-core-package/package --by …`, then submit, review, two approvals and publish. `health-publication`'s `tests/core_package.rs` takes it through that whole flow, and the result verifies under the root.

## Tests

- **`health-fhir-validation`'s `tests/core_package.rs`:** the snapshots compile for tier 3, with HL7's `SimpleQuantity` from the `hl7.fhir.r5.core` dependency, which R5's own snapshots name. Each example passes, and each profile's constraints are enforced. A valid resource of each pinned type passes its profile, a resource of another type does not, and R5's cardinalities still hold.
- **`health-publication`'s `tests/core_package.rs`:** the package publishes through the lifecycle.

## Not yet

- **JSON Schemas for configuration:** they ship with the software release, not this package (#86). Each one comes with the feature that reads it.
- **Required bindings,** such as a template's mode being `managed` or `seed`. Tier 4 checks them (#91).
