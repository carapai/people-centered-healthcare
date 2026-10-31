//! Argon2id password hashes (offline-auth.md § Credentials): m = 19456 KiB, t = 2, p = 1, with an
//! optional pepper held outside the database (the OS keystore on edges).

use argon2::password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use health_identity::{PasswordPolicy, PasswordRejected};

use crate::breached::BreachedPasswords;

/// Hashing failed (bad parameters or pepper); never reports the password.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("password hashing failed")]
pub struct HashError;

/// Why a new password was not set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SetPasswordError {
    /// The policy refuses it: too short, or breached.
    #[error(transparent)]
    Rejected(#[from] PasswordRejected),
    /// Hashing failed.
    #[error(transparent)]
    Hash(#[from] HashError),
}

/// Hashes and verifies passwords.
#[derive(Clone)]
pub struct Credentials {
    pepper: Vec<u8>,
    /// A hash of nothing, checked when a username is unknown so the response takes as long.
    decoy: String,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials").finish_non_exhaustive()
    }
}

impl Credentials {
    /// The parameters offline-auth.md fixes.
    fn argon2(pepper: &[u8]) -> Result<Argon2<'_>, HashError> {
        let params = Params::new(19_456, 2, 1, None).map_err(|_| HashError)?;
        if pepper.is_empty() {
            Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
        } else {
            Argon2::new_with_secret(pepper, Algorithm::Argon2id, Version::V0x13, params)
                .map_err(|_| HashError)
        }
    }

    /// Credentials with `pepper` (empty for none).
    pub fn new(pepper: Vec<u8>) -> Result<Self, HashError> {
        let mut credentials = Self {
            pepper,
            decoy: String::new(),
        };
        credentials.decoy = credentials.hash("decoy password, never matched")?;
        Ok(credentials)
    }

    /// A new password's hash, once it passes `policy` and is not on `breached` (offline-auth.md
    /// § Credentials): the check for every password a user sets or changes, at central and on an
    /// edge alike. `device_factor` is whether the user signs in on an enrolled device.
    pub fn set_password(
        &self,
        policy: &PasswordPolicy,
        breached: &BreachedPasswords,
        password: &str,
        device_factor: bool,
    ) -> Result<String, SetPasswordError> {
        policy.check(password, device_factor, breached.contains(password))?;
        Ok(self.hash(password)?)
    }

    /// The PHC string for `password`, with a fresh salt.
    pub fn hash(&self, password: &str) -> Result<String, HashError> {
        let salt = SaltString::generate(&mut rand_core::OsRng);
        Self::argon2(&self.pepper)?
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|_| HashError)
    }

    /// Whether `password` matches `phc`, in constant time for a well-formed hash.
    pub fn verify(&self, password: &str, phc: &str) -> bool {
        let (Ok(parsed), Ok(argon2)) = (PasswordHash::new(phc), Self::argon2(&self.pepper)) else {
            return false;
        };
        argon2.verify_password(password.as_bytes(), &parsed).is_ok()
    }

    /// Spends the time of one verification, for an unknown username.
    pub fn verify_decoy(&self, password: &str) {
        let _ = self.verify(password, &self.decoy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_argon2id_with_the_fixed_parameters() {
        let credentials = Credentials::new(b"pepper".to_vec()).expect("credentials");
        let phc = credentials.hash("correct horse").expect("hash");
        assert!(phc.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"), "{phc}");
        assert!(credentials.verify("correct horse", &phc));
        assert!(!credentials.verify("wrong horse", &phc));
        // The pepper is part of the key: the same hash fails without it.
        assert!(
            !Credentials::new(Vec::new())
                .expect("credentials")
                .verify("correct horse", &phc)
        );
        assert!(!credentials.verify("x", "not a hash"));
    }
}
