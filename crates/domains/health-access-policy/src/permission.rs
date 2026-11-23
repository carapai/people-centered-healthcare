use std::collections::BTreeSet;

/// One FHIR interaction class, as in SMART v2 (`c r u d s`).
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
    fn letter(self) -> char {
        match self {
            Self::Create => 'c',
            Self::Read => 'r',
            Self::Update => 'u',
            Self::Delete => 'd',
            Self::Search => 's',
        }
    }
}

/// A user's resolved permissions: codes `{Type|*}.{letters}` (e.g. `Observation.rs`,
/// `*.r`), as the role → permission mappings in the configuration package name them. Other
/// codes (`break-glass`, `payment.record`) are kept for the use cases that ask for them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Permissions(pub BTreeSet<String>);

impl Permissions {
    /// Whether `code` is held literally.
    pub fn has(&self, code: &str) -> bool {
        self.0.contains(code)
    }

    /// Whether these permissions give everything `code` gives: it is held literally, or it is a
    /// resource code (`{Type|*}.{letters}`) whose every interaction some held code grants. A
    /// wildcard (`*.r`) is covered only by a held wildcard. An administrator grants only codes
    /// their own session covers (#888).
    pub fn covers(&self, code: &str) -> bool {
        if self.has(code) {
            return true;
        }
        let Some((target, letters)) = code.split_once('.') else {
            return false;
        };
        if letters.is_empty() || !letters.chars().all(|c| "cruds".contains(c)) {
            return false;
        }
        letters.chars().all(|letter| {
            self.0.iter().any(|held| {
                held.split_once('.').is_some_and(|(t, l)| {
                    (t == "*" || t == target)
                        && !l.is_empty()
                        && l.chars().all(|c| "cruds".contains(c))
                        && l.contains(letter)
                })
            })
        })
    }

    /// Whether some code grants `interaction` on `resource_type`.
    pub fn allows(&self, resource_type: &str, interaction: Interaction) -> bool {
        self.0.iter().any(|code| {
            code.split_once('.').is_some_and(|(target, letters)| {
                (target == "*" || target == resource_type)
                    && !letters.is_empty()
                    && letters.chars().all(|c| "cruds".contains(c))
                    && letters.contains(interaction.letter())
            })
        })
    }
}

impl<const N: usize> From<[&str; N]> for Permissions {
    fn from(codes: [&str; N]) -> Self {
        Self(codes.iter().map(|c| (*c).to_owned()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_grant_by_type_and_letter() {
        let p = Permissions::from(["Observation.rs", "*.r", "break-glass", "payment.record"]);
        assert!(p.allows("Observation", Interaction::Search));
        assert!(p.allows("Patient", Interaction::Read));
        assert!(!p.allows("Patient", Interaction::Search));
        assert!(!p.allows("Observation", Interaction::Update));
        assert!(p.has("break-glass"));
        // A use-case code is not a resource grant, even though "record" contains an `r`.
        assert!(!Permissions::from(["payment.record"]).allows("payment", Interaction::Read));
    }

    #[test]
    fn a_code_is_covered_literally_or_by_every_interaction_it_grants() {
        let p = Permissions::from(["Observation.crs", "*.r", "access.administer"]);
        assert!(p.covers("access.administer"));
        assert!(!p.covers("places.manage"));
        assert!(p.covers("Observation.rs"));
        assert!(p.covers("Patient.r"));
        assert!(!p.covers("Patient.rs"));
        assert!(!p.covers("Observation.crud"));
        assert!(
            !p.covers("*.rs"),
            "a wildcard is covered only by a wildcard"
        );
        assert!(Permissions::from(["*.cruds"]).covers("*.rs"));
        // A use-case code is never covered by resource grants.
        assert!(!Permissions::from(["*.cruds"]).covers("payment.record"));
    }
}
