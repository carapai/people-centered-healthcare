# health-pricing

Layer: **domain** (pricing). Price Books, Price Rules and the six fixed pricing stages (pricing-and-insurance.md; ADR 0031; #120).

- **`PriceBook`:** effective-dated, scoped (`Scope`: payer contract, program, department, facility, organization, tenant) and immutable once `Approved` by one finance approver. Each `Entry` prices a Healthcare Service or product, with optional quantity `Tier`s.
- **`PriceRule`:** effective-dated, with a `Stage` (surcharge, discount, subsidy), a priority (lower runs first), the items it may apply to, an optional FHIRPath applicability, and an `Adjustment` (percent, amount, or a stage-3 bundle).
- **`validate(rules)`** refuses:
  - two rules of the same stage and priority whose periods and items overlap (applicability cannot be decided ahead of time);
  - a bundle outside stage 3 or of fewer than two items;
  - a negative adjustment, or a discount or subsidy over 100%.
- **`price(request, books, rules, applicability)`** runs the stages over a charge's lines. Only approved versions in effect at the service time, in the charge's scopes and currency, count:
  1. **base price:** from the most specific book that holds the item, at its tier price. Two equally specific books refuse the charge (`Ambiguous`);
  2. **surcharges;**
  3. **bundles, then discounts:** a bundle whose items are all on the charge zeroes them, each referencing a new bundle line at the bundle's price. A discount takes off at most the line's price;
  4. **the coverage split:** coverages in order, each paying what remains after its deductible and copay, up to its limit, unless it excludes the item. Deductibles and limits run down across the lines. Cached accumulators make the split an `estimate`;
  5. **subsidies and waivers,** on the patient share only;
  6. **manual overrides** of the patient share, with who and why. The caller checks the permission and audits.
- **Recorded versions:** every `Component` names the book or rule version it came from, and `Priced` lists every version used.
- **`check(priced, service_time, books, rules)`** is the authority's check that each version was approved and in effect then. A `Mismatch` becomes a Reconciliation Record, never a silent re-price.
- **Applicability** is evaluated through the `Applicability` port, over the pricing context (ChargeItem, encounter, program, coverages, eligibility categories) that the application builds.
- **Lifecycle** (`approve_book`, `approve_rule`, `retire_book`, `retire_rule`): approved once by a finance approver who is not the `author`, never overlapping another approved version of itself, immutable afterwards except for retirement from a time not yet passed. `PricingStore` keeps every version.
- **Rounding:** amounts round to the currency's minor units, half away from zero.

- **`PricedLine::deductibles`** (#125): what each coverage's deductible took of the line, so central's accumulators can follow it.

## Tests

`src/tests.rs`: a charge through all six stages (book precedence, tiers, a surcharge, a bundle, a discount, two coverages with a deductible, copay, limits and an exclusion, a subsidy and an override), then the book rules, waivers, overrides, applicability failures, rule conflicts, the authority's check, and JSON round trips.
