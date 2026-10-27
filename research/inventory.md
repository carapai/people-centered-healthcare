# Research: inventory and supply-chain patterns for facility stock ledgers

Ticket: #9 · Researched 2026-10-27 · Spec anchor: `MASTER_PROMPT.md` Part 11 (§71–73), §87 sync authority table ("Facility stock: facility stock ledger authoritative for its scope; central aggregates and reconciles"), §89 ("Stock → transaction ledger reconciliation").

## Summary (answer)

Use an **append-only stock movement ledger keyed at the stock-unit level (store × product × batch/lot × expiry × funding source/program)**, with **derived, rebuildable balances** (on-hand, reserved, available) kept as projections, not as editable truth. That is the shape of every mature open-source health LMIS checked here: OpenLMIS keeps one stock card per facility × program × orderable × lot and computes stock on hand from ordered line items. Open mSupply stores each batch as a `stock_line` with separate `total` and `available` pack counts, builds its ledger as a view over invoice lines, and ships a "ledger fix" module that finds drift between cached balances and the ledger. OpenBoxes models every warehouse event as transactions and refreshes a derived `ProductAvailability` table. Use **FEFO** for allocation (WHO GSDP 12.30). Value stock at **FIFO or weighted average** (IAS 2), with costs carried on each movement. **Reservations** should be separate allocation records that reduce *available* but never *on-hand*. For offline operation, follow the Open mSupply rule that a store is writable on exactly one site: the **Facility Edge is the sole writer for its stores' ledgers**, central receives an idempotent, append-only movement stream and *reconciles* (never merges balances). FHIR R5 inventory resources are low-maturity (InventoryItem FMM 0 Draft, InventoryReport FMM 0, SupplyRequest/SupplyDelivery FMM 1). Use them as **interchange façades**, not as ledger storage.

## Findings by source

### FHIR R5 inventory and supply resources

| Resource | Maturity / status | Fit |
|---|---|---|
| InventoryItem | FMM 0, **Draft**; "functional description of an inventory item used in inventory and supply-related workflows"; `instance` carries `lotNumber`, `expiry`, `location`; `inventoryStatus` (recalled, in use, discarded…) — https://hl7.org/fhir/R5/inventoryitem.html | Product/batch master façade only; too immature to persist as canonical ledger state |
| InventoryReport | FMM 0, Trial Use; `countType` = `snapshot` \| `difference`; `inventoryListing` by location/status/countingDateTime with item+quantity — https://hl7.org/fhir/R5/inventoryreport.html | Good fit for **stock counts (snapshot)** and for **edge→central/DHIS2 summary exchange (difference)** |
| SupplyRequest | FMM 1, Trial Use; one item per request; `deliverFrom`/`deliverTo`; `basedOn` added in R5 — https://hl7.org/fhir/R5/supplyrequest.html | Requisitions / internal orders (ward → pharmacy, facility → NMS) |
| SupplyDelivery | FMM 1, Trial Use; records "actual movement of supplies… from one location to another" incl. internal transfers; `suppliedItem` repeats in R5; explicitly *not* a dispense (MedicationDispense/DeviceDispense cover that) — https://hl7.org/fhir/R5/supplydelivery.html | Receipts and transfers façade; patient issues come from **MedicationDispense**, consistent with spec §73 |

FHIR has **no resource for an individual ledger entry, an adjustment, or a reservation**. Those must be internal domain records, optionally exposed via the above or via Basic/extension (design choice, not researched further).

### OpenLMIS (v3 stock management service)

