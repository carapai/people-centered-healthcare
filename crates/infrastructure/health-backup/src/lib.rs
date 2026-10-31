//! Encrypted logical backups (backup-and-recovery.md; ADR 0046).
//!
//! A backup directory holds:
//!
//! ```text
//! {root}/{created}/manifest.json        what the backup holds, with checksums
//! {root}/{created}/export.surql.enc     the snapshot-consistent export, encrypted
//! {root}/blobs/{sha256}.enc             attachment blobs, encrypted, shared by every backup
//! ```
//!
//! - **Always encrypted:** XChaCha20-Poly1305 with the node's backup key (wrapped and escrowed
//!   in central KMS, ADR 0046), a fresh 24-byte nonce per file, and associated data that binds
//!   each ciphertext to its role (the export, or one blob hash), so files cannot be swapped.
//! - **Snapshot-consistent:** SurrealDB's export, taken inside the node's process (embedded
//!   engines; central's remote engine exports over HTTP instead). It includes the outbox, so
//!   un-uploaded work is kept.
//! - **Blobs** are added only when their hash is new.
//! - **Verified after writing:** [`verify`] checks every checksum and decrypts every file; a
//!   corrupt or wrong-key backup is refused, never restored.
//! - **Restore** imports into an empty database in memory (the decrypted export never touches
//!   disk) and puts the blobs back into the node's store.

use std::fs;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use health_files::{BlobHash, BlobRef, FsBlobStore};
use health_surrealdb::Connection;
use health_time::Timestamp;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The manifest format.
pub const FORMAT: u32 = 1;
const NONCE: usize = 24;

/// A node's 256-bit backup key.
#[derive(Clone)]
pub struct BackupKey([u8; 32]);

impl std::fmt::Debug for BackupKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BackupKey(…)")
    }
}

impl BackupKey {
    /// The key from its 32 bytes (unwrapped from the node's keystore or KMS).
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new((&self.0).into())
    }
}

/// A backup failed, or a backup is not trustworthy.
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    /// A checksum, the decryption or a hash did not match: the backup is corrupt, or the key is
    /// wrong. It must not be restored.
    #[error("backup {0} failed verification")]
    Corrupt(String),
    /// The database export or import failed.
    #[error("database: {0}")]
    Database(String),
    /// The blob store failed.
    #[error(transparent)]
    Blobs(#[from] health_files::BlobError),
    /// The filesystem failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// One encrypted file and its checksums.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Part {
    /// The file name, relative to the backup directory.
    pub file: String,
    /// The plaintext's size in bytes.
    pub bytes: u64,
    /// The SHA-256 of the plaintext, in hex.
    pub plaintext_sha256: String,
    /// The SHA-256 of the encrypted file, in hex.
    pub file_sha256: String,
}

/// What a backup holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The manifest format.
    pub format: u32,
    /// The node that took it.
    pub node: String,
    /// When (RFC 3339).
    pub created_at: String,
    /// The database export.
    pub export: Part,
    /// The blob hashes it relies on (stored once under the backup root).
    pub blobs: Vec<String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn seal(key: &BackupKey, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, BackupError> {
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let mut out = nonce.to_vec();
    out.extend(
        key.cipher()
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| BackupError::Corrupt("encryption".into()))?,
    );
    Ok(out)
}

fn open(key: &BackupKey, sealed: &[u8], aad: &[u8], what: &str) -> Result<Vec<u8>, BackupError> {
    if sealed.len() < NONCE {
        return Err(BackupError::Corrupt(what.to_owned()));
    }
    let (nonce, ciphertext) = sealed.split_at(NONCE);
    key.cipher()
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| BackupError::Corrupt(what.to_owned()))
}

fn blob_path(root: &Path, hash: &str) -> PathBuf {
    root.join("blobs").join(format!("{hash}.enc"))
}

fn blob_aad(hash: &str) -> Vec<u8> {
    format!("health-backup v{FORMAT} blob {hash}").into_bytes()
}

const EXPORT_AAD: &[u8] = b"health-backup v1 export";

/// Writes `bytes` to `path` durably: a temporary file, synced, then renamed.
fn write_durably(path: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    let temp = path.with_extension("partial");
    {
        use std::io::Write;
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)?;
    Ok(())
}

