//! Platform releases at central and on nodes (#136; deployment.md § Releases; ADRs 0041, 0045):
//! reading a release's signed artifacts from the directory its GitHub release was downloaded
//! into, checking every signature against the keys nodes pin, and the checks a node makes on an
//! artifact before staging it.
//!
//! - An edge update package's signature is ECDSA P-256 over the whole package, hex `r || s`, by
//!   the release-signing key (facility-edge.md § Updates).
//! - A desktop artifact's is the Tauri updater's: minisign over its BLAKE2b-512, the `.sig` file
//!   as Tauri writes it (base64 of the signature file), by the updater key.

use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use health_configuration::package::Version;
use health_configuration::rollout::{Artifact, Target};
use p256::ecdsa::signature::Verifier as _;
use p256::ecdsa::{Signature, VerifyingKey};
use sha2::Digest as _;

/// Why a release or an artifact was refused. Never carries file contents.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReleaseError {
    /// A file the release must have is not there, or cannot be read.
    #[error("{0} is missing or unreadable")]
    Missing(String),
    /// A file's sha-256 is not the one `SHA256SUMS` or the roll-out names.
    #[error("{0} does not match its sha-256")]
    Digest(String),
    /// A signature does not verify against the pinned key, or is malformed.
    #[error("{0}'s signature does not verify against the pinned key")]
    Signature(String),
    /// A pinned key is malformed.
    #[error("the {0} is not a valid public key")]
    Key(&'static str),
}

/// The keys nodes pin, to check a release against.
#[derive(Debug, Clone)]
pub struct ReleaseKeys {
    /// The edge release-signing key.
    pub edge: VerifyingKey,
    /// The Tauri updater's public key, as Tauri prints it (base64 of the key file).
    pub updater: String,
}

impl ReleaseKeys {
    /// The keys from their texts: the edge key as hex SEC1 (as `HEALTH_UPDATE_KEY_FILE` holds
    /// it), the updater key as Tauri prints it.
    pub fn from_text(edge_hex: &str, updater: &str) -> Result<Self, ReleaseError> {
        let edge = hex(edge_hex.trim())
            .and_then(|sec1| VerifyingKey::from_sec1_bytes(&sec1).ok())
            .ok_or(ReleaseError::Key("edge release-signing key"))?;
        let updater = updater.trim().to_owned();
        updater_key(&updater)?;
        Ok(Self { edge, updater })
    }
}

