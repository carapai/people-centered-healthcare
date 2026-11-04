//! Published Definition packages on disk and in transit (ADR 0024; configuration.md § Package
//! format, § Signing).
//!
//! - [`publish`] packs a package's files into a deterministic tarball with a generated
//!   `.index.json`, refuses it if any publication rule fails (an unpinned dependency, among
//!   others: [`health_configuration::package`]) or any `configuration/` file does not validate
//!   against the release's [`Schemas`], and signs it.
//! - [`verify`] is what every node does before staging, whether the package came over sync or on
//!   a USB stick: it checks the sha-256 the activation names, the signature against the national
//!   root, the archive's safety, the index, and the publication rules and schemas again.
//!
//! A package is identified by `name@version` and its sha-256 ([`Digest`]).

mod archive;
pub mod bump;
mod contents;
mod schemas;
mod signature;

use std::fmt;

use health_configuration::package::{INDEX, PackageContents, Problem};
use health_pki::Trust;
use health_time::Timestamp;
use sha2::Digest as _;

pub use archive::{Files, Limits};
pub use schemas::Schemas;
pub use signature::{LocalSigner, PublicationSigner, TYP};

/// A sha-256.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Digest(pub [u8; 32]);

impl Digest {
    /// The sha-256 of `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        Self(sha2::Sha256::digest(bytes).into())
    }
}

impl fmt::Display for Digest {
    /// Lower-case hex.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sha256:{self}")
    }
}

/// Why a package was not published or not accepted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Not a readable gzip-compressed tar.
    #[error("archive: {0}")]
    Archive(String),
    /// An entry that could write outside the package: a link, an absolute or `..` path, a file
    /// outside `package/`, or a duplicate.
    #[error("unsafe archive entry {0}")]
    Unsafe(String),
    /// Past the [`Limits`].
    #[error("too large: {0}")]
    TooLarge(String),
    /// A JSON file that does not read.
    #[error("{path}: {detail}")]
    Json {
        /// Its path under `package/`.
        path: String,
        /// Why.
        detail: String,
    },
    /// `.index.json` does not list the package's artefacts.
    #[error(".index.json does not match the package's artefacts")]
    Index,
    /// A `configuration/` file that is not JSON, names no known `$schema`, or does not validate.
    #[error("{path}: {detail}")]
    Configuration {
        /// Its path under `package/`, or the schema's URI when the schema itself is unusable.
        path: String,
        /// Why.
        detail: String,
    },
    /// The package breaks publication rules.
    #[error("not publishable: {}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))]
    NotPublishable(Vec<Problem>),
    /// Not the package expected: its sha-256 differs.
    #[error("the package's sha-256 is {actual}, not {expected}")]
    Digest {
        /// What the activation named.
        expected: String,
        /// What the bytes hash to.
        actual: String,
    },
    /// The signature is malformed or does not match.
    #[error("signature: {0}")]
    Signature(String),
    /// The signing certificate does not chain to the national root as a publication signer.
    #[error("not signed by a trusted publisher: {0}")]
    Untrusted(String),
}

/// A published package: what central distributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    /// `name@version`.
    pub id: String,
    /// The tarball, `<name>-<version>.tgz`.
    pub tgz: Vec<u8>,
    /// Its sha-256.
    pub digest: Digest,
    /// The detached signature, `<name>-<version>.tgz.sig`.
    pub signature: String,
}

/// Packs `files` (paths under `package/`; any `.index.json` is replaced), checks the
/// publication rules and `configuration/` against `schemas`, and signs with `signer` at `at`.
pub fn publish(
    files: &Files,
    schemas: &Schemas,
    signer: &dyn PublicationSigner,
    at: Timestamp,
) -> Result<Published, Error> {
    let mut files = files.clone();
    files.remove(INDEX);
    let index = contents::index(&files)?;
    files.insert(INDEX.to_owned(), index);
    let package = contents::read(&files)?;
    let problems = package.problems_for_publication();
    if !problems.is_empty() {
        return Err(Error::NotPublishable(problems));
    }
    schemas.check(&files)?;
    let tgz = archive::write(&files)?;
    let digest = Digest::of(&tgz);
    let signature = signature::sign(&digest, signer, at)?;
    Ok(Published {
        id: package.id(),
        tgz,
        digest,
        signature,
    })
}

/// A package that passed verification: safe to stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    /// What it holds.
    pub contents: PackageContents,
    /// Its files.
    pub files: Files,
    /// Its sha-256.
    pub digest: Digest,
    /// When it was signed.
    pub signed_at: Timestamp,
}

/// Verifies a package before staging (configuration.md § Distribution and activation on nodes).
///
/// In order: the tarball's sha-256 is `expected` when the activation names one; the signature
/// is the root's publication signer's over that sha-256; the archive unpacks safely within
/// `limits`; `.index.json` lists exactly its artefacts; it meets every publication rule; and its
/// `configuration/` validates against `schemas`.
pub fn verify(
    tgz: &[u8],
    jws: &str,
    expected: Option<&Digest>,
    trust: &Trust,
    schemas: &Schemas,
    limits: Limits,
    now: Timestamp,
) -> Result<Verified, Error> {
    let digest = Digest::of(tgz);
    if let Some(expected) = expected
        && expected != &digest
    {
        return Err(Error::Digest {
            expected: expected.to_string(),
            actual: digest.to_string(),
        });
    }
    let signed_at = signature::verify(jws, &digest, trust, now)?;
    let files = archive::read(tgz, limits)?;
    let mut unindexed = files.clone();
    unindexed.remove(INDEX);
    if files.get(INDEX) != Some(&contents::index(&unindexed)?) {
        return Err(Error::Index);
    }
    let contents = contents::read(&files)?;
    let problems = contents.problems_for_publication();
    if !problems.is_empty() {
        return Err(Error::NotPublishable(problems));
    }
    schemas.check(&files)?;
    Ok(Verified {
        contents,
        files,
        digest,
        signed_at,
    })
}
