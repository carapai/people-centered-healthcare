# architecture-checks

Layer: **tooling**. Enforces the crate-layer rules in [dependency-rules.md](../../docs/architecture/dependency-rules.md) over `cargo metadata --no-deps` and each member's raw `Cargo.toml`. It runs offline and needs no lockfile.

```sh
cargo run -p architecture-checks                      # check this workspace
cargo run -p architecture-checks -- --manifest-path other/Cargo.toml --allowlist allowlist.toml
```

It exits with 0 when the workspace is clean, 1 when it finds violations, and 2 on an error. Each violation is printed as `[rule] crate: detail`.

| Rule | Fails when |
|---|---|
| `missing-layer-tag` | `[package.metadata.health] layer` is missing or unknown |
| `missing-context` | A domain or application crate has no `context` |
| `upward-edge` | A normal or build dependency points to the same or a higher layer. Allowed exceptions: per-context contract → shared contract, and infrastructure → infrastructure |
| `domain-to-domain` / `application-to-application` | A sideways edge in those layers |
| `runtime-is-leaf` | Something depends on a runtime crate, other than an `apps/` composition root |
| `tooling-is-leaf` | Something depends on a tooling crate |
| `testing-as-normal-dependency` | A testing crate is used outside `[dev-dependencies]` |
| `domain-external` | A domain crate uses an external crate outside `allowlist.toml` |
| `features-in-domain-or-application` | A domain or application crate has `[features]` |
| `inline-version` | A dependency is not `{ workspace = true }`, including path dependencies and `target.*` tables |
| `missing-workspace-lints` | `[lints] workspace = true` is missing and the crate is not lint-exempt |
| `cycle` | The normal and build dependency graph has a cycle |

`allowlist.toml` holds the reviewed policy: domain externals, the explicitly forbidden list, and lint exemptions. Changing it is reviewed like an ADR.

The tests in `tests/rules.rs` generate a valid fixture workspace, break one rule at a time, and also run the checks on this repository.
