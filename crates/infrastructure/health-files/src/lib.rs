//! The content-addressed blob store for attachments (ADR 0021; synchronization.md § Attachments).
//!
//! Blobs are files on the node filesystem, keyed by their SHA-256, outside SurrealDB:
//!
//! ```text
//! {root}/blobs/ab/cd/abcd…(64 hex)   stored blobs, read-only once written
//! {root}/uploads/{hash}.part         resumable uploads in progress
//! {root}/tmp/                        whole-blob writes in progress
//! {root}/quarantine/{hash}.{n}       blobs that failed verification on read
//! ```
//!
//! - **Writes** stream into `tmp/` (or `uploads/`) while hashing, are synced, then renamed into
//!   place, so a stored blob is always complete. An existing hash is never rewritten: duplicate
//!   uploads deduplicate.
//! - **Reads** recompute the hash. A mismatch moves the file to `quarantine/` and fails, so a
//!   corrupt blob is never served and can be fetched again.
//! - **Backups** copy `blobs/` alongside the logical export, adding only new hashes
//!   (backup-and-recovery.md); [`FsBlobStore::hashes`] lists them.
//! - **Garbage collection** removes a blob only when nothing references it and it is older than
//!   the retention horizon ([`FsBlobStore::collect`]).
//!
//! The API is synchronous file I/O; async callers run it on a blocking thread.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

/// The v1 size cap per attachment (ADR 0021).
pub const MAX_BLOB_SIZE: u64 = 50 * 1024 * 1024;

/// The upload chunk size (synchronization.md § Attachments). Chunks may be smaller at the end.
pub const CHUNK_SIZE: u64 = 1024 * 1024;

/// A blob's SHA-256.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlobHash([u8; 32]);

impl BlobHash {
    /// The hash of `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// The raw digest.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlobHash({self})")
    }
}

/// A hash written as 64 lowercase hex digits was expected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a blob hash is 64 lowercase hex digits")]
pub struct InvalidHash;

impl FromStr for BlobHash {
    type Err = InvalidHash;

    fn from_str(text: &str) -> Result<Self, InvalidHash> {
        let bytes = text.as_bytes();
        if bytes.len() != 64 {
            return Err(InvalidHash);
        }
        let digit = |b: u8| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(InvalidHash),
        };
        let mut hash = [0u8; 32];
        for (i, pair) in bytes.as_chunks::<2>().0.iter().enumerate() {
            hash[i] = (digit(pair[0])? << 4) | digit(pair[1])?;
        }
        Ok(Self(hash))
    }
}

/// A stored blob: what a resource records to reference it (with its content type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlobRef {
    /// The SHA-256 of the content.
    pub hash: BlobHash,
    /// The size in bytes.
    pub size: u64,
}

/// The result of storing a blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stored {
    /// The blob.
    pub blob: BlobRef,
    /// False when the store already held this hash (the upload deduplicated).
    pub created: bool,
}

/// A blob store failure.
#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    /// The content exceeds [`MAX_BLOB_SIZE`] (or the declared size).
    #[error("blob exceeds {limit} bytes")]
    TooLarge {
        /// The limit that was exceeded.
        limit: u64,
    },
    /// The content's hash or size is not the one declared for it.
    #[error("content does not match the declared hash {expected}")]
    Mismatch {
        /// The declared hash.
        expected: BlobHash,
    },
    /// The stored file no longer matches its hash; it has been quarantined.
    #[error("stored blob {0} failed verification and was quarantined")]
    Corrupt(BlobHash),
    /// No such blob.
    #[error("no blob {0}")]
    NotFound(BlobHash),
    /// A chunk does not start where the upload has reached.
    #[error("chunk at offset {offset}, but the upload has {received} bytes")]
    OutOfOrder {
        /// The chunk's offset.
        offset: u64,
        /// The bytes received so far: where the next chunk must start.
        received: u64,
    },
    /// The filesystem failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Answers whether any live or historic resource version still references a blob (ADR 0021).
/// The persistence adapter implements it over `Attachment` references.
pub trait BlobReferences {
    /// True when `hash` is referenced.
    fn is_referenced(&self, hash: &BlobHash) -> Result<bool, BlobError>;
}

/// What a garbage-collection pass removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Collected {
    /// Unreferenced blobs older than the horizon.
    pub blobs: Vec<BlobHash>,
    /// Abandoned uploads and temporary files older than the horizon.
    pub partials: usize,
}

/// The blob store over one directory.
#[derive(Debug, Clone)]
pub struct FsBlobStore {
    root: PathBuf,
}

