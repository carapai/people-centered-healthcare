# `meta.versionId` is the op_id of the write, and our clients create with PUT

A resource version's `meta.versionId` is the UUIDv7 `op_id` of the write that produced it. The Authority Node assigns a separate integer, `version_seq`, which increases monotonically per resource and is used only for ordering and history keys. A Candidate Version written on a desktop or edge therefore keeps the same version id when it is fast-forwarded, so the parent-version pointers used by sync stay valid on every node. Our clients create resources with `PUT [type]/[client-uuid]` (update-as-create, declared as `updateCreate`). This is because R5 requires a server to ignore a client-supplied id on `POST`, and our offline creates depend on client-generated ids (ADR 0013). External clients can still use `POST`, and then the server mints the id.

## Consequences

- ETags are `W/"<uuid>"`, not small integers.
- Our clients must send `If-Match` on every update. A concurrent writer that loses its retry receives 412.
