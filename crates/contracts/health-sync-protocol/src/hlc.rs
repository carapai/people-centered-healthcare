use health_time::Timestamp;
use serde::{Deserialize, Serialize};

/// A hybrid logical clock reading: wall-clock milliseconds and a counter for events within the
/// same millisecond, or while the wall clock lags a peer's.
///
/// It orders items for audit and display. It never decides a conflict (synchronization.md
/// § Envelope); `parent_version_id` does that.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Hlc {
    /// Milliseconds since the Unix epoch.
    pub wall_ms: i64,
    /// Events at the same `wall_ms`.
    pub logical: u32,
}

impl Hlc {
    /// The reading for a local event or a send, after `self`.
    #[must_use]
    pub fn tick(self, now: Timestamp) -> Self {
        let wall_ms = now.as_millisecond();
        if wall_ms > self.wall_ms {
            Self {
                wall_ms,
                logical: 0,
            }
        } else {
            Self {
                wall_ms: self.wall_ms,
                logical: self.logical.saturating_add(1),
            }
        }
    }

    /// The reading after receiving `remote`: later than both `self` and `remote`, so a peer's
    /// clock running ahead never makes this node's next event sort before it.
    #[must_use]
    pub fn observe(self, remote: Hlc, now: Timestamp) -> Self {
        let wall_ms = now.as_millisecond().max(self.wall_ms).max(remote.wall_ms);
        let logical = match (wall_ms == self.wall_ms, wall_ms == remote.wall_ms) {
            (true, true) => self.logical.max(remote.logical).saturating_add(1),
            (true, false) => self.logical.saturating_add(1),
            (false, true) => remote.logical.saturating_add(1),
            (false, false) => 0,
        };
        Self { wall_ms, logical }
    }
}
