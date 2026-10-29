//! Time for the platform: `jiff` timestamps and an injectable [`Clock`].
//!
//! Domain and application code never reads the system clock directly; the runtime injects a
//! [`Clock`] (dependency-rules.md rule 6), and tests use [`FixedClock`].

use std::sync::Mutex;

pub use jiff::{SignedDuration, Timestamp};

/// The source of "now".
pub trait Clock: Send + Sync {
    /// The current instant.
    fn now(&self) -> Timestamp;
}

/// The operating-system clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}

/// A clock that only moves when told to, for tests.
#[derive(Debug)]
pub struct FixedClock(Mutex<Timestamp>);

impl FixedClock {
    /// A clock stopped at `at`.
    pub fn new(at: Timestamp) -> Self {
        Self(Mutex::new(at))
    }

    /// Moves the clock forward (or back, with a negative duration).
    pub fn advance(&self, by: SignedDuration) {
        let mut now = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *now = now.checked_add(by).unwrap_or(*now);
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_clock_moves_only_when_advanced() {
        let start: Timestamp = "2026-10-29T08:00:00Z".parse().expect("timestamp");
        let clock = FixedClock::new(start);
        assert_eq!(clock.now(), start);
        clock.advance(SignedDuration::from_secs(90));
        assert_eq!(clock.now().to_string(), "2026-10-29T08:01:30Z");
    }
}
