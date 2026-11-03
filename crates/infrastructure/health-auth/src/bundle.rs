//! An edge's provisioning bundle (ADR 0059; offline-auth.md § Credentials): a facility's staff
//! Login Accounts and their signed permission snapshots, from central to one edge.
//!
//! - **Signed** with central's snapshot key, as a compact JWS of type `health-bundle+jwt`, so an
//!   edge accepts only what central sent, for its facility and its node.
//! - **Hashes encrypted to the edge.** ECDH between a fresh P-256 key (`epk`, in the bundle) and the
//!   edge's facility intermediate public key, HKDF-SHA-256 to a 32-byte key, then each hash under
//!   XChaCha20-Poly1305 with the account id as associated data. Only the edge, whose intermediate
//!   key is sealed in its keystore, can read them; the edge re-encrypts each under its own pepper
//!   key on import.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use health_identity::{AccountId, AccountStatus};
use health_time::Timestamp;
use p256::elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
use p256::{EncodedPoint, PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::snapshot::{SnapshotSigner, SnapshotVerifier};
use crate::store::LocalConfig;

/// The JWS type of a provisioning bundle.
pub const BUNDLE_TYP: &str = "health-bundle+jwt";
/// The bundle format's version.
const VERSION: u32 = 1;

/// A Login Account in a bundle, with its plain Argon2id hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleAccount {
    /// The account id.
    pub id: AccountId,
    /// The username.
    pub username: String,
    /// Active or disabled.
    pub status: AccountStatus,
    /// The plain PHC string (ADR 0059): never stored as it is.
    pub phc: String,
}

/// What central provisions to one edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    /// The facility.
    pub facility: String,
    /// The edge it is for.
    pub node_id: String,
    /// When central made it.
    pub issued_at: Timestamp,
    /// The facility's staff.
    pub accounts: Vec<BundleAccount>,
    /// Their signed permission snapshots (compact JWS), for this facility.
    pub snapshots: Vec<String>,
    /// The country's role mappings and locally grantable roles, for offline administration.
    pub local: LocalConfig,
}

/// Why a bundle could not be made or opened.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BundleError {
    /// Not signed by a trusted central key, or not a bundle.
    #[error("the bundle is not signed by central")]
    Signature,
    /// For another node.
    #[error("the bundle is for {0}, not this node")]
    WrongNode(String),
    /// A hash does not decrypt with this node's key: the bundle is for another intermediate.
    #[error("the bundle's hashes are not encrypted to this node's key")]
    WrongKey,
    /// Malformed.
    #[error("the bundle is malformed: {0}")]
    Malformed(&'static str),
}

#[derive(Serialize, Deserialize)]
struct Claims {
    version: u32,
    facility: String,
    node_id: String,
    issued_at: Timestamp,
    /// The ephemeral public key, SEC1 uncompressed, base64url.
    epk: String,
    accounts: Vec<SealedAccount>,
    snapshots: Vec<String>,
    #[serde(default)]
    local: LocalConfig,
}

#[derive(Serialize, Deserialize)]
struct SealedAccount {
    id: AccountId,
    username: String,
    active: bool,
    /// The nonce, then the ciphertext of the PHC string, base64url.
    hash: String,
}

/// The XChaCha20-Poly1305 key both ends derive.
fn derive(
    shared: &p256::ecdh::SharedSecret,
    node_id: &str,
) -> Result<XChaCha20Poly1305, BundleError> {
    let hkdf = shared.extract::<sha2_010::Sha256>(Some(b"health-bundle-v1"));
    let mut key = [0u8; 32];
    hkdf.expand(node_id.as_bytes(), &mut key)
        .map_err(|_| BundleError::Malformed("key derivation"))?;
    XChaCha20Poly1305::new_from_slice(&key).map_err(|_| BundleError::Malformed("key derivation"))
}

