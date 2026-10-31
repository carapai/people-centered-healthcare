//! The authorization matrix (testing.md § Security): who may do what to which record, as one
//! table. Facility isolation, sensitivity labels, purpose of use and break-glass.

use std::collections::BTreeSet;

use health_access_policy::{
    Access, BTG, BreakGlassPolicy, BreakGlassRefused, Decision, Deny, Interaction, Label,
    LabelRule, Obligation, Pdp, Permissions, Subject,
};
use health_time::{SignedDuration, Timestamp};

const ACT: &str = "http://terminology.hl7.org/CodeSystem/v3-ActCode";
const CONF: &str = "http://terminology.hl7.org/CodeSystem/v3-Confidentiality";

fn pdp() -> Pdp {
    Pdp::new(vec![
        LabelRule {
            label: Label::new(ACT, "HIV"),
            program: "hiv".into(),
        },
        LabelRule {
            label: Label::new(ACT, "PSY"),
            program: "mental-health".into(),
        },
        LabelRule {
            label: Label::new(CONF, "R"),
            program: "restricted".into(),
        },
    ])
}

fn now() -> Timestamp {
    Timestamp::from_second(1_800_000_000).expect("time")
}

fn subject(name: &str) -> Subject {
    let base = |permissions: Permissions, programs: &[&str], purpose: bool| Subject {
        account: name.into(),
        facility: "F1".into(),
        permissions,
        programs: programs
            .iter()
            .map(|p| (*p).to_owned())
            .collect::<BTreeSet<_>>(),
        purpose: purpose.then(|| "TREAT".into()),
        break_glass: None,
    };
    match name {
        "clinician" => base(
            Permissions::from(["*.rs", "Observation.cu", "break-glass"]),
            &[],
            true,
        ),
        "hiv-counsellor" => base(Permissions::from(["*.rs"]), &["hiv"], true),
        "hiv-no-purpose" => base(Permissions::from(["*.rs"]), &["hiv"], false),
        "cashier" => base(Permissions::from(["Account.rs", "Invoice.crus"]), &[], true),
        other => panic!("unknown subject {other}"),
    }
}

struct Row {
    who: &'static str,
    resource: &'static str,
    interaction: Interaction,
    facility: Option<&'static str>,
    labels: &'static [(&'static str, &'static str)],
    expect: Result<(), Deny>,
}

const fn row(
    who: &'static str,
    resource: &'static str,
    interaction: Interaction,
    facility: Option<&'static str>,
    labels: &'static [(&'static str, &'static str)],
    expect: Result<(), Deny>,
) -> Row {
    Row {
        who,
        resource,
        interaction,
        facility,
        labels,
        expect,
    }
}

#[test]
fn the_matrix_holds() {
    use Interaction::{Create, Delete, Read, Search};
    let hiv = Deny::SensitiveLabel(Label::new(ACT, "HIV"));
    let rows = [
        // Roles.
        row("clinician", "Observation", Read, Some("F1"), &[], Ok(())),
        row("clinician", "Observation", Create, Some("F1"), &[], Ok(())),
        row(
            "clinician",
            "Observation",
            Delete,
            Some("F1"),
            &[],
            Err(Deny::NoPermission),
        ),
        row(
            "cashier",
            "Observation",
            Read,
            Some("F1"),
            &[],
            Err(Deny::NoPermission),
        ),
        row("cashier", "Invoice", Create, Some("F1"), &[], Ok(())),
        // Facility isolation.
        row(
            "clinician",
            "Observation",
            Read,
            Some("F2"),
            &[],
            Err(Deny::OtherFacility),
        ),
        row("clinician", "Patient", Read, None, &[], Ok(())),
        // Sensitivity labels and program assignments.
        row(
            "clinician",
            "Observation",
            Read,
            Some("F1"),
            &[(ACT, "HIV")],
            Err(hiv.clone()),
        ),
        row(
            "hiv-counsellor",
            "Observation",
            Read,
            Some("F1"),
            &[(ACT, "HIV")],
            Ok(()),
        ),
        row(
            "hiv-counsellor",
            "Observation",
            Read,
            Some("F1"),
            &[(ACT, "HIV"), (ACT, "PSY")],
            Err(Deny::SensitiveLabel(Label::new(ACT, "PSY"))),
        ),
        row(
            "hiv-counsellor",
            "Observation",
            Read,
            Some("F1"),
            &[(CONF, "R")],
            Err(Deny::SensitiveLabel(Label::new(CONF, "R"))),
        ),
        // A label outside the configured rules does not restrict.
        row(
            "clinician",
            "Observation",
            Read,
            Some("F1"),
            &[(CONF, "N")],
            Ok(()),
        ),
        // Purpose of use.
        row(
            "hiv-no-purpose",
            "Observation",
            Read,
            Some("F1"),
            &[(ACT, "HIV")],
            Err(Deny::PurposeRequired),
        ),
        row(
            "hiv-no-purpose",
            "Observation",
            Read,
            Some("F1"),
            &[],
            Ok(()),
        ),
        // Labels do not open another facility's records.
        row(
            "hiv-counsellor",
            "Observation",
            Search,
            Some("F2"),
            &[(ACT, "HIV")],
            Err(Deny::OtherFacility),
        ),
    ];
    let pdp = pdp();
    for (i, r) in rows.iter().enumerate() {
        let labels: Vec<Label> = r.labels.iter().map(|(s, c)| Label::new(s, c)).collect();
        let access = Access {
            resource_type: r.resource,
            interaction: r.interaction,
            facility: r.facility,
            labels: &labels,
            patient: Some("p1"),
        };
        let got = match pdp.decide(&subject(r.who), &access, now()) {
            Decision::Permit(obligations) => {
                assert!(obligations.is_empty(), "row {i}: no break-glass here");
                Ok(())
            }
            Decision::Deny(deny) => Err(deny),
        };
        assert_eq!(
            got, r.expect,
            "row {i}: {} {:?} {} {:?} {:?}",
            r.who, r.interaction, r.resource, r.facility, r.labels
        );
    }
}

