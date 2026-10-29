# Patches to octofhir-fhirpath 0.4.57

Each patch is a commit on top of the unchanged vendoring commit. Its diff is kept in `patches/` so it can be offered upstream (https://github.com/octofhir/fhirpath-rs) and re-applied on an update.

## 0001 — Decimals from the JSON number's text (ADR 0052)

**Problem.** Every place the engine reads a JSON number as a decimal went through `f64` (`as_f64()` plus `Decimal::from_f64`, `from_f64_retain` or `try_from(f64)`). That loses precision and scale: `0.010` became `0.0100000000000000002081668171`, `1.50` lost its trailing zero, and a 30-digit value became `1.2345678901234568e+29`. For lab values and doses this is a clinical-safety defect.

**Change.** A new `core::node::decimal_from_number(&serde_json::Number)` builds a `rust_decimal::Decimal` from the number's text. With `serde_json/arbitrary_precision`, that text is the number as written. It uses `Decimal::from_str_exact` (or `from_scientific` for exponents), keeps the scale, and returns `None` beyond 28 significant digits instead of rounding. `FhirNode::as_decimal` wraps it. Every JSON → decimal read now goes through it:

- `core/value.rs`: `json_to_fhirpath_value`, the model-provider variant (which now errors beyond 28 digits) and Quantity values;
- `core/types.rs`: `json_to_evaluation_result` and the min/max value constraints, which compared via `f64`;
- `evaluator/operations/equals_operator.rs` and `evaluator/evaluator.rs`: Quantity extraction;
- `evaluator/operations/equivalent_operator.rs`;
- `evaluator/result.rs`;
- `evaluator/functions/children_function.rs`;
- `evaluator/functions/weight_function.rs`: ordinal and item-weight `valueDecimal`.

**Not changed.** Writing decimals back into JSON (`serde_json::Number::from_f64` in the `with_property`, `extension` and `quantity` factory functions) still goes through `f64`; those build values for the caller, which the platform does not store. The floating-point functions (`sqrt`, `ln`, `log`, `power`, `exp`) stay `f64` by design; ADR 0052 requires explicit `round(n)` before a stored value uses them.

**Beyond 28 digits.** `json_to_fhirpath_value` cannot return an error, so it still falls back to a string there. `health-fhir-validation` refuses to evaluate a resource holding such a decimal (it fails closed) before any expression runs.
