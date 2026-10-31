# Billing, payments and the financial ledger

Status: **accepted** (ADRs 0030, 0032, 0033). Produced while resolving the Wayfinder ticket [How do pricing, coverage, billing, payments and the ledger fit together?](https://github.com/carapai/people-centered-healthcare/issues/27). Background: [docs/research/health-financing.md](../research/health-financing.md). Spec: MASTER_PROMPT §63, §68–70. Pricing and payers are in [pricing-and-insurance.md](pricing-and-insurance.md). Ledger operations sync as `kind: op` items ([synchronization.md](synchronization.md)).

## Overview

```mermaid
flowchart LR
  CL[Clinical events<br/>Encounter, specimen, dispense, procedure, bed-day] -- charge triggers --> CI[ChargeItem<br/>billing]
  CI -- pricing stages --> CI
  CI --> ACC[Account<br/>billing]
  CI -- charge posting --> L[(Financial ledger<br/>financial-ledger)]
  PAY[Payments<br/>payments] -- postings --> L
  COV[Claims / remittances<br/>coverage, central] -- postings --> L
  L -- projected balance --> ACC
  ACC --> INV[Invoice view]
  WF[Workflow engine] -- gate query --> FG{Financial Gate<br/>Finance contract}
  FG -- pass / defer / block --> WF
```

## The ledger (ADR 0030)

- **One ledger per facility.** It is written only by the facility's Authority Node, or by a lease holder for a leased sub-scope such as an outreach till.
- **Double-entry and append-only.** Every posting balances, and errors are corrected by reversal. Balances are always derived from postings.
- **Chart of accounts.** The templates come from country configuration:
  - receivables per funding source;
  - patient deposits (a liability);
  - payments in transit (clearing);
  - cash per till;
  - revenue per service class;
  - waivers and subsidies (contra-revenue);
  - rounding;
  - till variance.
- **Dimensions on every posting:** facility, department, service, program, **funding source**, `fund` (restricted grant), encounter or episode, and the price version references.
- **Money.** `rust_decimal`, with the ISO 4217 scale per currency (UGX: 0) and round-half-even. Largest-remainder allocation is used for splits, with residue posted to the rounding account. There is one functional currency per ledger; foreign amounts keep their original value and rate.
- **Implementation** (#119): `health-financial-ledger` holds the exact money, the chart template, balanced append-only postings with reversals, and the derived balances, including the projection for FHIR `Account.balance[]`. Persistence and the posting use cases come with billing.
- **Free care** creates ChargeItems priced at a reference tariff and allocated to the government or donor funding source (§70). The patient share is zero, but consumption is still recorded.

## Charge capture

- **Charge triggers** are configuration records that map events to charges. Examples:
  - Encounter started at a service point;
  - specimen collected;
  - MedicationDispense completed;
  - Procedure completed;
  - each bed-day accrued.
- **Ids.** ChargeItem ids are UUIDv5 values derived from the source event and the trigger canonical (identifiers.md), so replays never double-charge.
- **Subscription.** Clinical Modules emit events, and `billing` subscribes. No clinical code contains payment logic.
- **Lifecycle:** `planned` → `billable` → `billed` (on an Invoice or Claim). A cancellation posts a reversal and sets `entered-in-error` or `not-billable`.
- **Accounts.** Each ChargeItem belongs to an Account (per encounter, or per episode for inpatient and long programs). The Account carries guarantor and coverage order.

## Financial Gates (§70; ADR 0033)

- **Configuration records** per facility, service, payer or program, attached to a workflow step. Example: "before sample collection: requires pass".
- **Outcomes** of the Finance contract query `evaluate_gate(context)`:

  | Outcome | When |
  |---|---|
  | `pass` | Paid, covered (including an approved preauthorization where required), or free |
  | `defer` | Emergency or urgent triage category (**always**), or a deferral recorded by an authorized user |
  | `block` | Only for non-urgent care, and only where the gate allows blocking |

- **Deferred charges** stay as receivables on the Account. Converting one to a waiver needs a permission (social worker or finance role) and is audited.
- **Placement** of gates in each flow is in [workflow-engine.md](workflow-engine.md).

## Payments (ADR 0032)

```mermaid
stateDiagram-v2
  [*] --> Pending: non-cash payment recorded (provider ref persisted first)
  Pending --> Posted: provider evidence (callback, poll, statement)
  Pending --> Voided: provider failure / cashier void
  Pending --> Expired: no evidence by expiry
  [*] --> Posted: cash at till (cashier shift)
  Posted --> Reversed: error correction (new reversal op)
```

| Method | Behaviour |
|---|---|
| Cash | Final at the till; posted within a cashier shift |
| Mobile money (MTN, Airtel), card, bank transfer | Two-phase: pending against **payments in transit**, then posted only on provider evidence. Receipts say "provisional" until then |
| Customer-reported (offline) | Pending-unverified, with the reference from the customer's SMS; verified later against the provider statement |

- **Provider integrations run only at central** (`health-integration`). An edge asks central to initiate, and the outcome syncs back. An edge that is offline cannot initiate mobile money.
- **Idempotency.** The provider reference (e.g. MTN `X-Reference-Id` UUIDv4) is persisted in the same transaction as the pending entry, before initiation. Recovery queries by that reference and never re-initiates. The reference is also the reconciliation key.
- **Merchant accounts.** Each facility uses its own merchant account, or a licensed aggregator. The platform never holds funds (Bank of Uganda licensing is in the map's legal fog).

## Other flows

| Flow | Posting pattern |
|---|---|
| Deposit | Credit to the patient's deposit liability. Applied to invoices later |
| Partial payment | Allocated to the patient-share lines, oldest first |
| Credit | Receivable with terms. Ageing is projected into `Account.balance[].term` |
| Refund | A new outgoing payment operation (cash out, or provider disbursement through central). Needs a permission |
| Reversal | Error correction only, never an edit |
| Till settlement | At shift close, counted cash is compared with expected. A variance is posted to till variance and needs supervisor approval |
| Provider statements | A daily reconciliation job at central matches pending and posted entries to statement lines. Exceptions go to a queue |
| Payer remittance | PaymentReconciliation; postings clear the payer receivable, and shortfalls become adjustments or patient-share transfers |

## Documents and numbering

- **Invoice** is a view over the Account's ChargeItems and postings. Its status follows the ledger (`issued`, `balanced`, `cancelled`).
- **Receipt and invoice numbers** are node-prefixed local sequences (ADR 0014).
- **Fiscal receipting** has an adapter port on `billing`. Whether URA EFRIS e-invoicing applies to VAT-registered private facilities is unverified and sits in the map's legal fog.

## FHIR exposure

- Account, ChargeItem, Invoice, PaymentReconciliation and PaymentNotice are served as **projections** through the FHIR API, using locally pinned profiles.
- External systems may create ChargeItems, which pass through the same pricing stages. They never write ledger postings directly.
