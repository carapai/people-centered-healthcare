# Adding a form

> Status: planned layout. No code exists yet; where package sources live in the repository is to be fixed by the scaffold.

## When to use

A new clinical or administrative form: a Questionnaire, optionally with an Extraction Mapping that turns answers into clinical resources, and a Form Layout. Forms are **Published Definitions**, not code. Ordinary command forms (e.g. a payment dialog) are TanStack Form + generated Zod schemas instead; see [adding-a-ui-component.md](adding-a-ui-component.md).

## Steps

1. **Questionnaire** (metadata): clinical structure only, plus intrinsic SDC rendering such as `questionnaire-itemControl`. Canonical `url|version`, own semver.
2. **Live behaviour** in FHIRPath: `enableWhenExpression`, `calculatedExpression`, answer constraints. Every expression must evaluate identically in fhirpath.js and `octofhir-fhirpath`.
3. **Extraction Mapping** (only for items that should produce resources; default is no extraction): SDC template extraction with `templateExtractContext` / `templateExtractValue`. Each target declares `mode` (`managed` or `seed`), `retraction` and `owner_use_case`, and must be on the target Owner's opt-in list.
4. **Population** (optional): SDC expression-based or observation-based `$populate`.
5. **Program context**: if a program lists the form, extracted resources get the `workflow-episodeOfCare` extension.
6. **Form Layout** (configuration): sections and layout keyed to the Questionnaire `url|version`, with declared override points.
7. **Fixtures**: sample QuestionnaireResponses and expected output Bundles in the package `tests/`.
8. **Package and publish** through the lifecycle: Validation → Simulation → Review → two approvers → signed → activated per scope ([configuration.md](../architecture/configuration.md)).

## Expected files

```text
<package>/package/Questionnaire-<id>.json
<package>/package/<Extraction Mapping>.json   (resource representation to be fixed by the scaffold)
<package>/package/configuration/<form layout>.json
<package>/package/tests/<fixture responses and expected bundles>
<package>/package/package.json, Library-manifest.json
```

## Contracts

- Questionnaire and Extraction Mapping canonicals; the QuestionnaireResponse extension that records the mapping `url|version`.
- Owner opt-in lists of extractable types/profiles.
- The Form Layout JSON Schema shipped with the platform release.
- `health-fhir-extraction` (pure extractor and `$populate`) and the client `QuestionnaireRenderer`.

## Tests (publication gates)

- **Metadata and terminology:** package validation; semver diff rules; every expression parses and type-checks; every fixture produces exactly its expected Bundle; every output validates against its profile; FHIRPath conformance across fhirpath.js and octofhir.
- **Frontend:** renderer component tests and axe for the layout.
- **Offline and sync:** re-derivation at authority (`extraction_mismatch`) is covered by the sync simulation.

## Governing decisions

[ADR 0022](../adr/0022-template-extraction-with-separate-mappings-re-derived-at-authority.md), [ADR 0023](../adr/0023-managed-and-seed-extraction-targets.md), [ADR 0024](../adr/0024-signed-fhir-npm-packages-with-pinned-manifests.md), [ADR 0025](../adr/0025-publication-separate-from-scoped-activation.md), [ADR 0038](../adr/0038-fhirpath-js-for-live-form-behaviour.md), [questionnaire-extraction.md](../architecture/questionnaire-extraction.md), [metadata.md](../architecture/metadata.md).
