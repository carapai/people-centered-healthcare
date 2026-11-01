//! The policy decision point's rules (offline-auth.md § Authorization model; security.md §95–97).
//!
//! Pure decisions, made the same way at central and on an edge:
//!
//! - **Permissions** (RBAC): `{Type|*}.{cruds}` codes resolved from the user's roles.
//! - **Facility isolation:** a facility-scope record is accessed only at its own facility.
//! - **Sensitivity labels:** a record carrying a configured `meta.security` label needs the
//!   matching program assignment, and the access must declare a purpose of use.
//! - **Break-glass:** one patient, one hour, a `BTG` reason and free text; it opens sensitive and
//!   other-facility records of that patient only, and every use is reported for audit.
//!
//! [`Pdp::decide`] answers one access; [`Pdp::search_filter`] gives what a search must exclude,
//! so restricted records are filtered inside the database query.

mod break_glass;
mod decision;
mod facility;
mod patient;
mod permission;
mod purpose;

pub use break_glass::{
    BREAK_GLASS_PERMISSION, BTG, BreakGlass, BreakGlassEvent, BreakGlassPolicy, BreakGlassRefused,
};
pub use decision::{
    Access, Decision, Deny, Label, LabelRule, NodeScope, Obligation, Pdp, PurposeOfUse,
    SearchFilter, Subject,
};
pub use facility::{FACILITY_SCOPE_TYPES, FACILITY_TAG_SYSTEM, is_facility_scope};
pub use patient::PatientSubject;
pub use permission::{Interaction, Permissions};
pub use purpose::{DEFAULT_PURPOSE, PURPOSES_OF_USE, UnknownPurpose, purpose_of_use};
