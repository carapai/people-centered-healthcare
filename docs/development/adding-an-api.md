# Adding an API endpoint

> Status: planned layout. No code exists yet; paths are confirmed when the scaffold lands.

## When to use

A new command endpoint (a business operation) or a FHIR operation. Plain FHIR CRUD/search for a supported type is covered by [adding-a-fhir-resource.md](adding-a-fhir-resource.md).

## Steps

1. **Use case first.** The endpoint calls one application use case in `health-<context>-app`. Handlers hold no business logic.
2. **Router.** Add the route to `health-http` (command endpoints, Axum/Tower) or `health-fhir-server` (FHIR REST and `$operations`). The router is shared: central, the edge and the desktop (through Tauri IPC and tower `oneshot`) all run it.
3. **Transport-agnostic handler.** Take the authenticated principal from the request extensions the transport populates (central OIDC, edge DPoP, desktop local session). Never read transport-specific headers in the handler.
4. **DTOs.** Define request/response DTOs and derive their OpenAPI schema with `utoipa`. Money and FHIR decimals stay lossless (`rust_decimal`, serialized as strings).
5. **Errors.** Return an OperationOutcome for FHIR routes; validation failures reject with an OperationOutcome.
6. **Idempotency.** Writes carry an `op_id` / `Idempotency-Key`; replays return the stored result ([identifiers.md](../architecture/identifiers.md)). Updates require `If-Match`.
7. **Authorization.** Check permissions from the permission snapshot at action time ([offline-auth.md](../architecture/offline-auth.md)).
8. **Regenerate** the OpenAPI document, then the TypeScript types and Zod schemas, and commit them.
9. **Client.** Add the method to the context's command port in the client; repositories call it through `Transport`.
10. If the endpoint is served on the edge, also read [adding-an-edge-api.md](adding-an-edge-api.md).

## Expected files

- `crates/infrastructure/health-http/src/…` or `crates/infrastructure/health-fhir-server/src/…`
- `crates/application/health-<context>-app/src/…`
- Committed OpenAPI document and generated TS/Zod types (paths to be fixed by the scaffold)
- `apps/client/src/…` command port (path to be fixed by the scaffold)

## Contracts

- The request envelope: method, path, query, header subset, body, identical over HTTP and IPC.
- The `utoipa` OpenAPI document and its generated TS types and Zod schemas (responses validated with Zod at the transport).
- Application use-case signatures.

## Tests

- **API contract:** OpenAPI and generated TS types match (CI regenerates and diffs); HTTP and IPC transports return identical envelopes.
- **Security:** authorization-matrix cases for the endpoint.
- **Unit and domain** for the rule behind it.

## Governing decisions

[ADR 0010](../adr/0010-edge-is-always-the-local-authorization-server.md), [ADR 0036](../adr/0036-ports-over-swappable-transport-with-tauri-ipc-into-the-shared-router.md), [client-application.md](../architecture/client-application.md), [deployment.md](../architecture/deployment.md).
