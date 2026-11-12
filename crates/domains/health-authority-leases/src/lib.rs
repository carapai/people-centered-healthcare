//! Authority Leases (write-authority.md § Hand-over rules; #135).
//!
//! An edge, the Facility Authority Node, can hand a desktop write authority over a scope for a
//! time-box: an outreach session's sub-store, whose stock the desktop then moves as the single
//! writer while away from the LAN. Authority never switches implicitly (§90):
//!
//! ```text
//! EdgeHolds --grant--> Active --holder asks--> Returning --outbox drained, edge acknowledges--> Returned
//! Active --expired, not returned--> Frozen --holder reconnects--> Returning
//! Frozen --admin-forced recovery (audited)--> Recovered
//! ```
//!
//! - **While a lease holds** (active, returning or frozen), the edge refuses writes in its scope:
//!   only the holder's operations, already made, may still arrive.
//! - **The holder writes** only while the lease is active and unexpired. Asking to return stops
//!   its new writes, so its outbox can drain; expiry freezes the scope at both ends.
//! - **Scopes do not overlap.** A store is under at most one holding lease.
//!
//! The rules are pure: the caller stores leases, reports the time and whether the holder's outbox
//! for the scope has drained, and raises the alert a freeze returns.

use std::collections::BTreeSet;

use health_time::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The longest lease: a time-box, not a hand-over of the store.
pub const MAX_LIFETIME: SignedDuration = SignedDuration::from_hours(30 * 24);

/// Where a lease stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LeaseState {
    /// The holder writes in the scope; the edge does not.
    Active,
    /// The holder asked to return it: it writes no more, and its outbox is draining.
    Returning {
        /// When it asked.
        requested_at: Timestamp,
    },
    /// It expired unreturned: nobody writes in the scope.
    Frozen {
        /// When it froze: the expiry.
        since: Timestamp,
    },
    /// Returned: the edge holds the scope again.
    Returned {
        /// When the edge acknowledged the drained outbox.
        at: Timestamp,
    },
    /// An admin took the scope back from a frozen lease.
    Recovered {
        /// Who.
        by: String,
        /// Why, for the audit.
        reason: String,
        /// When.
        at: Timestamp,
    },
}

/// A lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    /// Its id.
    pub id: Uuid,
    /// The desktop holding it: its enrolled device id.
    pub holder: String,
    /// The stores it covers.
    pub stores: BTreeSet<String>,
    /// Who granted it.
    pub granted_by: String,
    /// When.
    pub granted_at: Timestamp,
    /// When it expires unless returned.
    pub expires_at: Timestamp,
    /// Where it stands.
    pub state: LeaseState,
}

/// Why a lease was not granted or moved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeaseRefused {
    /// No store named.
    #[error("a lease covers at least one store")]
    Empty,
    /// The lifetime is not positive, or longer than [`MAX_LIFETIME`].
    #[error("a lease lasts more than nothing and at most 30 days")]
    Lifetime,
    /// A store is already under a holding lease.
    #[error("store {0} is already leased")]
    Overlaps(String),
    /// Asked by someone other than the holder.
    #[error("only the holder can do that")]
    NotHolder,
    /// Not allowed from where the lease stands.
    #[error("a lease that is {0} cannot do that")]
    State(&'static str),
    /// The holder's outbox for the scope has not drained.
    #[error("the holder's outbox for the lease has not drained")]
    NotDrained,
    /// A recovery needs a reason.
    #[error("a recovery needs a reason")]
    NoReason,
}

