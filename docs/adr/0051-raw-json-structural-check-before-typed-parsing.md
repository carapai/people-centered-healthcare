# Tier 1 runs on raw JSON before typed parsing; stored JSON is the validated input; failed external payloads go to an Inbound Dead Letter

**Raw check first.** Tier 1 structural validation (ADR 0016) runs on the raw JSON, before any typed parse. It checks:
- JSON syntax;
- a known `resourceType`;
- unknown elements;
- cardinality, including required elements;
- primitive formats;
- choice exclusivity.

It lives in `health-fhir-validation` and uses an element table generated from `definitions.json.zip`, so it does not depend on the model crate. Only a resource that passes it is parsed into `fhir-r5` types (ADR 0050).

**Unknown elements are errors.** An unknown element is anything not in the R5 definition that is not an extension. It is a tier 1 error for every source and is never silently dropped. Extensions with unknown URLs are left to tier 3 profiles.

**Parse failures after the check are our bugs.** If typed parsing fails after the raw check passed, the checker is incomplete. The request gets a server-error OperationOutcome and an alert is raised.

**What is stored.** The stored resource is the validated input JSON in canonical form. The server sets `id`, `meta.versionId` and `meta.lastUpdated` in that JSON, and the typed re-serialization is never written back.
- **Our own resources.** Resources the platform creates itself, such as extraction output and workflow-created resources, are built as typed values and serialized from them.

**Reads never need typed parsing.** FHIR read, search, `_history` and sync serve the stored JSON bytes directly. Only domain operations that need typed values parse, and they fail closed with an alert if parsing fails.

**On failure, by source:**

| Source | Tier 1 failure |
|---|---|
| Our UI or API | Rejected with an OperationOutcome |
| Sync ingest | Per-item `rejected` status plus a review record (ADR 0016) |
| External sources: integrations, pulled feeds, and R4 adapters after conversion | Rejected to the caller with an OperationOutcome, and the raw payload is kept in an **Inbound Dead Letter** |
| Package artefacts | Fail package Validation, so the package cannot be published |

- **No lenient typed path** exists for any source.
- **R4 conversion.** An R4 → R5 conversion that cannot produce valid R5 also goes to the Inbound Dead Letter.
- **Inbound Dead Letter store.** It is owned by the integration module, encrypted and audited.
  - Its retention is set per integration, with a default of 90 days.
  - Entries can be reviewed, and replayed once the mapping or the source is fixed.
- **Package examples.** Examples and test fixtures in a package are validated but never activated.

We chose this because the spike showed that typed parsing both refuses some data (a missing required element) and silently loses some (unknown elements and `_id`). The first makes it a poor validator and the second makes it a poor storage format. Checking raw JSON and storing what passed keeps every valid resource intact, whatever gaps the crate has, and gives errors with FHIRPath locations instead of serde messages. A pulled feed has no caller to receive the rejection, so the Inbound Dead Letter is what stops external data being lost.

## Considered Options

- **Lenient typed parsing for external sources.** Rejected: it would accept data we cannot store faithfully, and it would contradict ADR 0016's structural rule.
- **Storing the typed re-serialization.** Rejected: it would bake the crate's losses (`_id`, any future gap) into the record of record.
- **Quarantining structural failures like profile failures.** Rejected: a structurally invalid resource cannot be indexed or served as FHIR. Keeping the raw payload in the Inbound Dead Letter gives the same protection against losing data.

## Consequences

- `health-fhir-validation` ships a generated R5 element table, and CI cross-checks it against the HL7 Java validator oracle.
- The persistence write path takes the validated JSON plus the server-set fields, not a typed value.
- A model-crate regression can break domain operations but never access to records.
