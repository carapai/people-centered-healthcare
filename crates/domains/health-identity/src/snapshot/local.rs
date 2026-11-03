//! Offline user and role administration on an edge (offline-auth.md § Offline user and role
//! administration; ADR 0011): a facility admin grants locally grantable roles for at most 7 days,
//! until central ratifies them, and revokes local access at once.
//!
//! What may never be granted offline: administration, the privacy officer's review, break-glass
//! eligibility, and sensitive-program access. Grants carry roles only, never program assignments,
//! and a role is refused if any of its permissions is one of [`NEVER_OFFLINE`].

use health_time::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{PermissionSnapshot, RoleAssignment, RolePermissions};
use crate::AccountId;

/// The permission to administer access on an edge offline.
pub const ADMINISTER_LOCAL: &str = "access.administer-local";

/// Permissions a role may not carry to be granted offline: administration, and break-glass
/// eligibility, its alerts and the privacy officer's review.
pub const NEVER_OFFLINE: &[&str] = &[
    ADMINISTER_LOCAL,
    "break-glass",
    "break-glass.alerts",
    "break-glass.review",
];

/// A role granted at an edge, offline, until central ratifies it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalGrant {
    /// The grant's id.
    pub id: Uuid,
    /// Whose access it extends.
    pub account: AccountId,
    /// Where.
    pub facility: String,
    /// The role granted.
    pub role: String,
    /// The facility admin who granted it.
    pub granted_by: AccountId,
    /// When.
    pub granted_at: Timestamp,
    /// When it lapses, unless central ratifies it first.
    pub not_after: Timestamp,
}

/// Why a role was not granted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GrantRefused {
    /// The configuration does not define the role.
    #[error("{0} is not a role")]
    Unknown(String),
    /// The country does not let it be granted locally.
    #[error("{0} is not locally grantable")]
    NotGrantable(String),
    /// It carries a permission never granted offline.
    #[error("{0} can never be granted offline")]
    NeverOffline(String),
    /// Past the 7 days a local grant may last.
    #[error("a local grant lasts at most {0} days")]
    TooLong(i64),
}

/// The local-grant rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalGrantPolicy {
    /// The longest a grant lasts before central must ratify it.
    pub max: SignedDuration,
}

impl Default for LocalGrantPolicy {
    /// Seven days.
    fn default() -> Self {
        Self {
            max: SignedDuration::from_hours(24 * 7),
        }
    }
}

impl LocalGrantPolicy {
    /// Grants `role` to `account` at `facility` for `days`, by `granted_by` at `now`, if the role
    /// is defined, on the country's `grantable` list, carries no [`NEVER_OFFLINE`] permission,
    /// and the grant fits the policy.
    #[allow(clippy::too_many_arguments)]
    pub fn grant(
        &self,
        role: &str,
        roles: &RolePermissions,
        grantable: &[String],
        account: AccountId,
        facility: &str,
        granted_by: AccountId,
        days: i64,
        now: Timestamp,
    ) -> Result<LocalGrant, GrantRefused> {
        let permissions = roles
            .0
            .get(role)
            .ok_or_else(|| GrantRefused::Unknown(role.to_owned()))?;
        if permissions
            .iter()
            .any(|p| NEVER_OFFLINE.contains(&p.as_str()))
        {
            return Err(GrantRefused::NeverOffline(role.to_owned()));
        }
        if !grantable.iter().any(|g| g == role) {
            return Err(GrantRefused::NotGrantable(role.to_owned()));
        }
        let max_days = self.max.as_hours() / 24;
        if !(1..=max_days).contains(&days) {
            return Err(GrantRefused::TooLong(max_days));
        }
        Ok(LocalGrant {
            id: Uuid::now_v7(),
            account,
            facility: facility.to_owned(),
            role: role.to_owned(),
            granted_by,
            granted_at: now,
            not_after: now
                .checked_add(SignedDuration::from_hours(24 * days))
                .unwrap_or(now),
        })
    }
}

