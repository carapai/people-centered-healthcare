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

**Not here yet:** storing the postings and payments (#254).
