//! The blob store against a real filesystem (ADR 0021): deduplication, verification on read,
//! the size cap, resumable uploads and garbage collection.

use std::collections::HashSet;
use std::io::{self, Read};
use std::time::{Duration, SystemTime};

use health_files::{
    BlobError, BlobHash, BlobRef, BlobReferences, CHUNK_SIZE, FsBlobStore, MAX_BLOB_SIZE,
};

fn store() -> (tempfile::TempDir, FsBlobStore) {
    let dir = tempfile::tempdir().expect("tmp");
    let store = FsBlobStore::open(dir.path()).expect("open");
    (dir, store)
}

fn files_in(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir).map(|d| d.count()).unwrap_or(0)
}

/// Deterministic, incompressible-looking content.
fn content(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

#[test]
fn hashes_print_and_parse_as_lowercase_hex() {
    let hash = BlobHash::of(b"abc");
    let text = hash.to_string();
    assert_eq!(
        text,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(text.parse::<BlobHash>(), Ok(hash));
    assert!(text.to_uppercase().parse::<BlobHash>().is_err());
    assert!(text[1..].parse::<BlobHash>().is_err());
}

#[test]
fn duplicate_uploads_deduplicate() {
    let (_dir, store) = store();
    let bytes = content(10_000, 1);
    let first = store.put(bytes.as_slice()).expect("put");
    assert!(first.created);
    assert_eq!(
        first.blob,
        BlobRef {
            hash: BlobHash::of(&bytes),
            size: 10_000
        }
    );
    let second = store.put(bytes.as_slice()).expect("put again");
    assert!(!second.created);
    let third = store
        .put_expecting(first.blob, bytes.as_slice())
        .expect("put declared");
    assert!(!third.created);
    assert_eq!(store.hashes().expect("hashes"), vec![first.blob.hash]);
    assert_eq!(store.read(&first.blob.hash).expect("read"), bytes);
}

#[test]
fn declared_content_must_match() {
    let (dir, store) = store();
    let bytes = content(5_000, 2);
    let declared = BlobRef {
        hash: BlobHash::of(b"something else"),
        size: 5_000,
    };
    assert!(matches!(
        store.put_expecting(declared, bytes.as_slice()),
        Err(BlobError::Mismatch { .. })
    ));
    let short = BlobRef {
        hash: BlobHash::of(&bytes),
        size: 4_000,
    };
    assert!(store.put_expecting(short, bytes.as_slice()).is_err());
    assert!(store.hashes().expect("hashes").is_empty());
    assert_eq!(files_in(&dir.path().join("tmp")), 0);
}

#[test]
fn reads_verify_integrity_and_quarantine_corruption() {
    let (dir, store) = store();
    let bytes = content(4_096, 3);
    let stored = store.put(bytes.as_slice()).expect("put");
    let hex = stored.blob.hash.to_string();
    let path = dir
        .path()
        .join("blobs")
        .join(&hex[..2])
        .join(&hex[2..4])
        .join(&hex);
    // Bit rot: the stored bytes change underneath the store.
    let mut rotten = bytes.clone();
    rotten[100] ^= 0x01;
    std::fs::write(&path, &rotten).expect("corrupt");

    assert!(
        matches!(store.read(&stored.blob.hash), Err(BlobError::Corrupt(h)) if h == stored.blob.hash)
    );
    assert!(!store.contains(&stored.blob.hash));
    assert_eq!(files_in(&dir.path().join("quarantine")), 1);
    // The blob can be fetched again.
    assert!(store.put(bytes.as_slice()).expect("re-put").created);
    assert_eq!(store.read(&stored.blob.hash).expect("read"), bytes);
    assert!(matches!(
        store.read(&BlobHash::of(b"absent")),
        Err(BlobError::NotFound(_))
    ));
}

#[test]
fn the_size_cap_is_enforced_while_streaming() {
    let (dir, store) = store();
    let oversized = io::repeat(7).take(MAX_BLOB_SIZE + 1);
    assert!(
        matches!(store.put(oversized), Err(BlobError::TooLarge { limit }) if limit == MAX_BLOB_SIZE)
    );
    assert_eq!(files_in(&dir.path().join("tmp")), 0);
    let exactly = io::repeat(7).take(MAX_BLOB_SIZE);
    assert_eq!(
        store.put(exactly).expect("at the cap").blob.size,
        MAX_BLOB_SIZE
    );
    let declared = BlobRef {
        hash: BlobHash::of(b"x"),
        size: MAX_BLOB_SIZE + 1,
    };
    assert!(matches!(
        store.write_chunk(&declared, 0, b"x"),
        Err(BlobError::TooLarge { .. })
    ));
}

#[test]
fn uploads_resume_in_chunks_across_restarts() {
    let (dir, store) = store();
    let chunk = usize::try_from(CHUNK_SIZE).expect("chunk");
    let bytes = content(chunk * 2 + chunk / 2, 4);
    let blob = BlobRef {
        hash: BlobHash::of(&bytes),
        size: bytes.len() as u64,
    };

    assert_eq!(store.received(&blob).expect("received"), 0);
    assert_eq!(
        store.write_chunk(&blob, 0, &bytes[..chunk]).expect("first"),
        None
    );
    // The node restarts; a new handle resumes where the upload stopped.
    let store = FsBlobStore::open(dir.path()).expect("reopen");
    let offset = store.received(&blob).expect("received");
    assert_eq!(offset, CHUNK_SIZE);
    assert!(matches!(
        store.write_chunk(&blob, 0, &bytes[..chunk]),
        Err(BlobError::OutOfOrder { offset: 0, received }) if received == CHUNK_SIZE
    ));
    assert_eq!(
        store
            .write_chunk(&blob, offset, &bytes[chunk..chunk * 2])
            .expect("second"),
        None
    );
    let done = store
        .write_chunk(&blob, CHUNK_SIZE * 2, &bytes[chunk * 2..])
        .expect("last")
        .expect("stored");
    assert!(done.created);
    assert_eq!(store.read(&blob.hash).expect("read"), bytes);
    assert_eq!(store.received(&blob).expect("received"), blob.size);
    assert_eq!(files_in(&dir.path().join("uploads")), 0);
    // A sender that retries a stored blob is told it is done.
    assert!(
        !store
            .write_chunk(&blob, 0, &bytes[..chunk])
            .expect("retry")
            .expect("stored")
            .created
    );
}

#[test]
fn a_corrupted_upload_restarts() {
    let (_dir, store) = store();
    let bytes = content(3_000, 5);
    let blob = BlobRef {
        hash: BlobHash::of(&bytes),
        size: 3_000,
    };
    store.write_chunk(&blob, 0, &bytes[..1_000]).expect("first");
    let mut wrong = bytes[1_000..].to_vec();
    wrong[0] ^= 0xff;
    assert!(matches!(
        store.write_chunk(&blob, 1_000, &wrong),
        Err(BlobError::Mismatch { .. })
    ));
    assert_eq!(store.received(&blob).expect("received"), 0);
    assert!(!store.contains(&blob.hash));
}

struct Referenced(HashSet<BlobHash>);

impl BlobReferences for Referenced {
    fn is_referenced(&self, hash: &BlobHash) -> Result<bool, BlobError> {
        Ok(self.0.contains(hash))
    }
}

#[test]
fn garbage_collection_removes_only_unreferenced_blobs_past_the_horizon() {
    let (dir, store) = store();
    let kept = store.put(&b"referenced"[..]).expect("put").blob.hash;
    let orphan = store.put(&b"orphan"[..]).expect("put").blob.hash;
    let pending = BlobRef {
        hash: BlobHash::of(b"pending upload"),
        size: 14,
    };
    store.write_chunk(&pending, 0, b"pending").expect("partial");
    let references = Referenced(HashSet::from([kept]));
    let horizon = Duration::from_secs(90 * 24 * 3600);

    // Within the horizon nothing goes, referenced or not: the resource may still be arriving.
    let now = SystemTime::now();
    assert_eq!(
        store.collect(&references, horizon, now).expect("collect"),
        Default::default()
    );

    let later = now + horizon + Duration::from_secs(60);
    let collected = store.collect(&references, horizon, later).expect("collect");
    assert_eq!(collected.blobs, vec![orphan]);
    assert_eq!(collected.partials, 1);
    assert!(store.contains(&kept));
    assert!(!store.contains(&orphan));
    assert_eq!(files_in(&dir.path().join("uploads")), 0);
}
