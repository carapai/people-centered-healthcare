# Adding a clinical program

> Status: planned layout. No code exists yet; where package sources live in the repository is to be fixed by the scaffold.

## When to use

A new program such as ART, TB, ANC or immunization outreach. A program is a **program package** of Published Definitions, owned by metadata; program enrolment records (EpisodeOfCare) are owned by the `programs` Module at runtime. Country-specific behaviour lives in packages, never in generic engines (AGENTS.md).

## Steps

1. **Program definition** (metadata): the program type, the forms it lists, its workflow definitions.
2. **Profiles** for program records, in the package (validation tier 3).
3. **Forms** and their Extraction Mappings ([adding-a-form.md](adding-a-form.md)). Forms the program lists get the `workflow-episodeOfCare` extension on extracted resources, indexed by a custom SearchParameter.
4. **Workflows**: an EpisodeOfCare-scoped definition (enrollment → assessment → initiation → follow-up → outcome), with appointment timers and lost-to-follow-up escalation ([adding-a-workflow.md](adding-a-workflow.md)).
5. **Program numbers**: a NamingSystem and a per-type number format (node-prefixed sequence by default; reserved blocks only if mandated) ([identifiers.md](../architecture/identifiers.md)).
6. **Terminology**: ValueSets with pinned expansions in the manifest.
7. **Indicators**: Measure / Library definitions where needed; the DHIS2 export mechanism is still in the map's analytics fog.
8. **Financial and stock rules**, if any: funding-source eligibility (e.g. donor ART stock only for enrolled patients) is configuration, and program Price Books are pricing records ([adding-a-price-rule.md](adding-a-price-rule.md)).
9. **Package** as kind "Program": two approvers (program clinical leads), signed, then activated per facility or program scope.

## Expected files

```text
<program package>/package/package.json            kind: program
<program package>/package/Library-manifest.json   pinned dependencies and expansions
<program package>/package/*.json                  program definition, Questionnaires, mappings, workflows, profiles, NamingSystem
<program package>/package/configuration/          Form Layouts, override points
<program package>/package/tests/                  extraction fixtures, workflow scenarios
```

## Contracts

The program definition's canonical; EpisodeOfCare (owned by `programs`); the `workflow-episodeOfCare` extension and SearchParameter; NamingSystem and number format.

## Tests (publication gates)

- **Metadata and terminology:** package validation, semver diff, extraction fixtures, FHIRPath conformance.
- **Workflow:** program scenarios, including timers and lost-to-follow-up.
- **End-to-end:** the ART/TB acceptance scenarios (§118) on central + edge + desktop with an offline period.

## Governing decisions

[ADR 0003](../adr/0003-metadata-owns-published-definitions.md), [ADR 0024](../adr/0024-signed-fhir-npm-packages-with-pinned-manifests.md), [ADR 0025](../adr/0025-publication-separate-from-scoped-activation.md), [ADR 0026](../adr/0026-overrides-only-at-declared-configuration-override-points.md), [configuration.md](../architecture/configuration.md), [workflow-engine.md](../architecture/workflow-engine.md), [questionnaire-extraction.md](../architecture/questionnaire-extraction.md).
