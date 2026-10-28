# Adding an edge API

> Status: planned layout. No code exists yet; `health-edge-runtime` and its modules are created by the scaffold.

## When to use

An endpoint that LAN clients reach on the Facility Edge Node. The edge and central run **one Rust API** (ADR 0010, ADR 0036), so a business endpoint is added once, in the shared router, and is available on the edge whenever the edge hosts that Bounded Context. Only node-local concerns (enrollment, discovery, admin status, health probes, sync for desktops) are edge-specific.

## Steps

1. **Is the context hosted on the edge?** The edge hosts Clinical Records, Clinical Operations, Care Coordination, Programs & Public Health, Finance, Supply Chain, Workflow, Patient Engagement and Identity & Access, plus read-only Registries and Terminology & Metadata. Analytics and external integrations stay central ([resource-ownership.md § Deployables](../architecture/resource-ownership.md#deployables)).
2. **Business endpoint:** follow [adding-an-api.md](adding-an-api.md). The handler must behave correctly when the edge is offline from central (no synchronous call to central, except calls that already have an offline path, e.g. `$match` with a 3 s timeout and local fallback).
3. **Authority:** the edge accepts writes only for records whose Authority Node it is; anything global is a read-only replica or a request to central ([write-authority.md](../architecture/write-authority.md)).
4. **Auth:** every route except the app shell, the enrollment flow and `/health/live` requires a device-bound DPoP token issued by the edge ([lan-security.md](../architecture/lan-security.md), [offline-auth.md](../architecture/offline-auth.md)).
5. **Edge-only endpoint:** put it in the matching module of `health-edge-runtime` (`-api`, `-admin`, `-auth`, `-discovery`, `-sync`). Runtime crates hold no business logic; call application use cases.
6. **Health and readiness:** `/health/ready` must stay false until migrations finish and while draining.
7. **Version range:** LAN clients run the edge's own app build; central supports edge N and N-1 on sync.

## Expected files

- Shared routes: `crates/infrastructure/health-http/…` or `health-fhir-server/…`
- Edge-only routes: `crates/edge/health-edge-runtime/src/<module>/…`
- Wiring: `apps/server/facility-edge/…`

## Contracts

The shared router and OpenAPI document; DPoP token claims (`sub`, facility, `snapshot_version`, `amr`, `acr`); the permission snapshot; `/health/live` and `/health/ready`.

## Tests

- **API contract** (same as central).
- **Security:** authorization matrix, facility isolation, unenrolled-device refusal.
- **Offline and sync:** the endpoint works with central partitioned.
- **End-to-end:** LAN browser scenarios (Playwright, nightly).

## Governing decisions

[ADR 0007](../adr/0007-embedded-single-process-edge-with-cold-spare.md), [ADR 0009](../adr/0009-device-bound-enrollment-on-the-lan.md), [ADR 0010](../adr/0010-edge-is-always-the-local-authorization-server.md), [ADR 0012](../adr/0012-authority-node-follows-record-scope.md), [facility-edge.md](../architecture/facility-edge.md).
