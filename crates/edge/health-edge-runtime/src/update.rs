//! Signed updates with automatic rollback (facility-edge.md § Updates and migrations).
//!
//! A package is a gzipped tar of `manifest.json` and the binary it names, with a detached
//! ECDSA P-256 signature over the whole package. The release key's public half is pinned by the
//! installer. The update directory holds:
//!
//! | Path | |
//! |---|---|
//! | `staged/` | a verified, unpacked package, waiting to be applied |
//! | `previous/` | the binary that was running before the last apply |
//! | `pre-update-store/` | a cold copy of the store, taken before the new binary first opens it |
//! | `failed-store/` | the store a failed update left, kept for diagnosis |
//! | `journal.json` | the last update's state |
//!
//! The sequence:
//! 1. [`stage`] verifies and unpacks a package. The node may keep running.
//! 2. [`apply`], with the service stopped, keeps the current binary in `previous/`, swaps in the
//!    staged one and journals the update as pending.
//! 3. The new binary's first start ([`before_open`]) copies the store to `pre-update-store/`
//!    before opening it, then migrates. Ready commits the update ([`commit`]).
//! 4. A failed migration marks the update failed ([`fail`]), and the next start rolls it back
//!    ([`roll_back`]) before opening the store: the pre-update store is restored first, then the
//!    previous binary, so an old binary never runs against a migrated store. A sandboxed service
//!    cannot write its own binary, so it restores the store, journals `rolling-back`, and exits;
//!    the service manager's privileged step (the previous binary's `update rollback`) puts the
//!    previous binary back. The new binary never starts while the rollback is unfinished.
//!    A new binary that never gets that far is rolled back by the previous one
//!    (`previous/facility-edge update rollback`), which the service manager runs on failure.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// How many starts of a new binary may fail to reach ready before [`before_open`] rolls back.
pub const MAX_ATTEMPTS: u32 = 3;
/// The largest package, and the largest file in one, accepted.
const MAX_PACKAGE: u64 = 512 * 1024 * 1024;

/// What a package contains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The release version, e.g. `0.2.0`.
    pub version: String,
    /// The binary's file name in the package, e.g. `facility-edge`.
    pub binary: String,
    /// The binary's SHA-256, lowercase hex.
    pub sha256: String,
}

/// The last update's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Journal {
    /// Applied, not yet ready. `attempts` counts the new binary's starts.
    Pending {
        /// The version replaced.
        from: String,
        /// The version applied.
        to: String,
        /// Starts so far.
        attempts: u32,
    },
    /// The new binary reached ready.
    Committed {
        /// The version replaced.
        from: String,
        /// The version running.
        to: String,
    },
    /// The update failed and the previous store is back, but the previous binary is not yet:
    /// the service may not write its own binary, so a privileged step finishes the rollback.
    /// The new binary never starts again.
    RollingBack {
        /// The version being restored.
        from: String,
        /// The version that failed.
        to: String,
    },
    /// The update failed, and the previous binary and store are back.
    RolledBack {
        /// The version running again.
        from: String,
        /// The version that failed.
        to: String,
    },
}

/// Why an update step was refused or failed. Never carries file contents.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    /// The signature does not verify against the pinned release key, or is malformed.
    #[error("the package signature does not verify")]
    Signature,
    /// The package is not a gzipped tar with a manifest and its binary, or is too large.
    #[error("the package is malformed: {0}")]
    Malformed(&'static str),
    /// The binary's SHA-256 is not the manifest's.
    #[error("the binary does not match its manifest")]
    Digest,
    /// There is no staged package to apply.
    #[error("no package is staged")]
    NothingStaged,
    /// There is no update to roll back.
    #[error("no update is pending")]
    NothingPending,
    /// A file operation failed.
    #[error("update file operation failed")]
    Io(#[from] std::io::Error),
}

/// The update directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Updates {
    dir: PathBuf,
}

