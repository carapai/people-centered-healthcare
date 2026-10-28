# The one React app uses ports over a swappable transport, and Tauri IPC dispatches into the same Axum router in-process

The UI depends on two kinds of TypeScript ports:
- a generic **`FhirClient`**: read, vread, search, history and operations;
- **use-case command ports per Bounded Context** that mirror the server use cases. The UI never assembles the FHIR writes for a business operation.

**Transports.** Both kinds run over a **transport**:
- **`HttpTransport`** serves central and the LAN. The two differ only in base URL and auth, because edge and central share one Rust API (ADR 0010).
- **`TauriIpcTransport`** sends the same request/response envelope through Tauri `invoke`. The desktop runtime dispatches it into the **same Axum router** in-process, via tower `oneshot`.

**The §78 names** (`CentralHttpRepository`, `FacilityLanHttpRepository`, `TauriLocalRepository`) are combinations of the ports with one of these transports.

**Composition.** The runtime is chosen once, at the composition boundary. The Tauri build is a Vite mode; a browser build reads the host's `runtime.json`. That choice produces a `Platform` object holding the transport, session, event source and capabilities, passed through React and router context.

**Enforcement.** Components read capabilities and never check the runtime. A lint rule confines `@tauri-apps/*` imports to `src/platform/`.

**Types.** FHIR TypeScript types are generated from the same R5 definitions the Rust models use. Command DTOs come from a `utoipa` OpenAPI document as TypeScript types plus Zod schemas. Decimals are parsed losslessly.

We chose this for three reasons:
- One router means every runtime runs identical validation, authorization and use-case code.
- The desktop gets no loopback port, so there is no local network attack surface or port conflict.
- Command ports keep business rules out of the UI.

## Considered Options

- **A loopback Axum server inside Tauri, reached with the HTTP adapter.** Rejected: any local process could reach it, and it raises port-binding and firewall problems on facility PCs.
- **Hand-written Tauri commands per use case.** Rejected: a second API surface would drift from the HTTP one.
- **One monolithic `ClinicalRepository`.** Rejected: it would grow into a god-interface. Per-context ports follow the Bounded Contexts.

## Consequences

- Rust handlers must be transport-agnostic. Auth comes from the transport, so IPC carries the local session rather than DPoP.
- The scaffold spike, [Does the frontend scaffold build on Bun, Vite 8, Panda 2, React Compiler and TS 7?](https://github.com/carapai/people-centered-healthcare/issues/38), verifies the IPC round trip.
