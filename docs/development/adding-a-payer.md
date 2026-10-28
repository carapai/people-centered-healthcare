# Adding a payer

> Status: planned layout. No code exists yet; paths are confirmed when the scaffold lands.

## When to use

A new insurer, Health Membership Organisation, or government/donor funding source. Payer records are owned by `coverage` at central. A payer whose eligibility, preauthorization or claims are exchanged electronically also needs an **adapter**, which runs only at central.

## Steps

1. **Records** (central, `coverage`): the payer as an Organization, the payer Contract, its Benefit Plans as InsurancePlan (copay, deductible, limits, exclusions).
2. **Prices**: a payer-contract Price Book if the payer has its own tariff ([adding-a-price-rule.md](adding-a-price-rule.md)).
3. **Ledger**: a receivable account per funding source from the country chart-of-accounts template ([billing-and-payments.md](../architecture/billing-and-payments.md)).
4. **Adapter** (only for electronic exchange): implement the payer port in `health-integration`. It uses FHIR Claim where the payer supports it, otherwise an export format or portal file. Payer adapters are a genuine plug-in point, so they are used as `Arc<dyn …>` with the small dyn-compatible boxed-future variant ([dependency-rules.md](../architecture/dependency-rules.md)).
5. **R4 payers**: convert at the boundary with `health-fhir-mapping`, attaching Provenance to the source.
6. **Flows**: eligibility (cached with age on the edge; offline shows "pending verification"), preauthorization (`Claim.use = preauthorization`), claims per Account and payer, ClaimResponse/remittance → ExplanationOfBenefit and PaymentReconciliation → ledger postings.
7. **Government and donor funders** are funding sources with their own payer records; their claims are reports or exports, not adjudication.
8. **Secrets** for the payer API stay at central; nothing payer-specific reaches the edge.

## Expected files

- `crates/infrastructure/health-integration/src/<payer>/…` (adapter)
- `crates/domains/health-coverage/src/…` (port trait, if it does not exist yet; see the note below)
- Payer, Contract and InsurancePlan records (authored at central)

Note: [pricing-and-insurance.md](../architecture/pricing-and-insurance.md) places the payer port "in `health-integration`", while [dependency-rules.md](../architecture/dependency-rules.md) puts driven ports in the Module's domain crate. Where the trait lives is to be fixed by the scaffold.

## Contracts

The payer port; CoverageEligibilityRequest/Response, Claim, ClaimResponse, ExplanationOfBenefit, PaymentReconciliation with locally pinned financial profiles; accumulator snapshots (central).

## Tests

- **Unit and domain:** the stage-4 coverage split, accumulators, secondary coverage.
- Integration tests against a mock of the payer (no dedicated payer suite is listed in [testing.md](../architecture/testing.md); its shape is to be fixed by the scaffold).
- **Security:** telemetry redaction (no member names or numbers in traces).
- **End-to-end:** the finance acceptance scenario (§119).

## Governing decisions

[ADR 0030](../adr/0030-double-entry-ledger-is-the-financial-source-of-truth.md), [ADR 0031](../adr/0031-staged-pricing-from-versioned-price-books.md), [ADR 0047](../adr/0047-no-health-data-in-telemetry-and-in-country-observability.md), [pricing-and-insurance.md](../architecture/pricing-and-insurance.md), [adding-an-integration.md](adding-an-integration.md).
