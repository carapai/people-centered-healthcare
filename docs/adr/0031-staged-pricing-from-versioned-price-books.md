# Prices come from versioned Price Books through a fixed sequence of stages, evaluated wherever the charge is recorded

**Price Books.** They are owned by `pricing`, with central as authority. They are effective-dated and immutable once approved, by one finance approver. They are keyed by Healthcare Service or product, and are represented in FHIR as profiled ChargeItemDefinitions.

**Price Rules.** Surcharges, discounts, bundles, quantity tiers, subsidies and waivers are effective-dated rules with a priority. Their FHIRPath applicability is evaluated over a pricing context: the ChargeItem, encounter, service point, coverage and patient-eligibility categories.

**Base price.** It comes from the most specific Price Book that contains the item. The order, most specific first, is: payer contract, program, department, facility, organization, tenant.

**Adjustments** are applied in fixed stages:
1. base price;
2. surcharges;
3. discounts and bundles;
4. coverage split into payer and patient shares, using the Benefit Plan's copay, deductible, limits and exclusions;
5. subsidies and waivers, applied to the patient share only;
6. a manual override, which needs a permission and a reason and is audited.

**Conflicts.** Two rules in the same stage with the same priority that could both apply are rejected at validation.

**Charge capture** comes from configured triggers on clinical events, with deterministic ids. Clinical code contains no payment logic.

**Where pricing runs:** on the node where the charge is recorded, using the replicated Price Books and cached coverage.
- **Recording.** Each ChargeItem stores its price components, the Price Book version and the rule versions applied.
- **Check at authority.** The Authority Node confirms those versions were active at the service time. A mismatch becomes a Reconciliation Record.
- **Accumulators.** Deductible and limit counters live at central. A facility using its cached snapshot offline marks the split as an estimate.

We chose this because §64 demands explicit precedence across nine scopes, and §66 demands that historical pricing decisions are preserved. ChargeItemDefinition alone can express neither payer-contract precedence nor coverage splits, and its pricing model is "under development".

## Considered Options

- **"Most specific rule wins" everywhere, with no stages.** Rejected: discounts, coverage and waivers then interact unpredictably, for example a waiver reducing the payer's share.
- **Price Books in the metadata package pipeline** (two approvers, packages). Rejected: prices change often and are a finance matter, not a clinical-definition one.
- **Re-pricing at central.** Rejected: the receipt the patient was given must match the ledger.

## Consequences

- Prices cannot be changed by a facility while it is offline.
- Benefit Plans are effective-dated records owned by `coverage`.