/// `snapshot` with the roles of `grants` that are for its account and facility and unexpired at
/// `at`. Program assignments are never added.
pub fn with_grants(
    mut snapshot: PermissionSnapshot,
    grants: &[LocalGrant],
    at: Timestamp,
) -> PermissionSnapshot {
    for grant in grants {
        let applies = grant.account == snapshot.account
            && grant.facility == snapshot.facility
            && at < grant.not_after;
        if applies
            && !snapshot
                .roles
                .iter()
                .any(|r| r.role == grant.role && r.locations.is_empty())
        {
            snapshot.roles.push(RoleAssignment {
                role: grant.role.clone(),
                locations: Vec::new(),
            });
        }
    }
    snapshot
}

/// Local access revoked at an edge: the user signs in there no more, and their sessions end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRevocation {
    /// Whose access.
    pub account: AccountId,
    /// The facility admin who revoked it.
    pub revoked_by: AccountId,
    /// When.
    pub revoked_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    fn roles() -> RolePermissions {
        RolePermissions(BTreeMap::from([
            (
                "nurse".to_owned(),
                BTreeSet::from(["Observation.cru".to_owned()]),
            ),
            (
                "clerk".to_owned(),
                BTreeSet::from(["Patient.cru".to_owned()]),
            ),
            (
                "in-charge".to_owned(),
                BTreeSet::from(["Patient.cru".to_owned(), "break-glass.alerts".to_owned()]),
            ),
            (
                "admin".to_owned(),
                BTreeSet::from([ADMINISTER_LOCAL.to_owned()]),
            ),
        ]))
    }

    fn at(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    #[test]
    fn only_grantable_roles_without_offline_forbidden_permissions_for_up_to_seven_days() {
        let policy = LocalGrantPolicy::default();
        let grantable = vec![
            "nurse".to_owned(),
            "in-charge".to_owned(),
            "admin".to_owned(),
        ];
        let (who, admin) = (AccountId(Uuid::nil()), AccountId(Uuid::max()));
        let now = at("2026-11-03T09:00:00Z");
        let grant = |role: &str, days| {
            policy.grant(role, &roles(), &grantable, who, "F1", admin, days, now)
        };

        let ok = grant("nurse", 7).expect("granted");
        assert_eq!(ok.not_after, at("2026-11-10T09:00:00Z"));
        assert_eq!(grant("nurse", 8), Err(GrantRefused::TooLong(7)));
        assert_eq!(grant("nurse", 0), Err(GrantRefused::TooLong(7)));
        assert_eq!(
            grant("clerk", 1),
            Err(GrantRefused::NotGrantable("clerk".into()))
        );
        assert_eq!(
            grant("matron", 1),
            Err(GrantRefused::Unknown("matron".into()))
        );
        // Even if the country's list names them.
        assert_eq!(
            grant("in-charge", 1),
            Err(GrantRefused::NeverOffline("in-charge".into()))
        );
        assert_eq!(
            grant("admin", 1),
            Err(GrantRefused::NeverOffline("admin".into()))
        );
    }

    #[test]
    fn a_grant_adds_its_role_to_the_snapshot_until_it_lapses() {
        let policy = LocalGrantPolicy::default();
        let (who, admin) = (AccountId(Uuid::nil()), AccountId(Uuid::max()));
        let now = at("2026-11-03T09:00:00Z");
        let grant = policy
            .grant(
                "nurse",
                &roles(),
                &["nurse".to_owned()],
                who,
                "F1",
                admin,
                2,
                now,
            )
            .expect("granted");
        let snapshot = PermissionSnapshot {
            account: who,
            facility: "F1".into(),
            roles: vec![RoleAssignment {
                role: "clerk".into(),
                locations: Vec::new(),
            }],
            programs: Vec::new(),
            issued_at: now,
            not_after: at("2026-11-17T09:00:00Z"),
            snapshot_version: 1,
        };
        let names =
            |s: &PermissionSnapshot| s.roles.iter().map(|r| r.role.clone()).collect::<Vec<_>>();
        let grants = std::slice::from_ref(&grant);
        assert_eq!(
            names(&with_grants(snapshot.clone(), grants, now)),
            ["clerk", "nurse"]
        );
        assert_eq!(
            names(&with_grants(
                snapshot.clone(),
                grants,
                at("2026-11-05T09:00:00Z")
            )),
            ["clerk"]
        );
        let mut elsewhere = snapshot.clone();
        elsewhere.facility = "F2".into();
        assert_eq!(names(&with_grants(elsewhere, grants, now)), ["clerk"]);
        assert!(with_grants(snapshot, grants, now).programs.is_empty());
    }
}
