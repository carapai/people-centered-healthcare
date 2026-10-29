# FHIRPath decimals are exact or fail closed: we carry a patched `octofhir-fhirpath`, and stored computed values are explicitly rounded

**Patched engine.** The R5 model-crate spike found that `octofhir-fhirpath` 0.4.57 loses decimal precision. In 14 places it converts a JSON number through `as_f64()` and `Decimal::from_f64_retain`, which adds float noise (0.010 → 0.0100000000000000002…) and drops trailing zeros.
- **The patch.** We build each decimal from the JSON number's text instead, using `serde_json` `arbitrary_precision` and `Decimal::from_str_exact`.
- **Upstream and fork.** The patch is offered upstream. Until it is merged, we carry it as a fork under our organization, pinned to a commit through `[patch.crates-io]` and kept behind the `FhirPathEngine` trait (ADR 0015).
- **If upstream stalls or refuses,** we keep the fork.

**Rules:**
- **Fail closed beyond 28 digits.** `rust_decimal` holds at most 28 significant digits. An expression over a decimal with more digits fails with an evaluation error and never approximates. A failing guard or extraction goes to review. Tier 1 (ADR 0051) still accepts longer external values and stores them unchanged. Our own input widgets cap decimals at 28 significant digits.
- **Scale is kept.** Results keep the source value's scale, so 1.50 stays 1.50. The extractor writes decimal results back as JSON text in that scale.
- **Stored computed values are explicitly rounded.** An Extraction Mapping value expression that does arithmetic, or calls a floating-point function (`sqrt`, `ln`, `log`, `power`, `exp`), must wrap its result in `round(n)`. Otherwise publication fails. Guards and decision tables may use these functions without rounding, because they only compare and never store.
- **The server recomputes browser calculations.** On submit, the server recomputes every `calculatedExpression` that `fhirpath.js` evaluated in the browser (ADR 0038). If the two values differ, the server's value is stored and the divergence is logged.
- **A decimal conformance suite** runs through `FhirPathEngine` on every engine bump, alongside the 1,176 official tests. It covers:
  - trailing zeros, 28-digit values, `-0` and exponent forms;
  - the fail-closed case at 29 digits;
  - comparisons and `~` at precision boundaries;
  - quantity comparison across UCUM units;
  - a corpus property: every decimal in the R5 examples, read by its path, comes back with the same text.

  The suite's decimal fixtures also join the shared fhirpath.js conformance fixtures.

We chose this because FHIRPath computes stored clinical values (extraction), workflow thresholds (guards and decision tables) and invariants. Float noise or lost significant figures in a lab value or dose is a clinical-safety defect, not just a formatting one. The defect is narrow and mechanical, and the engine otherwise passes the official suite, so a patch is far cheaper than any replacement.

## Considered Options

- **`helios-fhirpath`:** runs only over typed Helios models (ADR 0050 chose `fhir-r5`), and it is missing functions. Rejected.
- **Our own FHIRPath engine:** a large rewrite of a conformant engine. Rejected.
- **Wrapping decimals at the facade:** the conversion happens inside the engine, so a wrapper cannot reach it. Rejected.

## Consequences

- We own a small fork until upstream merges, and rebasing it is part of each engine bump.
- The publication gates of Extraction Mappings gain the explicit-rounding check (questionnaire-extraction.md).
- Browser-calculated values are previews. Clinicians see the stored value after submission.
