# Research: inventory and supply chain

A condensed version of the research ticket [What inventory and supply-chain patterns fit facility stock ledgers?](https://github.com/carapai/people-centered-healthcare/issues/9). The full findings, with citations, are on the unmerged branch `research/inventory`. Decisions: [inventory-and-procurement.md](../architecture/inventory-and-procurement.md) (ADRs 0034, 0035).

## FHIR R5

| Resource | Maturity | Fit |
|---|---|---|
| InventoryItem | FMM 0, Draft | Product and batch view only |
| InventoryReport | FMM 0 | Counts (`snapshot`), summaries (`difference`) |
| SupplyRequest | FMM 1 | Requisitions, one item per request |
| SupplyDelivery | FMM 1 | Receipts and transfers; explicitly not a dispense |

FHIR has no resource for a ledger entry, an adjustment or a reservation.

## Prior art

- **OpenLMIS:**
  - stock cards are keyed facility × program × orderable × lot;
  - stock on hand is computed rather than stored (`CalculatedStockOnHand` is a projection);
  - line items are ordered by occurred date, then processed date, so backdating works;
  - the program in the key separates donor and vertical stock.
- **Open mSupply** (Rust):
  - each `stock_line` has a batch, expiry, pack size and cost, separate `available` and `total` counts, an on-hold flag, and donor, program and campaign ids;
  - movements are a view over invoice lines, so unpicked lines reduce available but are not movements;
  - a `ledger_fix` module detects drift between cached balances and the ledger;
  - each store is editable on exactly one sync site.
- **mSupply:** inventory changes happen only through transactions, and cancellation is a reversing transaction.
- **OpenBoxes:**
  - every warehouse event is an outgoing transaction at the source and an incoming one at the destination;
  - available = on hand − on hold − picked − recalled;
  - picking takes the first-expiring lot;
  - it allows deleting transactions, which we do not copy.

## Standards

- **WHO TRS 1025 Annex 7 (GSDP 2020):** FEFO (§12.30); records are updated after each operation, with periodic reconciliation (§13.1); root cause and CAPA for discrepancies (§13.2); returns go to quarantine (§9.2).
- **IAS 2:** FIFO or weighted average for interchangeable items.
- **GS1:** GTIN (01), expiry (17), batch (10), serial (21) in DataMatrix; SSCC for logistic units; GLN for locations.

## Uganda

- **NMS** supplies public facilities. HC II/III get kits, higher levels order through CSSP against a budget, and FEFO and redistribution are advised.
- **JMS** supplies PNFP facilities.
- **NMS facility tools:** bin cards, issue vouchers, delivery notes, loss and wastage forms, and SOPs.
- **HMIS 105 §6** reports stock status, quantities dispensed and expiries. Reporting completeness was 47% in March 2021.
- **RxSolution** eLMIS was in about 270 higher-level facilities by 2019; its current status is unverified.
- **Unverified:** a GS1 mandate, the HMIS form numbers, the valuation method required for public facilities, and supplier APIs.

## Risks carried into the design

- **FHIR maturity:** the inventory resources are at FMM 0–1, so they are kept as façades.
- **Negative stock** from late offline entries: handled by a policy plus Reconciliation Records.
- **Projection drift:** handled by a nightly rebuild check.
- **Pack-size and base-unit conversion errors:** quantities are stored in base units, with the pack size recorded.
- **Donor and program stock rules** need configuration, not code.
