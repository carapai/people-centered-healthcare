//! FHIR R5 validation (ADR 0016; fhir-r5-model.md § Validation). Pure: no I/O.
//!
//! - **Tier 1, structure** ([`validate_structure`]) runs on the raw JSON before any typed
//!   parse (ADR 0051), from an element table generated from the core StructureDefinitions. It
//!   does not depend on the model crate.
//! - **Tier 2, core invariants** ([`validate_invariants`]) evaluates the R5 core invariants with
//!   a [`FhirPathEngine`]. The default, [`OctofhirEngine`], is the vendored, decimal-exact
//!   `octofhir-fhirpath` (ADR 0052) over a model provider built from the same element table.
//! - **Tier 3, profiles** ([`SnapshotValidator`], behind [`ProfileValidator`]) checks a
//!   resource against compiled StructureDefinition snapshots ([`ProfileSet`], ADR 0053).
//! - **Tier 4, terminology:** the same walk checks each coded value against its element's
//!   binding through the [`Bindings`] port, which the node's terminology answers. A required
//!   binding not met is an error, an extensible or preferred one a warning
//!   ([`Severity::Warning`]), and a required binding the node cannot check offline a warning.

mod bindings;
mod fhirpath;
mod invariants;
mod profile;
mod questionnaire;
mod structure;

pub use bindings::{Bindings, Membership, Strength};
pub use fhirpath::{
    FhirPathEngine, FhirPathFailure, FhirPathItem, OctofhirEngine, Results, TableModelProvider,
};
pub use invariants::{KNOWN_ENGINE_GAPS, validate_invariants};
pub use profile::{CompileError, ProfileSet, ProfileValidator, SnapshotValidator};
pub use questionnaire::check_response;
pub use structure::{
    Issue, IssueCode, Severity, resource_types, skeleton, validate_structure, validate_value,
};
