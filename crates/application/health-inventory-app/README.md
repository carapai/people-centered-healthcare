# health-inventory-app

Layer: **application** (inventory). Recording stock movements over any `health_persistence::Store` whose transaction is a `health_inventory::StockStore` (inventory-and-procurement.md § Balances and concurrency).

- **`Inventory::record(movement, origin)`:** one transaction. It reads the Stock Unit's balance record, decides with the store's `Policy`, appends the movement and writes the balance.
  - **The balance record is the guard.** Two concurrent issues of the last pack both write the same balance record, so one loses a write conflict. It is rerun (up to 8 times, then `InventoryError::Busy`), re-reads the balance and is refused for insufficient stock.
- **`change_status(movement, status, origin)`:** moves stock between statuses (usable → quarantine) in one transaction, out of one Stock Unit and into the other.
- **`balance(key)`** and **`nightly_check()`:** the check rebuilds each balance from its movements and reports any drift.

## Tests

`tests/stock.rs`, over SurrealDB's memory engine:
- six concurrent issues of the last pack on a multi-threaded runtime: exactly one succeeds, and the rest are refused;
- a status change, and one refused for more than is usable, which changes neither unit;
- the nightly check finds no drift after either.

**Not here yet:** the change log and outbox for sync (with leases, #135), Reconciliation Records (with counts, #129), reservations and FEFO (#128), and the HTTP API.