impl Bundle {
    /// The bundle as a JWS signed by `signer`, its hashes encrypted to `recipient`, the edge's
    /// facility intermediate public key.
    pub fn seal(
        &self,
        signer: &SnapshotSigner,
        recipient: &PublicKey,
    ) -> Result<String, BundleError> {
        let ephemeral = p256::ecdh::EphemeralSecret::random(&mut OsRng);
        let cipher = derive(&ephemeral.diffie_hellman(recipient), &self.node_id)?;
        let accounts = self
            .accounts
            .iter()
            .map(|account| {
                let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
                let aad = account.id.to_string();
                let sealed = cipher
                    .encrypt(
                        &nonce,
                        Payload {
                            msg: account.phc.as_bytes(),
                            aad: aad.as_bytes(),
                        },
                    )
                    .map_err(|_| BundleError::Malformed("encryption"))?;
                let mut bytes = nonce.to_vec();
                bytes.extend(sealed);
                Ok(SealedAccount {
                    id: account.id,
                    username: account.username.clone(),
                    active: account.status == AccountStatus::Active,
                    hash: URL_SAFE_NO_PAD.encode(bytes),
                })
            })
            .collect::<Result<_, BundleError>>()?;
        let claims = Claims {
            version: VERSION,
            facility: self.facility.clone(),
            node_id: self.node_id.clone(),
            issued_at: self.issued_at,
            epk: URL_SAFE_NO_PAD.encode(ephemeral.public_key().to_encoded_point(false).as_bytes()),
            accounts,
            snapshots: self.snapshots.clone(),
            local: self.local.clone(),
        };
        let claims = serde_json::to_value(claims).map_err(|_| BundleError::Malformed("claims"))?;
        Ok(signer.sign_as(BUNDLE_TYP, &claims))
    }

    /// The bundle in `jws`, if central signed it for `node_id`, with its hashes decrypted by
    /// `recipient`, this node's facility intermediate key. The snapshots inside are still to be
    /// verified, as every snapshot is at login.
    pub fn open(
        jws: &str,
        verifier: &SnapshotVerifier,
        node_id: &str,
        recipient: &SecretKey,
    ) -> Result<Self, BundleError> {
        let claims: Value = verifier
            .verify_as(BUNDLE_TYP, jws)
            .map_err(|_| BundleError::Signature)?;
        let claims: Claims =
            serde_json::from_value(claims).map_err(|_| BundleError::Malformed("claims"))?;
        if claims.version != VERSION {
            return Err(BundleError::Malformed("an unknown version"));
        }
        if claims.node_id != node_id {
            return Err(BundleError::WrongNode(claims.node_id));
        }
        let point: PublicKey = URL_SAFE_NO_PAD
            .decode(&claims.epk)
            .ok()
            .and_then(|bytes| EncodedPoint::from_bytes(bytes).ok())
            .and_then(|point| Option::from(PublicKey::from_encoded_point(&point)))
            .ok_or(BundleError::Malformed("the ephemeral key"))?;
        let shared = p256::ecdh::diffie_hellman(recipient.to_nonzero_scalar(), point.as_affine());
        let cipher = derive(&shared, node_id)?;
        let accounts = claims
            .accounts
            .into_iter()
            .map(|account| {
                let bytes = URL_SAFE_NO_PAD
                    .decode(&account.hash)
                    .map_err(|_| BundleError::Malformed("a hash"))?;
                if bytes.len() < 24 {
                    return Err(BundleError::Malformed("a hash"));
                }
                let (nonce, sealed) = bytes.split_at(24);
                let aad = account.id.to_string();
                let phc = cipher
                    .decrypt(
                        XNonce::from_slice(nonce),
                        Payload {
                            msg: sealed,
                            aad: aad.as_bytes(),
                        },
                    )
                    .map_err(|_| BundleError::WrongKey)?;
                Ok(BundleAccount {
                    id: account.id,
                    username: account.username,
                    status: if account.active {
                        AccountStatus::Active
                    } else {
                        AccountStatus::Disabled
                    },
                    phc: String::from_utf8(phc).map_err(|_| BundleError::Malformed("a hash"))?,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            facility: claims.facility,
            node_id: claims.node_id,
            issued_at: claims.issued_at,
            accounts,
            snapshots: claims.snapshots,
            local: claims.local,
        })
    }
}
