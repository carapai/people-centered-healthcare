# One desktop binary serves two roles, and a desktop under an edge fails over to local work inside Rust

The Tauri desktop links the same application crates as the edge into its core process. Its role is set at provisioning:

| Role | Authority | Offline scope |
|---|---|---|
| **Standalone (mode B)** | The Facility Authority Node for its small facility | Everything the facility does, except central-only functions: payer and payment integrations, NIN checks, golden-record matching, claims, analytics, authoring |
| **Under an edge** | Writes Candidate Versions, or holds an Authority Lease | Registration (provisional), clinical capture, orders, dispensing, worklists in scope. Not admin, conflict resolution, count approval or user/device administration |

- **Lifetime.** Services and sync run while the app runs. In mode B, closing the window minimizes it to the tray. The desktop is never an OS service; an always-on server is a Facility Edge Node.
- **Failover under an edge.** While the edge is reachable, the desktop's Rust runtime **passes writes through to the edge**, so the authority confirms them at once, and the local replica follows by pull.
  - After the edge has been unreachable for 30 s (configurable), it switches to local candidates and shows a "working offline" banner.
  - It returns to pass-through once its outbox has drained.
  - The UI always uses the same IPC transport (ADR 0036).
- **Local authentication.** In mode B and in offline fallback, the desktop runs the same `health-auth` as the edge (ADR 0010). Only staff whose permission snapshot is on the device can log in offline.

We chose this for three reasons:
- One codebase for both roles avoids a second set of business logic (§32).
- Passing writes through while online removes most conflicts and gives clinicians immediate authoritative confirmation.
- Keeping failover in Rust stops the UI from having to know about connectivity routing.

## Considered Options

- **Always local-first, even online.** Rejected: every LAN write would be a delayed candidate, so later rejections would surprise users and conflicts would rise.
- **Failover in the TypeScript transport.** Rejected: it would put routing and session logic in the webview and duplicate it across runtimes.
- **Running the desktop as an OS service.** Rejected: that is the edge's role (ADR 0007).

## Consequences

- Embedded SurrealDB on the desktop must pass [Does embedded SurrealDB run reliably inside the Tauri process on facility PCs?](https://github.com/carapai/people-centered-healthcare/issues/39) before use (§75). If it fails, the fallback is edge-only deployment for that facility type.