impl Lease {
    /// Grants `holder` the `stores` for `lifetime` from `now`, by `by`, unless one of them is
    /// under a holding lease among `existing`.
    pub fn grant(
        id: Uuid,
        holder: impl Into<String>,
        stores: BTreeSet<String>,
        by: impl Into<String>,
        now: Timestamp,
        lifetime: SignedDuration,
        existing: &[Lease],
    ) -> Result<Self, LeaseRefused> {
        if stores.is_empty() {
            return Err(LeaseRefused::Empty);
        }
        if lifetime <= SignedDuration::ZERO || lifetime > MAX_LIFETIME {
            return Err(LeaseRefused::Lifetime);
        }
        if let Some(store) = stores.iter().find(|store| {
            existing
                .iter()
                .any(|lease| lease.holds() && lease.covers(store))
        }) {
            return Err(LeaseRefused::Overlaps(store.clone()));
        }
        Ok(Self {
            id,
            holder: holder.into(),
            stores,
            granted_by: by.into(),
            granted_at: now,
            expires_at: now
                .checked_add(lifetime)
                .map_err(|_| LeaseRefused::Lifetime)?,
            state: LeaseState::Active,
        })
    }

    /// Whether the scope is out of the edge's hands: active, returning or frozen.
    pub fn holds(&self) -> bool {
        matches!(
            self.state,
            LeaseState::Active | LeaseState::Returning { .. } | LeaseState::Frozen { .. }
        )
    }

    /// Whether `store` is in its scope.
    pub fn covers(&self, store: &str) -> bool {
        self.stores.contains(store)
    }

    /// Whether `holder` may write in `store` at `now`: the lease is active, unexpired, theirs and
    /// covers it.
    pub fn holder_may_write(&self, holder: &str, store: &str, now: Timestamp) -> bool {
        self.state == LeaseState::Active
            && now < self.expires_at
            && self.holder == holder
            && self.covers(store)
    }

    /// Freezes the lease if it is active and expired at `now`. True when it froze: the caller
    /// raises the alert.
    pub fn expire(&mut self, now: Timestamp) -> bool {
        if self.state == LeaseState::Active && now >= self.expires_at {
            self.state = LeaseState::Frozen {
                since: self.expires_at,
            };
            true
        } else {
            false
        }
    }

    /// `holder` asks to return it at `now`: active or frozen (it reconnected), it starts
    /// returning, and the holder writes no more.
    pub fn request_return(&mut self, holder: &str, now: Timestamp) -> Result<(), LeaseRefused> {
        if self.holder != holder {
            return Err(LeaseRefused::NotHolder);
        }
        match self.state {
            LeaseState::Active | LeaseState::Frozen { .. } => {
                self.state = LeaseState::Returning { requested_at: now };
                Ok(())
            }
            _ => Err(LeaseRefused::State(self.state_name())),
        }
    }

    /// The edge acknowledges the return at `now`, once the holder's outbox for the scope has
    /// `drained`: the edge holds the scope again.
    pub fn complete_return(&mut self, drained: bool, now: Timestamp) -> Result<(), LeaseRefused> {
        if !matches!(self.state, LeaseState::Returning { .. }) {
            return Err(LeaseRefused::State(self.state_name()));
        }
        if !drained {
            return Err(LeaseRefused::NotDrained);
        }
        self.state = LeaseState::Returned { at: now };
        Ok(())
    }

    /// An admin, `by`, takes a frozen lease's scope back at `now` for `reason`: audited, and the
    /// holder's unsynced operations reconciled when they arrive.
    pub fn recover(
        &mut self,
        by: impl Into<String>,
        reason: impl Into<String>,
        now: Timestamp,
    ) -> Result<(), LeaseRefused> {
        if !matches!(self.state, LeaseState::Frozen { .. }) {
            return Err(LeaseRefused::State(self.state_name()));
        }
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(LeaseRefused::NoReason);
        }
        self.state = LeaseState::Recovered {
            by: by.into(),
            reason,
            at: now,
        };
        Ok(())
    }

    fn state_name(&self) -> &'static str {
        match self.state {
            LeaseState::Active => "active",
            LeaseState::Returning { .. } => "returning",
            LeaseState::Frozen { .. } => "frozen",
            LeaseState::Returned { .. } => "returned",
            LeaseState::Recovered { .. } => "recovered",
        }
    }
}

/// Whether the edge itself may write in `store`: no holding lease among `leases` covers it.
pub fn edge_may_write(leases: &[Lease], store: &str) -> bool {
    !leases
        .iter()
        .any(|lease| lease.holds() && lease.covers(store))
}

#[cfg(test)]
mod tests;
