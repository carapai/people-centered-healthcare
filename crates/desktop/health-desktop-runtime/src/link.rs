//! The desktop's link to its edge (desktop-client.md § Online and offline under an edge; #133).
//!
//! ```text
//! PassThrough --(edge unreachable for 30 s)--> Offline --(edge reached)--> Draining
//! Draining --(outbox drained)--> PassThrough;   Draining --(edge lost)--> Offline
//! ```
//!
//! - **PassThrough:** commands go to the edge, the authority. The local replica follows by pull.
//!   A failure inside the 30 s grace is answered as unavailable, and the client retries, so a
//!   write that may have reached the edge is never also made locally.
//! - **Offline:** commands are answered locally. Writes become Candidate Versions in the outbox.
//! - **Draining:** still local, keeping order, until the outbox has gone up.
//!
//! The link only decides; the caller reports what happened. It holds no clock: each report
//! carries its instant, so the rules are tested without waiting.

use std::time::{Duration, Instant};

/// How long the edge may be unreachable before the desktop works offline.
pub const GRACE: Duration = Duration::from_secs(30);

/// Where the link stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Commands go to the edge.
    PassThrough,
    /// Commands are answered locally; the edge cannot be reached.
    Offline,
    /// The edge is back; commands stay local until the outbox has drained.
    Draining,
}

/// Where a command goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// To the edge.
    Edge,
    /// To the local node.
    Local,
}

/// The link's state.
#[derive(Debug, Clone)]
pub struct Link {
    mode: Mode,
    /// When the edge was first missed since it was last reached, while passing through.
    failing_since: Option<Instant>,
    grace: Duration,
}

impl Default for Link {
    fn default() -> Self {
        Self::new(GRACE)
    }
}

impl Link {
    /// A link that passes through until the edge has been unreachable for `grace`.
    pub fn new(grace: Duration) -> Self {
        Self {
            mode: Mode::PassThrough,
            failing_since: None,
            grace,
        }
    }

    /// Where it stands.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Where the next command goes.
    pub fn route(&self) -> Route {
        match self.mode {
            Mode::PassThrough => Route::Edge,
            Mode::Offline | Mode::Draining => Route::Local,
        }
    }

    /// The edge answered, by a command or a probe, at `now`.
    pub fn reached(&mut self, _now: Instant) {
        self.failing_since = None;
        if self.mode == Mode::Offline {
            self.mode = Mode::Draining;
        }
    }

    /// The edge could not be reached at `now`.
    pub fn unreachable(&mut self, now: Instant) {
        match self.mode {
            Mode::PassThrough => {
                let since = *self.failing_since.get_or_insert(now);
                if now.saturating_duration_since(since) >= self.grace {
                    self.mode = Mode::Offline;
                    self.failing_since = None;
                }
            }
            Mode::Draining => self.mode = Mode::Offline,
            Mode::Offline => {}
        }
    }

    /// The outbox has nothing left for the edge: draining ends.
    pub fn drained(&mut self) {
        if self.mode == Mode::Draining {
            self.mode = Mode::PassThrough;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_edge_is_used_until_it_has_been_unreachable_for_thirty_seconds() {
        let start = Instant::now();
        let mut link = Link::default();
        assert_eq!(link.route(), Route::Edge);
        link.unreachable(start);
        link.unreachable(start + Duration::from_secs(29));
        assert_eq!(link.mode(), Mode::PassThrough, "inside the grace");
        // Reaching it resets the grace.
        link.reached(start + Duration::from_secs(29));
        link.unreachable(start + Duration::from_secs(31));
        assert_eq!(link.mode(), Mode::PassThrough);
        link.unreachable(start + Duration::from_secs(61));
        assert_eq!(link.mode(), Mode::Offline);
        assert_eq!(link.route(), Route::Local);
    }

    #[test]
    fn back_online_it_drains_before_passing_through_again() {
        let start = Instant::now();
        let mut link = Link::new(Duration::ZERO);
        link.unreachable(start);
        assert_eq!(link.mode(), Mode::Offline);
        // Draining only follows being offline.
        link.drained();
        assert_eq!(link.mode(), Mode::Offline);
        link.reached(start);
        assert_eq!(link.mode(), Mode::Draining);
        assert_eq!(link.route(), Route::Local, "writes keep their order");
        // Lost again while draining: offline at once.
        link.unreachable(start);
        assert_eq!(link.mode(), Mode::Offline);
        link.reached(start);
        link.drained();
        assert_eq!(link.mode(), Mode::PassThrough);
        assert_eq!(link.route(), Route::Edge);
    }
}
