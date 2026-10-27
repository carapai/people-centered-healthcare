# Research: finance, insurance and payment facts that constrain the billing design

Ticket: #8 (Wayfinder research). Researched 2026-10-27. Spec context: `MASTER_PROMPT.md` Part 10 (sections 63-70) and section 24 (Group G crates).

## Summary

- **The FHIR R5 billing-side resources are immature.** Invoice is FMM 0. ChargeItem and ChargeItemDefinition are FMM 1. Account, Claim, ClaimResponse and ExplanationOfBenefit are FMM 2. Only Coverage, CoverageEligibility\*, PaymentNotice and PaymentReconciliation reach FMM 4, and none are Normative. The spec itself says ChargeItemDefinition's pricing model is "currently under development", and that Account's balance calculation stays inside the billing system. FHIR describes the inputs and outputs of billing. It is not an accounting system.
- **Uganda has no operating national health insurance.** The NHIS Bill passed Parliament on 31 March 2021 but was not assented to. As of October 2026 it has not been re-enacted. Private insurers and Health Membership Organisations (HMOs) are licensed by the Insurance Regulatory Authority (IRA) under the Insurance Act 2017. Public facilities have charged no user fees since 2001, except in private wings. PNFP and private facilities charge fees. Donor funding is significant, and a new 2026-2036 Health Financing Strategy aims to reduce dependence on it.
- **Mobile money is asynchronous and only eventually confirmed.** MTN MoMo returns `202 Accepted`, sends one callback with no retries, and tells integrators to poll the status endpoint. `X-Reference-Id` is a client-generated UUID v4, and reusing one returns `409`. Airtel Money follows the same async pattern (collect, then callback with `TS`/`TF` status codes, plus a status enquiry), but its official docs could not be read (they are JS-rendered), so the details are unverified.
- **Money needs exact decimals with currency-aware scale.** UGX has ISO 4217 minor unit 0. FHIR `decimal` requires precision to be preserved. `rust_decimal` (96-bit mantissa, about 28 digits, Postgres `NUMERIC` support) and `decimal.js` (immutable, `ROUND_HALF_EVEN` available) fit.
- **Conclusion:** use FHIR financial resources as the interoperable boundary and projection, and an internal append-only double-entry ledger as the financial source of truth. Offline payments are recorded as *pending* ledger postings that are confirmed only by provider evidence.

## 1. FHIR R5 financial module

### 1.1 Maturity (FMM) of the relevant resources

Source: https://hl7.org/fhir/R5/resourcelist.html (all are Trial Use. R5 is an STU release for these resources.)

| Resource | FMM | Note |
|---|---|---|
| Invoice | **0** | https://hl7.org/fhir/R5/invoice.html |
| ChargeItem | 1 | https://hl7.org/fhir/R5/chargeitem.html |
| ChargeItemDefinition | 1 | https://hl7.org/fhir/R5/chargeitemdefinition.html |
| Account | 2 | https://hl7.org/fhir/R5/account.html |
| Claim / ClaimResponse / ExplanationOfBenefit | 2 | |
| Coverage, CoverageEligibilityRequest/Response | 4 | |
| PaymentNotice, PaymentReconciliation | 4 | https://hl7.org/fhir/R5/paymentreconciliation.html |

The module's scope is "the costing, contracting, financial transactions and billing". It names a FIAB (Financial Accounts and Billing) sub-domain and lists future work such as a "Statement" resource (https://hl7.org/fhir/R5/financial-module.html).

### 1.2 Account

Source: https://hl7.org/fhir/R5/account.html (FMM 2)
- Account is "a financial tool for tracking value accrued for a particular purpose". It works as a bucket that ChargeItems point at.
- "The Account itself does not include information about the charges, payments or adjustments". Those live in linked resources.
- Account "does conceptually have a balance", but expressing it directly is "challenging". R5 adds `balance[]` (`aggregate`, `term` aging bucket, `estimate`, `amount`) and `calculatedAt`, which are values *calculated by the finance system*. The internal balance calculation is expected to stay in the billing system.
- R5 also adds `billingStatus`, `procedure[]`, `diagnosis[]`, `relatedAccount[]` and `currency`. `guarantor[]` (with on-hold) and `coverage[]` (with priority) identify who pays.

