# health-payments

Layer: **domain** (Finance, payments). Tills and two-phase payments (#123; billing-and-payments.md § Payments, § Other flows; ADR 0032; MASTER_PROMPT §68).

- **`Shift`:** a cashier's shift at a till. It opens with a float and takes cash (`Payment::cash`); cash refunds are paid out of it (`pay_out`).
  - **Close:** `close(counted)` compares the counted cash with what is expected (float + taken − paid out). Without a variance the shift is settled; with one it waits for `approve` by a supervisor other than the cashier.
- **`Payment`:**
  - **Cash** is final at once, taken only within an open shift.
  - **Mobile money (MTN, Airtel), card and bank transfer** are `pending`, with the provider reference required before they are recorded, which makes them idempotent.
  - **Customer-reported** payments (offline, the reference from the customer's SMS) are pending and unverified.
  - **Confirming:** `confirm(evidence)` posts a payment on a callback, poll or statement line; a customer-reported one needs a statement line.
  - **Other steps:** `void`, `expire` (only once its expiry has passed), `reverse` (only a posted payment), and `refund` (only a posted payment, with the refund permission, up to what was paid).
  - **Receipts:** `is_confirmed` is true only once posted, and `receipt` is `Provisional` until then. **A pending payment never appears as confirmed.**
- **`reconcile(payments, lines)`:** the daily provider-statement reconciliation, matching by provider reference.
  - **To confirm:** the pending payments the statement confirms.
  - **Exceptions, for a person:** mismatched amounts (nothing is confirmed on one), unknown statement lines, and posted payments the statement does not have.

## Tests

`tests/payments.rs` covers:
- provisional payments are never confirmed, for every non-cash method;
- references and evidence;
- voiding, expiry and reversal;
- cash needing an open shift, and refunds needing the permission;
- shift settlement with a supervised variance;
- statement reconciliation with its exceptions.

- **`PaymentStore`** is the storage port for payments and shifts; `health-surrealdb` implements it (migration `0018_payments`). A payment carries its `receipt` number once issued.

The API and the till screens are in `health-fhir-server` and the web client (#254). Provider callbacks and polls come from mobile money at central (#124).
