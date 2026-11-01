//! Caregiver access, as an authorization matrix (testing.md § Security; §96): a guardian never
//! sees an adolescent's sensitive records without the adolescent's consent.

use health_consent::{
    Authority, CaregiverGrant, Confidentiality, Day, Refused, Request, Revoker, Scope, Verification,
};
use health_time::Timestamp;

const HIV: (&str, &str) = ("http://terminology.hl7.org/CodeSystem/v3-ActCode", "HIV");

fn policy() -> Confidentiality {
    Confidentiality {
        sensitive: vec![(HIV.0.into(), HIV.1.into())],
        ..Confidentiality::default()
    }
}

fn now() -> Timestamp {
    Timestamp::from_second(1_800_000_000).expect("time")
}

fn today() -> Day {
    Day::new(2027, 1, 15).expect("day")
}

fn grant(authority: Authority) -> CaregiverGrant {
    CaregiverGrant {
        id: "consent-1".into(),
        caregiver: "person-mother".into(),
        related_person: "rp-1".into(),
        patient: "patient-child".into(),
        authority,
        scopes: Scope::ALL.to_vec(),
        expires: Timestamp::from_second(1_900_000_000).expect("time"),
        verification: Some(Verification::StaffChecked {
            staff: "clerk-1".into(),
            document: "birth-certificate".into(),
        }),
        adolescent_consents: false,
        revoked: None,
    }
}

fn ask<'a>(
    grant: &CaregiverGrant,
    scope: Scope,
    birth_date: Option<&'a str>,
    labels: &'a [(String, String)],
) -> Result<(), Refused> {
    grant.decide(
        &policy(),
        &Request {
            scope,
            birth_date,
            labels,
            today: today(),
            now: now(),
        },
    )
}

/// One matrix row: the grant's authority, whether the adolescent consents, the birth date, the
/// scope asked for, the record's labels, and the answer.
type Row<'a> = (
    Authority,
    bool,
    Option<&'a str>,
    Scope,
    &'a [(String, String)],
    Result<(), Refused>,
);

#[test]
fn the_authorization_matrix() {
    let sensitive = vec![(HIV.0.to_owned(), HIV.1.to_owned())];
    let ordinary: Vec<(String, String)> = Vec::new();
    // (authority, adolescent consents, birth date, scope, labels, expected)
    let rows: Vec<Row<'_>> = vec![
        // A child of 8: the guardian sees everything they are granted.
        (
            Authority::Guardian,
            false,
            Some("2018-06-01"),
            Scope::ViewRecords,
            &sensitive,
            Ok(()),
        ),
        (
            Authority::Guardian,
            false,
            Some("2018-06-01"),
            Scope::ViewRecords,
            &ordinary,
            Ok(()),
        ),
        // An adolescent of 14: ordinary records yes, sensitive ones only with their consent.
        (
            Authority::Guardian,
            false,
            Some("2012-03-10"),
            Scope::ViewRecords,
            &ordinary,
            Ok(()),
        ),
        (
            Authority::Guardian,
            false,
            Some("2012-03-10"),
            Scope::ViewRecords,
            &sensitive,
            Err(Refused::Confidential),
        ),
        (
            Authority::Guardian,
            true,
            Some("2012-03-10"),
            Scope::ViewRecords,
            &sensitive,
            Ok(()),
        ),
        // Confidentiality is about records: booking is still the guardian's.
        (
            Authority::Guardian,
            false,
            Some("2012-03-10"),
            Scope::BookAppointments,
            &sensitive,
            Ok(()),
        ),
        // Twelve today: protected from today.
        (
            Authority::Guardian,
            false,
            Some("2015-01-15"),
            Scope::ViewRecords,
            &sensitive,
            Err(Refused::Confidential),
        ),
        (
            Authority::Guardian,
            false,
            Some("2015-01-16"),
            Scope::ViewRecords,
            &sensitive,
            Ok(()),
        ),
        // Only a birth year: the earliest it allows, so protected sooner.
        (
            Authority::Guardian,
            false,
            Some("2015"),
            Scope::ViewRecords,
            &sensitive,
            Err(Refused::Confidential),
        ),
        // Of age (18), or of unknown age: a guardian has nothing.
        (
            Authority::Guardian,
            true,
            Some("2009-01-15"),
            Scope::ViewRecords,
            &ordinary,
            Err(Refused::OfAge),
        ),
        (
            Authority::Guardian,
            false,
            Some("2009-01-16"),
            Scope::ViewRecords,
            &ordinary,
            Ok(()),
        ),
        (
            Authority::Guardian,
            false,
            Some("2009"),
            Scope::ViewRecords,
            &ordinary,
            Err(Refused::OfAge),
        ),
        (
            Authority::Guardian,
            false,
            None,
            Scope::ViewRecords,
            &ordinary,
            Err(Refused::OfAge),
        ),
        (
            Authority::Guardian,
            false,
            Some("not a date"),
            Scope::ViewRecords,
            &ordinary,
            Err(Refused::OfAge),
        ),
        // An adult's delegate: what the adult granted, sensitive records included.
        (
            Authority::Delegated,
            false,
            Some("1950-05-05"),
            Scope::ViewRecords,
            &sensitive,
            Ok(()),
        ),
        (
            Authority::Delegated,
            false,
            None,
            Scope::RequestRefills,
            &ordinary,
            Ok(()),
        ),
    ];
    for (i, (authority, consents, birth, scope, labels, expected)) in rows.into_iter().enumerate() {
        let mut g = grant(authority);
        g.adolescent_consent(consents);
        assert_eq!(ask(&g, scope, birth, labels), expected, "row {i}");
    }
}

