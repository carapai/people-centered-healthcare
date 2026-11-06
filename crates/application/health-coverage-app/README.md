# health-coverage-app

Layer: **application** (coverage). Coverage for pricing and claims (#125).

- **`pricing_terms(request, coverages, plans, accumulators)`:** the patient's coverages in force on the service day, primary first, as `health-pricing` terms.
  - From `Accumulators::Cached` (a facility's snapshots) each split is an **estimate**.
  - From `Accumulators::Central` it is final, and a coverage with no use yet starts from zero.
  - A coverage without its Benefit Plan is `NoPlan`.
- **`settle_claim(charges, coverages, plans, accumulators, books, rules, applicability)`:** central's claim-time correction.
  - It walks the charges in service order.
  - Each estimated charge is priced again with central's terms, and each line whose split differs is a `Correction`.
  - Every charge's use (`PricedLine::deductibles` and payer shares) then runs the accumulators on, so the next charge sees it.
  - It returns the corrections, the charges priced again, and the accumulators after.
- **Not yet:**
  - storing the accumulators and Coverages at central, and syncing snapshots to facilities;
  - posting the corrections (Reconciliation Records and adjustment postings, billing);
  - claim assembly and submission (Claim, ClaimResponse, ExplanationOfBenefit);
  - payer adapters (`health-integration`);
  - the HTTP API.

Tests: `tests/coverage.rs`, covering an offline estimate corrected at claim time, central's own splits being final while running the accumulators on, and coverages in force needing their plans.
