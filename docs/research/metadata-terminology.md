# Research: metadata and terminology packaging

There was no separate research ticket for packaging. These facts come from [How do FHIR R5 workflow, SDC extraction and WHO SMART Guidelines fit a generic engine?](https://github.com/carapai/people-centered-healthcare/issues/7), whose full findings are on the unmerged branch `research/clinical-workflows`, and they are also summarised in [clinical-workflows.md](clinical-workflows.md). Decisions: [terminology.md](../architecture/terminology.md), [metadata.md](../architecture/metadata.md), [configuration.md](../architecture/configuration.md) (ADRs 0024–0026).

## CRMI 2.0.0 (STU 2, FHIR R4)

- **Status:** draft → active → retired. "An active artifact SHALL NOT transition back to draft. A new version of the artifact is required." The same applies to retired.
- **Canonicals.** A canonical `url` is "globally unique, version-independent", and references may be `url|version`.
- **Versions.** Artifacts "SHALL specify a version" and SHOULD use `<major>.<minor>.<patch>[-<label>]`. A `package` versioning policy is allowed.
- **Manifests.** A CRMIManifestLibrary uses `composed-of` relatedArtifacts and pins unversioned references, including "stable value set expansion". "Released content is immutable".
- **Operations:** `$release`, `$package`, `$data-requirements`, `$infer-manifest-parameters`.
- **Packaging.** Packages are transaction or collection Bundles with the main artefact first, then its dependencies and test cases. Conditional create on `url` plus `version` is recommended.

## FHIR NPM packages (R5)

- **Contents.** A tarball with a `package` folder, `package.json`, JSON resources, optional `examples/`, and `.index.json`.
- **Fields.** `name` is globally unique, and `version` "SHALL … conform to Semantic Versioning". `dependencies` must include a core package such as `hl7.fhir.r5.core`. `fhirVersions` is an array.
- **Immutability** of a published version is conventional registry practice, not stated in the spec (unverified).

## Terminology (R5)

- **Resources:** CodeSystem, ValueSet, ConceptMap, NamingSystem and TerminologyCapabilities.
- **Operations:** `$expand`, `$validate-code`, `$lookup`, `$translate`, `$subsumes`, `$closure`.
- **UCUM** is documented by HL7 Terminology (THO 7.5.0). That the system URI is `http://unitsofmeasure.org` is standard but was unverified in that session.

## Risks

- **R4 content.** Every SMART, SDC, CRMI and CPG IG is R4, so imports need R4 → R5 conversion, and the exact Questionnaire differences were not verified.
- **SNOMED CT licensing** for Uganda has not been researched (it is in the map's legal fog).
- **UgandaEMR migration** needs ConceptMaps from CIEL / OpenMRS concepts.
