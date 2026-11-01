//! `status.json` under the backup root: the last backup, failure and test restore, for the
//! status page and its alerts (facility-edge.md § Health monitoring).

use std::fs;
use std::io::Write as _;
use std::path::Path;

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

/// What the scheduler last did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// The last attempt, good or not; the next is one interval later.
    pub last_attempt: Option<Timestamp>,
    /// The last backup written and verified.
    pub last_backup: Option<Done>,
    /// The last failed backup.
    pub last_failure: Option<Failed>,
    /// The last test restore.
    pub last_test: Option<Test>,
}

/// A backup written and verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Done {
    /// When.
    pub at: Timestamp,
    /// Its directory name under the root.
    pub backup: String,
}

/// A backup that failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failed {
    /// When.
    pub at: Timestamp,
    /// Why.
    pub error: String,
}

/// A test restore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Test {
    /// When.
    pub at: Timestamp,
    /// The backup restored, if there was one.
    pub backup: Option<String>,
    /// Why it failed; `None` when it passed.
    pub error: Option<String>,
}

const FILE: &str = "status.json";

impl Status {
    /// The status under `root`; empty before the first attempt, or when unreadable.
    pub fn read(root: &Path) -> Self {
        fs::read(root.join(FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Replaces the status under `root`: a temporary file, synced, then renamed.
    pub(super) fn write(&self, root: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let temp = root.join(format!("{FILE}.partial"));
        let mut file = fs::File::create(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(temp, root.join(FILE))
    }
}
