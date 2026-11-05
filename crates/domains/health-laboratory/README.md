# health-laboratory

Layer: **domain** (laboratory). The laboratory workflow (#109). The order is a ServiceRequest the orders side owns. Laboratory follows it as a `LabTest`.

- **Stages:**
  - ordered, collected, resulted, verified, reviewed;
  - a specimen may be rejected once collected (it must be ordered again);
  - a test may be cancelled before results;
  - results may be corrected until verified.
- **`flag`:** each result against its ranges, in exact decimals.
  - A quantity is checked against its critical limits first (`LL`, `HH`), then its reference range (`L`, `H`, `N`).
  - A coded or text result is flagged by the analyst (`A`, `AA`).
- **Verification is a second person's:** whoever entered the results cannot verify them (`SameVerifier`).
- **Release:** `LabTest::released` is true only once verified. The clinician never sees unverified results.
- **Critical results:** `LabEvent::Verified` carries `critical`, so a critical result is routed to the clinician's inbox as urgent.
- **Events:** `laboratory.collected.v1`, `resulted`, `verified`, `reviewed` and `closed`.
- **`LabStore`:** the module's port. `health-surrealdb` keeps tests in `lab_test`, migration `0034_lab_tests`.