- `StockCard` is unique per **facilityId × programId × orderableId × lotId** (index on facility, program, orderable). `stockOnHand` is `@Transient`, so it is computed rather than stored on the card. Line items are sorted by **occurredDate → processedDate → reason priority → id**, which allows backdated entries and recalculation. Source: https://github.com/OpenLMIS/openlmis-stockmanagement/blob/master/src/main/java/org/openlmis/stockmanagement/domain/card/StockCard.java
- A separate `CalculatedStockOnHand` table stores the balance per stock card per (occurredDate, processedDate): a materialised running-balance projection. Source: https://github.com/OpenLMIS/openlmis-stockmanagement/blob/master/src/main/java/org/openlmis/stockmanagement/domain/event/CalculatedStockOnHand.java
- Stock cards and line items carry an `originEvent` (a `StockEvent`). Movements enter as events and are projected into cards. Reasons (consumption, receipt, kit unpack) are configured entities. Source: https://docs.openlmis.org/en/latest/components/stockmanagementService.html
- Physical inventory has a draft → submit workflow (stock count): https://openlmis.atlassian.net/wiki/spaces/OP/pages/113345646/Enter+Stock+Data
- **Program** in the stock-card key is how OpenLMIS separates vertical/donor programmes. This is directly relevant to Uganda's donor-funded stock.

### mSupply / Open mSupply

- Classic mSupply: "changes to inventory are only possible through a transaction", batches are tracked separately, and issue is FEFO-sorted. Cancelling a supplier invoice creates a **reversing transaction** instead of a delete. Finalising a customer invoice to another store auto-creates the receiving store's supplier invoice. Sources: https://docs.msupply.org.nz/receiving_goods:supplier_invoices, https://docs.msupply.org.nz/other_stuff:transaction_status_codes
- Open mSupply (Rust, the same stack family as this project): `stock_line` has `batch`, `expiry_date`, `pack_size`, `cost_price_per_pack`, **`available_number_of_packs` vs `total_number_of_packs`**, `on_hold`, `location_id`, **`donor_id`, `program_id`, `campaign_id`**. Source: https://github.com/msupply-foundation/open-msupply/blob/develop/server/repository/src/db_diesel/stock_line_row.rs
- Its `stock_movement` is a **SQL view over invoice lines**, timestamped by picked/received/verified datetime per invoice type. Unpicked draft lines therefore do not count as movements, but they do reduce `available`. Source: https://github.com/msupply-foundation/open-msupply/blob/develop/server/repository/src/migrations/views/stock_movement.rs
- It has a `ledger_fix` module (`find_ledger_discrepancies.rs`) that detects stock lines whose cached balance disagrees with the ledger. This is evidence that **cached balances drift** even in a mature product. Source: https://github.com/msupply-foundation/open-msupply/blob/develop/server/service/src/ledger_fix/find_ledger_discrepancies.rs
- Sync authority: each store is **active on exactly one sync site**, the only place its data can be edited. Central keeps a read-only collector copy. Sources: https://docs.msupply.org.nz/synchronisation:sync_sites, https://docs.msupply.org.nz/synchronisation:introduction

### OpenBoxes

- "All warehouse events… have two transactions (an incoming transaction for the destination warehouse and an outgoing transaction for the source warehouse)". Transactions carry entries, a type (order, transfer, stock count), and supervisor confirmation. Saving or deleting a transaction publishes `RefreshProductAvailabilityEvent`, which updates a derived `ProductAvailability` table. Source: https://github.com/openboxes/openboxes/blob/develop/grails-app/domain/org/pih/warehouse/inventory/Transaction.groovy
- Quantity available = on hand − on hold − picked − recalled ("ATP"). Picking allocates the **first-expiring lot**, with ties going to the smallest bin quantity. Hold bins block use. Sources: https://openboxes.atlassian.net/wiki/x/AQBqZw (content summarised by search; direct fetch returned empty, so **partially unverified**), https://openboxes.helpscoutdocs.com/article/68-stock-transfers
- Caution: OpenBoxes allows transaction **delete** (`afterDelete` hook). That is weaker than append-only and should not be copied.

### GS1 identifiers

- Healthcare AI set: **(01) GTIN** N14, **(17) expiry** YYMMDD, **(10) batch/lot** X..20 (must accompany a GTIN), **(21) serial**, carried in GS1 DataMatrix. **(00) SSCC** identifies logistic units (pallets/cartons). **(410)/GLN** identifies locations and parties. Sources: https://www.gs1.org/sites/default/files/docs/barcodes/GS1%20Application%20Identifiers.pdf, https://www.gs1.org/sites/gs1/files/2021-07/gs1_general_specifications.pdf (2021 release; current release not checked), https://www.gs1uk.org/knowledge-hub/how-do-i-capture-a-gtin-and-production-information-pi-in-a-barcode
- Uganda NDA/NMS mandating GS1 barcodes on medicines: **unverified** (no primary source found).

