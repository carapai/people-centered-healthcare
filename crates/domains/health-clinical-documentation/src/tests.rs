use super::*;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn soap() -> Vec<Section> {
    NoteKind::Progress
        .sections()
        .iter()
        .map(|title| Section {
            title: (*title).into(),
            text: format!("{title} text"),
        })
        .collect()
}

fn draft() -> Note {
    Note::draft(
        "n1",
        NoteKind::Progress,
        "Patient/p-1",
        Some("Encounter/e-1"),
        "clinician-1",
        soap(),
        at("2026-11-05T10:00:00Z"),
    )
    .expect("draft")
}

#[test]
fn a_draft_is_edited_then_signed() {
    let note = draft();
    assert_eq!(note.status, NoteStatus::Preliminary);
    let edited = note
        .edit(
            vec![Section {
                title: "Plan".into(),
                text: "Review in a week".into(),
            }],
            at("2026-11-05T10:05:00Z"),
        )
        .expect("edited");
    let signed = edited
        .sign("clinician-1", at("2026-11-05T10:10:00Z"))
        .expect("signed");
    assert_eq!(signed.status, NoteStatus::Final);
    assert_eq!(
        signed.signed,
        Some(Signature {
            by: "clinician-1".into(),
            at: at("2026-11-05T10:10:00Z")
        })
    );
    assert_eq!(
        signed.edit(soap(), at("2026-11-05T11:00:00Z")),
        Err(NoteRefused::Signed),
        "a signed note is not edited"
    );
    assert_eq!(
        signed.sign("clinician-2", at("2026-11-05T11:00:00Z")),
        Err(NoteRefused::Signed)
    );
    assert_eq!(
        note.edit(
            vec![Section {
                title: "Plan".into(),
                text: " ".into()
            }],
            at("2026-11-05T10:05:00Z")
        ),
        Err(NoteRefused::Empty)
    );
    assert_eq!(
        Note::draft(
            "n2",
            NoteKind::Letter,
            "Patient/p-1",
            None,
            "c",
            vec![],
            at("2026-11-05T10:00:00Z")
        ),
        Err(NoteRefused::Empty)
    );
}

#[test]
fn a_signed_note_changes_only_by_amendment_or_addendum() {
    let original = draft()
        .sign("clinician-1", at("2026-11-05T10:10:00Z"))
        .expect("signed");
    let fix = vec![Section {
        title: "Assessment".into(),
        text: "Malaria, not typhoid".into(),
    }];
    let amendment = original
        .amend(
            "n2",
            Relation::Replaces,
            fix.clone(),
            "clinician-2",
            at("2026-11-06T09:00:00Z"),
        )
        .expect("amended");
    assert_eq!(amendment.status, NoteStatus::Amended);
    assert_eq!(
        amendment.relates_to,
        Some((Relation::Replaces, "n1".into()))
    );
    assert_eq!(
        (
            amendment.author.as_str(),
            amendment.kind,
            amendment.patient.as_str()
        ),
        ("clinician-2", NoteKind::Progress, "Patient/p-1")
    );
    assert!(
        amendment.signed.is_some(),
        "an amendment is signed as it is made"
    );
    let addendum = original
        .amend(
            "n3",
            Relation::Appends,
            fix.clone(),
            "clinician-1",
            at("2026-11-06T09:00:00Z"),
        )
        .expect("appended");
    assert_eq!(addendum.status, NoteStatus::Appended);
    assert_eq!(
        draft().amend(
            "n4",
            Relation::Replaces,
            fix,
            "c",
            at("2026-11-06T09:00:00Z")
        ),
        Err(NoteRefused::NotSigned)
    );
}

#[test]
fn signed_versions_are_never_replaced_in_place_nor_deleted() {
    use NoteStatus::{Amended, Appended, Cancelled, Corrected, EnteredInError, Final, Preliminary};
    for signed in [Final, Amended, Appended, Corrected] {
        assert!(signed.is_signed());
        assert_eq!(
            may_replace(signed, signed, false),
            Err(NoteRefused::Signed),
            "edited in place"
        );
        assert_eq!(
            may_replace(signed, Preliminary, true),
            Err(NoteRefused::Signed),
            "unsigned"
        );
        assert_eq!(
            may_replace(signed, EnteredInError, false),
            Err(NoteRefused::Signed),
            "withdrawn with its content changed"
        );
        assert_eq!(
            may_replace(signed, EnteredInError, true),
            Ok(()),
            "marked entered in error, as it was"
        );
        assert_eq!(may_delete(signed), Err(NoteRefused::SignedNotDeleted));
    }
    for open in [Preliminary, Cancelled, EnteredInError] {
        assert!(!open.is_signed());
        assert_eq!(may_replace(open, Final, false), Ok(()));
        assert_eq!(may_delete(open), Ok(()));
    }
    for status in [
        Preliminary,
        Final,
        Amended,
        Appended,
        Corrected,
        Cancelled,
        EnteredInError,
    ] {
        assert_eq!(NoteStatus::from_code(status.code()), Some(status));
    }
    assert_eq!(NoteStatus::from_code("draft"), None);
}

#[test]
fn drafts_are_withdrawn_and_signed_notes_entered_in_error() {
    let cancelled = draft()
        .cancel(at("2026-11-05T10:20:00Z"))
        .expect("cancelled");
    assert_eq!(cancelled.status, NoteStatus::Cancelled);
    assert_eq!(
        cancelled.sign("c", at("2026-11-05T10:30:00Z")),
        Err(NoteRefused::Closed("cancelled"))
    );
    let signed = draft()
        .sign("c", at("2026-11-05T10:10:00Z"))
        .expect("signed");
    let wrong = signed.enter_in_error();
    assert_eq!(
        (wrong.status, &wrong.sections),
        (NoteStatus::EnteredInError, &signed.sections)
    );
    for kind in [
        NoteKind::Progress,
        NoteKind::Consultation,
        NoteKind::Admission,
        NoteKind::Procedure,
        NoteKind::Discharge,
        NoteKind::Letter,
    ] {
        assert!(!kind.loinc().0.is_empty() && !kind.sections().is_empty());
    }
}
