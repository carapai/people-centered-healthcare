use serde_json::{Value, json};

use super::*;

const URL: &str = "http://health.go.ug/fhir/CodeSystem/anc-signs";

fn signs(version: &str) -> Value {
    json!({
        "resourceType": "CodeSystem", "id": "anc-signs", "url": URL, "version": version,
        "status": "active", "content": "complete", "count": 4,
        "concept": [{ "code": "danger", "display": "Danger signs", "concept": [
            { "code": "bleeding", "display": "Vaginal bleeding", "concept": [
                { "code": "heavy-bleeding", "display": "Heavy bleeding" }
            ]},
            { "code": "fits", "display": "Convulsions", "definition": "Fits of any kind" }
        ]}]
    })
}

fn published() -> BTreeSet<String> {
    codes(&signs("1.0.0"))
}

fn add(code: &str, parent: Option<&str>) -> ConceptEdit {
    ConceptEdit::Add {
        code: code.into(),
        display: Some(format!("{code} display")),
        definition: None,
        parent: parent.map(str::to_owned),
    }
}

fn change(
    code: &str,
    new_code: &str,
    parent: Option<&str>,
    definition: Option<&str>,
) -> ConceptEdit {
    ConceptEdit::Change {
        code: code.into(),
        new_code: new_code.into(),
        display: Some("Shown".into()),
        definition: definition.map(str::to_owned),
        parent: parent.map(str::to_owned),
    }
}

#[test]
fn adding_a_code_under_another_is_minor_and_takes_the_next_version() {
    let previous = signs("1.0.0");
    let edited = apply(
        &signs("1.0.0"),
        &add("spotting", Some("bleeding")),
        &published(),
        Some(&previous),
    )
    .expect("added");
    assert_eq!(
        edited.pointer("/concept/0/concept/0/concept/1/code"),
        Some(&json!("spotting"))
    );
    assert_eq!(edited["count"], 5);
    assert_eq!(edited["version"], "1.1.0");
    let found = versioning(Some(&previous), &edited);
    assert_eq!(found.bump, Bump::Minor);
    assert_eq!(found.next.as_deref(), Some("1.1.0"));
    assert_eq!(found.published.as_deref(), Some("1.0.0"));
    assert_eq!(found.changes.len(), 1);
    assert_eq!(found.changes[0].at, "concept[spotting]");
    // A top-level code, and one under a code that had none, work the same way.
    let top = apply(&edited, &add("other", None), &published(), Some(&previous)).expect("top");
    assert_eq!(top.pointer("/concept/1/code"), Some(&json!("other")));
    let under = apply(
        &edited,
        &add("brief", Some("fits")),
        &published(),
        Some(&previous),
    )
    .expect("under a leaf");
    assert_eq!(
        under.pointer("/concept/0/concept/1/concept/0/code"),
        Some(&json!("brief"))
    );
}

#[test]
fn a_code_already_held_or_an_unknown_parent_is_refused() {
    let none = BTreeSet::new();
    assert_eq!(
        apply(&signs("1.0.0"), &add("fits", None), &none, None),
        Err(EditRefused::Taken("fits".into()))
    );
    assert_eq!(
        apply(&signs("1.0.0"), &add("x", Some("nope")), &none, None),
        Err(EditRefused::UnknownCode("nope".into()))
    );
    assert_eq!(
        apply(&signs("1.0.0"), &add(" x", None), &none, None),
        Err(EditRefused::BadCode(" x".into()))
    );
    assert_eq!(
        apply(
            &json!({"resourceType": "ValueSet"}),
            &add("x", None),
            &none,
            None
        ),
        Err(EditRefused::NotACodeSystem)
    );
}

#[test]
fn a_published_code_cannot_be_deleted_renamed_or_moved() {
    let previous = signs("1.0.0");
    let refused = Err(EditRefused::Published("fits".into()));
    assert_eq!(
        apply(
            &signs("1.0.0"),
            &ConceptEdit::Delete {
                code: "fits".into()
            },
            &published(),
            Some(&previous)
        ),
        refused
    );
    assert_eq!(
        apply(
            &signs("1.0.0"),
            &change("fits", "seizures", Some("danger"), Some("Fits of any kind")),
            &published(),
            Some(&previous)
        ),
        refused
    );
    assert_eq!(
        apply(
            &signs("1.0.0"),
            &change("fits", "fits", Some("bleeding"), Some("Fits of any kind")),
            &published(),
            Some(&previous)
        ),
        refused
    );
}