impl FsBlobStore {
    /// Opens (creating if needed) the store at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, BlobError> {
        let root = root.into();
        for dir in ["blobs", "uploads", "tmp", "quarantine"] {
            fs::create_dir_all(root.join(dir))?;
        }
        Ok(Self { root })
    }

    fn path(&self, hash: &BlobHash) -> PathBuf {
        let hex = hash.to_string();
        self.root
            .join("blobs")
            .join(&hex[..2])
            .join(&hex[2..4])
            .join(hex)
    }

    fn upload_path(&self, hash: &BlobHash) -> PathBuf {
        self.root.join("uploads").join(format!("{hash}.part"))
    }

    /// Whether the store holds `hash`.
    pub fn contains(&self, hash: &BlobHash) -> bool {
        self.path(hash).is_file()
    }

    /// Stores `content`, streaming and hashing it; at most [`MAX_BLOB_SIZE`] bytes.
    pub fn put(&self, content: impl Read) -> Result<Stored, BlobError> {
        let mut temp = tempfile_in(&self.root.join("tmp"))?;
        let (hash, size) = copy_hashing(content, &mut temp.file, MAX_BLOB_SIZE)?;
        self.commit(temp, hash, size)
    }

    /// Stores `content` that was declared as `expected`; anything else is refused.
    pub fn put_expecting(
        &self,
        expected: BlobRef,
        content: impl Read,
    ) -> Result<Stored, BlobError> {
        if expected.size > MAX_BLOB_SIZE {
            return Err(BlobError::TooLarge {
                limit: MAX_BLOB_SIZE,
            });
        }
        if self.contains(&expected.hash) {
            return Ok(Stored {
                blob: expected,
                created: false,
            });
        }
        let mut temp = tempfile_in(&self.root.join("tmp"))?;
        let (hash, size) = copy_hashing(content, &mut temp.file, expected.size)?;
        if hash != expected.hash || size != expected.size {
            return Err(BlobError::Mismatch {
                expected: expected.hash,
            });
        }
        self.commit(temp, hash, size)
    }

    fn commit(&self, temp: Temp, hash: BlobHash, size: u64) -> Result<Stored, BlobError> {
        let blob = BlobRef { hash, size };
        let target = self.path(&hash);
        if target.is_file() {
            return Ok(Stored {
                blob,
                created: false,
            });
        }
        temp.file.sync_all()?;
        let parent = target.parent().unwrap_or(&self.root);
        fs::create_dir_all(parent)?;
        fs::rename(&temp.path, &target)?;
        sync_dir(parent)?;
        temp.keep();
        Ok(Stored {
            blob,
            created: true,
        })
    }

    /// Reads a blob, verifying its hash. A mismatch quarantines the file.
    pub fn read(&self, hash: &BlobHash) -> Result<Vec<u8>, BlobError> {
        let path = self.path(hash);
        let bytes = fs::read(&path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => BlobError::NotFound(*hash),
            _ => BlobError::Io(e),
        })?;
        if BlobHash::of(&bytes) == *hash {
            return Ok(bytes);
        }
        self.quarantine(hash, &path)?;
        Err(BlobError::Corrupt(*hash))
    }

    fn quarantine(&self, hash: &BlobHash, path: &Path) -> Result<(), BlobError> {
        let dir = self.root.join("quarantine");
        let mut n = 0;
        let mut target = dir.join(format!("{hash}.{n}"));
        while target.exists() {
            n += 1;
            target = dir.join(format!("{hash}.{n}"));
        }
        fs::rename(path, target)?;
        Ok(())
    }

    /// How many bytes of a resumable upload of `blob` have arrived: the offset of the next chunk.
    /// A stored blob reports its full size, so the sender skips it.
    pub fn received(&self, blob: &BlobRef) -> Result<u64, BlobError> {
        if self.contains(&blob.hash) {
            return Ok(blob.size);
        }
        match fs::metadata(self.upload_path(&blob.hash)) {
            Ok(meta) => Ok(meta.len()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
            Err(e) => Err(e.into()),
        }
    }

    /// Appends one chunk of a resumable upload of `blob` at `offset`, which must equal
    /// [`received`](Self::received). The chunk that completes the blob verifies it and stores it;
    /// then the result is `Some`.
    pub fn write_chunk(
        &self,
        blob: &BlobRef,
        offset: u64,
        chunk: &[u8],
    ) -> Result<Option<Stored>, BlobError> {
        if blob.size > MAX_BLOB_SIZE {
            return Err(BlobError::TooLarge {
                limit: MAX_BLOB_SIZE,
            });
        }
        if self.contains(&blob.hash) {
            return Ok(Some(Stored {
                blob: *blob,
                created: false,
            }));
        }
        let received = self.received(blob)?;
        if offset != received {
            return Err(BlobError::OutOfOrder { offset, received });
        }
        let length = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
        if received.saturating_add(length) > blob.size {
            return Err(BlobError::TooLarge { limit: blob.size });
        }
        let path = self.upload_path(&blob.hash);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(chunk)?;
        if received + length < blob.size {
            file.sync_data()?;
            return Ok(None);
        }
        drop(file);
        let mut file = File::open(&path)?;
        file.seek(SeekFrom::Start(0))?;
        let (hash, size) = copy_hashing(&mut file, &mut io::sink(), blob.size)?;
        if hash != blob.hash || size != blob.size {
            // The upload restarts from zero.
            fs::remove_file(&path)?;
            return Err(BlobError::Mismatch {
                expected: blob.hash,
            });
        }
        let file = OpenOptions::new().append(true).open(&path)?;
        self.commit(
            Temp {
                file,
                path,
                keep: false,
            },
            hash,
            size,
        )
        .map(Some)
    }

    /// Every stored hash, for backups and audits (unordered).
    pub fn hashes(&self) -> Result<Vec<BlobHash>, BlobError> {
        let mut hashes = Vec::new();
        for first in read_dirs(&self.root.join("blobs"))? {
            for second in read_dirs(&first)? {
                for entry in fs::read_dir(&second)? {
                    let entry = entry?;
                    if let Some(hash) = entry.file_name().to_str().and_then(|n| n.parse().ok()) {
                        hashes.push(hash);
                    }
                }
            }
        }
        Ok(hashes)
    }

    /// Removes every blob that `references` reports unreferenced and that was stored more than
    /// `horizon` before `now`, and abandoned uploads and temporary files older than `horizon`.
    /// A blob newer than the horizon is kept even when unreferenced: its resource may not have
    /// arrived yet.
    pub fn collect(
        &self,
        references: &dyn BlobReferences,
        horizon: Duration,
        now: SystemTime,
    ) -> Result<Collected, BlobError> {
        let cutoff = now.checked_sub(horizon).unwrap_or(SystemTime::UNIX_EPOCH);
        let older = |path: &Path| -> Result<bool, BlobError> {
            Ok(fs::metadata(path)?.modified()? < cutoff)
        };
        let mut collected = Collected::default();
        for hash in self.hashes()? {
            let path = self.path(&hash);
            if older(&path)? && !references.is_referenced(&hash)? {
                fs::remove_file(&path)?;
                collected.blobs.push(hash);
            }
        }
        for dir in ["uploads", "tmp"] {
            for entry in fs::read_dir(self.root.join(dir))? {
                let path = entry?.path();
                if older(&path)? {
                    fs::remove_file(&path)?;
                    collected.partials += 1;
                }
            }
        }
        collected.blobs.sort();
        Ok(collected)
    }
}

