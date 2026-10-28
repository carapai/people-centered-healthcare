# Modular monolith first: central-server and facility-edge

We ship two server Deployables first, `central-server` (every Bounded Context as a module, gateway built in) and `facility-edge` (the operational contexts plus read-only Registries and Terminology & Metadata), both compiled from the same domain crates, plus the Tauri desktop host. The per-context directories listed in `MASTER_PROMPT.md` §13 (`apps/server/clinical-service`, `business-service`, …) are created only when a context is actually extracted. This follows §14 ("begin with cohesive deployable applications… do not turn every domain crate into a separate microservice") and keeps the edge and central runtimes on one code path; it defers, rather than rejects, the §13 service layout.

## Consequences

- Module boundaries must be enforced by crate dependency rules and architecture checks, not by process boundaries.
- Extraction later requires replacing in-process calls between contexts with typed HTTP/FHIR APIs and events; contracts are designed for that from the start.
