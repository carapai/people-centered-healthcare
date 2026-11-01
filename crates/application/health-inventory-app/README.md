# health-inventory-app

Layer: **application** (inventory). Recording stock movements over any `health_persistence::Store` whose transaction is a `health_inventory::StockStore` (inventory-and-procurement.md § Balances and concurrency).

- **`Inventory::record(movement, origin)`:** one transaction. It reads the Stock Unit's balance record, decides with the store's `Policy`, appends the movement and writes the balance.
  - **The balance record is the guard.** Two concurrent issues of the last pack both write the same balance record, so one loses a write conflict. It is rerun (up to 8 times, then `InventoryError::Busy`), re-reads the balance and is refused for insufficient stock.
- **`change_status(movement, status, origin)`:** moves stock between statuses (usable → quarantine) in one transaction, out of one Stock Unit and into the other.
- **Transfers** (#129): `dispatch`, `receive` and `direct_transfer` are each one transaction. A step records every movement it makes, each decided by the store's policy, so a dispatch beyond what is available moves nothing. It also writes the transfer's state and a Reconciliation Record for each discrepancy found on receipt.
- **Reconciliation Records:** any late operation that drives a balance negative opens one in its movement's transaction. `resolve` records the root cause, and closes a transfer once its last record is resolved. `open_reconciliations` lists the open ones, oldest first.
- **Stock counts** (#129): `start_count` (every unit of the store, or the given units for a cycle count; nothing is frozen), `record_count` and `submit_count`.
  - **`approve_count`** is one transaction. It posts a count-variance movement and a Reconciliation Record for each variance. It also resolves the open negative-balance records of the counted units, each with its root cause (`Cause`; `CauseRequired` without one, GSDP §13.2), and unflags their balances.
  - **`inventory_report`** is the count as a FHIR `InventoryReport` snapshot.
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

`tests/counts.rs`, over SurrealDB:
- a count during issues posts only the true variance, with its record, and exports the InventoryReport;
- a large variance waits for a supervisor;
- a count settles a flagged negative balance only with its root cause.

**Not here yet:**
- central matching of inter-facility dispatches with receipts (#247, with sync);
- the change log and outbox for sync (#135);
- reservations and FEFO (#128);
- the HTTP API.
