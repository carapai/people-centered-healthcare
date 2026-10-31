//! Signing and verifying permission snapshots (ADR 0011). Central signs each snapshot as a
//! compact ES256 JWS (`typ` `snapshot+jwt`) whose payload is the snapshot; an edge verifies it
//! with central's snapshot key before any login, so a changed byte means no login.

use health_identity::snapshot::{PermissionSnapshot, RolePermissions, SnapshotPolicy};
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{SigningKey, VerifyingKey};
use serde_json::{Value, json};

use crate::jose::{EcJwk, JoseError, sign, split};

const TYP: &str = "snapshot+jwt";

/// A snapshot could not be signed or verified.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SnapshotError {
    /// Not a snapshot JWS, or signed by a key we do not trust.
    #[error("invalid snapshot signature or format")]
    Invalid,
    /// It names a role the configuration does not define.
    #[error(transparent)]
    UnknownRole(#[from] health_identity::snapshot::UnknownRole),
}

impl From<JoseError> for SnapshotError {
    fn from(_: JoseError) -> Self {
        Self::Invalid
    }
}

/// Central's snapshot-signing key.
#[derive(Clone)]
pub struct SnapshotSigner {
    key: SigningKey,
    kid: String,
}

impl std::fmt::Debug for SnapshotSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnapshotSigner")
            .field("kid", &self.kid)
            .finish_non_exhaustive()
    }
}

impl SnapshotSigner {
    /// A signer over `key`; its `kid` is the key's thumbprint.
    pub fn new(key: SigningKey) -> Self {
        let kid = EcJwk::of(key.verifying_key()).thumbprint();
        Self { key, kid }
    }

    /// A fresh random key (tests and development).
    pub fn generate() -> Self {
        Self::new(SigningKey::random(&mut rand_core::OsRng))
    }

    /// The public key edges are provisioned with.
    pub fn public(&self) -> EcJwk {
        EcJwk::of(self.key.verifying_key())
    }

    /// Signs `snapshot` after checking its roles against `roles`.
    pub fn sign(
        &self,
        snapshot: &PermissionSnapshot,
        roles: &RolePermissions,
    ) -> Result<String, SnapshotError> {
        roles.validate(snapshot)?;
        let claims = serde_json::to_value(snapshot).map_err(|_| SnapshotError::Invalid)?;
        Ok(sign(
            &self.key,
            &json!({ "alg": "ES256", "typ": TYP, "kid": self.kid }),
            &claims,
        ))
    }
}

/// Verifies snapshots against central's snapshot keys (more than one during a rotation).
#[derive(Debug, Clone)]
pub struct SnapshotVerifier {
    keys: Vec<(String, VerifyingKey)>,
}

impl SnapshotVerifier {
    /// Trusts `keys`.
    pub fn new(keys: &[EcJwk]) -> Result<Self, SnapshotError> {
        let keys = keys
            .iter()
            .map(|jwk| Ok((jwk.thumbprint(), jwk.key()?)))
            .collect::<Result<_, JoseError>>()?;
        Ok(Self { keys })
    }

    /// The snapshot in `jws`, if a trusted key signed exactly these bytes.
    pub fn verify(&self, jws: &str) -> Result<PermissionSnapshot, SnapshotError> {
        let (header, claims, input, signature) = split(jws)?;
        if header["alg"] != "ES256" || header["typ"] != TYP {
            return Err(SnapshotError::Invalid);
        }
        let (_, key) = self
            .keys
            .iter()
            .find(|(kid, _)| header["kid"] == kid.as_str())
            .ok_or(SnapshotError::Invalid)?;
        key.verify(input.as_bytes(), &signature)
            .map_err(|_| SnapshotError::Invalid)?;
        serde_json::from_value::<PermissionSnapshot>(claims).map_err(|_| SnapshotError::Invalid)
    }
}

/// What an edge's authorization server checks before a login or refresh (offline-auth.md): a
/// snapshot for this facility, signed by central, unexpired at a clock that has not gone back.
#[derive(Debug, Clone)]
pub struct SnapshotGate {
    /// The facility this node serves (an edge), or `None` where the user chooses it at sign-in
    /// among the facilities they hold snapshots for (central, ADR 0056).
    pub facility: Option<String>,
    /// Central's snapshot keys.
    pub verifier: SnapshotVerifier,
    /// The country's lifetimes.
    pub policy: SnapshotPolicy,
}

/// The claims a snapshot adds to tokens.
pub(crate) fn token_claims(snapshot: &PermissionSnapshot) -> Value {
    json!({ "facility": snapshot.facility, "snapshot_version": snapshot.snapshot_version })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use health_identity::AccountId;
    use health_identity::snapshot::RoleAssignment;
    use health_time::Timestamp;

    use super::*;
    use crate::jose::b64;

    fn snapshot() -> PermissionSnapshot {
        PermissionSnapshot {
            account: AccountId(uuid::Uuid::now_v7()),
            facility: "F1".into(),
            roles: vec![RoleAssignment {
                role: "clinician".into(),
                locations: vec![],
            }],
            programs: vec!["hiv".into()],
            issued_at: Timestamp::UNIX_EPOCH,
            not_after: Timestamp::from_second(86_400).unwrap_or(Timestamp::UNIX_EPOCH),
            snapshot_version: 3,
        }
    }

    fn roles() -> RolePermissions {
        RolePermissions(BTreeMap::from([(
            "clinician".into(),
            BTreeSet::from(["record.read".into()]),
        )]))
    }

    #[test]
    fn a_signed_snapshot_verifies_and_any_change_is_detected() {
        let signer = SnapshotSigner::generate();
        let verifier = SnapshotVerifier::new(&[signer.public()]).expect("verifier");
        let jws = signer.sign(&snapshot(), &roles()).expect("sign");
        assert_eq!(verifier.verify(&jws).map(|s| s.snapshot_version), Ok(3));

        // Tampering: an extra program in the payload, signature unchanged.
        let mut parts: Vec<String> = jws.split('.').map(str::to_owned).collect();
        let mut forged = snapshot();
        forged.programs.push("mental-health".into());
        parts[1] = b64(serde_json::to_string(&forged).expect("json").as_bytes());
        assert_eq!(
            verifier.verify(&parts.join(".")),
            Err(SnapshotError::Invalid)
        );

        // Another key, or an access token, is not a snapshot.
        let other =
            SnapshotVerifier::new(&[SnapshotSigner::generate().public()]).expect("verifier");
        assert_eq!(other.verify(&jws), Err(SnapshotError::Invalid));
        let token =
            crate::TokenSigner::generate().sign(&serde_json::to_value(snapshot()).expect("json"));
        assert_eq!(verifier.verify(&token), Err(SnapshotError::Invalid));
    }

    #[test]
    fn central_refuses_to_sign_an_unknown_role() {
        let mut s = snapshot();
        s.roles.push(RoleAssignment {
            role: "ghost".into(),
            locations: vec![],
        });
        assert!(matches!(
            SnapshotSigner::generate().sign(&s, &roles()),
            Err(SnapshotError::UnknownRole(_))
        ));
    }
}
