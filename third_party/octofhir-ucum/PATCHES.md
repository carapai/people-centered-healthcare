# Patches to octofhir-ucum 0.5.2

Each patch is a commit on top of the unchanged vendoring commit, with its diff in `patches/` to offer upstream (https://github.com/octofhir/ucum-rs).

## 0001 — Never slice unit codes inside a character

**Problem.** Four places split or slice a unit code at byte offsets that need not be character boundaries, and panic on non-ASCII codes:
- `performance.rs` `find_unit_optimized`, `lib.rs` `get_common_display`, and the `find_unit` that `build.rs` generates, all of which try every prefix length;
- `evaluator.rs`, which checks 2–3-byte prefixes;
- `parser.rs`, which looks for an implicit exponent after the last non-digit character.

The official R5 examples hold `m²`, so FHIRPath evaluation of a Quantity with such a unit panicked.

**Change.** Skip offsets that are not character boundaries, and step over the whole last non-digit character. ASCII codes, which are all of UCUM's own, behave exactly as before.
