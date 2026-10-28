# QuestionnaireResponses are extracted by SDC templates held in separate Extraction Mappings, and re-derived at authority

The only extraction mechanism at runtime is SDC **template-based** extraction: FHIRPath-annotated resource templates that produce a transaction plan.
- **Observation-based items** (`observationExtract`) are accepted, but are converted into templates at publish time.
- **Separate definitions.** Templates live in an **Extraction Mapping**: a Published Definition owned by Terminology & Metadata, with its own canonical `url|version` and the Questionnaire version range it applies to. They are not contained in the Questionnaire.
- **When it runs.** Extraction is a pure function of the QuestionnaireResponse, the Questionnaire and the context recorded on the response (subject, encounter, author, `authored`). It runs on the writing node when the response becomes `completed` or `amended`, in the same transaction as the response, the extracted resources and their Provenance.
- **All or nothing.** Any extraction or validation error fails completion; authorization-only failures are accepted and flagged (ADR 0011).
- **Re-derived at authority.** The Authority Node re-runs extraction with the mapping version recorded on the response and compares content hashes. A mismatch rejects the sync group.

We chose this for three reasons:
- Template extraction needs only FHIRPath, which we already depend on, and covers any resource type with conditionals and repeats.
- A separate mapping can be corrected without issuing a new Questionnaire version, and the deterministic id name already keys on the mapping canonical (ADR 0013).
- Purity makes re-derivation a cheap integrity check across nodes.

## Considered Options

- **Definition-based extraction.** Deferred: it needs StructureDefinition snapshot walking in Rust. The mapping format leaves room for it.
- **StructureMap / FML.** Deferred: it needs a mapping-language engine, and none exists in Rust.
- **SDC's "log errors and continue".** Rejected: partial extraction silently loses clinical data.
- **Templates contained in the Questionnaire, as SDC does.** Rejected as the storage form, so that mappings can be versioned independently. SDC imports are split into a Questionnaire and a generated mapping.
- **Validating extracted resources at authority without re-deriving them.** Rejected: it cannot detect a node that extracted the wrong values.

## Consequences

- Mapping FHIRPath has no access to `now()`, queries or `launchContext`.
- Every mapping ships with fixture QuestionnaireResponses and their expected output, and cannot be published unless they pass.
- Previews come from a dry-run `$extract` on the node. There is no TypeScript extractor.
