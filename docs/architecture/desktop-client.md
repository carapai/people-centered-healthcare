# Tauri desktop client

Status: **accepted** (ADRs 0039, 0040, 0041). Produced while resolving the Wayfinder ticket [What can a standalone Tauri client do offline, and how is it packaged?](https://github.com/carapai/people-centered-healthcare/issues/30). Background: [docs/research/tauri.md](../research/tauri.md). Spec: MASTER_PROMPT §31, §75, §77, §90, §114. MASTER_PROMPT §8 lists no desktop doc, so this page is an addition. Related decisions:
- roles and leases: [write-authority.md](write-authority.md);
- transport: [client-application.md](client-application.md) (ADR 0036);
- embedded storage: [persistence.md](persistence.md);
- sync scope: [synchronization.md](synchronization.md);
- enrollment: [lan-security.md](lan-security.md);
- sessions: [offline-auth.md](offline-auth.md).

## Roles (ADR 0039)

One binary (`apps/client/src-tauri` over `health-desktop-runtime`) links the same application crates as the edge. Its role is set at provisioning.

| | Mode B: standalone | Under an edge |
|---|---|---|
| Authority | Facility Authority Node for its facility | Candidate Versions; an Authority Lease for outreach |
| Upstream | Central | Its edge (pinned) |
| Sync Scope | Full facility scope | Its service points, worklists and leased sub-stores, plus the last 90 days |
| Serves the LAN | No | No |
| Offline: available | Everything the facility does | Registration (provisional), triage, consultation, forms, orders, dispensing (candidates or leased sub-store), worklists |
| Offline: unavailable | Central-only functions: payer and payment integrations, NIN checks, golden-record matching, claims, analytics, metadata and price authoring | Those, plus admin, conflict resolution, count approval, and user and device administration |

## Process

```mermaid
flowchart LR
  W[Webview<br/>React app] -- invoke envelope --> CORE
  subgraph CORE [Tauri core process]
    R[Axum router via tower oneshot] --> APP[Application crates<br/>same as edge]
    APP --> DB[(Embedded SurrealDB<br/>RocksDB, sync=every)]
    APP --> BL[(Blob store)]
    SY[health-desktop sync] --> DB
    AU[health-auth] --> KS[OS keystore]
    FO[Upstream router<br/>pass-through / local] --> R
  end
  SY <-- sync/v1 --> UP[(Edge or central)]
  FO -- pass-through when online --> UP
```

- **Lifetime.** Services and sync run while the app runs. In mode B, closing the window minimizes it to the tray, and starting at login is optional. The desktop never installs an OS service; a facility that needs always-on uses a Facility Edge Node.
- **Commands.** All commands are `async`. Large payloads use raw IPC bodies, and progress streams use `Channel`.
- **Single instance.** The single-instance plugin prevents a second app instance, and the RocksDB `LOCK` backs it up.

## Online and offline under an edge

```mermaid
stateDiagram-v2
  [*] --> PassThrough: edge reachable
  PassThrough --> Offline: edge unreachable ≥ 30 s
  Offline --> Draining: edge reachable again
  Draining --> PassThrough: outbox drained
  Offline --> Offline: writes become local candidates
```

- **PassThrough.** The Rust runtime forwards commands to the edge, which confirms them as the authority. The local replica follows by pull.
- **Offline.** A "working offline" banner is shown. Writes become local Candidate Versions in the outbox.
- **Draining.** Writes stay local until the outbox has drained, to keep ordering, then pass-through resumes.
- **Invisible to the UI.** The UI never sees the switch: it always uses `TauriIpcTransport`.

## Storage and backup (ADR 0040)

- **Data directory:** per machine (`%ProgramData%\<app>` on Windows, `/var/lib/<app>` on Linux), shared by every user's sessions on that PC.
- **Engine:** embedded SurrealDB on RocksDB with `sync=every`, plus the content-addressed blob store (ADR 0021).
- **Encryption at rest.** SurrealDB has no native encryption at rest. Provisioning checks for **OS full-disk encryption** (BitLocker or LUKS), and country configuration decides whether its absence warns or blocks.
- **Backups:**

  | Role | Backup |
  |---|---|
  | Mode B | Edge design: hourly encrypted logical export, to a second volume or USB, key escrowed at central (backup-and-recovery.md) |
  | Under an edge | Hourly encrypted export, protecting the unsent outbox |

- **Validation gate.** §75 allows embedded SurrealDB on the desktop only after validation: [Does embedded SurrealDB run reliably inside the Tauri process on facility PCs?](https://github.com/carapai/people-centered-healthcare/issues/39). If it fails, those facility types run an edge instead.

## Secrets

- **Where:** the device and node keys, tokens and the backup key live in the **OS keystore**, through the `keyring` crate called from Rust: Windows Credential Manager, or Linux Secret Service.
- **Never in JavaScript.**
- **No keystore, no install.** Provisioning is refused without a working keystore. There is no Stronghold or file fallback.
- **TPM-backed keys** are a later hardening step.

## Provisioning

| Role | Steps |
|---|---|
| Under an edge | 1. Pairing and device enrollment with the edge (lan-security.md)<br>2. Seed of the Sync Scope from the edge |
| Mode B | 1. A central admin issues a one-time provisioning code for the facility<br>2. The desktop gets a node certificate from central for mTLS<br>3. Seed of the full facility scope from central, over the network or signed USB |

- **Local login (ADR 0010).** In mode B and in offline fallback, the desktop runs `health-auth` itself, with the same rules as the edge: snapshot expiry, PIN switching, a 15-minute idle lock and the clock floor.
- **Who can log in offline:** only staff whose permission snapshot is on the device. Mode B gets snapshots for all of the facility's staff. A desktop under an edge gets them for staff assigned to its service points.
- **Online under an edge,** the edge is the authorization server. The session travels over IPC (ADR 0036).

## Revocation

| Kind | Behaviour |
|---|---|
| `retire` | Push the outbox, then wipe local data and keys |
| `compromised` | Session refused. The desktop wipes on its next contact; unsynced data is lost |
| Scope shrink | Audited purge of out-of-scope records (persistence.md) |

## Platforms and packaging (ADR 0041)

- **Tauri** at least 2.11.6 (2026 origin and channel advisories), pinned to the 2.12 line.
- **Targets:**

  | Platform | Packages | Webview |
  |---|---|---|
  | Windows 10/11 x64 | Per-machine NSIS: a standard bootstrapper, and a WebView2 `offlineInstaller` variant (+127 MB) for offline provisioning | WebView2 |
  | Ubuntu 22.04 / 24.04 LTS x64 | AppImage, deb | WebKitGTK 4.1 |

- **macOS is deferred** (see the map's Not yet specified).
- **Web baseline.** The React build targets es2022, and end-to-end tests run on WebView2 and WebKitGTK.
- **Capabilities.** One main window with an explicit command allowlist (`AppManifest::commands`). No remote URLs are granted capabilities.

## Signing and updates

| Artifact | Signing |
|---|---|
| Windows installer | Authenticode via Azure Artifact Signing |
| Tauri updater artifacts | Updater key in central's KMS/HSM, with an offline backup from a key ceremony; `TAURI_SIGNING_*` is never exposed to Vite |
| AppImage | GPG (`APPIMAGETOOL_FORCE_SIGN=1`) |
| deb | Signed apt repository |

- **Staged roll-out.** A dynamic updater endpoint at central returns the version allowed for each facility, like package activations.
- **Offline sites** install a signed installer from USB.
- **When updates install:** at the next restart, never mid-session.
  1. A local backup is taken.
  2. Forward-only migrations run before the app reports ready.
- **Version range.** A desktop must stay within N-1 of its upstream (sync handshake). Otherwise it can only push, quarantined.
- **deb installs** update through apt.

## Peripherals

- **Printing** (receipts, labels) goes through the OS.
- **Barcode scanners** are supported in keyboard-wedge mode, with no driver work.
- **Serial or HL7 analyser links** are out of v1 and sit in the map's integration-catalogue fog.
