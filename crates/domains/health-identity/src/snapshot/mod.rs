//! Signed permission snapshots (ADR 0011; offline-auth.md § Permission snapshots): what a user
//! may do at one facility while the edge cannot reach central. Central signs them; this module
//! holds the rules for their contents, lifetime and use, and the clock floor that keeps expiry
//! honest when the local clock is wrong.

mod clock;
mod roles;

use health_time::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

use crate::AccountId;

pub use clock::{ClockBroken, ClockFloor};
pub use roles::{Permission, RolePermissions, UnknownRole};

/// A role held at the snapshot's facility, optionally limited to some of its locations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleAssignment {
    /// The role code, as the configuration package names it.
    pub role: String,
    /// Location ids the role is limited to; empty means the whole facility.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<String>,
}

/// One user's permissions at one facility, as central issued them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionSnapshot {
    /// Whose permissions.
    pub account: AccountId,
    /// Where they apply.
    pub facility: String,
    /// Roles (RBAC).
    pub roles: Vec<RoleAssignment>,
    /// Program assignments (ABAC): they open records carrying the matching sensitivity labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub programs: Vec<String>,
    /// Central's time at issue.
    pub issued_at: Timestamp,
    /// The hard expiry: no login at or after it.
    pub not_after: Timestamp,
    /// Increases with every snapshot for this user and facility; carried in tokens.
    pub snapshot_version: u64,
}

/// Lifetimes from the country configuration package (offline-auth.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotPolicy {
    lifetime: SignedDuration,
    /// How long before expiry the user is warned.
    pub warn_before: SignedDuration,
}

/// A configured lifetime outside 7 to 30 days.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a snapshot lifetime is 7 to 30 days, not {days}")]
pub struct LifetimeOutOfRange {
    /// The days configured.
    pub days: i64,
}

impl Default for SnapshotPolicy {
    /// 14 days, with a warning from 3 days before expiry.
    fn default() -> Self {
        Self {
            lifetime: SignedDuration::from_hours(14 * 24),
            warn_before: SignedDuration::from_hours(3 * 24),
        }
    }
}

impl SnapshotPolicy {
    /// The policy for a country that sets the lifetime to `days`.
    pub fn with_lifetime_days(days: i64) -> Result<Self, LifetimeOutOfRange> {
        if !(7..=30).contains(&days) {
            return Err(LifetimeOutOfRange { days });
        }
        Ok(Self {
            lifetime: SignedDuration::from_hours(days * 24),
            ..Self::default()
        })
    }

    /// The configured lifetime.
    pub fn lifetime(&self) -> SignedDuration {
        self.lifetime
    }

    /// The `not_after` of a snapshot central issues at `issued_at`.
    pub fn not_after(&self, issued_at: Timestamp) -> Timestamp {
        issued_at.checked_add(self.lifetime).unwrap_or(issued_at)
    }

    /// Whether `snapshot` allows a login at `now`, which must already have passed the clock
    /// floor ([`ClockFloor::check`]).
    pub fn check(
        &self,
        snapshot: &PermissionSnapshot,
        now: Timestamp,
    ) -> Result<SnapshotStatus, SnapshotRefused> {
        if now >= snapshot.not_after {
            return Err(SnapshotRefused::Expired {
                at: snapshot.not_after,
            });
        }
        // A snapshot from the future means one of the clocks is wrong; it is not trusted.
        if now < snapshot.issued_at {
            return Err(SnapshotRefused::NotYetValid);
        }
        let warn_from = snapshot
            .not_after
            .checked_sub(self.warn_before)
            .unwrap_or(snapshot.not_after);
        if now >= warn_from {
            Ok(SnapshotStatus::ExpiresSoon {
                at: snapshot.not_after,
            })
        } else {
            Ok(SnapshotStatus::Valid)
        }
    }
}

/// A usable snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotStatus {
    /// More than the warning period remains.
    Valid,
    /// Usable, but the user should be warned it expires at `at`.
    ExpiresSoon {
        /// The expiry.
        at: Timestamp,
    },
}

/// Why a snapshot does not allow a login.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SnapshotRefused {
    /// Past `not_after`.
    #[error("the permission snapshot expired at {at}")]
    Expired {
        /// The expiry.
        at: Timestamp,
    },
    /// Issued after the current time.
    #[error("the permission snapshot is not valid yet")]
    NotYetValid,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(n: i64) -> Timestamp {
        Timestamp::UNIX_EPOCH
            .checked_add(SignedDuration::from_hours(n * 24))
            .unwrap_or(Timestamp::UNIX_EPOCH)
    }

    fn snapshot(policy: &SnapshotPolicy) -> PermissionSnapshot {
        PermissionSnapshot {
            account: AccountId(uuid::Uuid::nil()),
            facility: "F1".into(),
            roles: vec![RoleAssignment {
                role: "clinician".into(),
                locations: vec![],
            }],
            programs: vec![],
            issued_at: day(100),
            not_after: policy.not_after(day(100)),
            snapshot_version: 1,
        }
    }

    #[test]
    fn a_snapshot_lasts_fourteen_days_and_warns_for_the_last_three() {
        let policy = SnapshotPolicy::default();
        let s = snapshot(&policy);
        assert_eq!(s.not_after, day(114));
        assert_eq!(policy.check(&s, day(100)), Ok(SnapshotStatus::Valid));
        assert_eq!(
            policy.check(&s, day(111)),
            Ok(SnapshotStatus::ExpiresSoon { at: day(114) })
        );
        assert_eq!(
            policy.check(&s, day(114)),
            Err(SnapshotRefused::Expired { at: day(114) })
        );
        assert_eq!(policy.check(&s, day(99)), Err(SnapshotRefused::NotYetValid));
    }

    #[test]
    fn the_lifetime_is_seven_to_thirty_days() {
        assert_eq!(
            SnapshotPolicy::with_lifetime_days(7).map(|p| p.lifetime()),
            Ok(SignedDuration::from_hours(168))
        );
        assert!(SnapshotPolicy::with_lifetime_days(30).is_ok());
        assert_eq!(
            SnapshotPolicy::with_lifetime_days(6),
            Err(LifetimeOutOfRange { days: 6 })
        );
        assert_eq!(
            SnapshotPolicy::with_lifetime_days(31),
            Err(LifetimeOutOfRange { days: 31 })
        );
    }
}
