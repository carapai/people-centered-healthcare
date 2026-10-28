# Adding a price rule

> Status: planned layout. No code exists yet; paths are confirmed when the scaffold lands.

## When to use

A new price, surcharge, discount, bundle, subsidy or waiver. Price Books and Price Rules are **records owned by `pricing`** with central as their authority. They are data, not code: a new rule needs code only if it needs a pricing-context input that does not exist yet.

## Steps

1. **Decide the stage** (fixed order, ADR 0031): 1 base price (from the most specific Price Book: payer contract → program → department → facility → organization → tenant; quantity tiers here), 2 surcharges, 3 discounts and bundles, 4 coverage split, 5 subsidies and waivers (patient share only), 6 manual override.
2. **Write the rule** as a profiled ChargeItemDefinition `propertyGroup` with an `applicability` FHIRPath over the pricing context, and the **priority extension**. Base prices are profiled ChargeItemDefinition entries in a versioned Price Book.
3. **Pricing context** available to the expression: the ChargeItem (code, quantity, service time), the Encounter (class, service point, department, triage category, after-hours), the program, the ordered Coverages and Benefit Plans, and patient-eligibility categories from configuration. Never hard-code categories.
4. **No ambiguous rules:** two rules with the same priority that could both apply are rejected at validation.
5. **Effective dates:** rules and books are effective-dated and immutable once approved.
6. **Approval:** one finance approver at central. Facility finance staff edit their facility's books at central only; prices cannot change offline.
7. **Distribution:** approved books and rules sync down in the metadata lane as global replicas.
8. **New context input (code)**: add it to the pricing-context builder in the Finance application crate, and to the pure stage logic in `health-pricing`.

## Expected files

- Price Book and Price Rule records (authored at central; any repository-held seed data location is to be fixed by the scaffold)
- Pricing fixture cases (location to be fixed by the scaffold)
- For new inputs: `crates/domains/health-pricing/src/…`, `crates/application/health-finance-app/src/…`

## Contracts

- The locally pinned ChargeItemDefinition profiles and the priority extension ([docs/fhir/README.md](../fhir/README.md)).
- What each ChargeItem records: price components (`base`, `surcharge`, `discount`, `deduction`, `tax`, `informational`), Price Book version, every applied rule id and version, the coverage split.
- `rust_decimal` money with ISO 4217 scale and round-half-even.

## Tests

- **Unit and domain:** the pricing stages, precedence, bundle and tier behaviour, same-priority rejection.
- Pricing fixtures in `integration-tests.yml`.
- **End-to-end:** the finance acceptance scenario (§119).
- At authority, a recorded version not active at service time becomes a Reconciliation Record; cover it in **Offline and sync**.

## Governing decisions

[ADR 0030](../adr/0030-double-entry-ledger-is-the-financial-source-of-truth.md), [ADR 0031](../adr/0031-staged-pricing-from-versioned-price-books.md), [pricing-and-insurance.md](../architecture/pricing-and-insurance.md), [billing-and-payments.md](../architecture/billing-and-payments.md).
