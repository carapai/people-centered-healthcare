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
  - A charge already taken in (each accumulator remembers its charges) is skipped, so a replayed settlement changes nothing.
- **`Coverages`**, over a store whose transaction is a `CoverageStore`:
  - **`settle(claim)`:** one transaction. It reads central's accumulators for the claim's coverages and years, settles, and writes back what changed. Each accumulator guards its coverage and year, so concurrent settlements conflict and are rerun (up to 8 times, then `Busy`).
  - **`snapshot(coverage, year, now)`:** what a facility caches to price estimates.
  - **`check_eligibility(payer, coverage, day, now)`:** asks the payer and keeps the answer. Offline, the kept answer stands, pending verification.
- **`remittance_posting(remittance, coverage, facility, currency)`** (#126): payments in transit debited, `receivable/{coverage}` (which billing debits with each payer share) credited.
- **`shortfall_posting(settled, facility, currency, at)`:** **the patient pays what the payer did not** (decided 2026-11-06). Each shortfall moves from `receivable/{coverage}` to `receivable/patient`, one line pair per charge. It gives `None` when the claim was paid in full.
- **Not yet:**
  - syncing snapshots to facilities, and reading Coverages and Benefit Plans from FHIR Coverage and InsurancePlan, which needs the pinned local profile (ADR 0030);
  - posting the corrections (Reconciliation Records and adjustment postings, billing);
  - assembling claims from billing's stored charges, keeping claims, and ExplanationOfBenefit;
  - payer adapters (`health-integration`);
  - the HTTP API.

Tests: `tests/remittance.rs` checks, through the ledger, that a remittance clears the receivable once and that the patient then owes the shortfall. `tests/central.rs` over SurrealDB covers a settlement kept and never taken in twice, concurrent settlements of one coverage all counting, and eligibility kept and pending offline. `tests/coverage.rs` covers an offline estimate corrected at claim time, central's own splits being final while running the accumulators on, and coverages in force needing their plans.