### 1.3 ChargeItem

Source: https://hl7.org/fhir/R5/chargeitem.html (FMM 1)
- A ChargeItem is "not an actual financial transaction". It is source data for billing.
- Status values: `planned | billable | not-billable | aborted | billed | entered-in-error | unknown`.
- It links to `definitionCanonical` (ChargeItemDefinition) and `account[]`. The account link "can be maintained in a later process step".
- `unitPriceComponent` / `totalPriceComponent` together with `overrideReason` let a user override the rule-derived price. Normally a billing engine applies the definition's rules.

### 1.4 ChargeItemDefinition (how pricing works)

Source: https://hl7.org/fhir/R5/chargeitemdefinition.html (FMM 1)
- It is a canonical, versioned resource with `url`, `version`, `versionAlgorithm[x]`, `status`, `effectivePeriod`, `replaces` and `derivedFromUri`. This supports effective dating and keeping historical prices.
- `propertyGroup[]` holds `priceComponent[]`, which are MonetaryComponent values. Each group has `applicability[]`, a boolean `Expression` (e.g. FHIRPath) evaluated with `%context` set to the ChargeItem. The spec's examples are outpatient-only rules, after-hours surcharges, age-based discounts and once-per-encounter limits. "If no applicability rules are established for the group, then all properties always apply."
- The spec says the resource "requires profiling for each type of billing code system", and that its pricing model is "currently under development".
- MonetaryComponent type codes: `base | surcharge | deduction | discount | tax | informational` (https://hl7.org/fhir/R5/valueset-price-component-type.html).
- An `$apply` operation exists, but the spec does not fully define its calculation semantics. This is unverified beyond its existence.

**Constraint:** ChargeItemDefinition cannot directly express payer-contract prices, coverage-dependent copay splits, or the precedence rules in spec §64 (tenant, organization, facility, department, service, program, payer contract, patient eligibility, transaction). Each would need a custom profile or extension, plus an engine of our own.

### 1.5 Invoice

Source: https://hl7.org/fhir/R5/invoice.html (FMM 0)
- "The invoice is the hospital's view, whereas the Claim is the payer's view on the performed services."
- `status`: `draft | issued | balanced | cancelled | entered-in-error`. A cancelled invoice requires `cancelledReason`.
- `account` is "the Account being balanced by this Invoice". Each `lineItem[]` has `chargeItem[x]` (a ChargeItem reference or a CodeableConcept) and `priceComponent[]`. It also has `totalPriceComponent[]`, `totalNet` (excluding tax), `totalGross` (including tax) and `paymentTerms`.
- Invoice does not track payments. The link from a payment is `PaymentReconciliation.allocation.target`.

### 1.6 PaymentReconciliation (and PaymentNotice)

Source: https://hl7.org/fhir/R5/paymentreconciliation.html (FMM 4)
- Its framing is "bulk payment details associated with a payment by the payor".
- R5 adds point-of-sale-like fields: `kind` (e.g. "kiosk, deposit on account, periodic payment"), `issuerType` (patient or insurance), `paymentIssuer` (Organization, Patient or RelatedPerson), `method`, `cardBrand`, `accountNumber` (last digits), `tenderedAmount` and `returnedAmount` ("change").
- `allocation[].target` can reference Claim, Account, Invoice, ChargeItem, Encounter or Contract.
- The spec does not describe refunds or reversals. Refunds and reversals must be modelled through profiles or extensions, or through the internal ledger (inferred from the absence of any guidance).

### 1.7 Money and decimal

Source: https://hl7.org/fhir/R5/datatypes.html
- The `decimal` precision rule: "0.010 is regarded as different to 0.01 … Implementations SHALL handle decimal values in ways that preserve and respect the precision". In JSON it is a number. The page also states a limit of 18 digits.
- `Money` = `value` (decimal) + `currency` (ISO 4217 code, required binding). The page gives no guidance on currency minor units.
- **UGX minor unit = 0** (ISO 4217 Amendment 155, April 2013): https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/amendments/dl_currency_iso_amendment_155.pdf

## 2. Uganda financing context

### 2.1 National Health Insurance

- Parliament passed the National Health Insurance Scheme Bill 2019 on **31 March 2021**. The Ministry of Health had asked for it to be withdrawn. Sources: https://www.parliament.go.ug/node/5054 (Parliament press item; the TLS fetch failed, so it is cited via search results) and https://www.monitor.co.ug/uganda/news/national/parliament-passes-health-insurance-bill-3344440
- It was **not assented to**. Sources disagree on whether it was formally returned to Parliament:
  - The Daily Monitor of 2 Oct 2026 says the President "returned it" over employer financing and formal/informal cost sharing. MPs are calling for it to be "re-enacted": https://www.monitor.co.ug/uganda/news/national/mps-want-health-insurance-bill-fast-tracked-to-ease-care-for-older-persons-5616794
  - Other reports say it was never assented to and never returned, and that a revised bill sits at Cabinet. One Daily Monitor piece of March 2026 describes it as at Cabinet. This is seen only in search snippets and is **unverified**.
- A **Uganda Health Financing Strategy 2026-2036** is being drafted. It replaces the 2015/16-2024/25 strategy and prioritises domestic resource mobilisation and financial protection. It describes the NHIS as still not law: https://www.monitor.co.ug/uganda/news/national/uganda-drafts-10-year-health-financing-strategy-to-cut-donor-dependence-5584386 (newspaper report; the strategy document itself was not found and is **unverified**).
- The contribution model, accreditation and provider-payment mechanism of any future NHIS are **unknown**, and the design must not assume them.

### 2.2 Private insurance and HMOs

- Insurance Act 2017 (Act 6 of 2017), Part V covers "Licensing of insurers, HMOs and micro insurance organisations". An HMO is defined as an organisation "undertaking liability in respect of funding health care, by way of membership": https://ulii.org/akn/ug/act/2017/6
- The IRA publishes annual lists of licensed HMOs, e.g. https://ira.go.ug/wp-content/uploads/2023/01/HMOS-2023.pdf. The 2022 notice lists 3 HMOs: https://ira.go.ug/131-insurance-players-authorised-to-conduct-insurance-business-in-2022/. The current (2026) count is **unverified**.
- No national standard for claims exchange formats in Uganda was found. Insurer and HMO claim exchange is assumed to be bilateral (portals, paper, spreadsheets). This is **unverified**.

### 2.3 Public free care, private wings, PNFP and donors

- User fees were abolished in public health units in March 2001, except in hospital private wings. PNFP (mostly faith-based) and private facilities continue to charge fees: https://equityhealthj.biomedcentral.com/track/pdf/10.1186/1475-9276-8-39 and https://pmc.ncbi.nlm.nih.gov/articles/PMC2781807
- One facility can therefore be free (general ward) and paying (private wing) at the same time, which confirms spec §63's "same facility may use multiple models".
- Donor dependence is reported, e.g. an immunisation funding gap of USD 42M (from the 2026 strategy article above). Spec §70's rule that "Free care must still track actual resource consumption and funding" matters for donor reporting.

### 2.4 Payment regulation

- National Payment Systems Act 2020 (Act 15 of 2020): offering payment services requires a Bank of Uganda licence, and it defines "aggregator" as a payment service provider: https://ulii.org/akn/ug/act/2020/15
- If the platform itself collected funds on behalf of facilities, it could fall under this licensing regime. Integrating directly per facility with the facility's own merchant account, or through a licensed aggregator, avoids that. This is a design inference, not legal advice, and is **unverified**.

## 3. Mobile money APIs

### 3.1 MTN MoMo (Collection / RequestToPay)

- The official MTN MoMo docs widget (https://momoapi.mtn.com/content/html_widgets/lflwl.html) says:
  - RequestToPay and Transfer are asynchronous. The API "responds with '202 Accepted'", and "a callback with the final result" follows.
  - The callback URL goes in `X-Callback-Url`, must be on the domain registered for the API user, and must be HTTPS in production.
  - "The Wallet Platform sends the callback only once." "There is no retry mechanism".
  - MTN "strongly recommend[s]" polling the status GET as a fallback.
  - Callbacks may fail if the partner's certificate chain is not on MTN's approved CA list.
- Duplicate `X-Reference-Id` returns `409 Conflict` / `RESOURCE_ALREADY_EXIST`. Every request needs a fresh UUID v4, and status is queried with the original ID: https://momodevelopercommunity.mtn.com/errors-46/momo-api-common-errors-82. The official collection API reference page returned 404 at fetch time, so the 409 code is confirmed only from MTN's community forum and SDK typings.
- Status values `PENDING | SUCCESSFUL | FAILED` (plus `REJECTED`/`TIMEOUT` in some reports) come from secondary sources and are **unverified** against the official reference.
- **Implications:** the client-generated reference ID is the idempotency key. Persist it *before* sending the request. After a crash, query status by that ID and never resend. Callbacks are only a hint, and polling plus daily statement reconciliation is required.

### 3.2 Airtel Money (Airtel Africa Open API)

- The official portal (https://developers.airtel.africa) is JS-rendered and could not be read. **All details below are unverified secondary sources.**
- Collection uses a "USSD push" (the customer gets a PIN prompt). The initial response means accepted, not paid, and the final result comes by callback. Headers include `X-Country` and `X-Currency`. Sources: https://mctaba.com/learn/mpesa/airtel-money-integration-kenya and https://readme.hex.pm/airtel_money/0.1.1
- Callback payload: `transaction.id`, `transaction.status_code` (`TS` = success, `TF` = failed), `transaction.airtel_money_id`, and a top-level `hash` for signature verification. Authenticated callbacks must be enabled separately (https://readme.hex.pm/airtel_money/0.1.1, https://packagist.org/packages/bmatovu/laravel-airtel-money).
- A transaction status enquiry endpoint exists, and integrators recommend polling it when callbacks are missing.
- Whether `transaction.id` uniqueness is enforced (and so works as an idempotency key) is **unverified**.

### 3.3 Common pattern

Both providers use the same pattern: initiate with a client reference, get an async result, receive a callback that is not guaranteed, poll status, and reconcile against provider statements. Card and bank-transfer providers typically follow the same "pending until confirmed" lifecycle.

## 4. Double-entry ledger patterns

- Fowler, *Patterns for Accounting*: an account balance "is derived as the sum of all the Accounting Entrys". Corrections are made by **reversal** (offsetting entries then correct entries) or **difference** adjustments, which keep history. Replacement (deleting entries) loses history: https://martinfowler.com/eaaDev/AccountingNarrative.html
- TigerBeetle two-phase transfers: a *pending* transfer reserves amounts (`debits_pending`/`credits_pending`) and is later **posted** (fully or partially), **voided**, or **expires**. It can be resolved only once. The resolving transfer is a new immutable record with its own id that references `pending_id`. Balance invariants are checked at reservation time: https://docs.tigerbeetle.com/coding/two-phase-transfers/
- This pattern maps directly onto spec §68's "Do not treat provisional offline payments as externally confirmed".

## 5. Exact-decimal libraries

| Lib | Facts | Source |
|---|---|---|
| `rust_decimal` 1.43.0 | 96-bit integer mantissa + scale + sign. About 28-29 significant digits. Preserves trailing zeros. `RoundingStrategy` via `round_dp_with_strategy`. Serde as string/float/arbitrary-precision (`serde-str`, `serde-with-str`, `serde-arbitrary-precision`). Postgres `NUMERIC` via `db-postgres`/`db-tokio-postgres`; Diesel support. | https://docs.rs/rust_decimal/latest/rust_decimal/ |
| `decimal.js` | Arbitrary precision, default 20 significant digits (configurable). Rounding modes 0-8, including `ROUND_HALF_EVEN` (6); default `ROUND_HALF_UP`. Immutable instances. `toFixed` never uses exponent notation. | https://mikemcl.github.io/decimal.js/ |
| `bigdecimal` (Rust), `big.js`, `dinero.js` (TS) | Alternatives; not evaluated in depth (npm page blocked). | **unverified** |

The precise names of the `rust_decimal` `RoundingStrategy` variants (e.g. `MidpointNearestEven`) were not confirmed on the fetched page.

## Implications for the decision

### FHIR resources vs internal ledger

1. **The internal double-entry ledger (`health-financial-ledger`) is the financial source of truth.** It is append-only, uses balanced postings, and corrects by reversal. FHIR Account explicitly delegates balance computation to "the billing system", and Invoice is FMM 0. Neither can carry accounting integrity.
2. **FHIR resources are the interoperable boundary and the clinical-financial link:**
   - ChargeItem is created from clinical events (it records consumption, including free and donor-funded care, per §70).
   - Account represents encounter or episode buckets and guarantor/coverage ordering, with `balance[]` *projected* from the ledger.
   - Invoice is a document view.
   - Coverage, CoverageEligibility\* and Claim/ClaimResponse/EOB are used for payer exchange.
   - PaymentReconciliation/PaymentNotice are used for payer remittances and can also represent patient receipts.
3. **Pricing:** use ChargeItemDefinition as the *published, versioned price-list artifact* (canonical url, version, effectivePeriod, priceComponent types). The precedence and policy engine for §64/§66 (payer-contract prices, copays, waivers, bundles, quantity tiers) has to be our own and will need profiles or extensions. Store the evaluated price breakdown (MonetaryComponents plus the rule and definition version used) on the ChargeItem or in the ledger at charge time, so that historical pricing decisions are preserved.
4. **Pin profiles locally.** The low FMM means R6 may change these resources. Isolate them behind the `health-billing` and `health-coverage` contracts.

### Offline provisional payments

- Cash taken offline is final for the facility till. It is posted as a ledger entry with "edge-local" provenance and later synced.
- Mobile money, card and bank payments recorded offline (or with pending provider status) are posted as **pending/two-phase** transfers against a "payments-in-transit" clearing account.
  - They are *posted* only on provider evidence: a callback, a status poll, or a statement line.
  - Otherwise they are *voided* or expired.
  - Receipts must show the provisional status.
- The provider reference (MoMo `X-Reference-Id` UUID, Airtel transaction id) is generated and persisted locally *before* initiation. It doubles as the idempotency key and the reconciliation key. A device must never re-initiate. It re-queries by the same reference.
- Payment initiation needs connectivity, so a fully offline edge cannot *initiate* MoMo. It can record a customer-reported payment (customer shows the SMS) as a pending entry awaiting verification.

### Risks

- **Low FHIR maturity** (Invoice FMM 0, ChargeItem/ChargeItemDefinition FMM 1) can cause breaking changes in R6 and gives weak tooling and validator support.
- **NHIS uncertainty:** Uganda's NHIS may arrive with an unknown scheme design or claims format. Keep the payer adapter pluggable and do not hardcode a single national payer.
- **Mobile money reliability:** a missed MoMo callback with no retries can leave payments stuck in pending. Reconciliation jobs and operator tooling are mandatory.
- **Rounding:** UGX has no minor unit, but other currencies do. Per-currency scale, explicit rounding mode and rounding-residue accounts are needed when percentage splits (copay, discount) do not divide evenly.
- **Regulation:** if the platform holds funds on behalf of facilities, it may need a Bank of Uganda payment-service licence (unverified, needs legal review).
- **Decimal round-trips:** FHIR JSON `decimal` is a JSON number, and JS `JSON.parse` loses precision. The TS client must parse money with a lossless JSON parser, and the Rust side must serialise via `serde-arbitrary-precision` or an equivalent.

### Open questions

1. Will any Ugandan payer (NHIS, HMOs, insurers) accept FHIR Claim, or are claims exported to bespoke formats or portals?
2. What is the exact Airtel Money API contract (uniqueness of `transaction.id`, status codes beyond TS/TF, refund endpoint)? This needs a portal login.
3. Integrate MTN/Airtel directly, or through a licensed aggregator? This affects licensing, statement reconciliation and refunds.
4. Do donor and government funders need fund-accounting (restricted funds, grant periods) in the ledger chart of accounts, and which reporting formats (e.g. DHIS2 finance indicators)?
5. Should multi-currency (USD donor funds, UGX operations) be supported in the ledger from day one, and which FX-rate source?
6. Which emergency bypass or deferral policy (§70) defers charges rather than waiving them, and who authorises the conversion to a waiver?
