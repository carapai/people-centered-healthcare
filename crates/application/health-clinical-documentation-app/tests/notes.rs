//! The signed-note rule on FHIR resources (#107): a signed Composition or DocumentReference is
//! never edited in place nor deleted; it may only be marked entered in error, as it was.

use health_clinical_documentation::NoteRefused;
use health_clinical_documentation_app::notes::{check_delete, check_replace, status_of};
use serde_json::{Value, json};

fn composition(status: &str, text: &str) -> Value {
    json!({
        "resourceType": "Composition", "id": "n1", "status": status,
        "meta": { "versionId": "v1" },
        "title": "Progress note", "section": [{ "title": "Plan", "text": { "status": "generated", "div": text } }]
    })
}

#[test]
fn a_signed_composition_is_immutable() {
    let signed = composition("final", "review in a week");
    let mut restamped = signed.clone();
    restamped["meta"] = json!({ "versionId": "v2", "tag": [{ "system": "urn:health:facility", "code": "KAM013" }] });
    assert_eq!(
        check_replace(&signed, &composition("final", "review in two weeks")),
        Err(NoteRefused::Signed)
    );
    assert_eq!(
        check_replace(&signed, &composition("preliminary", "review in a week")),
        Err(NoteRefused::Signed)
    );
    assert_eq!(
        check_replace(&signed, &composition("entered-in-error", "something else")),
        Err(NoteRefused::Signed)
    );
    assert_eq!(
        check_replace(
            &signed,
            &composition("entered-in-error", "review in a week")
        ),
        Ok(())
    );
    let mut no_status = signed.clone();
    no_status.as_object_mut().expect("object").remove("status");
    assert_eq!(check_replace(&signed, &no_status), Err(NoteRefused::Signed));
    assert_eq!(check_delete(&signed), Err(NoteRefused::SignedNotDeleted));
    // Only meta differs: the same content, but still not an edit a signed note allows.
    assert_eq!(check_replace(&signed, &restamped), Err(NoteRefused::Signed));

    let draft = composition("preliminary", "draft");
    assert_eq!(
        check_replace(&draft, &composition("final", "signed text")),
        Ok(())
    );
    assert_eq!(check_delete(&draft), Ok(()));
}

#[test]
fn document_references_follow_their_doc_status_and_other_types_are_free() {
    let letter = json!({ "resourceType": "DocumentReference", "id": "d1", "status": "current", "docStatus": "final",
                         "content": [{ "attachment": { "contentType": "application/pdf", "url": "Binary/b1" } }] });
    let mut edited = letter.clone();
    edited["content"][0]["attachment"]["url"] = json!("Binary/b2");
    assert_eq!(check_replace(&letter, &edited), Err(NoteRefused::Signed));
    assert_eq!(check_delete(&letter), Err(NoteRefused::SignedNotDeleted));
    let mut unstated = letter.clone();
    unstated
        .as_object_mut()
        .expect("object")
        .remove("docStatus");
    assert_eq!(status_of(&unstated), None);
    assert_eq!(
        check_replace(&unstated, &edited),
        Ok(()),
        "a DocumentReference without docStatus is not under the rule"
    );

    let observation = json!({ "resourceType": "Observation", "status": "final" });
    assert_eq!(status_of(&observation), None);
    assert_eq!(
        check_replace(
            &observation,
            &json!({ "resourceType": "Observation", "status": "amended" })
        ),
        Ok(())
    );
    assert_eq!(check_delete(&observation), Ok(()));
}

mod mapping {
    use health_clinical_documentation::{Note, NoteKind, Relation, Section};
    use health_clinical_documentation_app::notes::{Act, fhir, note, provenance};
    use health_fhir_validation::validate_structure;
    use health_time::Timestamp;

    fn at(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    fn soap() -> Vec<Section> {
        vec![
            Section {
                title: "Subjective".into(),
                text: "Fever <3 days> & \"headache\"\nno vomiting".into(),
            },
            Section {
                title: "Plan".into(),
                text: "RDT".into(),
            },
        ]
    }

    #[test]
    fn a_note_round_trips_through_its_composition() {
        let draft = Note::draft(
            "0190f1a2-7c3d-7e4f-8a5b-000000000901",
            NoteKind::Progress,
            "Patient/p-1",
            Some("Encounter/e-1"),
            "clinician-1",
            soap(),
            at("2026-11-05T10:00:00Z"),
        )
        .expect("draft");
        let signed = draft
            .sign("clinician-1", at("2026-11-05T10:10:00Z"))
            .expect("signed");
        let amended = signed
            .amend(
                "0190f1a2-7c3d-7e4f-8a5b-000000000902",
                Relation::Replaces,
                vec![Section {
                    title: "Plan".into(),
                    text: "Treat".into(),
                }],
                "clinician-2",
                at("2026-11-06T09:00:00Z"),
            )
            .expect("amended");
        for each in [&draft, &signed, &amended] {
            let composition = fhir(each, "KAM013");
            let issues = validate_structure(&composition.to_string());
            assert!(issues.is_empty(), "{composition}: {issues:?}");
            assert_eq!(note(&composition).as_ref(), Some(each));
            assert_eq!(composition["meta"]["tag"][0]["code"], "KAM013");
        }
        let composition = fhir(&amended, "KAM013");
        assert_eq!(composition["status"], "amended");
        assert_eq!(composition["relatesTo"][0]["type"], "replaces");
        assert_eq!(
            composition["relatesTo"][0]["resourceReference"]["reference"],
            "Composition/0190f1a2-7c3d-7e4f-8a5b-000000000901"
        );
        assert_eq!(
            fhir(&signed, "KAM013")["attester"][0]["party"]["display"],
            "clinician-1"
        );
        assert!(fhir(&draft, "KAM013").get("attester").is_none());
        assert_eq!(
            note(&serde_json::json!({ "resourceType": "Observation" })),
            None
        );
    }

    #[test]
    fn signatures_and_amendments_have_their_provenance() {
        let signed = provenance(
            "0190f1a2-7c3d-7e4f-8a5b-000000000911",
            Act::Signed,
            "Composition/n1/_history/v2",
            None,
            "clinician-1",
            "2026-11-05T10:10:00Z",
        );
        let amended = provenance(
            "0190f1a2-7c3d-7e4f-8a5b-000000000912",
            Act::Amended,
            "Composition/n2/_history/v1",
            Some("Composition/n1/_history/v2"),
            "clinician-2",
            "2026-11-06T09:00:00Z",
        );
        for each in [&signed, &amended] {
            let issues = validate_structure(&each.to_string());
            assert!(issues.is_empty(), "{each}: {issues:?}");
        }
        assert_eq!(signed["activity"]["coding"][0]["code"], "LA");
        assert!(signed.get("entity").is_none());
        assert_eq!(
            amended["entity"][0],
            serde_json::json!({ "role": "revision", "what": { "reference": "Composition/n1/_history/v2" } })
        );
    }
}
