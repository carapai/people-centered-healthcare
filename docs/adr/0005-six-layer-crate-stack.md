# Six-layer crate stack with a separate application layer

Every Rust crate belongs to exactly one layer, and it may depend only on layers below it. The layers, from bottom to top, are foundation, contracts, domains, application, infrastructure and runtime. Testing crates sit beside the stack and are used only as dev-dependencies. We added `crates/application/` (one crate per Bounded Context), which is not in the `MASTER_PROMPT.md` §13 layout. We also moved `health-persistence` from infrastructure (§17) into contracts as pure traits. ADR 0004 lets one use case write through several Owners' repositories in a single transaction, and that only works if use cases and the unit-of-work abstraction sit above the domain crates without depending on SurrealDB.

## Considered Options

- Put use cases inside each domain crate. Rejected because a use case that spans Modules would force domain→domain dependencies.
- Keep `health-persistence` in infrastructure. Rejected because application crates would then depend on an infrastructure crate, which inverts the stack.
- Make "presentation" its own layer. Rejected because Axum routers and Tauri commands are adapters; they live in infrastructure (`health-http`, `health-fhir-server`) or in runtime crates.

## Consequences

- Application crates may depend on any domain crate, but never on another application crate. A use case that spans contexts is composed in a runtime crate or through events.
- Runtime crates (`crates/edge/*`, `crates/desktop/*`, `apps/server/*`) only wire layers together and add runtime-specific adapters. They contain no business logic, and nothing depends on them.
- The layer of every crate is declared in `[package.metadata.health]` and checked in CI by `tools/architecture-checks`.
