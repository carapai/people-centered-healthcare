//! The policy decision point's hook into every search (fhir-r5-model.md § v1 search feature set;
//! offline-auth.md).

use crate::query::Clause;

/// Restrictions the policy decision point adds to every search, as clauses ANDed with the
/// request's. They are index clauses like any other (a compartment, a facility, a security
/// label), so the restricted search is still range scans and `total` never counts resources
/// the caller cannot see.
pub trait SearchPolicy: Send + Sync {
    /// The clauses every search of `resource_type` must also satisfy.
    fn clauses(&self, resource_type: &str) -> Vec<Clause>;
}

/// No restriction. Only for system jobs that run without a user (reindex, backup checks); a
/// request on behalf of a user always carries the policy decision point's [`SearchPolicy`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Unrestricted;

impl SearchPolicy for Unrestricted {
    fn clauses(&self, _resource_type: &str) -> Vec<Clause> {
        Vec::new()
    }
}