#[test]
fn break_glass_opens_one_patient_for_one_hour_and_is_reported() {
    let pdp = pdp();
    let policy = BreakGlassPolicy::default();
    let mut clinician = subject("clinician");
    let labels = [Label::new(ACT, "HIV")];
    let access = |patient| Access {
        resource_type: "Observation",
        interaction: Interaction::Read,
        facility: Some("F2"),
        labels: &labels,
        patient: Some(patient),
    };

    let (grant, event) = policy
        .grant(
            &clinician,
            "p1",
            BTG,
            "unconscious, no history available",
            now(),
        )
        .expect("granted");
    assert_eq!(
        (
            event.patient.as_str(),
            event.reason.as_str(),
            event.facility.as_str()
        ),
        ("p1", BTG, "F1")
    );
    assert_eq!(
        event.expires_at,
        now()
            .checked_add(SignedDuration::from_hours(1))
            .expect("time")
    );
    clinician.break_glass = Some(grant);

    // The patient's sensitive, other-facility record: permitted, with the audit obligation.
    assert_eq!(
        pdp.decide(&clinician, &access("p1"), now()),
        Decision::Permit(vec![Obligation::BreakGlassUsed {
            patient: "p1".into()
        }])
    );
    // Any other patient: still denied.
    assert_eq!(
        pdp.decide(&clinician, &access("p2"), now()),
        Decision::Deny(Deny::OtherFacility)
    );
    // No extension, no second patient while it lasts.
    assert_eq!(
        policy.grant(&clinician, "p2", BTG, "another", now()),
        Err(BreakGlassRefused::AlreadyActive)
    );
    // After the hour it is gone.
    let later = now()
        .checked_add(SignedDuration::from_hours(1))
        .expect("time");
    assert_eq!(
        pdp.decide(&clinician, &access("p1"), later),
        Decision::Deny(Deny::OtherFacility)
    );
    // Break-glass never widens permissions: a delete stays denied.
    let delete = Access {
        interaction: Interaction::Delete,
        ..access("p1")
    };
    assert_eq!(
        pdp.decide(&clinician, &delete, now()),
        Decision::Deny(Deny::NoPermission)
    );
}

#[test]
fn break_glass_needs_the_permission_and_a_reason() {
    let policy = BreakGlassPolicy::default();
    assert_eq!(
        policy.grant(&subject("hiv-counsellor"), "p1", BTG, "x", now()),
        Err(BreakGlassRefused::NotPermitted)
    );
    assert_eq!(
        policy.grant(&subject("clinician"), "p1", "TREAT", "x", now()),
        Err(BreakGlassRefused::MissingReason)
    );
    assert_eq!(
        policy.grant(&subject("clinician"), "p1", BTG, "  ", now()),
        Err(BreakGlassRefused::MissingReason)
    );
}

#[test]
fn searches_exclude_what_the_subject_may_not_see() {
    let pdp = pdp();
    let names = |filter: health_access_policy::SearchFilter| {
        filter
            .excluded_labels
            .into_iter()
            .map(|l| l.code)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        pdp.search_filter(&subject("clinician"), "Observation", None, now())
            .map(names),
        Ok(vec!["HIV".into(), "PSY".into(), "R".into()])
    );
    assert_eq!(
        pdp.search_filter(&subject("hiv-counsellor"), "Observation", None, now())
            .map(names),
        Ok(vec!["PSY".into(), "R".into()])
    );
    // Without a purpose of use, every sensitive label is excluded, even with the program.
    assert_eq!(
        pdp.search_filter(&subject("hiv-no-purpose"), "Observation", None, now())
            .map(names),
        Ok(vec!["HIV".into(), "PSY".into(), "R".into()])
    );
    assert_eq!(
        pdp.search_filter(&subject("cashier"), "Observation", None, now()),
        Err(Deny::NoPermission)
    );

    // Break-glass lifts the exclusions only for a search confined to that patient.
    let mut clinician = subject("clinician");
    clinician.break_glass = Some(
        BreakGlassPolicy::default()
            .grant(&clinician, "p1", BTG, "emergency", now())
            .expect("grant")
            .0,
    );
    assert_eq!(
        pdp.search_filter(&clinician, "Observation", Some("p1"), now())
            .map(names),
        Ok(vec![])
    );
    assert_eq!(
        pdp.search_filter(&clinician, "Observation", None, now())
            .map(names),
        Ok(vec!["HIV".into(), "PSY".into(), "R".into()])
    );
}
