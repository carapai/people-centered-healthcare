# Vendored: octofhir-ucum 0.5.2

- **Source:** the published crate `octofhir-ucum = 0.5.2` from crates.io (`src/`, `build.rs`, `ucum-essence.xml`, `Cargo.toml`, `README.md`), unchanged in the vendoring commit. `octofhir-fhirpath 0.4.57` depends on it.
- **Licence:** Apache-2.0, per the crate's `Cargo.toml`. The published crate does not ship the licence text, and upstream (https://github.com/octofhir/ucum-rs) has no `v0.5.2` tag, so `LICENSE.md` comes from the default branch, as fetched 2026-10-29.
- **Why vendored:** unit lookup and parsing sliced unit codes at byte offsets, which panics on non-ASCII codes such as `m²` in the official R5 examples. A panic in the write path would drop the request. The workspace pins this copy with `[patch.crates-io]`.
- **Our changes:** listed in `PATCHES.md`. They are offered upstream, and the copy is dropped once `octofhir-fhirpath` uses a fixed release.
