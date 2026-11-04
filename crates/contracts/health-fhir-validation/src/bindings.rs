//! Tier 4, terminology (fhir-r5-model.md § Validation): the port the profile validator asks
//! whether a code is in the value set an element is bound to. The node's terminology service
//! answers it, from its own data (#90), so tier 4 needs no network.

/// Whether a code is in a value set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Membership {
    /// It is.
    In,
    /// It is not.
    NotIn,
    /// The node cannot tell: it does not hold the value set, or cannot expand it offline.
    Unknown(String),
}

/// What answers [`Membership`] questions: the node's terminology.
pub trait Bindings: Send + Sync {
    /// Whether `code`, from `system` when the value carries one, is in value set `value_set`
    /// (`url` or `url|version`). A bare `code` element carries no system.
    fn member(&self, value_set: &str, system: Option<&str>, code: &str) -> Membership;
}

/// How strongly an element is bound (FHIR `binding-strength`); `example` binds nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strength {
    /// Codes must come from the value set: anything else is an error.
    Required,
    /// Codes should come from it when it has a suitable one: anything else is a warning.
    Extensible,
    /// Codes are encouraged to come from it: anything else is a warning.
    Preferred,
}