impl Updates {
    /// The update directory at `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// The last update's state, if there was one.
    pub fn journal(&self) -> Result<Option<Journal>, UpdateError> {
        match fs::read(self.path("journal.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| UpdateError::Malformed("journal.json")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn write_journal(&self, journal: &Journal) -> Result<(), UpdateError> {
        fs::create_dir_all(&self.dir)?;
        let tmp = self.path("journal.json.tmp");
        let bytes = serde_json::to_vec_pretty(journal)
            .map_err(|_| UpdateError::Malformed("journal.json"))?;
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(tmp, self.path("journal.json"))?;
        Ok(())
    }

    /// The staged package's manifest, if one is staged.
    pub fn staged(&self) -> Result<Option<Manifest>, UpdateError> {
        match fs::read(self.path("staged/manifest.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| UpdateError::Malformed("manifest.json")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

/// The pinned release key, from its hex SEC1 encoding (compressed or not).
pub fn release_key(hex_text: &str) -> Option<VerifyingKey> {
    VerifyingKey::from_sec1_bytes(&hex(hex_text.trim())?).ok()
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

/// Builds a package of `binary`, named `name`, at `version` (for the release job and tests).
/// The caller signs the bytes.
pub fn pack(binary: &[u8], name: &str, version: &str) -> Result<Vec<u8>, UpdateError> {
    let manifest = Manifest {
        version: version.to_owned(),
        binary: name.to_owned(),
        sha256: to_hex(&Sha256::digest(binary)),
    };
    let manifest =
        serde_json::to_vec_pretty(&manifest).map_err(|_| UpdateError::Malformed("manifest"))?;
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    for (path, bytes, mode) in [
        ("manifest.json", manifest.as_slice(), 0o644),
        (name, binary, 0o755),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        tar.append_data(&mut header, path, bytes)?;
    }
    Ok(tar.into_inner()?.finish()?)
}

/// Verifies `package` against `signature` (hex, the 64-byte `r || s`) and the pinned `key`, and
/// unpacks it into `staged/`, replacing anything staged before. Nothing is unpacked unless the
/// signature and the binary's digest both verify.
pub fn stage(
    updates: &Updates,
    package: &[u8],
    signature: &str,
    key: &VerifyingKey,
) -> Result<Manifest, UpdateError> {
    let signature = hex(signature.trim())
        .and_then(|bytes| Signature::from_slice(&bytes).ok())
        .ok_or(UpdateError::Signature)?;
    key.verify(package, &signature)
        .map_err(|_| UpdateError::Signature)?;
    if package.len() as u64 > MAX_PACKAGE {
        return Err(UpdateError::Malformed("too large"));
    }
    let mut manifest: Option<Manifest> = None;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(package));
    for entry in archive
        .entries()
        .map_err(|_| UpdateError::Malformed("not a gzipped tar"))?
    {
        let mut entry = entry.map_err(|_| UpdateError::Malformed("not a gzipped tar"))?;
        let name = entry
            .path()
            .ok()
            .and_then(|p| p.to_str().map(str::to_owned))
            .filter(|n| !n.is_empty() && !n.contains(['/', '\\']) && n != "..")
            .ok_or(UpdateError::Malformed("an entry is not a plain file name"))?;
        let mut bytes = Vec::new();
        entry
            .by_ref()
            .take(MAX_PACKAGE + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| UpdateError::Malformed("not a gzipped tar"))?;
        if bytes.len() as u64 > MAX_PACKAGE {
            return Err(UpdateError::Malformed("too large"));
        }
        if name == "manifest.json" {
            manifest = Some(
                serde_json::from_slice(&bytes)
                    .map_err(|_| UpdateError::Malformed("manifest.json"))?,
            );
        } else {
            files.push((name, bytes));
        }
    }
    let manifest = manifest.ok_or(UpdateError::Malformed("no manifest.json"))?;
    let (_, binary) = files
        .iter()
        .find(|(name, _)| *name == manifest.binary)
        .ok_or(UpdateError::Malformed("the manifest's binary is missing"))?;
    if to_hex(&Sha256::digest(binary)) != manifest.sha256 {
        return Err(UpdateError::Digest);
    }
    let staged = updates.path("staged");
    let _ = fs::remove_dir_all(&staged);
    fs::create_dir_all(&staged)?;
    write_executable(&staged.join(&manifest.binary), binary)?;
    fs::write(
        staged.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|_| UpdateError::Malformed("manifest"))?,
    )?;
    Ok(manifest)
}

fn write_executable(path: &Path, bytes: &[u8]) -> Result<(), UpdateError> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Replaces `target` with `source`'s bytes. The running binary is renamed aside first, which
/// every supported OS allows, and the new file is renamed into place within its directory.
fn replace_binary(target: &Path, source: &Path) -> Result<(), UpdateError> {
    let incoming = target.with_extension("incoming");
    let outgoing = target.with_extension("outgoing");
    write_executable(&incoming, &fs::read(source)?)?;
    let _ = fs::remove_file(&outgoing);
    fs::rename(target, &outgoing)?;
    if let Err(error) = fs::rename(&incoming, target) {
        let _ = fs::rename(&outgoing, target);
        return Err(error.into());
    }
    // A running binary cannot be removed on Windows; the next update removes it.
    let _ = fs::remove_file(&outgoing);
    Ok(())
}

/// Applies the staged package, with the service stopped: keeps `binary` (the installed one,
/// at `current` version) in `previous/`, swaps in the staged binary and journals the update.
pub fn apply(updates: &Updates, binary: &Path, current: &str) -> Result<Manifest, UpdateError> {
    let manifest = updates.staged()?.ok_or(UpdateError::NothingStaged)?;
    let previous = updates.path("previous");
    let _ = fs::remove_dir_all(&previous);
    fs::create_dir_all(&previous)?;
    let name = binary
        .file_name()
        .ok_or(UpdateError::Malformed("the binary path"))?;
    write_executable(&previous.join(name), &fs::read(binary)?)?;
    let _ = fs::remove_dir_all(updates.path("pre-update-store"));
    updates.write_journal(&Journal::Pending {
        from: current.to_owned(),
        to: manifest.version.clone(),
        attempts: 0,
    })?;
    replace_binary(binary, &updates.path("staged").join(&manifest.binary))?;
    fs::remove_dir_all(updates.path("staged"))?;
    Ok(manifest)
}

/// What [`before_open`] decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Startup {
    /// No update is pending.
    Normal,
    /// An update is pending; this start may commit it.
    Updating,
    /// The update kept failing to reach ready and was rolled back; exit so the previous binary
    /// starts.
    RolledBack,
    /// The update was rolled back as far as this process may (the store), and the service
    /// manager must put the previous binary back.
    RollbackPending,
}

/// Run by the node before it opens `store`: on a pending update's first start, copies the store
/// to `pre-update-store/`; after [`MAX_ATTEMPTS`] starts that never reached ready, rolls back.
pub fn before_open(updates: &Updates, store: &Path, binary: &Path) -> Result<Startup, UpdateError> {
    let (from, to, attempts) = match updates.journal()? {
        Some(Journal::Pending { from, to, attempts }) if attempts < MAX_ATTEMPTS => {
            (from, to, attempts)
        }
        Some(Journal::Pending { .. } | Journal::RollingBack { .. }) => {
            // Under a sandboxed service the binary cannot be replaced here; the store can. The
            // service manager's privileged rollback finishes what this leaves.
            return Ok(match roll_back(updates, store, binary) {
                Ok(()) => Startup::RolledBack,
                Err(error) => {
                    tracing::warn!(error.kind = %error, "rollback left for the service manager");
                    Startup::RollbackPending
                }
            });
        }
        _ => return Ok(Startup::Normal),
    };
    // Only the first start copies: a later one would copy a half-migrated store.
    let copy = updates.path("pre-update-store");
    if attempts == 0 {
        let _ = fs::remove_dir_all(&copy);
        if store.exists() {
            copy_dir(store, &copy)?;
        }
    }
    updates.write_journal(&Journal::Pending {
        from,
        to,
        attempts: attempts + 1,
    })?;
    Ok(Startup::Updating)
}

/// Run when the new binary's migrations fail: the next start rolls back before anything opens
/// the store, rather than this process renaming a store it still has open.
pub fn fail(updates: &Updates) -> Result<(), UpdateError> {
    if let Some(Journal::Pending { from, to, .. }) = updates.journal()? {
        updates.write_journal(&Journal::Pending {
            from,
            to,
            attempts: MAX_ATTEMPTS,
        })?;
    }
    Ok(())
}

/// Run once the new binary is ready: the update is committed.
pub fn commit(updates: &Updates) -> Result<(), UpdateError> {
    if let Some(Journal::Pending { from, to, .. }) = updates.journal()? {
        updates.write_journal(&Journal::Committed { from, to })?;
    }
    Ok(())
}

/// Rolls back the pending update: the store first (the failed one moves to `failed-store/` and
/// the pre-update copy takes its place), then the previous binary over `binary`.
///
/// Each half is journalled, so a rollback interrupted between them resumes with the binary.
pub fn roll_back(updates: &Updates, store: &Path, binary: &Path) -> Result<(), UpdateError> {
    let (from, to) = match updates.journal()? {
        Some(Journal::Pending { from, to, .. }) => {
            let copy = updates.path("pre-update-store");
            let failed = updates.path("failed-store");
            let _ = fs::remove_dir_all(&failed);
            if store.exists() {
                fs::rename(store, &failed)?;
            }
            if copy.exists() {
                fs::rename(&copy, store)?;
            }
            updates.write_journal(&Journal::RollingBack {
                from: from.clone(),
                to: to.clone(),
            })?;
            (from, to)
        }
        Some(Journal::RollingBack { from, to }) => (from, to),
        _ => return Err(UpdateError::NothingPending),
    };
    let name = binary
        .file_name()
        .ok_or(UpdateError::Malformed("the binary path"))?;
    let previous = updates.path("previous").join(name);
    if previous != binary {
        replace_binary(binary, &previous)?;
    }
    updates.write_journal(&Journal::RolledBack { from, to })
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), UpdateError> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use p256::ecdsa::SigningKey;
    use p256::ecdsa::signature::Signer;

    use super::*;

    fn key() -> SigningKey {
        SigningKey::from_slice(&[7_u8; 32]).expect("key")
    }

    fn signed(package: &[u8]) -> String {
        let signature: Signature = key().sign(package);
        to_hex(&signature.to_bytes())
    }

    fn public() -> VerifyingKey {
        let hex = to_hex(key().verifying_key().to_encoded_point(true).as_bytes());
        release_key(&hex).expect("release key")
    }

    #[test]
    fn only_a_signed_package_with_a_matching_binary_is_staged() {
        let dir = tempfile::tempdir().expect("dir");
        let updates = Updates::new(dir.path());
        let package = pack(b"new binary", "facility-edge", "0.2.0").expect("pack");
        let mut tampered = package.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(matches!(
            stage(&updates, &tampered, &signed(&package), &public()),
            Err(UpdateError::Signature)
        ));
        assert!(matches!(
            stage(&updates, &package, "zz", &public()),
            Err(UpdateError::Signature)
        ));
        assert_eq!(updates.staged().expect("staged"), None);

        let manifest = stage(&updates, &package, &signed(&package), &public()).expect("stage");
        assert_eq!(manifest.version, "0.2.0");
        assert_eq!(updates.staged().expect("staged"), Some(manifest));
        assert_eq!(
            fs::read(dir.path().join("staged/facility-edge")).expect("binary"),
            b"new binary"
        );
    }

    #[test]
    fn a_manifest_that_lies_about_its_binary_is_refused() {
        let dir = tempfile::tempdir().expect("dir");
        let updates = Updates::new(dir.path());
        let manifest = br#"{"version":"0.2.0","binary":"facility-edge","sha256":"00"}"#;
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tar = tar::Builder::new(gz);
        for (path, bytes) in [
            ("manifest.json", manifest.as_slice()),
            ("facility-edge", b"x".as_slice()),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_cksum();
            tar.append_data(&mut header, path, bytes).expect("append");
        }
        let package = tar.into_inner().expect("tar").finish().expect("gz");
        assert!(matches!(
            stage(&updates, &package, &signed(&package), &public()),
            Err(UpdateError::Digest)
        ));
        assert!(matches!(
            stage(
                &updates,
                b"not a package",
                &signed(b"not a package"),
                &public()
            ),
            Err(UpdateError::Malformed(_))
        ));
    }

    /// An installed binary and a store, as on an edge.
    fn installed(root: &Path) -> (Updates, PathBuf, PathBuf) {
        let bin = root.join("bin");
        fs::create_dir_all(&bin).expect("bin");
        let binary = bin.join("facility-edge");
        fs::write(&binary, b"old binary").expect("binary");
        let store = root.join("store");
        fs::create_dir_all(store.join("sub")).expect("store");
        fs::write(store.join("sub/data"), b"old data").expect("data");
        (Updates::new(root.join("updates")), binary, store)
    }

    fn stage_new(updates: &Updates) {
        let package = pack(b"new binary", "facility-edge", "0.2.0").expect("pack");
        stage(updates, &package, &signed(&package), &public()).expect("stage");
    }

    #[test]
    fn an_applied_update_copies_the_store_on_first_start_and_commits_when_ready() {
        let dir = tempfile::tempdir().expect("dir");
        let (updates, binary, store) = installed(dir.path());
        assert!(matches!(
            apply(&updates, &binary, "0.1.0"),
            Err(UpdateError::NothingStaged)
        ));
        stage_new(&updates);
        apply(&updates, &binary, "0.1.0").expect("apply");
        assert_eq!(fs::read(&binary).expect("binary"), b"new binary");
        assert_eq!(
            fs::read(dir.path().join("updates/previous/facility-edge")).expect("previous"),
            b"old binary"
        );
        assert_eq!(updates.staged().expect("staged"), None);

        assert_eq!(
            before_open(&updates, &store, &binary).expect("start"),
            Startup::Updating
        );
        assert_eq!(
            fs::read(dir.path().join("updates/pre-update-store/sub/data")).expect("copy"),
            b"old data"
        );
        commit(&updates).expect("commit");
        assert_eq!(
            updates.journal().expect("journal"),
            Some(Journal::Committed {
                from: "0.1.0".into(),
                to: "0.2.0".into()
            })
        );
        assert_eq!(
            before_open(&updates, &store, &binary).expect("start"),
            Startup::Normal
        );
    }

    #[test]
    fn a_failed_update_restores_the_store_before_the_previous_binary() {
        let dir = tempfile::tempdir().expect("dir");
        let (updates, binary, store) = installed(dir.path());
        stage_new(&updates);
        apply(&updates, &binary, "0.1.0").expect("apply");
        before_open(&updates, &store, &binary).expect("start");
        // The new binary migrated the store, then failed.
        fs::write(store.join("sub/data"), b"migrated").expect("migrate");
        roll_back(&updates, &store, &binary).expect("roll back");
        assert_eq!(fs::read(store.join("sub/data")).expect("data"), b"old data");
        assert_eq!(fs::read(&binary).expect("binary"), b"old binary");
        assert_eq!(
            fs::read(dir.path().join("updates/failed-store/sub/data")).expect("failed"),
            b"migrated"
        );
        assert_eq!(
            updates.journal().expect("journal"),
            Some(Journal::RolledBack {
                from: "0.1.0".into(),
                to: "0.2.0".into()
            })
        );
        assert!(matches!(
            roll_back(&updates, &store, &binary),
            Err(UpdateError::NothingPending)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_service_that_cannot_write_its_binary_restores_the_store_and_leaves_the_binary() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("dir");
        let (updates, binary, store) = installed(dir.path());
        stage_new(&updates);
        apply(&updates, &binary, "0.1.0").expect("apply");
        before_open(&updates, &store, &binary).expect("start");
        fail(&updates).expect("fail");
        // The sandboxed service may not write the binary's directory.
        let bin = binary.parent().expect("bin").to_path_buf();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o555)).expect("chmod");
        assert_eq!(
            before_open(&updates, &store, &binary).expect("start"),
            Startup::RollbackPending
        );
        assert!(matches!(
            updates.journal().expect("journal"),
            Some(Journal::RollingBack { .. })
        ));
        assert_eq!(fs::read(store.join("sub/data")).expect("data"), b"old data");
        // The new binary is refused while the rollback is unfinished.
        assert_eq!(
            before_open(&updates, &store, &binary).expect("start"),
            Startup::RollbackPending
        );
        // The privileged step finishes it.
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).expect("chmod");
        roll_back(&updates, &store, &binary).expect("finish");
        assert_eq!(fs::read(&binary).expect("binary"), b"old binary");
        assert!(matches!(
            updates.journal().expect("journal"),
            Some(Journal::RolledBack { .. })
        ));
    }

    #[test]
    fn a_new_binary_that_never_gets_ready_is_rolled_back_after_its_attempts() {
        let dir = tempfile::tempdir().expect("dir");
        let (updates, binary, store) = installed(dir.path());
        stage_new(&updates);
        apply(&updates, &binary, "0.1.0").expect("apply");
        for _ in 0..MAX_ATTEMPTS {
            assert_eq!(
                before_open(&updates, &store, &binary).expect("start"),
                Startup::Updating
            );
            // Each start migrated a little, then crashed before ready.
            fs::write(store.join("sub/data"), b"half migrated").expect("write");
        }
        assert_eq!(
            before_open(&updates, &store, &binary).expect("start"),
            Startup::RolledBack
        );
        assert_eq!(fs::read(store.join("sub/data")).expect("data"), b"old data");
        assert_eq!(fs::read(&binary).expect("binary"), b"old binary");
    }
}
