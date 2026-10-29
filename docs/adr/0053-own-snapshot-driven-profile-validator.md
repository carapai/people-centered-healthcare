# Tier 3 profile validation is our own snapshot-driven validator, sharing the raw-JSON walker with tier 1

**Our own validator.** Tier 3 is built in `health-fhir-validation` as an extension of the raw-JSON tier 1 checker (ADR 0051). It is one walker over raw JSON, driven by compiled StructureDefinition snapshots: base R5 for tier 1, and the active profiles for tier 3. It sits behind a `ProfileValidator` trait, takes raw JSON plus the active profile set, and returns an OperationOutcome.
- **Invariants** run through our patched FHIRPath engine (ADR 0052).
- **Bindings** go to the tier 4 terminology service.

**v1 scope:**
- **In:**
  - profile snapshots;
  - min and max cardinality;
  - `fixed[x]` and `pattern[x]`;
  - type and target-profile restrictions;
  - extension definitions and their contexts;
  - FHIRPath constraints declared in profiles;
  - slicing with all five discriminator types (`value`, `pattern`, `exists`, `type`, `profile`), the slicing rules `closed`, `open` and `openAtEnd`, and `ordered`.
- **Out of v1:**
  - `mustSupport`, which is a UI concern;
  - Questionnaire-specific checks, which are extraction gates;
  - `profile` discriminators that need resolution outside the active package set.

**How profiles reach the validator:**
- **Snapshots come with packages.** Package publication rejects a profile that has only a differential. Generating snapshots is the authoring tool's job: the IG Publisher or SUSHI today, and the Studio's own generator, which is in the map's Studio fog. Nodes and central never generate snapshots, and no Java runs in the product.
- **Compiled at activation.** Profiles are compiled into validator schemas when a package is activated (ADR 0025) and cached per node. If any profile fails to compile, the activation fails and the previous active set stays in force. Nothing is compiled while a write waits.

**Acceptance and limits:**
- **Differential oracle.** The validator is accepted only after a differential test against the HL7 Java validator (the CI oracle in ADR 0016). It covers every official R5 example against base R5, the core-package profiles with their examples, and our slicing fixtures.
  - There must be zero disagreements on errors in our fixtures.
  - Each disagreement on the official corpus is listed with a reason and accepted one by one.
  - The test reruns on every bump of the validator or the oracle.
- **Unsupported constructs fail publication.** A profile using a construct the validator does not support, or gets wrong against the oracle, is reported as unsupported at package Validation, so that package cannot be published. It is never silently passed.
- **Budget.** Tiers 1–3 for a single clinical resource, without terminology, take at most 2 ms at p95 on the reference edge hardware. A CI performance test enforces it.

We chose this because neither Rust candidate can be relied on.
- **`helios-fhir-validator` 0.2.6** cannot share a build with `fhir-r5` or `octofhir-fhirpath`: Cargo fails on its exact `serde =1.0.224` pin. It also evaluates only `pattern` discriminators, which gives a false positive on the official vital-signs profile.
- **`octofhir-fhirschema` 0.3.30** drops a profile's slicing on any element that exists in the base, because its merge step does not copy `slicing`. It never enforces `fixed` or `pattern` values. So a missing vital-signs category or a heart rate in "kg" passes as valid.
  - It misses two `value[x]` fields, rejects valid `integer64` strings and extension-only primitives, and flags 17% of the official examples.
  - It pulls in two `reqwest` versions, `rusqlite` and `hyper`, it has one maintainer, and it has no LICENSE file.
- **Fixing it would mean writing most of tier 3 anyway,** while our tier 1 walker already exists.

## Considered Options

- **`helios-fhir-validator` as a separate process on every node:** a second process to package and supervise, for a validator that is wrong on value slicing. Rejected.
- **A patched fork of `octofhir-fhirschema`:** the fixes amount to most of tier 3, plus its dependency weight and single-maintainer risk. Rejected. Its slicing-merge bug is reported upstream when the task lands.
- **The HL7 Java validator on nodes:** the JVM on edges and desktops is ruled out (ADR 0016). Rejected.

## Consequences

- The backlog gains the task [Build the snapshot-driven profile validator (tier 3)](https://github.com/carapai/people-centered-healthcare/issues/145). Tiers 3–4 narrows to bindings and wiring at activation.
- Packages must ship snapshots, so authoring tools must produce them.
