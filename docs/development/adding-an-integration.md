# Adding an external integration

> Status: planned layout. No code exists yet; paths are confirmed when the scaffold lands.

## When to use

A connection to an external system: UgandaEMR/OpenMRS, DHIS2, a laboratory system, an insurer, a mobile-money or card gateway, NIRA, or a supplier. Payers have their own guide ([adding-a-payer.md](adding-a-payer.md)).

## Steps

1. **Run it at central.** Adapters live in `health-integration` (infrastructure) and run in `central-server` only; the edge never calls external systems ([resource-ownership.md § Deployables](../architecture/resource-ownership.md#deployables)). An edge asks central and the outcome syncs back.
2. **Port in the domain, adapter in infrastructure.** The owning Module declares the port; `health-integration` implements it. Genuine plug-in points (payers, payment gateways) use `Arc<dyn …>` with a boxed-future variant ([dependency-rules.md](../architecture/dependency-rules.md)).
3. **R4 at the boundary only.** Convert with `health-fhir-mapping`; every converted resource gets a Provenance to its source. R4 never enters storage.
4. **Validate inbound data:** tier 1 strict (reject on failure); tiers 2–4 failures go to the **quarantine** review queue ([fhir-r5-model.md](../architecture/fhir-r5-model.md)).
5. **Idempotency:** persist the external reference before calling out (e.g. MTN `X-Reference-Id` in the same transaction as the pending entry); recover by querying that reference, never by re-initiating ([billing-and-payments.md](../architecture/billing-and-payments.md)).
6. **Unreliable callbacks:** expect dropped, duplicate and late callbacks; poll and reconcile against statements.
7. **Telemetry:** no names, phone numbers, NINs, free text or sensitive codes in spans or logs (ADR 0047).
8. **Licences and secrets:** dependencies must pass `cargo-deny`; credentials come from central's secret store, never from the repository.
9. **Country specifics** (endpoints, code mappings such as CIEL → national ConceptMaps) live in configuration packages, not in the adapter code.

## Expected files

- `crates/infrastructure/health-integration/src/<system>/…`
- Port trait in `crates/domains/health-<module>/src/…`
- `crates/contracts/health-fhir-mapping/…` (if R4 conversion is new)
- Mock server or fixture data for tests (location to be fixed by the scaffold)

## Contracts

The domain port; external payload mappings; Provenance for converted resources; the quarantine review record.

## Tests

- Integration tests against a mock of the external system (dropped and duplicate responses).
- **Payment reconciliation** suite (nightly) for payment gateways: mock MoMo and Airtel with dropped callbacks, duplicates and statement mismatches.
- **FHIR conformance** for converted resources.
- **Security:** telemetry redaction test; ZAP baseline on staging.

## Governing decisions

[ADR 0001](../adr/0001-modular-monolith-first.md), [ADR 0015](../adr/0015-adopted-r5-model-crate-behind-a-facade.md), [ADR 0016](../adr/0016-validate-on-the-writing-node-and-again-at-authority.md), [ADR 0032](../adr/0032-provider-confirmed-two-phase-payments-via-central.md), [ADR 0047](../adr/0047-no-health-data-in-telemetry-and-in-country-observability.md).
