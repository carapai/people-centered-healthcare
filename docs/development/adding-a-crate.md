# Adding a crate

> Status: the root `Cargo.toml` (#42) and `tools/architecture-checks` (#43) exist. Run `cargo run -p architecture-checks` after adding a crate.

## When to use

Any new workspace member. Crates are created lazily, when the first implementation task needs them, and only if [crate-catalogue.md](../architecture/crate-catalogue.md) says the code starts as a crate or a split is now justified (a second consumer or a different dependency set).

## Steps

1. Pick the layer and its directory from [dependency-rules.md](../architecture/dependency-rules.md): `crates/foundation/`, `crates/contracts/`, `crates/domains/`, `crates/application/`, `crates/infrastructure/`, `crates/edge/`, `crates/desktop/`, `apps/…`, or `crates/testing/`.
2. Name it `health-<name>` following the catalogue (`health-<context>-contract`, `health-<context>-app`, `health-<module>`).
3. Write `Cargo.toml`:

   ```toml
   [package]
   name = "health-<name>"
   description = "<one line>"
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true
   publish.workspace = true

   [package.metadata.health]
   layer = "domain"           # foundation | contracts | domain | application | infrastructure | runtime | testing | tooling
   context = "<context>"      # omitted for foundation, shared contracts and infrastructure

   [lints]
   workspace = true

   [dependencies]
   serde = { workspace = true }
   ```

4. Add any new crate once, to root `[workspace.dependencies]`, including workspace crates as `{ path = "…" }` entries, so every dependency is `{ workspace = true }`. A new external in a domain crate needs an allowlist entry, reviewed like an ADR.
5. Do not add `[features]` to domain or application crates. Features in infrastructure or runtime crates only choose adapters.
6. Keep items `pub(crate)` by default and re-export the public API from `lib.rs`.
7. Add the crate to the root workspace members, and to `CODEOWNERS` if it is a sync or security crate ([deployment.md](../architecture/deployment.md)).
8. Update [crate-catalogue.md](../architecture/crate-catalogue.md) if the crate is new to the catalogue or was split out of a host.

## Expected files

- `crates/<layer-dir>/health-<name>/Cargo.toml`
- `crates/<layer-dir>/health-<name>/src/lib.rs`
- `crates/<layer-dir>/health-<name>/tests/` (integration-style tests, until they cross crates)
- Root `Cargo.toml` (members, `[workspace.dependencies]`)

## Contracts

- `[package.metadata.health]` layer and context tags, read by `tools/architecture-checks`.
- Error types with `thiserror` (or `anyhow` only in runtime crates and tooling).

## Tests

- `tools/architecture-checks`: layer tag present, no upward or sideways edges, no inline versions, lints inherited, no cycle, testing crates only as dev-dependencies.
- `cargo-deny` (licences, advisories, duplicates, bans). Allowed licences are listed in [deployment.md](../architecture/deployment.md).
- `cargo clippy -D warnings`, `cargo doc -D warnings`.
- Domain and application crates enter the 80% coverage floor ([testing.md](../architecture/testing.md)).

## Governing decisions

[ADR 0005](../adr/0005-six-layer-crate-stack.md), [ADR 0006](../adr/0006-pure-isolated-domain-crates.md), [dependency-rules.md](../architecture/dependency-rules.md), [crate-catalogue.md](../architecture/crate-catalogue.md).
