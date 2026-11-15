# Stock

Stock for staff with `stock.manage` (inventory-and-procurement.md; #130).

- **`StockPage`** (`/stock`): a store's stock on hand and its reorder suggestions.
  - **Stock on hand:** product, batch, expiry, status, on hand, and the last unit cost.
  - **Reorder suggestions:** per product, from the last 30 days' use. When one month or less is left, order up to three months.
- **`ReceivePage`** (`/stock/receive`): a goods received note, with the supplier, the delivery or invoice number, and a line per batch.
  - **Lines:** product, batch, expiry, quantity in base units, pack size, and unit cost.
  - **Barcode:** a GS1 DataMatrix, scanned or pasted, fills a line's batch and expiry. It is never required.
  - **Equipment lines** (ADR 0064): for a product tracked as equipment, one serial number or asset tag per unit instead of a batch and expiry. Each unit becomes an Equipment record, not stock.
- **`RequisitionsPage`** (`/stock/requisitions`, #250): a ward asks another store (the pharmacy) for products.
  - **Supplying store:** it sees the requisition, approves or refuses it, then picks a usable batch for each line from its stock on hand and sends it. The stock leaves as a transfer to the ward.
  - **Requesting store:** it sees each requisition's status.
- **On the server:** `/api/stock` (`health-fhir-server`'s `inventory_router`) checks and records everything.
- **Not yet:** receiving the transfer at the ward (counting it in), and FEFO picking (#128).

- **`ProductsPage`** (#128 follow-up), at `/stock/products`: the product master. It lists each stock product with its base unit and other names, and adds one (code, name, base unit, pack size, other names, and whether it is tracked as equipment). A product's code never changes, nor whether it is equipment, and a duplicate is refused.

- **`EquipmentPage`** (#453; ADR 0064), at `/stock/equipment`: the facility's Equipment, each unit with its location, status and latest change.
  - **Changes:** mark a unit available, faulty, under maintenance or retired, with a note of why (needed unless available); move it to another location; or send it to another facility by its code. Once sent, the unit is that facility's.
  - **Arriving:** a unit sent to this facility shows as in transit, and is received into one of its locations before it can be changed.
  - **History:** each change is a new version of the unit's Device (`Device/{id}/_history`).

## Tests

- `test/stock.test.ts`: the API, line completeness (stock and equipment lines), and the equipment calls.
- `e2e/requisitions.spec.ts`: a ward requisition approved, picked and sent from the pharmacy, which takes it off the pharmacy's stock.
- `e2e/equipment.spec.ts`: a product tracked as equipment received by serial, marked faulty, moved, and sent to another facility.
- `e2e/stock.spec.ts`: a storekeeper receives a delivery with its barcode, and the stock is on hand at its unit cost, with axe on each screen.
