# health-product-registry-app

Layer: **application** (product-registry). The product master as FHIR R5 Medication resources, kept at central.

- **The Medication:**
  - `identifier` holds the product code (`urn:health:product`) and the GTIN (`https://www.gs1.org/gtin`);
  - `code` holds the medicine codings and the name as `text`;
  - `status` is `active` or `inactive`;
  - extensions hold the base unit, each pack size and each synonym.
- **`medication(product, id)`** writes one, **`product(resource)`** reads one back (`None` for a Medication that is not a stock product), and **`products(found)`** reads all of a search.
- **`Products::save(id, product)`:** one transaction.
  - It validates the product.
  - A new product claims its code as a guard (`product|{code}`), so no two share it (`CodeTaken`).
  - A product's code never changes (`CodeChanged`), because Stock Units name products by code.
  - An update is `If-Match` the head read in the same transaction.

Tests: `tests/medication.rs` (valid R5 and a round trip, only stock products read, resolution) and `tests/products.rs` over SurrealDB (saved and updated, codes unique and unchanging, invalid refused).
