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
