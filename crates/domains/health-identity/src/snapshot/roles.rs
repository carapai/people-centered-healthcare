use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::PermissionSnapshot;

/// A permission code, as the configuration package names it (e.g. `clinical-record.read`).
pub type Permission = String;

/// Role → permission mappings: a Published Definition in the country configuration package
/// (offline-auth.md § Authorization model).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RolePermissions(pub BTreeMap<String, BTreeSet<Permission>>);

/// A snapshot names a role the configuration does not define.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown role {0:?}")]
pub struct UnknownRole(pub String);

impl RolePermissions {
    /// Checks that every role in `snapshot` is defined; central refuses to sign one that is not.
    pub fn validate(&self, snapshot: &PermissionSnapshot) -> Result<(), UnknownRole> {
        match snapshot
            .roles
            .iter()
            .find(|r| !self.0.contains_key(&r.role))
        {
            Some(role) => Err(UnknownRole(role.role.clone())),
            None => Ok(()),
        }
    }

    /// The union of the permissions of the snapshot's roles. Roles the configuration no longer
    /// defines grant nothing.
    pub fn resolve(&self, snapshot: &PermissionSnapshot) -> BTreeSet<Permission> {
        snapshot
            .roles
            .iter()
            .filter_map(|r| self.0.get(&r.role))
            .flatten()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use health_time::Timestamp;

    use super::*;
    use crate::AccountId;
    use crate::snapshot::RoleAssignment;

    #[test]
    fn roles_resolve_to_the_union_of_their_permissions() {
        let mapping: RolePermissions = RolePermissions(BTreeMap::from([
            (
                "clinician".into(),
                BTreeSet::from(["record.read".into(), "record.write".into()]),
            ),
            (
                "cashier".into(),
                BTreeSet::from(["payment.record".into(), "record.read".into()]),
            ),
        ]));
        let mut snapshot = PermissionSnapshot {
            account: AccountId(uuid::Uuid::nil()),
            facility: "F1".into(),
            roles: vec![
                RoleAssignment {
                    role: "clinician".into(),
                    locations: vec![],
                },
                RoleAssignment {
                    role: "cashier".into(),
                    locations: vec!["ward-1".into()],
                },
            ],
            programs: vec![],
            issued_at: Timestamp::UNIX_EPOCH,
            not_after: Timestamp::UNIX_EPOCH,
            snapshot_version: 1,
        };
        assert_eq!(mapping.validate(&snapshot), Ok(()));
        assert_eq!(mapping.resolve(&snapshot).len(), 3);
        snapshot.roles.push(RoleAssignment {
            role: "ghost".into(),
            locations: vec![],
        });
        assert_eq!(
            mapping.validate(&snapshot),
            Err(UnknownRole("ghost".into()))
        );
        assert_eq!(mapping.resolve(&snapshot).len(), 3);
    }
}
