//! The telemetry buffer (facility-edge.md § Health monitoring): every exported log line is
//! written to standard output and appended to segment files under the telemetry directory,
//! where the shipper picks them up when central is reachable. The buffer is
//! bounded; when it is full the oldest segments are dropped first.
//!
//! Lines arrive from the export layer, already through the allowlist, so the buffer holds no
//! personal health data. Nothing here logs: a line written while the buffer is locked would
//! deadlock.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use health_observability::{LogSink, Stdout};

/// The default bound: a week or more of a quiet node's lines.
pub const DEFAULT_CAP: u64 = 64 * 1024 * 1024;
/// A segment is closed once it reaches this size.
const SEGMENT_BYTES: u64 = 256 * 1024;
const SUFFIX: &str = "ndjson";

/// A bounded on-disk buffer of log lines; the edge's [`LogSink`].
#[derive(Debug)]
pub struct Buffer {
    dir: PathBuf,
    cap: u64,
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    next: u64,
    current: Option<(PathBuf, File, u64)>,
    buffered: u64,
    dropped: u64,
}

impl Buffer {
    /// Opens the buffer in `dir`, keeping what an earlier run left unshipped, bounded at `cap`
    /// bytes (at least two segments).
    pub fn open(dir: impl Into<PathBuf>, cap: u64) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        let segments = segments(&dir)?;
        let next = segments.last().map_or(0, |(seq, _, _)| seq + 1);
        let buffered = segments.iter().map(|(_, _, size)| size).sum();
        Ok(Self {
            dir,
            cap: cap.max(2 * SEGMENT_BYTES),
            state: Mutex::new(State {
                next,
                current: None,
                buffered,
                dropped: 0,
            }),
        })
    }

    /// Bytes waiting to be shipped, and bytes dropped because the buffer was full.
    pub fn stats(&self) -> (u64, u64) {
        let state = self.lock();
        (state.buffered, state.dropped)
    }

    /// Closes the segment being written and returns every segment, oldest first, for shipping.
    pub(crate) fn seal(&self) -> Vec<PathBuf> {
        self.lock().current = None;
        segments(&self.dir)
            .map(|s| s.into_iter().map(|(_, path, _)| path).collect())
            .unwrap_or_default()
    }

    /// Removes a segment the collector accepted.
    pub(crate) fn shipped(&self, segment: &Path) {
        let mut state = self.lock();
        let size = fs::metadata(segment).map_or(0, |m| m.len());
        if fs::remove_file(segment).is_ok() {
            state.buffered = state.buffered.saturating_sub(size);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn append(&self, line: &str) -> io::Result<()> {
        let mut state = self.lock();
        let full = state
            .current
            .as_ref()
            .is_none_or(|(_, _, size)| *size >= SEGMENT_BYTES);
        if full {
            let path = self.dir.join(format!("{:020}.{SUFFIX}", state.next));
            state.next += 1;
            let file = OpenOptions::new().create(true).append(true).open(&path)?;
            state.current = Some((path, file, 0));
        }
        let bytes = line.len() as u64 + 1;
        if let Some((_, file, size)) = state.current.as_mut() {
            file.write_all(line.as_bytes())?;
            file.write_all(b"\n")?;
            *size += bytes;
        }
        state.buffered += bytes;
        if state.buffered > self.cap {
            let current = state.current.as_ref().map(|(path, _, _)| path.clone());
            for (_, path, size) in segments(&self.dir)? {
                if state.buffered <= self.cap || Some(&path) == current.as_ref() {
                    break;
                }
                fs::remove_file(&path)?;
                state.buffered = state.buffered.saturating_sub(size);
                state.dropped += size;
            }
        }
        Ok(())
    }
}

impl LogSink for Buffer {
    fn write(&self, line: &str) {
        Stdout.write(line);
        // A full or failed disk loses telemetry, never the node.
        let _ = self.append(line);
    }
}

/// The segments in `dir`, oldest first, with their sizes.
fn segments(dir: &Path) -> io::Result<Vec<(u64, PathBuf, u64)>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some(SUFFIX) {
            continue;
        }
        let Some(seq) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse().ok())
        else {
            continue;
        };
        let size = fs::metadata(&path).map_or(0, |m| m.len());
        found.push((seq, path, size));
    }
    found.sort_by_key(|(seq, _, _)| *seq);
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: usize) -> String {
        format!(r#"{{"n":{n},"pad":"{}"}}"#, "x".repeat(1000))
    }

    #[test]
    fn lines_are_kept_across_restarts_and_shipped_segments_removed() {
        let dir = tempfile::tempdir().expect("dir");
        let buffer = Buffer::open(dir.path(), DEFAULT_CAP).expect("open");
        buffer.append("one").expect("append");
        buffer.append("two").expect("append");
        drop(buffer);
        let buffer = Buffer::open(dir.path(), DEFAULT_CAP).expect("reopen");
        assert_eq!(buffer.stats(), (8, 0));
        buffer.append("three").expect("append");
        let sealed = buffer.seal();
        assert_eq!(sealed.len(), 2, "a reopened buffer starts a new segment");
        let text: Vec<String> = sealed
            .iter()
            .map(|p| fs::read_to_string(p).expect("read"))
            .collect();
        assert_eq!(text, vec!["one\ntwo\n", "three\n"]);
        for segment in &sealed {
            buffer.shipped(segment);
        }
        assert_eq!(buffer.stats(), (0, 0));
        assert!(buffer.seal().is_empty());
    }

    #[test]
    fn a_full_buffer_drops_the_oldest_segments() {
        let dir = tempfile::tempdir().expect("dir");
        let buffer = Buffer::open(dir.path(), 0).expect("open"); // the floor: two segments
        for n in 0..1200 {
            buffer.append(&line(n)).expect("append");
        }
        let (buffered, dropped) = buffer.stats();
        assert!(buffered <= 2 * SEGMENT_BYTES, "{buffered}");
        assert!(dropped > 0);
        let sealed = buffer.seal();
        let newest = fs::read_to_string(sealed.last().expect("a segment")).expect("read");
        assert!(
            newest.ends_with(&format!("{}\n", line(1199))),
            "the newest lines are kept"
        );
        let oldest = fs::read_to_string(&sealed[0]).expect("read");
        assert!(!oldest.contains(&line(0)), "the oldest lines are gone");
    }
}