### Uganda supply chain

- NMS supplies public facilities (RRH, GH, HC IV, III, II). Higher levels order against annual procurement plans, HC II/III receive district-specific basic kits, supply is limited by per-facility budget, and ordering uses the **CSSP** portal. NMS advises facilities to follow FEFO and to redistribute excess via districts. Sources: https://nms.go.ug/health-facility-faqs/, https://nms.go.ug/?p=6
- NMS facility tools: **bin cards (stock cards)**, **issue vouchers & delivery notes**, emergency order form, consumption report templates, **loss/wastage report forms**, the Health Facility Stock Management Manual, and the 2019 SOP manual for supply chain at facility level. Source: https://nms.go.ug/resources-guides/
- JMS serves private-not-for-profit (faith-based) facilities: https://www.childhealthtaskforce.org/sites/default/files/2018-12/Uganda%20PSM%20Mission%20Report%202015_Nairobi%20iCCM%20Meeting%202016.pdf
- **HMIS 105 section 6** in DHIS2 reports stock status, quantities dispensed, and expiries. Only 47% of 4,520 facilities reported it in March 2021. Source: https://msh.org/story/active-leadership-involvement-drives-up-health-facility-stock-status-reporting-and-visibility/. Exact data elements (e.g. "days out of stock", tracer list): **unverified**.
- **RxSolution** (MSH/USAID UHSC eLMIS) handles inventory, purchase orders, issues and dispensing, and was deployed in ~270+ higher-level facilities by 2019. Sources: https://msh.org/resources/scale-up-of-rxsolution-elmis-in-uganda/, https://news.mak.ac.ug/2019/11/mak-launches-rxsolution-take-health-services-top. Current (2026) status and export formats: **unverified**.
- HMIS form numbers for the stock card / requisition & issue voucher (e.g. "HMIS 015/017"): **unverified**.

### FEFO, stock control, valuation

- WHO TRS 1025 Annex 7 (GSDP, 2020): FEFO defined as distributing "stock with the earliest expiry date… before an identical stock item with a later expiry date". §12.30 says "The 'first expired/first out' (FEFO) principle should be followed". §13.1 says stock records must be "updated after each operation (e.g. entries, issues, losses, adjustments)" and "Periodic stock reconciliation should be performed… by comparing the actual and recorded stock". §13.2 requires root cause and CAPA for discrepancies. Returns go to quarantine (§9.2). Source: https://www.who.int/publications/m/item/trs-1025-annex-7 (text read from https://www.gmp-compliance.org/files/guidemgr/TRS1025_Annex7.pdf)
- IAS 2: specific identification for non-interchangeable items, and **FIFO or weighted average** for interchangeable items. Source: https://www.ifrs.org/issued-standards/list-of-standards/ias-2-inventories/. LIFO is prohibited under IAS 2 per common knowledge, but the summary page did not state it: **unverified here**. Uganda public-sector valuation rules: **unverified**.

### Concurrency

- SurrealDB: every statement runs in its own transaction, and a transaction "always commits or cancels as a whole". Source: https://surrealdb.com/docs/surrealql/transactions. Isolation level and write-conflict detection semantics were **not documented on that page (unverified)**. The SurrealDB research ticket should confirm this before relying on DB-level guards against negative stock.
- Prior art pattern (OpenLMIS, Open mSupply): availability is checked and decremented in the same transaction as the movement or allocation write, on the single authoritative node.

## Implications for the decision

