//! SMART App Launch v2 scopes (offline-auth.md § Authorization model): the coarse grant carried in
//! a token. Fine-grained decisions are the policy decision point's.

use std::fmt;

/// Whose data a resource scope covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeContext {
    /// The patient in the launch context.
    Patient,
    /// Whatever the user may access.
    User,
    /// A backend client, without a user.
    System,
}

/// One FHIR interaction class, in the SMART v2 order `c r u d s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Interaction {
    /// Create.
    Create,
    /// Read and vread.
    Read,
    /// Update and patch.
    Update,
    /// Delete.
    Delete,
    /// Search and history.
    Search,
}

impl Interaction {
    const ALL: [(char, Self); 5] = [
        ('c', Self::Create),
        ('r', Self::Read),
        ('u', Self::Update),
        ('d', Self::Delete),
        ('s', Self::Search),
    ];
}

/// `{context}/{type or *}.{cruds}[?query]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceScope {
    /// Patient, user or system.
    pub context: ScopeContext,
    /// A resource type, or `None` for `*`.
    pub resource_type: Option<String>,
    /// The interactions granted, in order, without repeats.
    pub interactions: Vec<Interaction>,
    /// A search restriction (`?category=…`), when present.
    pub query: Option<String>,
}

/// One requested or granted scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// `openid`.
    OpenId,
    /// `fhirUser`.
    FhirUser,
    /// `offline_access`.
    OfflineAccess,
    /// `online_access`.
    OnlineAccess,
    /// A resource scope.
    Resource(ResourceScope),
    /// Anything else (launch context, `profile`, …): never granted by this server yet.
    Other(String),
}

/// A malformed resource scope.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("malformed SMART v2 scope {0:?}")]
pub struct ScopeError(pub String);

impl Scope {
    /// Parses one scope. SMART v1 forms (`.read`, `.write`, `.*`) are malformed here: this server
    /// speaks v2 only.
    pub fn parse(text: &str) -> Result<Self, ScopeError> {
        match text {
            "openid" => return Ok(Self::OpenId),
            "fhirUser" => return Ok(Self::FhirUser),
            "offline_access" => return Ok(Self::OfflineAccess),
            "online_access" => return Ok(Self::OnlineAccess),
            _ => {}
        }
        let Some((context, rest)) = text.split_once('/') else {
            return Ok(Self::Other(text.to_owned()));
        };
        let context = match context {
            "patient" => ScopeContext::Patient,
            "user" => ScopeContext::User,
            "system" => ScopeContext::System,
            _ => return Ok(Self::Other(text.to_owned())),
        };
        let malformed = || ScopeError(text.to_owned());
        let (target, permissions) = rest.split_once('.').ok_or_else(malformed)?;
        let (permissions, query) = match permissions.split_once('?') {
            Some((p, q)) if !q.is_empty() => (p, Some(q.to_owned())),
            Some(_) => return Err(malformed()),
            None => (permissions, None),
        };
        let valid_type = target == "*"
            || (target
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
                && target.chars().all(|c| c.is_ascii_alphanumeric()));
        if !valid_type || permissions.is_empty() {
            return Err(malformed());
        }
        // The letters must be a subsequence of "cruds": in order, each at most once.
        let mut interactions = Vec::new();
        let mut next = 0;
        for letter in permissions.chars() {
            let position = Interaction::ALL[next..]
                .iter()
                .position(|(c, _)| *c == letter)
                .ok_or_else(malformed)?;
            interactions.push(Interaction::ALL[next + position].1);
            next += position + 1;
        }
        Ok(Self::Resource(ResourceScope {
            context,
            resource_type: (target != "*").then(|| target.to_owned()),
            interactions,
            query,
        }))
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenId => f.write_str("openid"),
            Self::FhirUser => f.write_str("fhirUser"),
            Self::OfflineAccess => f.write_str("offline_access"),
            Self::OnlineAccess => f.write_str("online_access"),
            Self::Other(text) => f.write_str(text),
            Self::Resource(scope) => {
                let context = match scope.context {
                    ScopeContext::Patient => "patient",
                    ScopeContext::User => "user",
                    ScopeContext::System => "system",
                };
                let letters: String = scope
                    .interactions
                    .iter()
                    .filter_map(|i| {
                        Interaction::ALL
                            .iter()
                            .find(|(_, x)| x == i)
                            .map(|(c, _)| *c)
                    })
                    .collect();
                write!(
                    f,
                    "{context}/{}.{letters}",
                    scope.resource_type.as_deref().unwrap_or("*")
                )?;
                match &scope.query {
                    Some(query) => write!(f, "?{query}"),
                    None => Ok(()),
                }
            }
        }
    }
}

