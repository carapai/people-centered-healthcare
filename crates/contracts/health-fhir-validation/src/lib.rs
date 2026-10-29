//! FHIR R5 validation (ADR 0016; fhir-r5-model.md § Validation). Pure: no I/O.
//!
//! - **Tier 1, structure** ([`validate_structure`]) runs on the raw JSON before any typed
//!   parse (ADR 0051), from an element table generated from the core StructureDefinitions. It
//!   does not depend on the model crate.
//! - Tiers 2–4 (core invariants through FHIRPath, profiles, terminology) arrive with their own
//!   tasks.

mod structure;

pub use structure::{
    Issue, IssueCode, resource_types, skeleton, validate_structure, validate_value,
};
