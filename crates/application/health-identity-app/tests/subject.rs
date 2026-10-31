//! The request subject from a permission snapshot (ADR 0056).

use std::collections::{BTreeMap, BTreeSet};

use health_access_policy::{Interaction, purpose_of_use};
use health_identity::AccountId;
use health_identity::snapshot::{PermissionSnapshot, RoleAssignment, RolePermissions};
use health_identity_app::subject_of;
use health_time::Timestamp;

#[test]
fn the_subject_carries_the_snapshot_facility_roles_programs_and_purpose() {
    let roles = RolePermissions(BTreeMap::from([
        (
            "clinician".into(),
            BTreeSet::from(["*.rs".into(), "Observation.cu".into()]),
        ),
        ("cashier".into(), BTreeSet::from(["Invoice.crus".into()])),
    ]));
    let at = Timestamp::from_second(1_800_000_000).expect("time");
    let snapshot = PermissionSnapshot {
        account: AccountId(uuid::Uuid::nil()),
        facility: "F1".into(),
        roles: vec![RoleAssignment {
            role: "clinician".into(),
            locations: vec![],
        }],
        programs: vec!["hiv".into()],
        issued_at: at,
        not_after: at,
        snapshot_version: 3,
    };
    let subject = subject_of(&snapshot, &roles, purpose_of_use(None).expect("purpose"));
    assert_eq!(subject.facility, "F1");
    assert_eq!(subject.account, AccountId(uuid::Uuid::nil()).to_string());
    assert!(
        subject
            .permissions
            .allows("Observation", Interaction::Create)
    );
    assert!(!subject.permissions.allows("Invoice", Interaction::Create));
    assert!(subject.programs.contains("hiv"));
    assert_eq!(subject.purpose.as_deref(), Some("TREAT"));
    assert!(subject.break_glass.is_none());
}
