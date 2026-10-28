# Research: Rust, Axum and Tokio

There was no dedicated research ticket for the Rust server stack. This page records the crate versions observed on crates.io on **2026-10-28**, and summarizes how the architecture uses them. Decisions: [dependency-rules.md](../architecture/dependency-rules.md) and [crate-catalogue.md](../architecture/crate-catalogue.md) (ADRs 0005, 0006), [client-application.md](../architecture/client-application.md) (ADR 0036).

## Versions observed (2026-10-28)

Observed with `cargo search` (versions) and `cargo info` (licence and declared `rust-version`). A blank cell means it was not checked.

| Crate | Latest | Licence | `rust-version` | Role here |
|---|---|---|---|---|
| `axum` | 0.8.9 | MIT | 1.80 | Routers in `health-http` and `health-fhir-server` |
| `tokio` | 1.54.1 | MIT | 1.85 | Runtime in infrastructure and runtime crates; forbidden in domains |
| `tower` | 0.5.3 | | | Middleware; `oneshot` dispatch of Tauri IPC into the router |
| `tower-http` | 0.7.1 | MIT | 1.65 | HTTP middleware (which layers are used is not decided) |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | 1.56 | Allowed in domains |
| `serde_json` | 1.0.154 | | | JSON at rest and on the wire |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 | 1.77 | Errors in every non-runtime layer |
| `anyhow` | 1.0.104 | | | Runtime crates and tooling only |
| `rust_decimal` | 1.43.0 | MIT | 1.67.1 | Money and FHIR decimals; allowed in domains |
| `uuid` | 1.29.0 | Apache-2.0 OR MIT | 1.89.0 | UUIDv7 and UUIDv5 ids; allowed in domains |
| `tracing` | 0.1.44 | MIT | 1.65.0 | Macros only in domains |
| `tracing-subscriber` | 0.3.23 | | | Runtime and observability only |
| `opentelemetry` | 0.33.1 | | | `health-observability` |
| `reqwest` | 0.13.5 | | | Outbound HTTP in infrastructure; forbidden in domains |
| `utoipa` | 6.0.0 | MIT OR Apache-2.0 | 1.88 | OpenAPI from Rust handlers → TS types and Zod |
| `keyring` | 4.2.0 | MIT OR Apache-2.0 | 1.88.0 | Desktop secrets in the OS keystore (ADR 0040) |
| `surrealdb` | 3.3.4 | | | SDK, pinned exact 3.3.x ([surrealdb.md](surrealdb.md)) |
| `windows-service` | 0.8.1 | | | Edge on Windows |
| `sd-notify` | 0.5.0 | | | Edge on systemd |
| `cargo-nextest` | 0.9.150 | | | Test runner in `rust-ci.yml` |
| `cargo-deny` | 0.20.2 | | | Licences, advisories, duplicates, bans |
| `cargo-llvm-cov` | 0.9.1 | | | 80% line floor on domain and application crates |

- Every licence observed above is on the allowlist in [deployment.md](../architecture/deployment.md).
- The local toolchain on the research workstation was `rustc 1.98.0` (2026-08-18). This is not the pinned project toolchain, which is not chosen yet.
- The highest declared `rust-version` above is 1.89.0 (`uuid`). Tauri's stronghold and updater plugins need Rust ≥ 1.90 ([tauri.md](tauri.md)), so the pin cannot go below 1.90.

## How the architecture uses them

- **One router, three runtimes.** Axum routers live in infrastructure (`health-http`, `health-fhir-server`). Central and the edge serve them over HTTP. The desktop dispatches Tauri IPC envelopes into the same router in-process with tower `oneshot`, with no loopback port (ADR 0036). Handlers are transport-agnostic; auth comes from the transport.
- **Ports without macros.** Port traits use native return-position `impl Future<Output = …> + Send`, not `async-trait`. Use cases are generic over ports (static dispatch). `Arc<dyn …>` with a boxed-future variant only at plug-in points such as payer and payment-gateway adapters.
- **Pure domains.** Domain crates may use only `serde`, `thiserror`, `rust_decimal`, `uuid`, the chosen time crate and `tracing` macros. `tokio`, `axum`, `tower`, `surrealdb`, `reqwest`, `tauri`, `tracing-subscriber`, `opentelemetry` and `anyhow` are forbidden there.
- **Tokio tasks** run the edge's background work (outbox dispatcher, sync, timers, backups, certificate renewal, health checks) as supervised tasks in one process ([facility-edge.md](../architecture/facility-edge.md)).
- **Workspace hygiene.** Versions are declared once in `[workspace.dependencies]`; `unsafe_code = "forbid"`; `clippy -D warnings`; `cargo-deny` and `tools/architecture-checks` in CI.

## Not verified

- That `utoipa` 6 integrates with `axum` 0.8 as needed, and which companion crate (if any) is used.
- The IPC round trip through tower `oneshot` inside Tauri; this is part of [Does the frontend scaffold build on Bun, Vite 8, Panda 2, React Compiler and TS 7?](https://github.com/carapai/people-centered-healthcare/issues/38).
- The minimum Rust version that supports the port style above; not checked here.
- Which time crate is "the chosen time crate"; not decided.
- Which `tower-http` layers (tracing, compression, CORS, limits) are needed.
- Licences and `rust-version` for the crates with blank cells.
