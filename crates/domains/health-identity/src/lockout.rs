use health_time::{SignedDuration, Timestamp};

/// Lockout after failed logins (offline-auth.md § Credentials).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockoutPolicy {
    /// Consecutive failures that lock the account for [`lock_for`](Self::lock_for).
    pub threshold: u32,
    /// How long each lock lasts.
    pub lock_for: SignedDuration,
    /// Consecutive failures after which only an administrator can unlock.
    pub hard_cap: u32,
}

impl Default for LockoutPolicy {
    /// 10 failures lock for 15 minutes; the hard cap is 100.
    fn default() -> Self {
        Self {
            threshold: 10,
            lock_for: SignedDuration::from_mins(15),
            hard_cap: 100,
        }
    }
}

/// Why a login is refused before the password is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Locked {
    /// Locked until this instant.
    #[error("locked until {0}")]
    Until(Timestamp),
    /// Past the hard cap: an administrator must unlock it.
    #[error("locked until an administrator unlocks it")]
    HardCap,
}

/// One account's consecutive failures. A success resets them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Lockout {
    /// Consecutive failed logins.
    pub failures: u32,
    /// The end of the current lock, if any.
    pub locked_until: Option<Timestamp>,
}

impl Lockout {
    /// Whether a login may be attempted at `now`. A locked account is refused without checking
    /// the password, so a lock cannot be probed.
    pub fn check(&self, policy: &LockoutPolicy, now: Timestamp) -> Result<(), Locked> {
        if self.failures >= policy.hard_cap {
            return Err(Locked::HardCap);
        }
        match self.locked_until {
            Some(until) if now < until => Err(Locked::Until(until)),
            _ => Ok(()),
        }
    }

    /// The state after a failed password at `now`: every `threshold`th consecutive failure locks.
    #[must_use]
    pub fn failed(self, policy: &LockoutPolicy, now: Timestamp) -> Self {
        let failures = self.failures.saturating_add(1);
        let locked_until = if policy.threshold > 0 && failures.is_multiple_of(policy.threshold) {
            now.checked_add(policy.lock_for).ok()
        } else {
            self.locked_until
        };
        Self {
            failures,
            locked_until,
        }
    }

    /// The state after a successful login, or an administrator's unlock.
    #[must_use]
    pub fn reset(self) -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minute: i64) -> Timestamp {
        Timestamp::UNIX_EPOCH
            .checked_add(SignedDuration::from_mins(minute))
            .unwrap_or(Timestamp::UNIX_EPOCH)
    }

    #[test]
    fn ten_failures_lock_for_fifteen_minutes() {
        let policy = LockoutPolicy::default();
        let mut state = Lockout::default();
        for _ in 0..9 {
            state = state.failed(&policy, at(0));
            assert_eq!(state.check(&policy, at(0)), Ok(()));
        }
        state = state.failed(&policy, at(0));
        assert_eq!(state.check(&policy, at(14)), Err(Locked::Until(at(15))));
        assert_eq!(state.check(&policy, at(15)), Ok(()));
        assert_eq!(state.reset().check(&policy, at(0)), Ok(()));
    }

    #[test]
    fn the_hard_cap_needs_an_administrator() {
        let policy = LockoutPolicy::default();
        let mut state = Lockout::default();
        for i in 0..100 {
            state = state.failed(&policy, at(i * 20));
        }
        assert_eq!(state.check(&policy, at(10_000)), Err(Locked::HardCap));
        assert_eq!(state.reset().check(&policy, at(10_000)), Ok(()));
    }
}
