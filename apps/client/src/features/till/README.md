# Till

Cashier tills and payments (billing-and-payments.md § Payments; ADR 0032; #254).

- **`TillPage`** (`/till`): the cashier's till.
  - **Opening:** a closed till offers **Open shift** with an opening float.
  - **Taking payments:** cash, or MTN Mobile Money, Airtel Money, card, bank transfer, or a payment reported by the customer, each with its provider reference.
  - **Receipts this shift:** each with its number. A non-cash receipt says **Provisional** until the provider confirms it.
  - **Closing:** with the cash counted. A variance waits for a supervisor; the page shows the shift to approve.
- **`SupervisePage`** (`/till/supervise`): a supervisor enters a provider statement line, which confirms the pending payment it matches, and approves a closed shift's variance.
- **On the server:** `/api/payments` (`health-fhir-server`'s `payments_router`) checks the permissions and the rules. The screens enforce nothing.
- **Not yet:**
  - provider callbacks and status polls, from mobile money at central (#124);
  - refunds and voids on screen (the API serves them);
  - printing receipts.

## Tests

- `test/till.test.ts`: the API.
- `e2e/till.spec.ts`: a cashier's MTN receipt stays provisional until a supervisor enters the statement line, and a short close waits for the supervisor's approval, with axe on each screen.
