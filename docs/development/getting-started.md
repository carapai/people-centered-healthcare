# Getting started

> Status: planned layout. No code exists yet, so there is nothing to build or run. Paths and commands are confirmed when the workspace scaffold lands.

## Where the project is

- The repository holds the specification (`MASTER_PROMPT.md`), the ADRs (`docs/adr/`), the architecture docs (`docs/architecture/`) and research notes (`docs/research/`).
- Before any task, read [docs/planning/current-state.md](../planning/current-state.md) and [AGENTS.md](../../AGENTS.md). Never claim a check passed unless you ran it in your session.
- Terms such as Owner, Module, Authority Node and Published Definition are defined in [GLOSSARY.md](../../GLOSSARY.md).

## Prerequisites (planned)

| Tool | Version | Why | Source |
|---|---|---|---|
| Rust toolchain | As pinned in `rust-toolchain.toml` (file not created yet); Rust 2024 edition | All backend crates, edge and desktop runtimes | [dependency-rules.md](../architecture/dependency-rules.md), MASTER_PROMPT |
| C++ compiler and libclang | Platform default | Embedded RocksDB (`kv-rocksdb`) builds from C++ | [research/surrealdb.md](../research/surrealdb.md) |
| SurrealDB CLI | Pinned exact **3.3.x**, same as the SDK | `surreal validate` for migrations, local server for repository tests | [persistence.md](../architecture/persistence.md) |
| `surql-fmt` | To be fixed by the scaffold | Formatting `.surql` migrations | [persistence.md](../architecture/persistence.md) |
| Bun | 1.4 | Frontend workspace, `bun install --frozen-lockfile` | [client-application.md](../architecture/client-application.md) |
| Node | 22, **possibly** | Panda CSS 2 may need Node 22 even in a Bun setup; open until [Does the frontend scaffold build on Bun, Vite 8, Panda 2, React Compiler and TS 7?](https://github.com/carapai/people-centered-healthcare/issues/38) closes | [research/frontend.md](../research/frontend.md) |
| Cargo tools | `cargo-nextest`, `cargo-deny`, `cargo-llvm-cov` | The CI jobs in `rust-ci.yml` | [deployment.md](../architecture/deployment.md) |
| Tauri prerequisites | Tauri 2.12 line (floor 2.11.6); WebView2 on Windows, WebKitGTK 4.1 on Ubuntu | Desktop builds | [desktop-client.md](../architecture/desktop-client.md) |
| `gh` CLI | Any recent | Issues, the Wayfinder map | [docs/agents/issue-tracker.md](../agents/issue-tracker.md) |

- Tauri's stronghold and updater plugins need Rust 1.90 or later ([research/tauri.md](../research/tauri.md)), so the pinned toolchain cannot be older.
- Supported development targets follow the build matrix: Windows 10/11 x64 and Ubuntu 22.04/24.04 x64. macOS is not a supported artifact target in v1 ([deployment.md](../architecture/deployment.md)).

## Planned repository layout

```text
crates/foundation/      crates/contracts/      crates/domains/
crates/application/     crates/infrastructure/ crates/testing/
crates/edge/            crates/desktop/
apps/server/central-server   apps/server/facility-edge
apps/client/                 apps/client/src-tauri
tools/architecture-checks    tools/synthetic-data
```

Layers and their directories are fixed in [dependency-rules.md](../architecture/dependency-rules.md). The Bun workspace layout (beyond `apps/client/src/platform/`) is to be fixed by the scaffold.

## Reading order

1. [resource-ownership.md](../architecture/resource-ownership.md) and [write-authority.md](../architecture/write-authority.md): who owns and who accepts each record.
2. [dependency-rules.md](../architecture/dependency-rules.md) and [crate-catalogue.md](../architecture/crate-catalogue.md): where code goes.
3. [persistence.md](../architecture/persistence.md) and [synchronization.md](../architecture/synchronization.md): how writes are stored and moved.
4. [client-application.md](../architecture/client-application.md): how the one React app talks to every runtime.
5. [architecture-rules.md](architecture-rules.md) and [testing.md](testing.md) in this folder.

## Data

Development and CI use synthetic data only, from `tools/synthetic-data`. Real patient data never enters CI or staging ([testing.md](../architecture/testing.md)).
