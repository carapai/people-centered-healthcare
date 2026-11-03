//! Password hashes (offline-auth.md § Credentials; ADR 0059): plain Argon2id with m = 19456 KiB,
//! t = 2, p = 1, the same on every node, stored encrypted under the node's pepper key with
//! XChaCha20-Poly1305 and the account id as associated data. A copied database yields only
//! ciphertext, and a hash cannot be moved to another account. Without a pepper key (development
//! and tests) the hash is stored as it is.

use argon2::password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use health_identity::{AccountId, PasswordPolicy, PasswordRejected};

use crate::breached::BreachedPasswords;

/// The prefix of a stored credential encrypted under a pepper key.
const SEALED: &str = "xc20p1:";
/// A pepper key's length.
pub const PEPPER_LEN: usize = 32;

/// Hashing failed, or the pepper key is not 32 bytes; never reports the password.
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

/// Hashes passwords, and keeps their hashes under the node's pepper key.
#[derive(Clone)]
pub struct Credentials {
    pepper: Option<XChaCha20Poly1305>,
    /// A hash of nothing, checked when a username is unknown so the response takes as long.
    decoy: String,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials").finish_non_exhaustive()
    }
}

impl Credentials {
    /// The parameters offline-auth.md fixes, with no secret: the hash is the same on every node.
    fn argon2() -> Result<Argon2<'static>, HashError> {
        let params = Params::new(19_456, 2, 1, None).map_err(|_| HashError)?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }

    /// Credentials under `pepper`, a 32-byte key; empty for none.
    pub fn new(pepper: Vec<u8>) -> Result<Self, HashError> {
        let pepper = match pepper.len() {
            0 => None,
            PEPPER_LEN => Some(XChaCha20Poly1305::new_from_slice(&pepper).map_err(|_| HashError)?),
            _ => return Err(HashError),
        };
        let decoy = Self::hash("decoy password, never matched")?;
        Ok(Self { pepper, decoy })
    }

    /// A new password's stored credential for `account`, once it passes `policy` and is not on
    /// `breached` (offline-auth.md § Credentials): the check for every password a user sets or
    /// changes, at central and on an edge alike. `device_factor` is whether the user signs in on
    /// an enrolled device.
    pub fn set_password(
        &self,
        account: AccountId,
        policy: &PasswordPolicy,
        breached: &BreachedPasswords,
        password: &str,
        device_factor: bool,
    ) -> Result<String, SetPasswordError> {
        policy.check(password, device_factor, breached.contains(password))?;
        Ok(self.credential(account, password)?)
    }

    /// The stored credential for `password` on `account`: its hash, sealed.
    pub fn credential(&self, account: AccountId, password: &str) -> Result<String, HashError> {
        self.seal(account, &Self::hash(password)?)
    }

    /// The PHC string for `password`, with a fresh salt: what central provisions to edges.
    pub fn hash(password: &str) -> Result<String, HashError> {
        let salt = SaltString::generate(&mut rand_core::OsRng);
        Self::argon2()?
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|_| HashError)
    }

    /// `phc` as stored for `account`: encrypted under the pepper key, or as it is without one.
    pub fn seal(&self, account: AccountId, phc: &str) -> Result<String, HashError> {
        let Some(cipher) = &self.pepper else {
            return Ok(phc.to_owned());
        };
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let aad = account.to_string();
        let sealed = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: phc.as_bytes(),
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| HashError)?;
        let mut bytes = nonce.to_vec();
        bytes.extend(sealed);
        Ok(format!("{SEALED}{}", URL_SAFE_NO_PAD.encode(bytes)))
    }

    /// The PHC string stored as `stored` for `account`. Refuses a plain hash under a pepper key,
    /// and a sealed one without, so a key is never silently bypassed.
    pub fn open(&self, account: AccountId, stored: &str) -> Option<String> {
        match (&self.pepper, stored.strip_prefix(SEALED)) {
            (None, None) => Some(stored.to_owned()),
            (Some(cipher), Some(encoded)) => {
                let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
                if bytes.len() < 24 {
                    return None;
                }
                let (nonce, sealed) = bytes.split_at(24);
                let aad = account.to_string();
                let phc = cipher
                    .decrypt(
                        XNonce::from_slice(nonce),
                        Payload {
                            msg: sealed,
                            aad: aad.as_bytes(),
                        },
                    )
                    .ok()?;
                String::from_utf8(phc).ok()
            }
            _ => None,
        }
    }

    /// Whether `password` matches the credential stored for `account`, in constant time for a
    /// well-formed hash.
    pub fn verify(&self, account: AccountId, password: &str, stored: &str) -> bool {
        let Some(phc) = self.open(account, stored) else {
            // The same time as a real check, so a corrupt credential is not told apart.
            self.verify_decoy(password);
            return false;
        };
        Self::check(password, &phc)
    }

    fn check(password: &str, phc: &str) -> bool {
        let (Ok(parsed), Ok(argon2)) = (PasswordHash::new(phc), Self::argon2()) else {
            return false;
        };
        argon2.verify_password(password.as_bytes(), &parsed).is_ok()
    }

    /// Spends the time of one verification, for an unknown username.
    pub fn verify_decoy(&self, password: &str) {
        let _ = Self::check(password, &self.decoy);
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    const PEPPER: [u8; 32] = [7; 32];

    fn account() -> AccountId {
        AccountId(Uuid::now_v7())
    }

    #[test]
    fn hashes_are_plain_argon2id_with_the_fixed_parameters() {
        let phc = Credentials::hash("correct horse").expect("hash");
        assert!(phc.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"), "{phc}");
        // The same hash verifies on any node: there is no secret in it.
        let (a, b) = (
            Credentials::new(PEPPER.to_vec()).expect("credentials"),
            Credentials::new([9; 32].to_vec()).expect("credentials"),
        );
        let account = account();
        for node in [&a, &b] {
            let stored = node.seal(account, &phc).expect("seal");
            assert!(node.verify(account, "correct horse", &stored));
            assert!(!node.verify(account, "wrong horse", &stored));
        }
    }

    #[test]
    fn a_stored_hash_is_ciphertext_bound_to_its_account_and_key() {
        let credentials = Credentials::new(PEPPER.to_vec()).expect("credentials");
        let (nakato, okello) = (account(), account());
        let stored = credentials
            .credential(nakato, "correct horse")
            .expect("credential");
        assert!(
            stored.starts_with(SEALED) && !stored.contains("argon2"),
            "{stored}"
        );
        assert!(credentials.verify(nakato, "correct horse", &stored));
        // Moved to another account, or read under another key: refused.
        assert!(!credentials.verify(okello, "correct horse", &stored));
        let other = Credentials::new([9; 32].to_vec()).expect("credentials");
        assert!(!other.verify(nakato, "correct horse", &stored));
        // A key is never bypassed: no plain hash under a key, no sealed one without.
        let plain = Credentials::hash("correct horse").expect("hash");
        assert!(!credentials.verify(nakato, "correct horse", &plain));
        let none = Credentials::new(Vec::new()).expect("credentials");
        assert!(!none.verify(nakato, "correct horse", &stored));
        assert!(none.verify(nakato, "correct horse", &plain));
        assert!(!credentials.verify(nakato, "x", "xc20p1:not base64!"));
    }

    #[test]
    fn a_pepper_key_is_32_bytes() {
        assert_eq!(
            Credentials::new(b"pepper".to_vec()).map(|_| ()),
            Err(HashError)
        );
    }
}
