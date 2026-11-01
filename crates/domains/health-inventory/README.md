# health-inventory

Layer: **domain** (inventory). The stock ledger: Stock Units, movements and the negative-stock policy (inventory-and-procurement.md; ADR 0034).

- **Stock Unit** (`StockUnit`): store × product × batch × expiry × funding source × ownership (owned | consignment) × status (usable | quarantine | restricted). `StockUnit::key` is its balance record's key.
- **Movements** (`Movement`, `MovementKind`): receipt, issue, dispense, transfer-out and -in, return, adjustment, count variance, expiry write-off, status change and reversal.
  - Each carries a signed quantity in base units, the pack size, unit cost, `occurred_at` and `recorded_at`, the source document, actor, node and `op_id`.
  - Corrections are a `reversal`: the same quantity negated, naming the movement it reverses.
- **Balances** (`Balance`): on-hand, reserved, on-hold and the flag. `available = on-hand − reserved − on-hold`.
- **The negative-stock policy** (`Policy::decide`) takes the balance, the movement and its `Origin`, and gives the new balance or `Refused`:
  - quantities must fit their kind (a receipt adds, an issue removes), and an adjustment needs a reason;
  - at the authority, an issue, transfer or adjustment beyond what is available is blocked;
  - a dispense beyond it is allowed only as an emergency dispense, with the permission and a reason, and is flagged;
  - a late operation (an offline candidate) is appended, flags the unit and asks for reconciliation;
  - a store may tighten the policy (`block_emergency_dispense`) or relax it (`allow_negative_issues`, which allows and flags).
- **The stock card** (`stock_card`) orders movements by `occurred_at`, then `recorded_at`, then id, with a running balance, so a backdated movement lands in place. `rebuild` sums a unit's movements for the nightly check (`Drift`).
- **Transfers** (`transfers`, #129): two steps, from Picked to InTransit, then Received or Reconciling, then Closed.
  - `Transfer::dispatch` is a transfer-out of each line at the source.
  - `receive` takes the counted good and damaged quantities. It gives the transfer-ins (damaged stock into quarantine) and what was found: short, damaged or excess.
  - `direct` is the one-step transfer within one facility by one user, and is marked. A transfer between facilities is never direct.
  - `close` needs its records resolved.
  - `discrepancies` is the comparison central's matching of a dispatch with a receipt will use.
- **Reconciliation Records** (`reconciliation`): `ReconciliationRecord` is a discrepancy (short, damaged, excess, negative, count variance) with its signed quantity and source. It is open until `resolve` records its `RootCause` (GSDP §13.2): recording error, unrecorded issue, transit loss, damage, expiry, supplier discrepancy, theft, or other with a note. A record is resolved once.
- **Stock counts** (`counts`, #129): `StockCount` goes from Draft to Submitted to Approved, in full, cycle or blind mode (`sheet` hides what the ledger expects).
  - **No freeze:** `variances` are counted minus (the balance at the start plus the movements recorded since).
  - **Approval:** `approve` needs a reason for every variance, and a supervisor for any variance above the store's threshold.
- **Receiving** (`receiving`, #130):
  - **`GoodsReceived`:** a goods received note against a supplier delivery. `validate` needs the supplier, delivery and complete lines (product, batch, expiry, positive quantity and pack size, cost of zero or more).
  - **`parse_gs1`:** reads GS1 DataMatrix element strings, as scanned (with the group separator) or as printed (`(01)…`): GTIN 01, expiry 17 (day `00` is the month's last), batch 10 and serial 21. It is never required.
  - **`Requisition`:** a ward's SupplyRequest to the pharmacy, from Requested to Approved (or Rejected), then Fulfilled by a transfer.
  - **`suggest`:** min/max reorder levels from the average monthly consumption (`ReorderPolicy`, default 1 and 3 months). At or below the minimum it orders up to the maximum, rounded up; with no consumption it suggests nothing.
- **The `StockStore` port:** a unit's balance record and its movements, transfers, counts, goods received notes, requisitions, the units of a store, and Reconciliation Records, inside a store transaction.

## Tests

`tests/receiving.rs` covers note validation, GS1 as scanned and printed, requisitions, and reorder suggestions.

`tests/counts.rs` covers blind sheets, counting and submitting, movements during a count, and approval rules.

`tests/transfers.rs` covers transfer validation, dispatch and receipt, short, damaged and excess stock, direct transfers, closing, and resolving records.

`tests/policy.rs` covers the quantity and reason rules, blocking at the authority (including reserved and held stock), emergency dispense, store tightening and relaxing, late operations, the stock card with a backdated movement, and reversals.