/// A file in `tmp/`, removed on drop unless it was renamed into place.
struct Temp {
    file: File,
    path: PathBuf,
    keep: bool,
}

impl Temp {
    fn keep(mut self) {
        self.keep = true;
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn tempfile_in(dir: &Path) -> Result<Temp, BlobError> {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    for attempt in 0u32..100 {
        let path = dir.join(format!("{}-{nanos}-{attempt}", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                return Ok(Temp {
                    file,
                    path,
                    keep: false,
                });
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "no free temporary name").into())
}

/// Copies `from` into `to`, hashing it, failing past `limit` bytes.
fn copy_hashing(
    mut from: impl Read,
    to: &mut impl Write,
    limit: u64,
) -> Result<(BlobHash, u64), BlobError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = match from.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        size += u64::try_from(n).unwrap_or(u64::MAX);
        if size > limit {
            return Err(BlobError::TooLarge { limit });
        }
        hasher.update(&buffer[..n]);
        to.write_all(&buffer[..n])?;
    }
    Ok((BlobHash(hasher.finalize().into()), size))
}

fn read_dirs(dir: &Path) -> Result<Vec<PathBuf>, BlobError> {
    let mut dirs = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            dirs.push(entry.path());
        }
    }
    Ok(dirs)
}

/// Makes a rename durable (a no-op where directories cannot be opened, as on Windows).
fn sync_dir(dir: &Path) -> Result<(), BlobError> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}
