# Payment steps are configured Financial Gates, and clinically urgent care always defers rather than blocks

Clinical workflows contain no payment steps.

**Configuration.** A **Financial Gate** is a configuration record, per facility, service, payer or program, attached to a workflow step. An example is "before sample collection".

**Evaluation.** When a workflow reaches a gate, the workflow engine asks a Finance contract query for an outcome:
- `pass`: paid, covered or free;
- `defer`: with a reason;
- `block`: only for non-urgent care, and only where the gate allows it.

**Emergency.** An emergency or urgent triage category always produces `defer`, so urgent care is never blocked.

**Deferred charges** stay on the Account as receivables. Converting one to a waiver needs a permission (social worker or finance role) and is audited.

We chose this because §70 forbids hardcoded payment assumptions and requires urgent-care bypass. Facilities differ widely: free public wards, paying private wings and insured patients in the same building.

## Considered Options

- **Payment states inside each clinical workflow definition.** Rejected: every program and facility variant would need its own workflow.
- **Waiving charges automatically for emergencies.** Rejected: a waiver hides consumption and funding (§70). Deferring keeps the charge, and a waiver is a separate, accountable decision.

## Consequences

- Gate placement and engine semantics are part of the workflow-engine decision.
- Finance exposes the gate query in its contract crate, so the workflow engine never reads ledger internals.
