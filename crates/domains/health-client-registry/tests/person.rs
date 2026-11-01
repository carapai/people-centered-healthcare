//! A Login Account's Person: links at an assurance level, and what each level sees (ADR 0029).

use health_client_registry::person::{
    Assurance, Evidence, Origin, Person, PersonLinkRefused, PortalAccess, Verification,
};
use health_time::Timestamp;

fn verification(evidence: Evidence) -> Verification {
    Verification {
        evidence,
        at: Timestamp::from_second(1_800_000_000).expect("time"),
    }
}

#[test]
fn a_phone_account_is_never_linked_to_an_existing_patient() {
    let mut person = Person::new("person-1", "account-1");
    assert_eq!(
        person.link_existing("patient-9", &verification(Evidence::PhoneOtp)),
        Err(PersonLinkRefused::NeedsLevel3)
    );
    assert!(person.links.is_empty());
    assert_eq!(person.access("patient-9"), PortalAccess::None);
}

#[test]
fn linking_an_existing_patient_needs_a_nin_or_in_person_check() {
    for evidence in [
        Evidence::Nira {
            nin: "CM90123456789X".into(),
        },
        Evidence::InPerson {
            staff: "clerk-1".into(),
            document: "national-id".into(),
        },
    ] {
        assert_eq!(evidence.assurance(), Assurance::Level3);
        let mut person = Person::new("person-1", "account-1");
        person
            .link_existing("patient-9", &verification(evidence.clone()))
            .expect("linked");
        assert_eq!(person.access("patient-9"), PortalAccess::Records);
        assert_eq!(
            person.link_existing("patient-9", &verification(evidence)),
            Err(PersonLinkRefused::AlreadyLinked("patient-9".into()))
        );
    }
}

#[test]
fn below_level3_only_self_submitted_data_is_visible() {
    let mut person = Person::new("person-1", "account-1");
    person
        .link_self_registered("patient-new", Assurance::Level1)
        .expect("linked");
    let access = person.access("patient-new");
    assert_eq!(access, PortalAccess::SelfSubmitted);
    assert!(access.may_see(Origin::SelfSubmitted));
    assert!(!access.may_see(Origin::Facility));
    assert!(!PortalAccess::None.may_see(Origin::SelfSubmitted));

    // A later in-person check raises the link; a phone code never lowers it.
    let checked = verification(Evidence::InPerson {
        staff: "clerk-1".into(),
        document: "passport".into(),
    });
    assert_eq!(
        person.verify("patient-new", &checked),
        Ok(Assurance::Level3)
    );
    assert_eq!(
        person.verify("patient-new", &verification(Evidence::PhoneOtp)),
        Ok(Assurance::Level3)
    );
    let access = person.access("patient-new");
    assert!(access.may_see(Origin::Facility));
    assert_eq!(
        person.verify("patient-other", &checked),
        Err(PersonLinkRefused::NotLinked("patient-other".into()))
    );
}
