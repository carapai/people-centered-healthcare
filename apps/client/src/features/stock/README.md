# Stock

Stock for staff with `stock.manage` (inventory-and-procurement.md; #130).

- **`StockPage`** (`/stock`): a store's stock on hand and its reorder suggestions.
  - **Stock on hand:** product, batch, expiry, status, on hand, and the last unit cost.
  - **Reorder suggestions:** per product, from the last 30 days' use. When one month or less is left, order up to three months.
- **`ReceivePage`** (`/stock/receive`): a goods received note, with the supplier, the delivery or invoice number, and a line per batch.
  - **Lines:** product, batch, expiry, quantity in base units, pack size, and unit cost.
  - **Barcode:** a GS1 DataMatrix, scanned or pasted, fills a line's batch and expiry. It is never required.
- **`RequisitionsPage`** (`/stock/requisitions`, #250): a ward asks another store (the pharmacy) for products.
  - **Supplying store:** it sees the requisition, approves or refuses it, then picks a usable batch for each line from its stock on hand and sends it. The stock leaves as a transfer to the ward.
  - **Requesting store:** it sees each requisition's status.
- **On the server:** `/api/stock` (`health-fhir-server`'s `inventory_router`) checks and records everything.
- **Not yet:** receiving the transfer at the ward (counting it in), and FEFO picking (#128).

## Tests

- `test/stock.test.ts`: the API and line completeness.
- `e2e/requisitions.spec.ts`: a ward requisition approved, picked and sent from the pharmacy, which takes it off the pharmacy's stock.
- `e2e/stock.spec.ts`: a storekeeper receives a delivery with its barcode, and the stock is on hand at its unit cost, with axe on each screen.
