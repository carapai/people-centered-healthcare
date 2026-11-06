# Pharmacy

The pharmacy (#108, #128; inventory-and-procurement.md § Dispensing; ADR 0035).

- **`PharmacyPage`:** the facility's open prescriptions, oldest first. A prescription moves no stock.
- **Dispensing,** for each prescription:
  - **Prepare:** the pharmacist names the store, the stock product and the quantity in base units. The server reserves stock by first expiry and shows each reserved batch and its expiry. **Nothing moves yet.**
  - **Hand over:** the dispense is completed, and only now is the stock dispensed.
  - **Cancel the dispense:** the reserved stock is free again.
  - Each dispense has a client-generated UUIDv7 id (ADR 0013). Each panel is labelled by medicine and patient.
- **The API** (`api.ts`): `/pharmacy/prescriptions/{id}/prepare`, and `/pharmacy/dispenses/{id}/complete` and `/cancel`.
- **Not yet:** choosing batches by hand, partial hand-over and returns on screen (the API has them), and a product registry to map a medicine to its stock product.

## Tests

`e2e/dispensing.spec.ts`:
- a storekeeper receives stock, and a clinician prescribes;
- the pharmacist prepares, and the stock on hand is unchanged;
- after handing over, it is dispensed;
- axe runs on each screen.
