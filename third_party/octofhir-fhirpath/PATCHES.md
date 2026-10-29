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

## 0002–0007 — Correct evaluation of the R5 core invariants

Found by evaluating every R5 core invariant over all 2,824 official examples. Before these patches, hundreds of valid resources failed; after them, every remaining failure is a genuine error in an example, confirmed independently (`health-fhir-validation/tests/invariants.rs`).

- **0002, Quantity elements.** On a Quantity, `.code` returned the human-readable `unit`, and `.system` returned nothing. Navigation also synthesized a `code` from a UCUM-looking `unit`, and a `system` of `http://unitsofmeasure.org`. Now `.code` and `.system` are the element's own values, while UCUM arithmetic still resolves the unit from `unit`. This fixes `qty-3` ("code.empty() or system.exists()") on every coded or unit-only Quantity. Files: `evaluator/evaluator.rs`, `core/types.rs`.
- **0003, `%rootResource`.** The variable was unknown, so `ref-1` errored on every contained reference. It is now an alias of the evaluation root, like `%resource`. File: `evaluator/context.rs`.
- **0004, functions on empty input.** FHIRPath defines these functions on an empty collection, but they used `NullPropagationStrategy::Focus`, so the evaluator returned empty without calling them:
  - `hasValue()` → false;
  - `isDistinct()`, `allTrue()`, `all()` and `subsetOf()` → true;
  - `anyTrue()` → false;
  - `supersetOf()` and `union()` → depending on the argument.

  An empty criterion inside `all()` counts as false, so every "absent or valid" invariant failed when the element was absent (`per-1`, `exp-2`, `eld-13`, …). `subsetOf()` also returned false for `{}.subsetOf({})`. Files: `evaluator/functions/*_function.rs`.
- **0005, choice detection.** A key that starts with the property name followed by a capital letter was taken as a `[x]` variant even when the model names the property as an ordinary element, so `asNeeded` matched `asNeededFor` (`dos-1`). Choice detection now runs only when the model provider does not resolve the property. File: `evaluator/evaluator.rs`.
- **0006, the `not` prefix operator.** It matched `not` with no word boundary, so `notificationEvent` parsed as `not` followed by `ificationEvent` ("Unsupported operator", `sst-1`). It now uses `text::keyword("not")`, as the analysis parser already did. File: `parser/pratt.rs`.
- **0007, `htmlChecks()` attributes.** The other non-active HTML 4 attributes FHIR narratives allow are added: `span` on `col`, `summary`, `rules`, `start`, `value` and others. There are no event handlers or scripts. File: `evaluator/functions/html_checks_function.rs`.

## 0008 — `as` casts FHIR primitives

Found by the vital-signs profile's `vs-1`, `($this as dateTime).toString().length() >= 8`. `is(dateTime)` was true, but `as(dateTime)` returned empty: `as()` matched only the System primitives it converts and complex types by their static `TypeInfo` name, so every FHIR primitive (`dateTime`, `code`, `uri`, …) was dropped. Every official example claiming a vital-signs profile failed. `as()` now accepts a value whenever `is()` would; its conversions for `string`, `integer`, `decimal` and `boolean` are unchanged. Files: `evaluator/functions/type_checking/as_function.rs`, and `is_function.rs`, whose type check becomes a crate-visible associated function.

The engine's own 546 unit tests pass with all patches applied.
