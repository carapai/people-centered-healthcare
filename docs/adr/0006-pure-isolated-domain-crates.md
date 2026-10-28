# Domain crates are pure, per-Module and never depend on each other

Each Module from the resource-ownership matrix gets its own domain crate, created lazily when the first task needs it. This way the compiler enforces that only the Owner changes its records. A domain crate depends only on foundation crates, contract crates and an allowlist of external crates. It never depends on another domain crate, even one inside the same Bounded Context, and it has no Cargo features. Domain functions are pure: they return the new state and the domain events they produced, and the application layer persists those events. We chose this so that the same domain logic runs unchanged on central, the Facility Edge and the desktop, can be unit-tested without a runtime, and can later be extracted into another Deployable by swapping adapters rather than rewriting logic.

## Considered Options

- One crate per Bounded Context (13 crates), with Modules as Rust modules. Builds are faster, but ownership between Modules would rest on convention alone.
- Allow domain→domain dependencies within a context, such as orders → encounters. Rejected because it creates hidden coupling that blocks extraction, and the same need is met by a port the caller declares.
- Feature flags or `cfg` for an "edge mode". Rejected because runtime differences (write authority, id minting, clock) are injected through ports and configuration.

## Consequences

- When a domain needs another Module's data, it declares a port in its own crate. The application layer implements that port by calling the other Module, which makes the adapter an anti-corruption seam.
- Events, typed Workflow actions and context ids shared across contexts live in one contract crate per Bounded Context (`health-<context>-contract`).
- `tokio`, `axum`, `tower`, `surrealdb`, `reqwest`, `tauri`, `tracing-subscriber` and `opentelemetry` cannot appear in a domain crate's dependency list.
