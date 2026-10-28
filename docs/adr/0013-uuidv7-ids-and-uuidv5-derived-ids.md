# Client-generated UUIDv7 resource ids; UUIDv5 for anything derived

Every resource and internal record gets a UUIDv7 id from the node that creates it, so offline creates never collide and replaying a create is idempotent. The same applies to every node id, device id and sync operation id.

Records produced deterministically from another record get a UUIDv5 under one fixed platform namespace, so the same derivation run on any node, or replayed, converges on one id instead of duplicating:
- **Extraction:** the name is the QuestionnaireResponse id, the extraction mapping's canonical URL without version, the target item path with its repeat index, and the target type.
- **Other derivations:** the name is the source id or `op_id` and the derivation rule's canonical URL.

Because the mapping version is left out, re-extracting with a newer mapping produces a new version of the same resource rather than a second resource.

## Considered Options

- **Server-assigned ids:** break offline creation. Rejected.
- **ULID:** equivalent ordering, but non-standard in the FHIR and UUID tooling we use. Rejected.
- **Random ids for derived records with a separate dedup table:** duplicates appear whenever two nodes derive the same thing. Rejected.

## Consequences

- A UUIDv7 reveals creation time to the millisecond. This is accepted because ids are never shown to users or used as business identifiers; those live in `identifier`.
- Ids are never recycled. Merged Patients keep their ids, and tombstones keep theirs.
