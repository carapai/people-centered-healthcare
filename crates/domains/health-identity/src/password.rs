/// Which new passwords are acceptable (offline-auth.md § Credentials).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordPolicy {
    /// The minimum on an enrolled device, whose key is the second factor.
    pub min_with_device_factor: usize,
    /// The minimum where no device factor exists.
    pub min_without_device_factor: usize,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_with_device_factor: 8,
            min_without_device_factor: 15,
        }
    }
}

/// Why a password was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PasswordRejected {
    /// Shorter than the minimum, counted in characters.
    #[error("passwords need at least {minimum} characters")]
    TooShort {
        /// The minimum that applies.
        minimum: usize,
    },
    /// On the breached-password list.
    #[error("this password appears in known breaches")]
    Breached,
}

impl PasswordPolicy {
    /// Checks a new password. `breached` says whether it is on the breached-password list.
    pub fn check(
        &self,
        password: &str,
        device_factor: bool,
        breached: bool,
    ) -> Result<(), PasswordRejected> {
        let minimum = if device_factor {
            self.min_with_device_factor
        } else {
            self.min_without_device_factor
        };
        if password.chars().count() < minimum {
            return Err(PasswordRejected::TooShort { minimum });
        }
        if breached {
            return Err(PasswordRejected::Breached);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_minimum_depends_on_the_device_factor() {
        let policy = PasswordPolicy::default();
        assert_eq!(policy.check("abcdefgh", true, false), Ok(()));
        assert_eq!(
            policy.check("abcdefgh", false, false),
            Err(PasswordRejected::TooShort { minimum: 15 })
        );
        // Characters, not bytes.
        assert_eq!(
            policy.check("ééééééé", true, false),
            Err(PasswordRejected::TooShort { minimum: 8 })
        );
        assert_eq!(
            policy.check("password1234567", false, true),
            Err(PasswordRejected::Breached)
        );
    }
}