/// Takes a backup of `db` (and of `blobs`, if the node has a blob store) under `root`,
/// encrypted with `key`, then verifies it. Returns the backup directory.
pub async fn create(
    db: &Connection,
    blobs: Option<&FsBlobStore>,
    root: &Path,
    node: &str,
    key: &BackupKey,
    now: Timestamp,
) -> Result<PathBuf, BackupError> {
    let created_at = now.to_string();
    let dir = root.join(now.strftime("%Y%m%dT%H%M%S%.3fZ").to_string());
    fs::create_dir_all(&dir)?;
    fs::create_dir_all(root.join("blobs"))?;
    let export = db
        .export_sql()
        .await
        .map_err(|e| BackupError::Database(e.to_string()))?;
    let sealed = seal(key, &export, EXPORT_AAD)?;
    write_durably(&dir.join("export.surql.enc"), &sealed)?;
    let mut hashes = Vec::new();
    if let Some(store) = blobs {
        for hash in store.hashes()? {
            let name = hash.to_string();
            let path = blob_path(root, &name);
            if !path.exists() {
                let bytes = store.read(&hash)?;
                write_durably(&path, &seal(key, &bytes, &blob_aad(&name))?)?;
            }
            hashes.push(name);
        }
        hashes.sort();
    }
    let manifest = Manifest {
        format: FORMAT,
        node: node.to_owned(),
        created_at,
        export: Part {
            file: "export.surql.enc".into(),
            bytes: u64::try_from(export.len()).unwrap_or(u64::MAX),
            plaintext_sha256: sha256(&export),
            file_sha256: sha256(&sealed),
        },
        blobs: hashes,
    };
    let text =
        serde_json::to_vec_pretty(&manifest).map_err(|e| BackupError::Corrupt(e.to_string()))?;
    write_durably(&dir.join("manifest.json"), &text)?;
    verify(&dir, key)?;
    Ok(dir)
}

/// The export's plaintext, after checking the backup end to end.
fn checked(dir: &Path, key: &BackupKey) -> Result<(Manifest, Vec<u8>), BackupError> {
    let corrupt = |what: &str| BackupError::Corrupt(format!("{}: {what}", dir.display()));
    let manifest: Manifest = serde_json::from_slice(&fs::read(dir.join("manifest.json"))?)
        .map_err(|_| corrupt("manifest"))?;
    if manifest.format != FORMAT {
        return Err(corrupt("unknown format"));
    }
    let sealed = fs::read(dir.join(&manifest.export.file))?;
    if sha256(&sealed) != manifest.export.file_sha256 {
        return Err(corrupt("export checksum"));
    }
    let export = open(key, &sealed, EXPORT_AAD, "export")?;
    if sha256(&export) != manifest.export.plaintext_sha256 {
        return Err(corrupt("export content"));
    }
    let root = dir.parent().unwrap_or(dir);
    for hash in &manifest.blobs {
        let bytes = open(
            key,
            &fs::read(blob_path(root, hash)).map_err(|_| corrupt("missing blob"))?,
            &blob_aad(hash),
            hash,
        )?;
        if sha256(&bytes) != *hash {
            return Err(corrupt("blob content"));
        }
    }
    Ok((manifest, export))
}

/// Checks a backup: every checksum, every decryption, every blob's hash. Run after writing, and
/// by the weekly test restore.
pub fn verify(dir: &Path, key: &BackupKey) -> Result<Manifest, BackupError> {
    checked(dir, key).map(|(manifest, _)| manifest)
}

/// Restores a verified backup into `db`, which must be empty, and its blobs into `blobs`.
pub async fn restore(
    dir: &Path,
    key: &BackupKey,
    db: &Connection,
    blobs: Option<&FsBlobStore>,
) -> Result<Manifest, BackupError> {
    let (manifest, export) = checked(dir, key)?;
    let text = String::from_utf8(export)
        .map_err(|_| BackupError::Corrupt("export is not UTF-8".into()))?;
    db.import_sql(&text)
        .await
        .map_err(|e| BackupError::Database(e.to_string()))?;
    if let Some(store) = blobs {
        let root = dir.parent().unwrap_or(dir);
        for hash in &manifest.blobs {
            let bytes = open(
                key,
                &fs::read(blob_path(root, hash))?,
                &blob_aad(hash),
                hash,
            )?;
            let parsed: BlobHash = hash
                .parse()
                .map_err(|_| BackupError::Corrupt(hash.clone()))?;
            store.put_expecting(
                BlobRef {
                    hash: parsed,
                    size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                },
                bytes.as_slice(),
            )?;
        }
    }
    Ok(manifest)
}
