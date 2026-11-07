# health-product-registry

Layer: **domain** (product-registry). The product master (resource-ownership.md: product-registry owns product identity, never quantities; inventory-and-procurement.md § Records). Its authority is central.

- **`Product`:** a stock product.
  - Its code is the Stock Unit's `product`.
  - It has a name, GTIN, base unit and pack sizes, the medicine codings it fills, other names (synonyms), and whether it is active.
  - `validate` checks it: the code is a lowercase slug, the name and base unit are present, pack sizes are at least one, and a GTIN has a valid GS1 check digit (`valid_gtin`).
- **`resolve(products, medicine)`:** the stock product a prescribed medicine is dispensed as.
  - A coded medicine matches by its coding first.
  - Otherwise it matches by name or synonym, with case, spacing and punctuation ignored.
  - Two products that fit equally are `Ambiguous`, and none is `Unknown`. Inactive products are never chosen.

Tests: by code first, by name or synonym, ambiguous and unknown, and validation.
