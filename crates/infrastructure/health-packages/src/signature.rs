//! The detached publication signature (configuration.md § Signing): a compact JWS with its
//! payload detached (RFC 7515 Appendix F), `<header>..<signature>`.
//!
//! - **Header:** `alg` `ES256`, `typ` [`TYP`], `x5c` the publication-signing certificate and any
//!   chain up to (not including) the national root, and `iat` the signing time in seconds.
//! - **Payload:** the tarball's 32-byte sha-256. It is not carried: the verifier hashes the
//!   tarball it has and signs over that, so a changed byte anywhere fails the signature.
//! - **Time:** the certificate is checked at `iat`, not now: it is rotated yearly, and a node
//!   keeps every package it ever activated. A revoked certificate fails at any time, through the
//!   root's CRL. An `iat` in the future is refused.

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use health_pki::Trust;
use health_time::{SignedDuration, Timestamp};
use p256::ecdsa::signature::{Signer as _, Verifier as _};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::pkcs8::DecodePrivateKey as _;
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject as _;
use serde_json::{Value, json};

use crate::{Digest, Error};

/// The JWS `typ` of a package signature.
pub const TYP: &str = "health-package+jws";

/// How far ahead of the verifier's clock `iat` may be.
const SKEW: SignedDuration = SignedDuration::from_mins(5);

/// What signs packages: the publication-signing certificate's key, held in central's HSM or KMS
/// (security.md § Keys), so signing is a port.
pub trait PublicationSigner: Send + Sync {
    /// The publication-signing certificate, then any intermediates, as PEM.
    fn chain_pem(&self) -> &str;
    /// An ES256 signature (P-256, SHA-256) over `input`, as 64 bytes `r || s`.
    fn sign(&self, input: &[u8]) -> Result<[u8; 64], Error>;
}

/// A signer holding the key in memory: for tests and tooling, until the KMS adapter.
pub struct LocalSigner {
    key: SigningKey,
    chain: String,
}

impl LocalSigner {
    /// From the key as PKCS#8 PEM and the certificate chain as PEM.
    pub fn from_pem(key_pem: &str, chain_pem: impl Into<String>) -> Result<Self, Error> {
        Ok(Self {
            key: SigningKey::from_pkcs8_pem(key_pem)
                .map_err(|e| Error::Signature(format!("key: {e}")))?,
            chain: chain_pem.into(),
        })
    }
}

impl PublicationSigner for LocalSigner {
    fn chain_pem(&self) -> &str {
        &self.chain
    }

    fn sign(&self, input: &[u8]) -> Result<[u8; 64], Error> {
        let signature: Signature = self.key.sign(input);
        Ok(signature.to_bytes().into())
    }
}

fn signing_input(header: &str, digest: &Digest) -> String {
    format!("{header}.{}", URL_SAFE_NO_PAD.encode(digest.0))
}

/// Signs the tarball whose sha-256 is `digest`, at `at`.
pub(crate) fn sign(
    digest: &Digest,
    signer: &dyn PublicationSigner,
    at: Timestamp,
) -> Result<String, Error> {
    let x5c: Vec<String> = CertificateDer::pem_slice_iter(signer.chain_pem().as_bytes())
        .map(|der| der.map(|der| STANDARD.encode(der.as_ref())))
        .collect::<Result<_, _>>()
        .map_err(|e| Error::Signature(format!("certificate chain: {e}")))?;
    if x5c.is_empty() {
        return Err(Error::Signature("no certificate".into()));
    }
    let header = json!({ "alg": "ES256", "typ": TYP, "x5c": x5c, "iat": at.as_second() });
    let header = URL_SAFE_NO_PAD.encode(header.to_string());
    let signature = signer.sign(signing_input(&header, digest).as_bytes())?;
    Ok(format!("{header}..{}", URL_SAFE_NO_PAD.encode(signature)))
}

fn pem(der: &[u8]) -> String {
    let body = STANDARD.encode(der);
    let lines: Vec<&str> = body
        .as_bytes()
        .chunks(64)
        .map(|line| std::str::from_utf8(line).unwrap_or_default())
        .collect();
    format!(
        "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
        lines.join("\n")
    )
}

/// Verifies `jws` over the tarball whose sha-256 is `digest`, against `trust`, at `now`.
/// Returns when it was signed.
pub(crate) fn verify(
    jws: &str,
    digest: &Digest,
    trust: &Trust,
    now: Timestamp,
) -> Result<Timestamp, Error> {
    let refused = |why: &str| Error::Signature(why.to_owned());
    let mut parts = jws.trim().split('.');
    let (Some(header_b64), Some(""), Some(signature_b64), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(refused("not a detached compact JWS"));
    };
    let header: Value = URL_SAFE_NO_PAD
        .decode(header_b64)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| refused("unreadable header"))?;
    if header.get("alg").and_then(Value::as_str) != Some("ES256") {
        return Err(refused("alg must be ES256"));
    }
    if header.get("typ").and_then(Value::as_str) != Some(TYP) {
        return Err(refused("not a package signature"));
    }
    if header.get("crit").is_some() {
        return Err(refused("unknown critical header"));
    }
    let signed_at = header
        .get("iat")
        .and_then(Value::as_i64)
        .and_then(|seconds| Timestamp::from_second(seconds).ok())
        .ok_or_else(|| refused("no signing time"))?;
    if signed_at > now.checked_add(SKEW).unwrap_or(now) {
        return Err(refused("signed in the future"));
    }
    let chain: Vec<Vec<u8>> = header
        .get("x5c")
        .and_then(Value::as_array)
        .and_then(|certificates| {
            certificates
                .iter()
                .map(|c| c.as_str().and_then(|c| STANDARD.decode(c).ok()))
                .collect::<Option<Vec<_>>>()
        })
        .filter(|chain| !chain.is_empty())
        .ok_or_else(|| refused("no certificate"))?;
    let chain_pem: String = chain.iter().map(|der| pem(der)).collect();
    trust
        .verify_publisher(&chain_pem, signed_at)
        .map_err(|e| Error::Untrusted(e.to_string()))?;
    let leaf = health_pki::inspect(&chain_pem).map_err(|e| Error::Untrusted(e.to_string()))?;
    let key = VerifyingKey::from_sec1_bytes(&leaf.public_key)
        .map_err(|_| refused("the certificate's key is not P-256"))?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature_b64)
        .ok()
        .and_then(|bytes| Signature::from_slice(&bytes).ok())
        .ok_or_else(|| refused("unreadable signature"))?;
    key.verify(signing_input(header_b64, digest).as_bytes(), &signature)
        .map_err(|_| refused("the signature does not match the package"))?;
    Ok(signed_at)
}