#[test]
fn a_new_code_can_be_renamed_moved_and_deleted() {
    let previous = signs("1.0.0");
    let added = apply(
        &signs("1.0.0"),
        &add("spoting", Some("bleeding")),
        &published(),
        Some(&previous),
    )
    .expect("added");
    let renamed = apply(
        &added,
        &change("spoting", "spotting", Some("danger"), None),
        &published(),
        Some(&previous),
    )
    .expect("renamed and moved");
    assert_eq!(
        renamed.pointer("/concept/0/concept/2/code"),
        Some(&json!("spotting"))
    );
    assert!(
        renamed.pointer("/concept/0/concept/0/concept/1").is_none(),
        "moved out from under bleeding"
    );
    let deleted = apply(
        &renamed,
        &ConceptEdit::Delete {
            code: "spotting".into(),
        },
        &published(),
        Some(&previous),
    )
    .expect("deleted");
    assert_eq!(codes(&deleted), published());
    assert_eq!(deleted["version"], "1.0.0", "back to no change");
    // A code with codes under it is not deleted, and nothing goes under itself.
    let none = BTreeSet::new();
    assert_eq!(
        apply(
            &signs("1.0.0"),
            &ConceptEdit::Delete {
                code: "bleeding".into()
            },
            &none,
            None
        ),
        Err(EditRefused::HasChildren("bleeding".into()))
    );
    assert_eq!(
        apply(
            &signs("1.0.0"),
            &change("bleeding", "bleeding", Some("heavy-bleeding"), None),
            &none,
            None
        ),
        Err(EditRefused::UnderItself("bleeding".into()))
    );
}

#[test]
fn retiring_a_published_code_keeps_it_with_its_status_retired_and_is_minor() {
    let previous = signs("1.0.0");
    let retired = apply(
        &signs("1.0.0"),
        &ConceptEdit::Retire {
            code: "fits".into(),
        },
        &published(),
        Some(&previous),
    )
    .expect("retired");
    assert_eq!(codes(&retired), published(), "the code is kept");
    assert_eq!(
        retired.pointer("/concept/0/concept/1/property"),
        Some(&json!([{ "code": "status", "valueCode": "retired" }]))
    );
    assert_eq!(
        retired["property"],
        json!([{ "code": "status", "uri": STATUS_URI, "type": "code" }])
    );
    let found = versioning(Some(&previous), &retired);
    assert_eq!(found.bump, Bump::Minor);
    assert_eq!(retired["version"], "1.1.0");
}

#[test]
fn changing_a_published_codes_definition_is_major_with_the_reason() {
    let previous = signs("1.2.3");
    let redefined = apply(
        &signs("1.2.3"),
        &change(
            "fits",
            "fits",
            Some("danger"),
            Some("Generalised fits only"),
        ),
        &published(),
        Some(&previous),
    )
    .expect("redefined");
    let found = versioning(Some(&previous), &redefined);
    assert_eq!(found.bump, Bump::Major);
    assert_eq!(found.next.as_deref(), Some("2.0.0"));
    assert_eq!(redefined["version"], "2.0.0");
    let reasons: Vec<String> = found.changes.iter().map(bump::describe).collect();
    assert!(
        reasons.contains(&"Code fits: definition changed".to_owned()),
        "{reasons:?}"
    );
    // Only a new display is a patch.
    let shown = apply(
        &signs("1.2.3"),
        &change("fits", "fits", Some("danger"), Some("Fits of any kind")),
        &published(),
        Some(&previous),
    )
    .expect("displayed");
    assert_eq!(versioning(Some(&previous), &shown).bump, Bump::Patch);
    assert_eq!(shown["version"], "1.2.4");
}

#[test]
fn a_code_system_never_published_keeps_its_version() {
    let edited = apply(
        &signs("0.1.0"),
        &add("spotting", None),
        &BTreeSet::new(),
        None,
    )
    .expect("added");
    assert_eq!(edited["version"], "0.1.0");
    let found = versioning(None, &edited);
    assert_eq!((found.published, found.next), (None, None));
}
