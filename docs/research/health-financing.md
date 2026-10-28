# Research: health financing

A condensed version of the research ticket [What finance, insurance and payment facts constrain the billing design?](https://github.com/carapai/people-centered-healthcare/issues/8). The full findings, with citations, are on the unmerged branch `research/health-financing`. Decisions: [pricing-and-insurance.md](../architecture/pricing-and-insurance.md) and [billing-and-payments.md](../architecture/billing-and-payments.md) (ADRs 0030–0033).

## FHIR R5 financial module

| Resource | Maturity (FMM) | Key fact |
|---|---|---|
| Invoice | 0 | "The hospital's view"; does not track payments |
| ChargeItem | 1 | "Not an actual financial transaction"; status `planned … billed`; price override with a reason |
| ChargeItemDefinition | 1 | Canonical, versioned, `effectivePeriod`; `propertyGroup.applicability` FHIRPath; pricing model "currently under development" |
| Account | 2 | A bucket. "Does not include information about the charges, payments or adjustments"; `balance[]` is calculated by the billing system |
| Claim, ClaimResponse, ExplanationOfBenefit | 2 | |
| Coverage, CoverageEligibility\*, PaymentNotice, PaymentReconciliation | 4 | PaymentReconciliation has point-of-sale fields in R5. Refunds and reversals are not described |

- **Price component types:** `base | surcharge | deduction | discount | tax | informational`.
- **Precision.** FHIR `decimal` must preserve precision; JSON numbers lose it in JavaScript.

## Uganda

- **National insurance:** the NHIS Bill passed on 31 March 2021 but was not assented to, and it is not law as of October 2026. A 2026–2036 Health Financing Strategy is in draft.
- **Private insurance:** private insurers and Health Membership Organisations are licensed by the IRA under the Insurance Act 2017. No national claims-exchange standard was found (unverified).
- **User fees** were abolished in public units in 2001, except in private wings. PNFP and private facilities charge fees. Donor funding is significant.
- **Payment licensing:** the National Payment Systems Act 2020 requires a Bank of Uganda licence to offer payment services (including aggregators).

## Mobile money

- **MTN MoMo:**
  - async `202 Accepted`;
  - a single callback with no retries, so integrators must poll;
  - a client UUIDv4 `X-Reference-Id`, where reuse returns 409;
  - an HTTPS callback on a registered domain.
- **Airtel Money:** similar async collection with `TS`/`TF` callbacks and a status enquiry. Details are unverified.

## Ledger patterns

- **Balances and corrections (Fowler):** balance is the sum of entries, and corrections are made by reversal or adjustment, never deletion.
- **Two-phase transfers (TigerBeetle):** pending → posted, voided or expired, resolved once, by a new record.

## Libraries

- **`rust_decimal` 1.43:** a 96-bit mantissa, preserves scale, has rounding strategies and serde as a string.
- **`decimal.js`:** immutable, with `ROUND_HALF_EVEN` available.
- **UGX** has an ISO 4217 minor unit of 0.

## Open items carried forward

- Payer claim formats (FHIR or bespoke); the Airtel API details; direct integration versus an aggregator.
- Fund-accounting reporting formats; the FX-rate source.
- EFRIS obligations for private facilities (not researched).
