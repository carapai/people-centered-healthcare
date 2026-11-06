# health-coverage

Layer: **domain** (coverage). Coverage (pricing-and-insurance.md § Records, § Accumulators, § Eligibility; ADR 0031; #125).

- **`BenefitPlan`** (InsurancePlan): the copay (none, a percentage or an amount), the yearly deductible and limit, and exclusions.
- **`Coverage`:** a beneficiary's coverage by a payer under a plan, with its `order` (primary first), status and period. `in_force(coverages, day)` gives those that apply, in order.
- **`Accumulator`:** what a coverage's deductible and limit have used in a benefit year. It is kept at central, across facilities. `remaining(plan)` never goes below zero, and `consume(deductible, paid)` takes in one priced line.
- **`terms(coverage, plan, basis)`:** the terms for pricing's stage 4.
  - `Basis::Authoritative` (central's own accumulator) gives final terms.
  - `Basis::Cached` (a facility's `Snapshot`) and `Basis::Unknown` give **estimates**.
- **Eligibility:**
  - `check(payer, coverage, day, cached, now)` asks the payer through the **`Payer` port**. There is one adapter per payer in `health-integration`.
  - If the payer cannot be reached, the cached answer stands, marked `PendingVerification`; `Eligibility::age` is its age.
- **Claims** (`claims`, #126). No national payer or claim format is assumed.
  - **`assemble(ClaimFor, charges, at)`:** a claim (or preauthorization) per coverage, of the charges it pays part of, in service order. Each item claims the coverage's share.
  - **`Claim::submitted`** records the payer's reference.
  - **`Claim::adjudicate(Adjudication)`** settles the claim: approved, partial or denied. It returns what was approved and each item's `Shortfall` with the payer's reason. An adjudication approving more than was claimed is refused.
  - **`Claim::appeal`** is a new draft of what was not paid, naming the original.
  - **`Remittance`** is what the payer paid.
  - **`ClaimPayer`** is the port the adapters in `health-integration` implement.
- **`CoverageStore`:** the port for central's accumulators (keyed by coverage and year) and each coverage's latest eligibility. An accumulator remembers the charges it took in.
- **`corrections(estimated, actual)`:** the difference in each line's split, by coverage. Each is a Reconciliation Record and a patient-share adjustment.

Tests: claim assembly, adjudication in full, in part and denied, appeals, coverages in force and in order, central's terms final and a facility's estimated, offline eligibility, and corrections.
