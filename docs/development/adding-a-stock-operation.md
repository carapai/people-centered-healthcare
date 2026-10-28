# Adding a stock operation

> Status: planned layout. No code exists yet; paths are confirmed when the scaffold lands.

## When to use

A new way stock moves (a new movement type or a new source of an existing one, e.g. consumption from a new clinical event). Every stock change is a **ledger movement** written by an `inventory` use case; nothing edits a balance directly.

## Steps

1. **Map it onto a movement type** where possible: receipt, issue, dispense, transfer-out, transfer-in, return, adjustment (needs a reason code), count variance, expiry write-off, status change. Add a new type only if none fits, and record why in [inventory-and-procurement.md](../architecture/inventory-and-procurement.md).
2. **Movement contents:** signed quantity in base units plus pack size, unit cost, `occurred_at` and `recorded_at`, source document, actor, node and `op_id`. The Stock Unit key is store × product × batch × expiry × funding source × ownership × status.
3. **Use case** in the Supply Chain application crate, one transaction: read the Stock Unit balance record, apply the negative-stock policy, create the movement, update the balance record (on-hand, reserved, available), append `change_log` and an outbox `op`.
4. **Negative stock:** blocked at the authority for issue/transfer/adjustment; dispense only with the emergency-dispense permission; late offline candidates are appended and raise a Reconciliation Record. Store configuration may tighten or relax this.
5. **Triggered by another context?** Subscribe to its event (e.g. MedicationDispense completed). The movement id is UUIDv5 of the source record and derivation rule, so replays never double-post.
6. **Reservations and FEFO** if the operation picks stock: reservations reduce *available*, never *on-hand*.
7. **Corrections are reversals**, never edits or deletes.
8. **Financial postings** only where the store has "inventory accounting" enabled.
9. **FHIR façade**, if any (SupplyDelivery, MedicationDispense, InventoryReport), uses the locally pinned profiles.

## Expected files

- `crates/domains/health-inventory/src/…` (movement rules, policy)
- `crates/application/health-supply-chain-app/src/…` (use case, event subscriber)
- `crates/infrastructure/health-surrealdb/…` (repository; migration if a field is added)

## Contracts

The movement and Stock Unit types; the balance guard record; `kind: op` sync items; the source event from the publishing context's contract crate.

## Tests

- **Unit and domain:** policy and balance arithmetic, FEFO selection.
- **Repository:** guard-record conflicts on the same Stock Unit retry and re-read.
- **Stock concurrency** (nightly soak): concurrent issues and dispenses; balances equal a ledger rebuild.
- **Offline and sync:** late candidates and Reconciliation Records.

## Governing decisions

[ADR 0034](../adr/0034-stock-movement-ledger-with-guarding-balance-records.md), [ADR 0035](../adr/0035-stock-moves-on-fulfilment-with-reservations-and-fefo.md), [ADR 0012](../adr/0012-authority-node-follows-record-scope.md), [inventory-and-procurement.md](../architecture/inventory-and-procurement.md), [write-authority.md](../architecture/write-authority.md).