/// A space-separated scope string, parsed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scopes(pub Vec<Scope>);

impl Scopes {
    /// Parses `scope` as sent in an authorization request.
    pub fn parse(text: &str) -> Result<Self, ScopeError> {
        text.split_ascii_whitespace()
            .map(Scope::parse)
            .collect::<Result<_, _>>()
            .map(Self)
    }

    /// Whether `scope` is present.
    pub fn contains(&self, scope: &Scope) -> bool {
        self.0.contains(scope)
    }

    /// The subset this server grants: everything but `Other`.
    #[must_use]
    pub fn granted(&self) -> Self {
        Self(
            self.0
                .iter()
                .filter(|s| !matches!(s, Scope::Other(_)))
                .cloned()
                .collect(),
        )
    }

    /// Whether some scope grants `interaction` on `resource_type` in `context` without a query
    /// restriction. Query-restricted grants are the policy decision point's to apply.
    pub fn permits(
        &self,
        context: ScopeContext,
        resource_type: &str,
        interaction: Interaction,
    ) -> bool {
        self.0.iter().any(|scope| match scope {
            Scope::Resource(r) => {
                r.context == context
                    && r.query.is_none()
                    && r.resource_type
                        .as_deref()
                        .is_none_or(|t| t == resource_type)
                    && r.interactions.contains(&interaction)
            }
            _ => false,
        })
    }
}

impl fmt::Display for Scopes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for scope in &self.0 {
            if !first {
                f.write_str(" ")?;
            }
            first = false;
            scope.fmt(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_resource_scopes_parse_and_print() {
        for text in [
            "user/Patient.cruds",
            "patient/*.rs",
            "system/Observation.s?category=laboratory",
            "user/Encounter.u",
        ] {
            assert_eq!(
                Scope::parse(text).map(|s| s.to_string()).as_deref(),
                Ok(text)
            );
        }
        let Ok(Scope::Resource(scope)) = Scope::parse("user/*.rs") else {
            panic!("a resource scope")
        };
        assert_eq!(
            scope.interactions,
            vec![Interaction::Read, Interaction::Search]
        );
        assert_eq!(scope.resource_type, None);
    }

    #[test]
    fn letters_must_be_in_order_and_v1_forms_are_refused() {
        for text in [
            "user/Patient.sr",
            "user/Patient.rr",
            "user/Patient.read",
            "user/Patient.*",
            "user/patient.r",
            "user/Patient.",
            "user/Patient.r?",
        ] {
            assert!(Scope::parse(text).is_err(), "{text}");
        }
        assert_eq!(
            Scope::parse("launch/patient"),
            Ok(Scope::Other("launch/patient".into()))
        );
    }

    #[test]
    fn permits_matches_type_and_interaction() {
        let Ok(scopes) =
            Scopes::parse("openid user/Patient.rs user/*.c system/Observation.r?code=x")
        else {
            panic!("scopes")
        };
        assert!(scopes.permits(ScopeContext::User, "Patient", Interaction::Search));
        assert!(scopes.permits(ScopeContext::User, "Encounter", Interaction::Create));
        assert!(!scopes.permits(ScopeContext::User, "Patient", Interaction::Delete));
        assert!(!scopes.permits(ScopeContext::Patient, "Patient", Interaction::Read));
        assert!(!scopes.permits(ScopeContext::System, "Observation", Interaction::Read));
        assert_eq!(
            Scopes::parse("openid profile")
                .map(|s| s.granted().to_string())
                .as_deref(),
            Ok("openid")
        );
    }
}
