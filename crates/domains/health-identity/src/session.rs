use health_time::{SignedDuration, Timestamp};

/// Token and session lifetimes (offline-auth.md § Tokens and sessions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPolicy {
    /// Access-token lifetime.
    pub access_token: SignedDuration,
    /// A session is refreshable up to this long after its password login.
    pub hard_cap: SignedDuration,
}

impl Default for SessionPolicy {
    /// 15-minute access tokens; sessions end 12 hours after login.
    fn default() -> Self {
        Self {
            access_token: SignedDuration::from_mins(15),
            hard_cap: SignedDuration::from_hours(12),
        }
    }
}

/// A session's lifecycle: started by a password login, ended by logout, revocation or the cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Started at this `auth_time`.
    Live {
        /// The password login.
        auth_time: Timestamp,
    },
    /// Ended by logout or revocation; never revived.
    Ended,
}

impl SessionPolicy {
    /// When a session started at `auth_time` stops being refreshable.
    pub fn ends_at(&self, auth_time: Timestamp) -> Timestamp {
        auth_time.checked_add(self.hard_cap).unwrap_or(auth_time)
    }

    /// Whether `state` may issue a token at `now`.
    pub fn may_refresh(&self, state: SessionState, now: Timestamp) -> bool {
        match state {
            SessionState::Live { auth_time } => now < self.ends_at(auth_time),
            SessionState::Ended => false,
        }
    }

    /// An access token issued at `now` expires at the earlier of its lifetime and the session's
    /// end, so no token outlives the 12-hour cap.
    pub fn access_expiry(&self, auth_time: Timestamp, now: Timestamp) -> Timestamp {
        let token = now.checked_add(self.access_token).unwrap_or(now);
        token.min(self.ends_at(auth_time))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_end_twelve_hours_after_login() {
        let policy = SessionPolicy::default();
        let login = Timestamp::UNIX_EPOCH;
        let live = SessionState::Live { auth_time: login };
        let later = |h: i64, m: i64| {
            login
                .checked_add(SignedDuration::from_hours(h) + SignedDuration::from_mins(m))
                .unwrap_or(login)
        };
        assert!(policy.may_refresh(live, later(11, 59)));
        assert!(!policy.may_refresh(live, later(12, 0)));
        assert!(!policy.may_refresh(SessionState::Ended, login));
        assert_eq!(policy.access_expiry(login, later(0, 0)), later(0, 15));
        assert_eq!(policy.access_expiry(login, later(11, 50)), later(12, 0));
    }
}
