# No browser or webview ever persists clinical data, and clinical writes are confirmed, never optimistic

**Browsers in every mode** (central web, LAN) keep clinical data only in the in-memory TanStack Query cache:
- `gcTime` is 5 minutes;
- the cache is cleared on logout, idle lock and PIN user switch;
- tokens are held in memory only.

**What may persist in a browser:**
- UI preferences (theme, density, table layouts), keyed by user;
- on the LAN, the non-extractable DPoP key (ADR 0009) and the edge's app-shell service worker.

**Service workers** never cache API responses.

**Tauri** persists clinical data only on the Rust side, in the embedded database. Its webview follows the same in-memory rule as browsers.

**Clinical and financial writes** always wait for confirmation. In Tauri, the local Rust commit is the confirmation. Sync status is shown separately: saved locally, synced, under review or rejected.

**Drafts** in browser modes are saved server-side, e.g. a QuestionnaireResponse `in-progress`. If the connection drops, input stays in memory, a banner appears, and the user retries manually. There is no mutation queue in the browser (§74, §77).

**State ownership:**

| State | Owner |
|---|---|
| Server state | Query |
| URL | Router |
| Field drafts | Form |
| Processes (holding ids only) | XState |
| Widget state | Ark UI or React |

TanStack DB is not used in v1.

We chose this because shared facility PCs and unmanaged browsers make any persistent browser copy of clinical data a confidentiality risk. A browser-side outbox would also be a second, unsynchronized writer next to the sync protocol (ADR 0019).

## Considered Options

- **TanStack DB with `offline-transactions` in the browser.** Rejected: it adds a writer outside the Authority Node model and is pre-1.0. It may be revisited behind the repositories for live queries only.
- **Optimistic clinical updates.** Rejected: clinicians would act on writes that may yet be rejected by validation or authority.
- **Caching API responses in the service worker for faster LAN reloads.** Rejected: clinical data would persist on disk.

## Consequences

- A LAN outage stops browser clinical work. Tauri is the offline-fallback client.
- The map's validation spike "TanStack DB behind ClinicalRepository" is dropped.
