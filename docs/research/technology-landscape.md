# Research: technology landscape

An index of the chosen stack, with the versions already recorded in the other research pages. It makes no new claims: every version below is copied from the linked page, with that page's date. There was no dedicated research ticket for this page. Spec: MASTER_PROMPT §8.

## Backend

| Piece | Choice | Version recorded | Source |
|---|---|---|---|
| Language | Rust, 2024 edition | Toolchain pinned in `rust-toolchain.toml` (not yet created) | [rust-and-axum.md](rust-and-axum.md) |
| HTTP and async runtime | Axum, Tower, Tokio | axum 0.8.9, tower 0.5.3, tokio 1.54.1 (2026-10-28) | [rust-and-axum.md](rust-and-axum.md) |
| Database | SurrealDB, RocksDB engine, pinned exact 3.3.x | Latest stable 3.3.4 (crates.io, 2026-10-13); no LTS policy | [surrealdb.md](surrealdb.md), [embedded-storage.md](embedded-storage.md) |
| Money | `rust_decimal` | 1.43 | [health-financing.md](health-financing.md) |
| Windows service | `windows-service` | 0.8.1 | [facility-edge.md](facility-edge.md) |
| systemd notify | `sd-notify` | 0.5 | [facility-edge.md](facility-edge.md) |
| LAN discovery | `mdns-sd` | 0.21 | [facility-edge.md](facility-edge.md) |
| Edge certificates | `rcgen` with rustls | rcgen 0.14 | [facility-edge.md](facility-edge.md) |

## FHIR

| Piece | Candidates / choice | Version recorded (2026-10-27) | Source |
|---|---|---|---|
| R5 model crate | `fhir-r5` (ADR 0050) | `fhir-r5` =4.2.3; `fhir-r4` =4.2.2 | [fhir-r5.md](fhir-r5.md) |
| FHIRPath (Rust) | `octofhir-fhirpath` | 0.4.57, 1176/1176 official tests (2026-10-21) | [fhir-r5.md](fhir-r5.md) |
| UCUM | `octofhir-ucum` | 0.8 | [fhir-r5.md](fhir-r5.md) |
| Core package | `hl7.fhir.r5.core` | 5.0.0 | [fhir-r5.md](fhir-r5.md) |
| CI validation oracle | HL7 Java validator | Not recorded | [fhir-r5.md](fhir-r5.md) |

## Desktop

| Piece | Choice | Version recorded | Source |
|---|---|---|---|
| Shell | Tauri v2 | `tauri` 2.12.5, CLI 2.12.4 (21 Oct); floor 2.11.6 | [tauri.md](tauri.md) |
| Webviews | WebView2 (Windows), WebKitGTK 4.1 (Linux) | Evergreen / distro | [tauri.md](tauri.md) |
| Secrets | `keyring` crate (no Stronghold) | Not recorded there; see [rust-and-axum.md](rust-and-axum.md) | [tauri.md](tauri.md) |

## Frontend (as of 2026-10-27)

| Piece | Version recorded | Source |
|---|---|---|
| React, React Compiler | 19.3, 1.0 | [frontend.md](frontend.md) |
| TanStack Router, Query, Form | 1.170, 5.105, 1.33 | [frontend.md](frontend.md) |
| TanStack Table, Virtual | 9.3, 3.14 | [frontend.md](frontend.md) |
| XState, `@xstate/react` | 5.33, 6.1 | [frontend.md](frontend.md) |
| Ark UI (Zag) | 5.39 (1.45) | [frontend.md](frontend.md) |
| Panda CSS | 2.4 (ESM-only; Node ≥ 22) | [frontend.md](frontend.md) |
| Zod, Vite, Bun, Lucide | 4.6, 8.3, 1.4, 1.62 | [frontend.md](frontend.md) |
| FHIRPath (JS) | fhirpath.js (version not recorded) | [client-application.md](../architecture/client-application.md) |
| Money (JS) | `decimal.js` (version not recorded) | [health-financing.md](health-financing.md) |

Pre-1.0 pieces not used in v1: TanStack DB, Store, Devtools (dev only), Pacer (wrapped) ([frontend.md](frontend.md)).

## Other research pages

These carry design facts rather than library versions: [synchronization.md](synchronization.md), [offline-auth.md](offline-auth.md), [clinical-workflows.md](clinical-workflows.md), [metadata-terminology.md](metadata-terminology.md), [patient-identity.md](patient-identity.md), [inventory.md](inventory.md).

## Open validation spikes

- [Does embedded SurrealDB RocksDB survive hard power loss on facility hardware?](https://github.com/carapai/people-centered-healthcare/issues/34)
- [Which R5 model crate passes the round-trip spike: helios-fhir or fhir-r5?](https://github.com/carapai/people-centered-healthcare/issues/35)
- [Does the per-node commit counter sustain facility write rates on edge hardware?](https://github.com/carapai/people-centered-healthcare/issues/36)
- [Can SurrealDB blocking indexes serve patient matching at 45M-person scale?](https://github.com/carapai/people-centered-healthcare/issues/37)
- [Does the frontend scaffold build on Bun, Vite 8, Panda 2, React Compiler and TS 7?](https://github.com/carapai/people-centered-healthcare/issues/38)
- [Does embedded SurrealDB run reliably inside the Tauri process on facility PCs?](https://github.com/carapai/people-centered-healthcare/issues/39)
