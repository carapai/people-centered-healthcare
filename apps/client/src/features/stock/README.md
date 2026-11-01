# Stock

Stock for staff with `stock.manage` (inventory-and-procurement.md; #130).

- **`StockPage`** (`/stock`): a store's stock on hand and its reorder suggestions.
  - **Stock on hand:** product, batch, expiry, status, on hand, and the last unit cost.
  - **Reorder suggestions:** per product, from the last 30 days' use. When one month or less is left, order up to three months.
- **`ReceivePage`** (`/stock/receive`): a goods received note, with the supplier, the delivery or invoice number, and a line per batch.
  - **Lines:** product, batch, expiry, quantity in base units, pack size, and unit cost.
  - **Barcode:** a GS1 DataMatrix, scanned or pasted, fills a line's batch and expiry. It is never required.
- **On the server:** `/api/stock` (`health-fhir-server`'s `inventory_router`) checks and records everything.
- **Not yet:** the ward requisition screens (#250).

## Tests

- `test/stock.test.ts`: the API and line completeness.
- `e2e/stock.spec.ts`: a storekeeper receives a delivery with its barcode, and the stock is on hand at its unit cost, with axe on each screen.
