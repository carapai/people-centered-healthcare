# health-financial-ledger

Layer: **domain** (financial-ledger). The facility's double-entry ledger, the financial source of truth (billing-and-payments.md § The ledger; ADR 0030). FHIR Account, ChargeItem and Invoice are projections of it.

- **Money** (`Money`, `Currency`):
  - `rust_decimal` at the ISO 4217 scale (UGX 0, USD 2, KES 2, …). An amount with more places is refused unless rounded explicitly, half-even.
  - `Money::split` is the largest-remainder method. The shares always sum exactly (tested exhaustively), and the left-over minor units go to the largest remainders.
- **The chart of accounts** (`ChartOfAccounts::from_template`), from the country's template:
  - receivables and waivers (contra-revenue) per funding source;
  - patient deposits (a liability) and payments in transit (clearing);
  - cash per till and revenue per service class;
  - rounding and till variance.
- **Inventory accounting** (#131): with `ChartTemplate::inventory`, the chart gains `INVENTORY`, `COST_OF_GOODS`, `STOCK_ADJUSTMENT` and `PAYABLE`; it is off by default for public facilities. `health-inventory-app::inventory_postings` gives the postings.
- **Storage:** `PostingStore` is the port (append once; `health-surrealdb`, migration `0018_payments`), and `Ledger::from_postings` rebuilds a ledger from them, checking each again.
- **Postings** (`Ledger::post`) need at least two lines with positive amounts in the functional currency, on accounts in the chart and at the ledger's facility, with equal debits and credits.
  - **Dimensions:** every line carries them: facility, department, service, program, funding source, fund, encounter, and price versions.
  - **Foreign amounts** keep their original amount and rate.
  - **Corrections:** postings are appended and never changed. A correction is a `Ledger::reversal`, the same lines on opposite sides, made at most once and never of a reversal.
- **Balances** are derived from postings, never stored:
  - `Ledger::balance` gives an account's balance on its normal side, over a dimension filter;
  - `Ledger::trial_balance` is always zero;
  - `Ledger::encounter_balances` gives what is owed per funding source, which FHIR `Account.balance[]` projects.

## Tests

`tests/ledger.rs` covers:
- **Money:** currency scale and half-even rounding, largest-remainder splits (including refunds and an exhaustive sum check), and the chart template.
- **Postings:** a visit charged, paid and balanced, and every refusal.
- **Reversals:** made once only.
- **Funding:** a patient and insurer split, free care allocated to its funder (§70), and a foreign payment with its rate.

**Not here yet:** persistence and the posting use cases (charge capture, tills, payments), which come with billing.
