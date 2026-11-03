//! The package tarball: `package/…` entries in a gzip-compressed tar.
//!
//! Writing is deterministic, so one set of files always gives one sha-256: entries in path
//! order, mode 0644, owner 0, mtime 0, and a gzip header with no name or time. Reading takes an
//! archive from anywhere (sync, USB) and so trusts nothing in it: only regular files under
//! `package/`, no links, no `..` or absolute paths, no duplicates, and bounded sizes.

use std::collections::BTreeMap;
use std::io::Read;

use flate2::Compression;
use flate2::read::GzDecoder;
use health_configuration::package::ROOT;

use crate::Error;

/// A package's files, by path under `package/`.
pub type Files = BTreeMap<String, Vec<u8>>;

/// How much an archive may unpack to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The most entries.
    pub entries: usize,
    /// The largest single file, in bytes.
    pub file_bytes: u64,
    /// All files together, in bytes.
    pub total_bytes: u64,
}

impl Default for Limits {
    /// Room for a large terminology package: 50,000 files, 512 MiB each, 2 GiB in all.
    fn default() -> Self {
        Self {
            entries: 50_000,
            file_bytes: 512 << 20,
            total_bytes: 2 << 30,
        }
    }
}

/// Writes `files` as a deterministic package tarball.
pub(crate) fn write(files: &Files) -> Result<Vec<u8>, Error> {
    let encoder = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), Compression::best());
    let mut builder = tar::Builder::new(encoder);
    builder.mode(tar::HeaderMode::Deterministic);
    for (path, bytes) in files {
        check_path(path)?;
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        builder
            .append_data(&mut header, format!("{ROOT}{path}"), bytes.as_slice())
            .map_err(|e| Error::Archive(e.to_string()))?;
    }
    let encoder = builder
        .into_inner()
        .map_err(|e| Error::Archive(e.to_string()))?;
    encoder.finish().map_err(|e| Error::Archive(e.to_string()))
}

/// Reads a package tarball's files, within `limits`.
pub(crate) fn read(tgz: &[u8], limits: Limits) -> Result<Files, Error> {
    let mut archive = tar::Archive::new(GzDecoder::new(tgz));
    let mut files = Files::new();
    let mut total: u64 = 0;
    let entries = archive
        .entries()
        .map_err(|e| Error::Archive(e.to_string()))?;
    for (count, entry) in entries.enumerate() {
        if count >= limits.entries {
            return Err(Error::TooLarge(format!(
                "more than {} entries",
                limits.entries
            )));
        }
        let entry = entry.map_err(|e| Error::Archive(e.to_string()))?;
        let raw = entry
            .path()
            .map_err(|e| Error::Archive(e.to_string()))?
            .to_string_lossy()
            .into_owned();
        match entry.header().entry_type() {
            tar::EntryType::Directory => continue,
            tar::EntryType::Regular | tar::EntryType::Continuous => {}
            _ => return Err(Error::Unsafe(raw)),
        }
        let path = raw
            .strip_prefix(ROOT)
            .ok_or_else(|| Error::Unsafe(raw.clone()))?
            .to_owned();
        check_path(&path).map_err(|_| Error::Unsafe(raw.clone()))?;
        let size = entry
            .header()
            .size()
            .map_err(|e| Error::Archive(e.to_string()))?;
        total = total.saturating_add(size);
        if size > limits.file_bytes || total > limits.total_bytes {
            return Err(Error::TooLarge(raw));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
        entry
            .take(size)
            .read_to_end(&mut bytes)
            .map_err(|e| Error::Archive(e.to_string()))?;
        if files.insert(path, bytes).is_some() {
            return Err(Error::Unsafe(format!("{raw} (twice)")));
        }
    }
    Ok(files)
}

/// A relative path of plain components, `/`-separated: no `.`, `..`, empty parts, backslashes,
/// drive letters or NULs.
fn check_path(path: &str) -> Result<(), Error> {
    let plain = !path.is_empty()
        && path.split('/').all(|part| {
            !part.is_empty() && part != "." && part != ".." && !part.contains(['\\', ':', '\0'])
        });
    if plain {
        Ok(())
    } else {
        Err(Error::Unsafe(path.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> Files {
        Files::from([
            ("package.json".to_owned(), b"{}".to_vec()),
            ("configuration/layouts/a.json".to_owned(), b"[]".to_vec()),
        ])
    }

    #[test]
    fn writing_is_deterministic_and_reads_back() {
        let first = write(&files()).expect("write");
        assert_eq!(write(&files()).expect("write"), first);
        assert_eq!(read(&first, Limits::default()).expect("read"), files());
    }

    #[test]
    fn unsafe_paths_are_neither_written_nor_read() {
        for path in [
            "../etc/passwd",
            "/etc/passwd",
            "a/../../b",
            "a//b",
            "./a",
            "a\\b",
            "",
        ] {
            let mut bad = files();
            bad.insert(path.to_owned(), Vec::new());
            assert!(matches!(write(&bad), Err(Error::Unsafe(_))), "{path:?}");
        }
    }

    /// A tarball with one entry, its name written raw, as a hostile archive would.
    fn hostile(name: &str, kind: tar::EntryType) -> Vec<u8> {
        let mut header = tar::Header::new_gnu();
        let raw = &mut header.as_old_mut().name;
        raw[..name.len()].copy_from_slice(name.as_bytes());
        header.set_entry_type(kind);
        header.set_size(if kind == tar::EntryType::Regular {
            2
        } else {
            0
        });
        if kind == tar::EntryType::Symlink {
            header.set_link_name("/etc/passwd").expect("link name");
        }
        header.set_mode(0o644);
        header.set_cksum();
        let mut tar = Vec::from(header.as_bytes().as_slice());
        if kind == tar::EntryType::Regular {
            tar.extend_from_slice(b"{}");
            tar.resize(tar.len() + 510, 0);
        }
        tar.resize(tar.len() + 1024, 0);
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), Compression::fast());
        std::io::Write::write_all(&mut gz, &tar).expect("gzip");
        gz.finish().expect("gzip")
    }

    #[test]
    fn hostile_archives_are_refused() {
        for (name, kind) in [
            ("package/../../etc/cron.d/x", tar::EntryType::Regular),
            ("/etc/passwd", tar::EntryType::Regular),
            ("elsewhere/package.json", tar::EntryType::Regular),
            ("package/link.json", tar::EntryType::Symlink),
            ("package/hard.json", tar::EntryType::Link),
            ("package/fifo", tar::EntryType::Fifo),
        ] {
            assert!(
                matches!(
                    read(&hostile(name, kind), Limits::default()),
                    Err(Error::Unsafe(_))
                ),
                "{name}"
            );
        }
        assert_eq!(
            read(
                &hostile("package/package.json", tar::EntryType::Regular),
                Limits::default()
            )
            .ok(),
            Some(Files::from([("package.json".to_owned(), b"{}".to_vec())]))
        );
    }

    #[test]
    fn limits_bound_what_an_archive_unpacks_to() {
        let tgz = write(&files()).expect("write");
        let tight = Limits {
            entries: 1,
            ..Limits::default()
        };
        assert!(matches!(read(&tgz, tight), Err(Error::TooLarge(_))));
        let small = Limits {
            file_bytes: 1,
            ..Limits::default()
        };
        assert!(matches!(read(&tgz, small), Err(Error::TooLarge(_))));
        assert!(matches!(
            read(b"not a tarball", Limits::default()),
            Err(Error::Archive(_))
        ));
    }
}
