//! The private PKI (lan-security.md; ADRs 0008, 0058).
//!
//! ```text
//! national root                   offline; 25 y; CA, path length 2
//! ├── facility issuing CA         central HSM/KMS; 10 y, issuing for the first 5; CA, path
//! │   │                           length 1; serverAuth; names: .local, the facility DNS
//! │   │                           suffixes, the private IP ranges
//! │   └── facility intermediate   the edge's keystore; 5 y; CA, path length 0; serverAuth;
//! │       │                       names: <node-id>.local, its configured names, private IPs
//! │       └── edge server leaf    397 d; serverAuth; the node's names and addresses
//! └── publication signing         central HSM/KMS; not a CA; code signing only
//! ```
//!
//! - **[`Authority`]** is a CA whose key is at hand: the root at a ceremony, the issuing CA at
//!   central, the facility intermediate on its edge. It issues only what its tier may.
//! - **Issued from a request.** Everything below the root is issued from a PKCS#10 request. Only
//!   the request's public key is used; the subject, names, lifetime and extensions come from the
//!   issuer, so a request cannot ask for more, and the key never leaves its holder.
//! - **Refused, not trimmed.** A name outside the issuer's constraints, a public IP, or a
//!   lifetime past the issuer's own is refused at issuance, so nothing is issued that a
//!   verifier would reject later. This is also what limits the issuing CA to its first 5 years.
//! - **[`Revocations`]** is an issuer's revocation list, published as a CRL: the root's covers
//!   issuing CAs and publication signing, the issuing CA's covers facility intermediates. CRLs
//!   travel in the sync revocation lane.
//! - **[`Trust`]** verifies a chain against the root and the CRLs a node holds.
//! - **[`inspect`]** reads a certificate's validity and names back, so an edge knows when to
//!   renew its leaf.
//!
//! Every key is ECDSA P-256 with SHA-256. Certificates are backdated by
//! [`BACKDATE_SECONDS`] so a verifier whose clock runs slightly behind still accepts them.

mod authority;
mod inspect;
mod revocation;
mod trust;

pub use authority::{Authority, Facility, Issued, IssuingCa, Serial, Tier, new_key, request};
pub use inspect::{Inspected, inspect};
pub use rcgen::KeyPair;
pub use revocation::{Reason, Revocations, Revoked};
pub use trust::Trust;

/// The national root's lifetime.
pub const ROOT_YEARS: i32 = 25;
/// The facility issuing CA's lifetime; it issues only while a facility intermediate still fits.
pub const ISSUING_CA_YEARS: i32 = 10;
/// A facility intermediate's lifetime.
pub const FACILITY_YEARS: i32 = 5;
/// An edge server leaf's lifetime, inside Apple's 825-day ceiling for user-added roots.
pub const LEAF_DAYS: i64 = 397;
/// A publication-signing certificate's lifetime: rotated yearly, with a month's overlap.
pub const PUBLICATION_DAYS: i64 = 396;
/// How long a CRL is current: the root's is reissued at least this often (ADR 0058).
pub const CRL_DAYS: i64 = 396;
/// How far before issuance a certificate becomes valid.
pub const BACKDATE_SECONDS: i64 = 3600;

/// Why the PKI refused.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The certificate, request or CRL could not be built or parsed.
    #[error("certificate: {0}")]
    Certificate(#[from] rcgen::Error),
    /// The input is not the PEM or DER it should be.
    #[error("malformed: {0}")]
    Malformed(String),
    /// The issuer may not issue this.
    #[error("refused: {0}")]
    Refused(String),
    /// The chain does not verify.
    #[error("not trusted: {0}")]
    Untrusted(#[from] webpki::Error),
}

/// The PKI's result.
pub type Result<T> = std::result::Result<T, Error>;
