# health-fhir-models

Layer: **contracts**. The FHIR R5 model facade over `fhir-r5 =4.2.3` (ADRs 0015 and 0050; fhir-r5-model.md § Resource model, § In code).

- **Types:** `resources`, `types`, `codes`, `coded` and `validate` are re-exported. Domain crates import them from here, and no other crate may depend on `fhir-r5` (the `facade-bypass` architecture check).
- **JSON:** `parse_resource` and `to_json` are the non-generic entry points for the polymorphic `Resource`, so its serde code is built in this crate only. Use them rather than calling `serde_json` on `Resource` elsewhere.
- **Never the validator or the stored form:** parsing is strict (missing required elements fail) and drops unknown elements and `_id`. Tier 1 validates the raw JSON first, and the stored resource is the validated input (ADR 0051).

## Tests

`tests/round_trip.rs` covers:
- the committed official examples and hand-written edge cases: decimal text, `integer64` as a string beyond 2^53, primitive `_field` extensions with nulls, and unknown codes;
- strictness;
- with `HEALTH_FHIR_R5_EXAMPLES` set, the whole corpus: all 2,824 examples, 2,823 exact and 1 the model crate cannot parse although it is valid R5 (`KNOWN_UNPARSEABLE_EXAMPLES`).

```sh
tools/fhir/fetch-r5-examples.sh            # into target/fhir/r5-examples, SHA-256 checked
HEALTH_FHIR_R5_EXAMPLES=target/fhir/r5-examples/json cargo test -p health-fhir-models
```