fn hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| {
            text.get(i..i + 2)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        })
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decoded(base64: &str) -> Option<String> {
    STANDARD
        .decode(base64.trim())
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

fn updater_key(key: &str) -> Result<minisign_verify::PublicKey, ReleaseError> {
    decoded(key)
        .and_then(|text| minisign_verify::PublicKey::decode(&text).ok())
        .ok_or(ReleaseError::Key("updater key"))
}

/// The sha-256 of `bytes`, lowercase hex.
pub fn sha256(bytes: &[u8]) -> String {
    to_hex(&sha2::Sha256::digest(bytes))
}

/// Checks `bytes`, the artifact for `artifact.target`, against its sha-256 and its signature
/// under the pinned `keys`: what a node does before staging an update, and central at import.
pub fn verify(bytes: &[u8], artifact: &Artifact, keys: &ReleaseKeys) -> Result<(), ReleaseError> {
    if sha256(bytes) != artifact.sha256 {
        return Err(ReleaseError::Digest(artifact.file.clone()));
    }
    let refused = || ReleaseError::Signature(artifact.file.clone());
    match artifact.target {
        Target::EdgeLinux | Target::EdgeWindows => {
            let signature = hex(artifact.signature.trim())
                .and_then(|bytes| Signature::from_slice(&bytes).ok())
                .ok_or_else(refused)?;
            keys.edge.verify(bytes, &signature).map_err(|_| refused())
        }
        Target::DesktopWindows | Target::DesktopLinux => {
            verify_desktop(bytes, artifact, &keys.updater)
        }
    }
}

/// Checks `bytes`, a desktop artifact, against its sha-256 and its updater signature under
/// `updater`, the updater key as Tauri prints it: what a desktop does, which pins only that key.
pub fn verify_desktop(
    bytes: &[u8],
    artifact: &Artifact,
    updater: &str,
) -> Result<(), ReleaseError> {
    if sha256(bytes) != artifact.sha256 {
        return Err(ReleaseError::Digest(artifact.file.clone()));
    }
    let refused = || ReleaseError::Signature(artifact.file.clone());
    if !matches!(
        artifact.target,
        Target::DesktopWindows | Target::DesktopLinux
    ) {
        return Err(refused());
    }
    let key = updater_key(updater.trim())?;
    let signature = decoded(&artifact.signature)
        .and_then(|text| minisign_verify::Signature::decode(&text).ok())
        .ok_or_else(refused)?;
    // Tauri's signer hashes first; a legacy (unhashed) signature is refused.
    key.verify(bytes, &signature, false).map_err(|_| refused())
}

/// The file each target's artifact has in release `version` (release.yml).
pub fn file_name(target: Target, version: &Version) -> String {
    match target {
        Target::EdgeLinux => format!("facility-edge-{version}-linux-x64.tar.gz"),
        Target::EdgeWindows => format!("facility-edge-{version}-windows-x64.tar.gz"),
        Target::DesktopWindows => format!("health-platform-{version}-x64-setup.exe"),
        Target::DesktopLinux => format!("health-platform-{version}-x86_64.AppImage"),
    }
}

/// Reads release `version`'s artifacts from `dir` (its GitHub release, downloaded), each with
/// its `.sig`. Every file must match `SHA256SUMS` and verify against `keys`, or nothing is
/// returned. Returns each artifact with its bytes, for the blob store.
pub fn read_release(
    dir: &Path,
    version: &Version,
    keys: &ReleaseKeys,
) -> Result<Vec<(Artifact, Vec<u8>)>, ReleaseError> {
    let read = |name: &str| {
        std::fs::read(dir.join(name)).map_err(|_| ReleaseError::Missing(name.to_owned()))
    };
    let sums = String::from_utf8(read("SHA256SUMS")?)
        .map_err(|_| ReleaseError::Missing("SHA256SUMS".into()))?;
    let listed = |name: &str| {
        sums.lines().find_map(|line| {
            let (digest, file) = line.split_once(char::is_whitespace)?;
            (file.trim_start_matches([' ', '*']) == name).then(|| digest.to_owned())
        })
    };
    let mut artifacts = Vec::new();
    for target in Target::ALL {
        let file = file_name(target, version);
        let bytes = read(&file)?;
        let sig = format!("{file}.sig");
        let signature =
            String::from_utf8(read(&sig)?).map_err(|_| ReleaseError::Missing(sig.clone()))?;
        let sha256 = sha256(&bytes);
        for name in [&file, &sig] {
            let digest = listed(name)
                .ok_or_else(|| ReleaseError::Missing(format!("{name} in SHA256SUMS")))?;
            let actual = if *name == file {
                sha256.clone()
            } else {
                self::sha256(signature.as_bytes())
            };
            if digest != actual {
                return Err(ReleaseError::Digest(name.clone()));
            }
        }
        let artifact = Artifact {
            target,
            file,
            sha256,
            size: bytes.len() as u64,
            signature: signature.trim().to_owned(),
        };
        verify(&bytes, &artifact, keys)?;
        artifacts.push((artifact, bytes));
    }
    Ok(artifacts)
}

#[cfg(test)]
mod tests {
    use p256::ecdsa::SigningKey;
    use p256::ecdsa::signature::Signer as _;

    use super::*;

    // A throwaway Tauri updater key, and `an installer\n` signed with it by
    // `tauri signer sign` (2.12.4).
    const UPDATER: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEE2QUQxMjNEMTJFQjE4REQKUldUZEdPc1NQUkt0cGoyU1c4Q0pGMWkyVWZubzdZZTNpQmRyUzdVdG5wWWdKMVZHNVZwZ2V1ZjgK";
    const OTHER_UPDATER: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDMzODEwRUJCMkYyNzRFMjQKUldRa1RpY3Z1dzZCTXgvbnIydmpuM3pNSGZycmM1OTRHcE1PdVFQWmNkVDlpbk5KTUNrbUhqeGcK";
    const INSTALLER: &[u8] = b"an installer\n";
    const INSTALLER_SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUZEdPc1NQUkt0cGlSZzhORUhNM1JiTUJrK0ZNSC9FQXZxbG5lUTJlSGNTRXBFUVhvVlhDVThMeC9KVkl1d0gwRmpDaUo0djdDNlp6TnJmTDZEMGZJbGQrdmNnbWJWcFEwPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0ODE0ODg5CWZpbGU6c2V0dXAuZXhlCms0OGVNdmQ3M3lFVEhMUGZtY2JYeVU4TEE0UVIxdXUwV0ZkWC93Z3pkZG95aXBOZTNyU2VTeVlQZGlUM2RmbWZabDVvTDExd2YrbnZtR3VoK0s4UkJRPT0K";

    fn edge_key() -> SigningKey {
        SigningKey::from_slice(&[7; 32]).expect("key")
    }

    fn keys(updater: &str) -> ReleaseKeys {
        let edge = to_hex(edge_key().verifying_key().to_encoded_point(true).as_bytes());
        ReleaseKeys::from_text(&edge, updater).expect("keys")
    }

    /// A downloaded release: every artifact, its `.sig`, and `SHA256SUMS`.
    fn release(dir: &Path, version: &Version) {
        let mut sums = String::new();
        let mut put = |name: &str, bytes: &[u8]| {
            std::fs::write(dir.join(name), bytes).expect("write");
            sums.push_str(&format!("{}  {name}\n", sha256(bytes)));
        };
        for target in Target::ALL {
            let file = file_name(target, version);
            let (bytes, signature) = match target {
                Target::EdgeLinux | Target::EdgeWindows => {
                    let bytes = format!("{target} package").into_bytes();
                    let signature: Signature = edge_key().sign(&bytes);
                    (bytes, to_hex(&signature.to_bytes()))
                }
                Target::DesktopWindows | Target::DesktopLinux => {
                    (INSTALLER.to_vec(), INSTALLER_SIG.to_owned())
                }
            };
            put(&file, &bytes);
            put(&format!("{file}.sig"), signature.as_bytes());
        }
        std::fs::write(dir.join("SHA256SUMS"), sums).expect("sums");
    }

    #[test]
    fn a_release_whose_every_signature_verifies_is_read() {
        let dir = tempfile::tempdir().expect("dir");
        let version: Version = "1.1.0".parse().expect("version");
        release(dir.path(), &version);
        let artifacts = read_release(dir.path(), &version, &keys(UPDATER)).expect("read");
        let targets: Vec<_> = artifacts.iter().map(|(a, _)| a.target).collect();
        assert_eq!(targets, Target::ALL);
        let (installer, bytes) = &artifacts[2];
        assert_eq!(installer.file, "health-platform-1.1.0-x64-setup.exe");
        assert_eq!(bytes.as_slice(), INSTALLER);
        assert_eq!(installer.size, INSTALLER.len() as u64);
        // What a node checks before staging.
        assert_eq!(verify(bytes, installer, &keys(UPDATER)), Ok(()));
        assert_eq!(
            verify(b"another installer\n", installer, &keys(UPDATER)),
            Err(ReleaseError::Digest(installer.file.clone()))
        );
    }

    #[test]
    fn a_release_signed_by_another_key_or_tampered_with_is_refused() {
        let dir = tempfile::tempdir().expect("dir");
        let version: Version = "1.1.0".parse().expect("version");
        release(dir.path(), &version);
        assert_eq!(
            read_release(dir.path(), &version, &keys(OTHER_UPDATER)).map(|_| ()),
            Err(ReleaseError::Signature(
                "health-platform-1.1.0-x64-setup.exe".into()
            ))
        );
        // Another edge key.
        let other = SigningKey::from_slice(&[9; 32]).expect("key");
        let wrong = ReleaseKeys {
            edge: *other.verifying_key(),
            updater: UPDATER.into(),
        };
        assert_eq!(
            read_release(dir.path(), &version, &wrong).map(|_| ()),
            Err(ReleaseError::Signature(
                "facility-edge-1.1.0-linux-x64.tar.gz".into()
            ))
        );
        // A file changed after SHA256SUMS was written.
        std::fs::write(
            dir.path().join("facility-edge-1.1.0-windows-x64.tar.gz"),
            b"x",
        )
        .expect("write");
        assert_eq!(
            read_release(dir.path(), &version, &keys(UPDATER)).map(|_| ()),
            Err(ReleaseError::Digest(
                "facility-edge-1.1.0-windows-x64.tar.gz".into()
            ))
        );
        // Another version's release is not this one.
        let other_version: Version = "1.2.0".parse().expect("version");
        assert_eq!(
            read_release(dir.path(), &other_version, &keys(UPDATER)).map(|_| ()),
            Err(ReleaseError::Missing(
                "facility-edge-1.2.0-linux-x64.tar.gz".into()
            ))
        );
    }

    #[test]
    fn malformed_pinned_keys_are_refused() {
        assert!(matches!(
            ReleaseKeys::from_text("00", UPDATER),
            Err(ReleaseError::Key(_))
        ));
        let edge = to_hex(edge_key().verifying_key().to_encoded_point(true).as_bytes());
        assert!(matches!(
            ReleaseKeys::from_text(&edge, "not a key"),
            Err(ReleaseError::Key(_))
        ));
    }
}