#[test]
fn an_unverified_expired_revoked_or_out_of_scope_grant_gives_nothing() {
    let none: [(String, String); 0] = [];
    let child = Some("2018-06-01");
    let mut unverified = grant(Authority::Guardian);
    unverified.verification = None;
    assert_eq!(
        ask(&unverified, Scope::ViewRecords, child, &none),
        Err(Refused::Unverified)
    );
    unverified.verify(Verification::StaffChecked {
        staff: "clerk-1".into(),
        document: "birth-certificate".into(),
    });
    assert_eq!(ask(&unverified, Scope::ViewRecords, child, &none), Ok(()));

    let mut expired = grant(Authority::Guardian);
    expired.expires = now();
    assert_eq!(
        ask(&expired, Scope::ViewRecords, child, &none),
        Err(Refused::Expired)
    );

    let mut narrow = grant(Authority::Guardian);
    narrow.scopes = vec![Scope::BookAppointments];
    assert_eq!(
        ask(&narrow, Scope::ViewRecords, child, &none),
        Err(Refused::OutOfScope(Scope::ViewRecords))
    );

    let mut revoked = grant(Authority::Guardian);
    revoked
        .revoke(
            Revoker::Staff {
                staff: "clerk-1".into(),
            },
            &policy(),
            child,
            &[],
            today(),
            now(),
        )
        .expect("revoked");
    assert_eq!(
        ask(&revoked, Scope::ViewRecords, child, &none),
        Err(Refused::Revoked)
    );
}

#[test]
fn who_may_revoke() {
    let policy = policy();
    let revoke = |g: &mut CaregiverGrant,
                  by: Revoker,
                  birth: Option<&str>,
                  guardians: &[&CaregiverGrant]| {
        g.revoke(by, &policy, birth, guardians, today(), now())
    };
    // A minor may not end their guardian's access; once of age they may.
    let mut g = grant(Authority::Guardian);
    assert!(matches!(
        revoke(&mut g, Revoker::Patient, Some("2012-03-10"), &[]),
        Err(Refused::MayNotRevoke(_))
    ));
    // Of age only on the earliest birth date the year allows: not enough.
    assert!(matches!(
        revoke(&mut g, Revoker::Patient, Some("2009"), &[]),
        Err(Refused::MayNotRevoke(_))
    ));
    assert!(revoke(&mut g, Revoker::Patient, Some("2008-12-31"), &[]).is_ok());
    // An adult revokes their own delegation at any time.
    let mut d = grant(Authority::Delegated);
    assert!(revoke(&mut d, Revoker::Patient, None, &[]).is_ok());
    // Another guardian, only with authority over this patient.
    let other = CaregiverGrant {
        id: "consent-2".into(),
        caregiver: "person-father".into(),
        ..grant(Authority::Guardian)
    };
    let mut g = grant(Authority::Guardian);
    let by = Revoker::Guardian {
        grant: "consent-2".into(),
    };
    assert!(matches!(
        revoke(&mut g, by.clone(), Some("2012-03-10"), &[]),
        Err(Refused::MayNotRevoke(_))
    ));
    let mut lapsed = other.clone();
    lapsed.expires = now();
    assert!(matches!(
        revoke(&mut g, by.clone(), Some("2012-03-10"), &[&lapsed]),
        Err(Refused::MayNotRevoke(_))
    ));
    assert!(revoke(&mut g, by, Some("2012-03-10"), &[&other]).is_ok());
    assert_eq!(g.revoked.as_ref().map(|r| r.at), Some(now()));
}

#[test]
fn a_delivery_links_each_newborn_to_the_mother_at_once() {
    let majority = Timestamp::from_second(2_400_000_000).expect("time");
    let twins: Vec<CaregiverGrant> = ["baby-1", "baby-2"]
        .iter()
        .map(|baby| {
            CaregiverGrant::at_delivery(
                &format!("c-{baby}"),
                "person-mother",
                &format!("rp-{baby}"),
                baby,
                "enc-1",
                majority,
            )
        })
        .collect();
    for grant in &twins {
        assert_eq!(grant.authority, Authority::Guardian);
        assert_eq!(grant.scopes, Scope::ALL);
        assert!(matches!(
            grant.verification,
            Some(Verification::Delivery { .. })
        ));
        assert_eq!(
            ask(grant, Scope::ViewRecords, Some("2027-01-10"), &[]),
            Ok(())
        );
    }
    assert_ne!(twins[0].patient, twins[1].patient);
}

#[test]
fn days_and_ages() {
    assert_eq!(
        Day::earliest("2015").expect("year").to_string(),
        "2015-01-01"
    );
    assert_eq!(
        Day::latest("2016-02").expect("month").to_string(),
        "2016-02-29"
    );
    assert_eq!(Day::latest("2015").expect("year").to_string(), "2015-12-31");
    assert!(Day::earliest("2015-13").is_err());
    assert!(Day::earliest("2015-02-30").is_err());
    let leap = Day::new(2008, 2, 29).expect("day");
    assert_eq!(leap.age_on(Day::new(2026, 2, 28).expect("day")), 17);
    assert_eq!(leap.age_on(Day::new(2026, 3, 1).expect("day")), 18);
    assert_eq!(Day::new(2030, 1, 1).expect("day").age_on(today()), 0);
    assert_eq!(
        Scope::from_code("request-refills"),
        Some(Scope::RequestRefills)
    );
}
