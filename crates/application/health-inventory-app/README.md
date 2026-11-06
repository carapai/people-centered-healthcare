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
- **Receiving** (#130): `receive_goods` is one transaction. It writes a receipt of each line at its unit cost and pack size, and the note; a note is received once.
- **Requisitions:** `request`, `decide_requisition`, and `fulfil_requisition`, which dispatches the picked batches as a transfer from the pharmacy to the ward and marks the requisition fulfilled in the same transaction. `requisitions(store)` lists those a store asked for or was asked, newest first. `supply_requests` is a requisition as FHIR SupplyRequests.
- **Stock on hand:** `stock_on_hand(store)` gives each Stock Unit with its balance and the unit cost of its last costed movement.
- **Reorder:** `reorder(store, policy, days, now)` gives per-product suggestions from usable stock and the issues and dispensing over the last `days` days.
- **Valuation** (#131): `valuation(store, method)` rebuilds the value of each Stock Unit of a store from its movements, and the total.
- **Inventory accounting:** `inventory_postings` gives one balanced posting per costed movement for `health-financial-ledger`, with the unit's funding source as a dimension.
  - **Receipts:** inventory against supplier payables.
  - **Issues and dispensing:** cost of goods against inventory; a return reverses it.
  - **Write-offs, adjustments, count variances and reversals:** against stock adjustments.
  - **No posting:** transfers within the facility and status changes.
  - **Off by default:** a facility enables it through `ChartTemplate::inventory`, which adds the accounts. The financial ledger has no storage yet, so the postings are produced, not persisted.
- **Dispensing** (#128; ADR 0035). **Stock moves only when a dispense is completed.**
  - **`prepare(preparation, eligibility, rule)`:** one transaction. It reserves stock for a MedicationDispense by first expiry, or by the pharmacist's choice with a reason. Only stock the patient is eligible for is taken, and only batches in date past the course plus the buffer. Each reservation is written with its unit's balance record, so two preparations of the last packs conflict and the rerun is refused. Preparing the same dispense again returns its live reservations.
  - **`complete_dispense(completion)`:** one transaction. Each amount given becomes a dispense movement, decided by the store's policy and drawn from the dispense's own reservation, and the reservations are consumed. A partial dispense releases the rest. The transaction also writes `inventory.dispensed.v1`, the charge trigger. A lapsed reservation is `Lapsed`, and the dispense must be prepared again.
  - **`cancel_dispense(dispense, at)`:** releases the reservations, and nothing moves.
  - **`return_to_quarantine(return)`:** a patient's return, as a return movement into the batch's quarantine unit.
  - **Live reservations only:** every movement's decision, and `available(key, at)`, count only live reservations, so a lapsed one frees its stock at once.
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

`tests/receiving.rs`, over SurrealDB:
- a receipt increases the balance with its cost recorded, once;
- a ward requisition is approved, then fulfilled by a transfer;
- reorder suggestions follow the last month's consumption.

`tests/dispensing.rs`, over SurrealDB:
- stock moves only on a completed dispense, partial completion releases the rest, and there is no drift;
- a cancelled or lapsed preparation moves nothing and frees its stock;
- six concurrent preparations of the last packs: exactly one reserves them;
- donor stock is reserved only for enrolled patients, not even by a pharmacist's choice;
- reserved stock is not issued elsewhere, and a return goes to quarantine.

`tests/valuation.rs`, over SurrealDB:
- weighted average and FIFO totals rebuilt from the ledger;
- the postings balance, the inventory account equals the valuation, and consignment stock posts nothing.

**Not here yet:**
- persisting the postings, with the financial ledger's storage (billing);
- receiving a requisition's transfer at the ward on screen;
- upward requisitions through procurement adapters at central, and advance shipping notices (`health-integration`);
- central matching of inter-facility dispatches with receipts (#247, with sync);
- the change log and outbox for sync (#135);
- the pharmacy's dispense screens and HTTP routes, and the charge from `inventory.dispensed.v1` (#128 part 2);
- the HTTP API.
