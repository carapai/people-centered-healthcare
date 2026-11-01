# health-inventory-app

Layer: **application** (inventory). Recording stock movements over any `health_persistence::Store` whose transaction is a `health_inventory::StockStore` (inventory-and-procurement.md § Balances and concurrency).

- **`Inventory::record(movement, origin)`:** one transaction. It reads the Stock Unit's balance record, decides with the store's `Policy`, appends the movement and writes the balance.
  - **The balance record is the guard.** Two concurrent issues of the last pack both write the same balance record, so one loses a write conflict. It is rerun (up to 8 times, then `InventoryError::Busy`), re-reads the balance and is refused for insufficient stock.
- **`change_status(movement, status, origin)`:** moves stock between statuses (usable → quarantine) in one transaction, out of one Stock Unit and into the other.
- **Transfers** (#129): `dispatch`, `receive` and `direct_transfer` are each one transaction. A step records every movement it makes, each decided by the store's policy, so a dispatch beyond what is available moves nothing. It also writes the transfer's state and a Reconciliation Record for each discrepancy found on receipt.
- **Reconciliation Records:** any late operation that drives a balance negative opens one in its movement's transaction. `resolve` records the root cause, and closes a transfer once its last record is resolved. `open_reconciliations` lists the open ones, oldest first.
- **`balance(key)`** and **`nightly_check()`:** the check rebuilds each balance from its movements and reports any drift.

## Tests

`tests/stock.rs`, over SurrealDB's memory engine:
- six concurrent issues of the last pack on a multi-threaded runtime: exactly one succeeds, and the rest are refused;
- a status change, and one refused for more than is usable, which changes neither unit;
- the nightly check finds no drift after either.

`tests/transfers.rs`, over SurrealDB:
- a short and damaged receipt reconciles, then closes once both records are resolved, with no drift;
- a dispatch beyond what is available moves nothing;
- a direct transfer moves both sides at once;
- a late operation driving a balance negative opens a record.

**Not here yet:**
- stock counts and the InventoryReport export (#129 part 2);
- central matching of inter-facility dispatches with receipts (with sync, Phase 5);
- the change log and outbox for sync (#135);
- reservations and FEFO (#128);
- the HTTP API.
