# Stock is an append-only movement ledger per Stock Unit, and each Stock Unit's balance record guards concurrent issues

Every stock change is an immutable **movement**:
- receipt;
- issue;
- dispense;
- transfer-out and transfer-in;
- return;
- adjustment;
- count variance;
- expiry write-off;
- status change.

**Stock Unit key.** Each movement is keyed by **Stock Unit**: store × product × batch × expiry × funding source × ownership (owned or consignment) × status (usable, quarantine or restricted).

**What a movement carries:** a signed quantity in base units, the pack size, unit cost, `occurred_at` and `recorded_at`, the source document, actor, node and `op_id`.

**Corrections** are reversals.

**Balance records.** Each Stock Unit has one balance record (on-hand, reserved, available), written in the same transaction as the movement.
- **Guard.** Concurrent writers of the same Stock Unit conflict under snapshot isolation and retry, so the availability check and the decrement are atomic (persistence.md).
- **Rebuild and drift.** Balances can always be rebuilt from the ledger. A nightly check alerts on drift.
- **Backdating.** A backdated movement recomputes the running stock card from its `occurred_at`.

**Negative stock at the authority:**
- issues, transfers and adjustments that would go negative are blocked;
- dispensing that would go negative needs an "emergency dispense" permission and a reason, and is flagged.

**Late operations.** Offline candidates are always appended. One that drives a balance negative raises a Reconciliation Record, and the Stock Unit stays flagged until a count resolves it. Stores can tighten or relax this policy.

**Central** keeps the movement streams, re-derives balances and never accepts a balance overwrite.

We chose this for three reasons:
- §71 forbids relying on editable balance fields.
- Every mature health LMIS examined (OpenLMIS, Open mSupply, OpenBoxes) derives balances from transactions.
- SurrealDB's snapshot isolation needs a single record per invariant to serialize writers.

## Considered Options

- **Editable balances, with movements as history.** Rejected by §71; drift also becomes undetectable.
- **Allowing deletion of movements** (as OpenBoxes does). Rejected: it breaks audit and sync.
- **Hard-blocking every negative balance, including late offline ones.** Rejected: offline facts have already happened physically, so refusing them loses the record of a real dispense.

## Consequences

- The FHIR inventory resources (maturity levels 0–1) are façades only; ledger entries, reservations and balances stay internal.
- Valuation (weighted average by default, or FIFO) is a projection. Postings to the financial ledger happen only where inventory accounting is enabled.