### Ledger model
1. **Stock unit key** = `store/location × product (internal id; GTIN optional) × batch × expiry × funding source (program/donor) × ownership (owned | consignment) × status (usable | quarantine | restricted)`. This mirrors OpenLMIS's program+lot key and Open mSupply's donor/program/on_hold fields, and covers §71's donor-funded, restricted and consignment stock.
2. **Append-only movement entries** (receipt, issue, dispense, transfer-out/in, return, adjustment+reason, count-variance, expiry write-off, status change), each with signed quantity in base units, unit cost, `occurred_at` and `recorded_at`, source document reference, actor, node and operation id. Corrections are **reversals**, never edits or deletes (mSupply cancel pattern, not the OpenBoxes delete).
3. **Balances are projections**: on-hand per stock unit, a running balance by occurred date (as OpenLMIS `CalculatedStockOnHand` does), available = on-hand − reserved − on-hold. They must be fully rebuildable, with a scheduled **ledger-vs-projection discrepancy check** (Open mSupply `ledger_fix` shows drift happens).
4. **Transfers are two linked movements** (out at source, in at destination), with goods in transit between them (OpenBoxes, mSupply inter-store invoices).
5. **Stock counts** are a draft → submit document whose submission posts variance adjustments. Export as FHIR `InventoryReport` (snapshot).
6. **Valuation**: store cost on each receipt layer per batch. Support weighted average (simplest, a per-store/product projection) and FIFO (consume receipt layers) as a configurable policy. The ledger must not depend on the valuation method, which is a pure projection.

### Reservations
- Separate **allocation records** (store × stock unit × qty × reason/ref × expiry-of-hold), created by dispensing or picking workflows, **never** by MedicationRequest (§73). They reduce *available* only. A movement is posted at pick/dispense confirmation (Open mSupply: unpicked lines are not movements). Reservations need TTL/release to avoid phantom stock-outs.
- FEFO allocator: earliest expiry among usable, non-held, policy-eligible (programme/donor) stock units, with manual override recorded with a reason (OpenBoxes override pattern).

### Offline reconciliation
- **Single writer per store**: the Facility Edge owning a store is the only node that posts its ledger (Open mSupply "store active on one site", spec §87). Tauri standalone devices or LAN clients that act offline submit **movement commands with operation ids** to the edge. The edge validates them against its ledger and may accept them, reject them, or accept them flagged (e.g. negative-stock).
- Edge → central: push the immutable movement stream (idempotent by operation id, ordered by per-node sequence). Central **re-derives** balances and never accepts balance overwrites. Inter-facility transfers reconcile out↔in pairs, and unmatched or variance pairs go to a conflict queue (§89).
- Backdated movements (occurred < recorded) require recomputation of running balances from that date (OpenLMIS ordering by occurredDate then processedDate).
- Reporting: derive HMIS 105 §6 values (stock on hand, dispensed, expired) and NMS/JMS order quantities from the ledger. FHIR `InventoryReport(difference)` and `SupplyRequest` are candidate exchange formats.

### Risks
- FHIR inventory resources (FMM 0–1) may change in R6, so keep them at the façade/mapping layer.
- Negative stock from offline or late entries: decide whether the policy is hard-block, allow-and-flag, or per-store.
- Projection drift and rebuild cost on large ledgers: snapshot checkpoints are needed.
- SurrealDB isolation semantics for check-and-decrement are unconfirmed, so serialising writes per stock unit in the edge service may be needed.
- Unit-of-measure / pack-size conversions (pack vs base unit) are a classic source of error. Open mSupply stores `pack_size` per line.
- Donor and programme stock rules (who may consume what) are policy and need configuration, not hard-coding.

### Open questions
1. Exact HMIS 105 §6 data elements and the tracer medicine list for Uganda in 2026 (unverified).
2. Whether NMS CSSP or JMS expose APIs or file formats for orders, delivery notes and ASNs. If they do, can receipts be pre-populated (unverified)?
3. Whether NDA/NMS require GS1 DataMatrix on medicines in Uganda (unverified), which affects scan-to-receive and scan-to-dispense.
4. The valuation method mandated for Ugandan public/PNFP facilities (unverified). Is valuation needed at HC II/III at all?
5. Whether to interoperate with or migrate from RxSolution at existing hospitals (current footprint unverified).
6. Policy on negative stock and on offline LAN-less Tauri issuing against an edge-owned store.
