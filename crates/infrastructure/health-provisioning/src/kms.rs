//! Wrapping keys for escrow (ADR 0048; security.md § Keys): central's KMS, behind a port until a
//! KMS or HSM product is chosen.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand_core::RngCore;

/// What wraps an escrowed secret: a KMS or HSM key that never leaves it.
pub trait KeyWrap: Send + Sync {
    /// Wraps `secret`, bound to `context`: unwrapping needs the same context.
    fn wrap(&self, secret: &[u8], context: &str) -> Result<Vec<u8>, String>;
    /// Unwraps what [`KeyWrap::wrap`] returned for `context`.
    fn unwrap(&self, wrapped: &[u8], context: &str) -> Result<Vec<u8>, String>;
}

/// The local adapter: XChaCha20-Poly1305 under a 32-byte master key read from a file at start,
/// with a random 24-byte nonce before each ciphertext and the context as associated data.
pub struct LocalKeyWrap {
    cipher: XChaCha20Poly1305,
}

impl LocalKeyWrap {
    /// Under `key`.
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new(key.into()),
        }
    }
}

impl KeyWrap for LocalKeyWrap {
    fn wrap(&self, secret: &[u8], context: &str) -> Result<Vec<u8>, String> {
        let mut nonce = [0u8; 24];
        rand_core::OsRng.fill_bytes(&mut nonce);
        let sealed = self
            .cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: secret,
                    aad: context.as_bytes(),
                },
            )
            .map_err(|_| "the key could not be wrapped".to_owned())?;
        Ok([nonce.as_slice(), &sealed].concat())
    }

    fn unwrap(&self, wrapped: &[u8], context: &str) -> Result<Vec<u8>, String> {
        let (nonce, sealed) = wrapped
            .split_at_checked(24)
            .ok_or_else(|| "the wrapped key is too short".to_owned())?;
        self.cipher
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: sealed,
                    aad: context.as_bytes(),
                },
            )
            .map_err(|_| "the key does not unwrap under this master key and context".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrapped_key_unwraps_only_under_its_master_key_and_context() {
        let kms = LocalKeyWrap::new(&[7; 32]);
        let wrapped = kms.wrap(b"123456-123456", "node/a/system").expect("wrap");
        assert_ne!(
            wrapped,
            kms.wrap(b"123456-123456", "node/a/system").expect("wrap")
        );
        assert_eq!(
            kms.unwrap(&wrapped, "node/a/system").expect("unwrap"),
            b"123456-123456"
        );
        assert!(kms.unwrap(&wrapped, "node/b/system").is_err());
        assert!(
            LocalKeyWrap::new(&[8; 32])
                .unwrap(&wrapped, "node/a/system")
                .is_err()
        );
        assert!(kms.unwrap(&wrapped[..10], "node/a/system").is_err());
    }
}
