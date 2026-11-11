//! The OS keystore port (desktop-client.md § Secrets; ADR 0040): the device and node keys, tokens
//! and the backup key live in Windows Credential Manager or the Linux Secret Service, called from
//! Rust and never exposed to JavaScript. There is no file fallback: without a working keystore,
//! provisioning is refused. The `keyring` adapter comes with the Tauri binary; [`MemoryKeystore`]
//! is for tests.

use std::collections::BTreeMap;
use std::sync::Mutex;

/// Why the keystore failed.
#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    /// No keystore answers: the install is refused.
    #[error("no OS keystore is available: {0}")]
    Unavailable(String),
    /// The keystore refused the operation.
    #[error("the OS keystore refused: {0}")]
    Refused(String),
}

/// Secrets by name, in the OS keystore.
pub trait Keystore: Send + Sync {
    /// The secret named `name`, if it is held.
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>, KeystoreError>;
    /// Holds `secret` under `name`, replacing any.
    fn set(&self, name: &str, secret: &[u8]) -> Result<(), KeystoreError>;
    /// Forgets `name`; nothing if it was not held.
    fn delete(&self, name: &str) -> Result<(), KeystoreError>;
}

/// Checks that `keystore` can hold, return and forget a secret: provisioning runs it and refuses
/// to install without a working keystore.
pub fn check(keystore: &dyn Keystore) -> Result<(), KeystoreError> {
    const PROBE: &str = "health.keystore-probe";
    keystore.set(PROBE, b"probe")?;
    let read = keystore.get(PROBE)?;
    keystore.delete(PROBE)?;
    match read.as_deref() {
        Some(b"probe") => Ok(()),
        _ => Err(KeystoreError::Unavailable(
            "a secret written could not be read back".into(),
        )),
    }
}

/// A keystore in memory, for tests.
#[derive(Debug, Default)]
pub struct MemoryKeystore(Mutex<BTreeMap<String, Vec<u8>>>);

impl Keystore for MemoryKeystore {
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>, KeystoreError> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(name)
            .cloned())
    }

    fn set(&self, name: &str, secret: &[u8]) -> Result<(), KeystoreError> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(name.to_owned(), secret.to_vec());
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), KeystoreError> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A keystore that loses what it is given, as a broken Secret Service might.
    struct Forgetful;

    impl Keystore for Forgetful {
        fn get(&self, _: &str) -> Result<Option<Vec<u8>>, KeystoreError> {
            Ok(None)
        }
        fn set(&self, _: &str, _: &[u8]) -> Result<(), KeystoreError> {
            Ok(())
        }
        fn delete(&self, _: &str) -> Result<(), KeystoreError> {
            Ok(())
        }
    }

    #[test]
    fn provisioning_needs_a_keystore_that_keeps_secrets() {
        let memory = MemoryKeystore::default();
        check(&memory).expect("a working keystore");
        assert_eq!(
            memory.get("health.keystore-probe").expect("get"),
            None,
            "the probe is gone"
        );
        assert!(matches!(
            check(&Forgetful),
            Err(KeystoreError::Unavailable(_))
        ));
    }
}
