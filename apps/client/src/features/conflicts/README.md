# Sync conflicts

The Conflict Record review queue (#82; synchronization.md § Conflict Record and review queue; ADR 0020), at central, for staff whose roles grant `sync.resolve-conflicts`.

- **`ConflictsPage`** (`/sync/conflicts`):
  - **The queue:** the open Conflict Records (`GET /api/sync/conflicts`), oldest first. Each is marked overdue after 7 days.
  - **One record** (`GET /api/sync/conflicts/{id}`) shows the current version here beside the version the facility sent, without each node's `meta`, and the class's policy. Three decisions are offered (`POST /api/sync/conflicts/{id}`):
    - **keep the current version** (`keep-head`);
    - **use the facility's version**, written on top of the head;
    - **save a corrected version**, edited as JSON from the current one.
- **The server's part:** it keeps both branches, checks and audits each decision, and sends the result back to the facility. The resolution also clears the facility's held candidate. Nothing is merged on this screen.

## Tests

`test/conflicts.test.ts`:
- the queue and a refusal;
- reading a record and finding its candidate;
- comparing without `meta`;
- posting each decision, and a refused one.

The server's queue, decisions and audit are tested in `health-sync-server`.
