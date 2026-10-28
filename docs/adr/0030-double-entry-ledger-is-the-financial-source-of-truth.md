# An internal double-entry ledger is the financial source of truth; FHIR financial resources are projections and exchange formats

**The ledger.** Each facility has its own `financial-ledger`, written only by its Facility Authority Node (or a lease holder for its sub-scope).
- It is append-only double-entry: every posting balances, and corrections are reversals.
- Balances are always derived from postings.
- Country configuration supplies the chart-of-accounts templates.
- **Funding sources.** Every charge is allocated to one or more Funding Sources: the patient, an insurer, a government program, a donor or grant, or a waiver.
- **Free care** is priced at a reference tariff and allocated to its funder, so consumption is always tracked (§70).
- **Restricted funds** are a `fund` dimension on postings.

**FHIR resources** describe the money; they don't hold it:
- ChargeItem records consumption.
- Account is the encounter or episode bucket. Its `balance[]` is projected from the ledger.
- Invoice is a document view.
- Coverage, CoverageEligibility\*, Claim, ClaimResponse and ExplanationOfBenefit are used for payer exchange.
- PaymentReconciliation and PaymentNotice describe remittances and receipts.

All of these profiles are pinned locally.

**Money** uses `rust_decimal` with the ISO 4217 scale per currency and round-half-even. Splits use largest-remainder allocation, with any residue posted to a rounding account. Each ledger has one functional currency, and foreign amounts keep their original value and rate.

We chose this because FHIR's billing resources are immature (Invoice is at maturity level 0, ChargeItem at 1). FHIR also explicitly leaves balances to "the billing system", and none of its resources gives accounting integrity.

## Considered Options

- **FHIR Account, ChargeItem and Invoice as the system of record.** Rejected: balances, reversals and audit-grade integrity are outside FHIR's scope, and the resources are likely to change in R6.
- **A single-entry running balance per account.** Rejected: errors cannot be detected, and history is lost on correction.
- **An external accounting package as the ledger.** Rejected for v1: it needs online, per-facility integration, and offline facilities would have no ledger. An export adapter to accounting systems can be added later.

## Consequences

- The TypeScript client parses money with a lossless JSON parser. Internal APIs carry decimal strings.
- Ledger operations sync as `kind: op` items. Invariant breaches, such as an unbalanced till or a mismatched price version, become Reconciliation Records (ADR 0020).
