# Vendored: octofhir-fhirpath 0.4.57

- **Source:** the published crate `octofhir-fhirpath = 0.4.57` from crates.io (`src/`, `tests/`, `benches/`, `Cargo.toml`, `README.md`), unchanged in the vendoring commit. The licence texts are from the upstream repository at tag `v0.4.57` (https://github.com/octofhir/fhirpath-rs), because the published crate does not ship them. Licence: MIT OR Apache-2.0, see `LICENSE-MIT` and `LICENSE-APACHE`.
- **Why vendored:** ADR 0052 requires decimals built from the JSON number's text. 0.4.57 converts them through `f64`, so 0.010 becomes 0.0100000000000000002081668171. The workspace pins this copy with `[patch.crates-io]`.
- **Our changes:** listed in `PATCHES.md`, one entry per change. They are offered upstream, and the vendored copy is dropped once a release includes them.
- **Updating:** replace `src/`, `tests/` and `benches/` with the new release in one commit, then re-apply `PATCHES.md` in another, so each diff is reviewable on its own.
