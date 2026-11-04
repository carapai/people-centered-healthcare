# health-billing-contract

Layer: **contracts** (billing). Billing's contract (ADR 0033; billing-and-payments.md § Financial Gates; #122): the query the workflow engine asks at a gate state, so workflows contain no payment steps and never read ledger internals.

- **`GateQuery`:** the gate's canonical URL, the Account, the patient and the care's `Urgency` (routine, urgent, emergency).
- **`GateOutcome`:** `Pass`, `Defer` or `Block`, each with a reason. `code()` names the event the gate state raises (`gate.pass`, `gate.defer`, `gate.block`).
- **`FinancialGates`:** the port billing implements. Urgent and emergency care (`Urgency::never_blocks`) always defers when anything is owed.
