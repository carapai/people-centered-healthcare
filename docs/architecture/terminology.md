# Terminology layer

Status: **accepted** (ADRs 0003, 0024, 0025). Produced while resolving the Wayfinder ticket [How are terminology, metadata and configuration packaged and published?](https://github.com/carapai/people-centered-healthcare/issues/23). Packaging and lifecycle are in [configuration.md](configuration.md). The metadata layer is in [metadata.md](metadata.md). Background: [docs/research/metadata-terminology.md](../research/metadata-terminology.md).

## Scope

Terminology defines clinical **meaning** (§40). It is owned by the `terminology` Module (Terminology & Metadata).

| Artefact | Notes |
|---|---|
| CodeSystem | National and local code systems are authored here. External code systems are shipped as terminology packages |
| ValueSet | Every ValueSet a package binds to ships with a **pinned expansion** in the manifest |
| ConceptMap | E.g. CIEL / OpenMRS → national codes for UgandaEMR migration; ICD-10 ↔ ICD-11 |
| NamingSystem | Identifier systems (identifiers.md) |
| UCUM | Quantity units, handled through `octofhir-ucum` (fhir-r5-model.md) |

Metadata references terminology. It never copies concept definitions (§41).

## Runtime service

- **One service, every node.** `health-terminology` runs on every node (central, edge, desktop) and serves these operations over local data:
  - `$expand`
  - `$validate-code`
  - `$lookup`
  - `$translate`
  - `$subsumes`
- **No external terminology server at runtime.** Edges must work fully offline. Large external code systems arrive as their own terminology packages, and bindings use the pinned expansions from the manifest.
- **Versions.** `Coding.version` is recorded for locally defined, versioned CodeSystems, so historical codes resolve against the version that issued them.
- **Validation.** Binding validation (tier 4 in fhir-r5-model.md) uses the pinned expansion that was active when the record was written.

## Which code systems

**Which code systems a country uses** is a country-configuration choice: ICD-10 or ICD-11 for diagnoses and HMIS reporting, LOINC for lab, SNOMED CT subsets, CIEL for migration. The platform supports any of them.

**Licensing.** SNOMED CT needs an affiliate licence where the country is not a member. Uganda's status is in the map's legal fog.

## Lifecycle

Terminology packages follow the same lifecycle as every package ([configuration.md](configuration.md)).
- **Semver diff rules for terminology:**
  - adding a code is **minor**;
  - changing a display is **patch**;
  - removing or redefining a code is **major**.
- **Facility overrides** can never change terminology (ADR 0026).
