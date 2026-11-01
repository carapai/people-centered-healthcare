# health-payments-app

Layer: **application** (Finance, payments). Payments as `health-financial-ledger` postings (#123; ADR 0030, ADR 0032).

- **`Ledgering::post(payment, step, till, at)`** gives the balanced posting of each step:

  | Step | Debit | Credit |
  |---|---|---|
  | Cash taken | `cash/{till}` | the patient's receivable (or deposits) |
  | Non-cash recorded, pending | payments in transit | the patient's receivable (or deposits) |
  | Confirmed by the provider | the method's merchant account | payments in transit |
  | Voided or expired | the patient's receivable (or deposits) | payments in transit |
  | Reversed, or refunded | the patient's receivable (or deposits) | where the money is |

  A step that does not fit the payment's status is refused, so a pending payment cannot be posted as confirmed.
- **`Ledgering::variance(shift)`:** a till shortage or overage against till variance, once the shift is settled.
- **Accounts:** the patient's receivable, and a merchant account per non-cash method (`cash/merchant-mtn`, …) in the chart's tills.

## Tests

`tests/postings.rs` runs a shift through a real ledger:
- cash, a deposit, and an MTN payment pending, then confirmed;
- an Airtel payment voided;
- a refund, and a supervised shortage.

The pending money sits in transit, never counted as received; the trial balance is zero; and refused steps are covered.

- **`Payments`** (#254) are the use cases over any `Store` whose transaction is a `PaymentStore` and a `PostingStore`. Each step is one transaction: the payment, its posting (checked against the chart first), and any change to its shift.
  - **Shifts:** `open_shift` allows one open shift per till (a guard, released at close); `close_shift`; `approve_shift`, which posts the variance.
  - **Cash and pending payments:** `take_cash`; `record_pending`, which claims the provider reference once, so a reused one is `DuplicateReference`.
  - **Later steps:** `confirm`, `void`, `expire_due`, `reverse` and `refund`. A cash refund is paid out of the till's open shift, and needs the permission.
  - **Statements:** `reconcile_statement` confirms the pending payments a statement matches, and returns the exceptions.
  - **Views:** `payment`, `shift` and `balances` (from the stored postings).
  - **Receipts** are numbered from the node-prefixed sequence (ADR 0014).

`tests/service.rs`, over SurrealDB:
- a shift with cash and a confirmed MTN payment: receipts numbered, a reused reference refused, balances, a supervised shortage, and the till reopened;
- pending payments voided, expired, or confirmed by the statement, with a mismatch confirming nothing;
- cash refunds with the permission and an open shift.

**Not here yet:** the API and the till screens (#254 part 2).
