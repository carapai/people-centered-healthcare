# health-sync-server

Layer: **infrastructure**. These are the `/sync/v1` endpoints a node serves its downstream peers: central serves edges and standalone desktops (synchronization.md; ADRs 0019, 0062; #81).

- **Authentication (ADR 0062).** Every request carries `Authorization: Health-Node <jws>`, a compact ES256 JWS signed with the node's facility-intermediate key. It covers the request's method, path and body hash, and must be signed within 5 minutes of central's clock.
  - `NodeRegistry::authenticate_sync` (`health-provisioning`) checks it against the node's key on record and requires the node to be `active`.
  - The node's facility, from the registry, sets its `SyncScope`.
- **`POST /sync/v1/hello`** takes a `Hello` and answers a `Welcome`: central's node id and the verdict. The verdict is kept (`sync_peer`), and every later push and pull of the node is held to it.
- **`POST /sync/v1/push`** takes NDJSON items and answers NDJSON `GroupResult`s, one per transaction.
  - It goes through `health_sync_protocol::receive` over `SurrealAuthority`.
  - A quarantined node's items are held in `sync_quarantine` and answered `quarantined`.
  - A refused node gets 403.
- **`POST /sync/v1/pull`** takes a `PullRequest` and answers a `PullResponse` in the node's scope. An out-of-range node is sent nothing.
- **`GET /sync/v1/events`** is a server-sent event stream of `cursor` hints, so a connected node pulls without waiting for its poll.
- **Bodies** are uncompressed. zstd is a later transport change.
- **Conflict review (#82):** `review_router` is the Conflict Record queue. Central mounts it at `/api/sync/conflicts` behind the subject layer, for staff whose roles grant `sync.resolve-conflicts`.
  - `GET /` lists the open records, oldest first, with `overdue` after 7 days.
  - `GET /{id}` shows both branches.
  - `POST /{id}` takes `keep-head`, or `write` with the corrected version. The resolution and its AuditEvent commit together.
