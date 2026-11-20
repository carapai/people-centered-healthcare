use health_configuration::language::Status;
use serde_json::{Value, json};

use super::*;

const ANC: &str = "http://health.go.ug/fhir/Questionnaire/anc-visit";
const LAYOUT: &str = "http://health.go.ug/fhir/layout/anc-visit";
const SIGNS: &str = "http://health.go.ug/fhir/CodeSystem/danger-signs";

fn file(path: &str, value: &Value) -> (String, Vec<u8>) {
    (path.to_owned(), value.to_string().into_bytes())
}

/// `ug.moh.anc` with the item `bp` worded `bp_text`.
fn target(version: &str, bp_text: &str) -> Files {
    Files::from([
        file(
            "package.json",
            &json!({ "name": "ug.moh.anc", "version": version, "kind": "program" }),
        ),
        file(
            "Questionnaire-anc-visit.json",
            &json!({
                "resourceType": "Questionnaire", "url": ANC, "version": version,
                "item": [
                    { "linkId": "vitals", "type": "group", "text": "Vitals", "item": [
                        { "linkId": "bp", "type": "string", "prefix": "1.", "text": bp_text, "item": [
                            { "linkId": "bp-help", "type": "display", "text": "Seated, after rest",
                              "extension": [{
                                  "url": ITEM_CONTROL,
                                  "valueCodeableConcept": { "coding": [{ "code": "help" }] }
                              }] }
                        ] }
                    ] },
                    { "linkId": "visit-type", "type": "coding", "text": "Visit", "answerOption": [
                        { "valueCoding": { "system": "x", "code": "first", "display": "First visit" } },
                        { "valueString": "free text has no code" }
                    ] }
                ]
            }),
        ),
        file(
            "configuration/layouts/anc-visit.json",
            &json!({
                "$schema": FORM_LAYOUT, "url": LAYOUT, "version": version,
                "questionnaire": format!("{ANC}|{version}"),
                "sections": [
                    { "id": "vitals", "title": "Vital signs", "items": ["vitals"] },
                    { "title": "Untranslatable without an id", "items": [] }
                ]
            }),
        ),
    ])
}

/// `ug.moh.core`, a dependency binding the danger signs.
fn core() -> Files {
    Files::from([
        file(
            "CodeSystem-danger-signs.json",
            &json!({
                "resourceType": "CodeSystem", "url": SIGNS, "version": "2.1.0",
                "concept": [{ "code": "bleeding", "display": "Bleeding", "concept": [
                    { "code": "heavy", "display": "Heavy bleeding" }
                ] }]
            }),
        ),
        file(
            "ValueSet-signs.json",
            &json!({
                "resourceType": "ValueSet", "url": "http://health.go.ug/fhir/ValueSet/signs",
                "version": "2.1.0",
                "compose": { "include": [{ "system": SIGNS, "concept": [
                    { "code": "bleeding", "display": "Overridden by the CodeSystem" },
                    { "code": "fits", "display": "Convulsions" }
                ] }] }
            }),
        ),
    ])
}

fn item(link_id: &str, part: Part) -> Key {
    Key::Item {
        questionnaire: ANC.into(),
        link_id: link_id.into(),
        part,
    }
}

fn concept(code: &str) -> Key {
    Key::Concept {
        system: SIGNS.into(),
        code: code.into(),
    }
}

#[test]
fn every_translatable_string_is_found_by_its_address() {
    let english = english(&[&target("2.0.0", "Blood pressure"), &core()]).expect("english");
    let expected = BTreeMap::from([
        (item("vitals", Part::Text), "Vitals"),
        (item("bp", Part::Text), "Blood pressure"),
        (item("bp", Part::Prefix), "1."),
        (item("bp", Part::Help), "Seated, after rest"),
        (item("visit-type", Part::Text), "Visit"),
        (
            Key::Answer {
                questionnaire: ANC.into(),
                link_id: "visit-type".into(),
                code: "first".into(),
            },
            "First visit",
        ),
        (
            Key::Section {
                layout: LAYOUT.into(),
                section: "vitals".into(),
            },
            "Vital signs",
        ),
        (concept("bleeding"), "Bleeding"),
        (concept("heavy"), "Heavy bleeding"),
        (concept("fits"), "Convulsions"),
    ]);
    let found: BTreeMap<Key, &str> = english
        .iter()
        .map(|(key, text)| (key.clone(), text.as_str()))
        .collect();
    assert_eq!(found, expected);
}

fn entry(key: Key, english: &str, text: &str) -> Value {
    json!({ "key": key, "english": english, "text": text })
}

fn pack(entries: Vec<Value>) -> Packed {
    let files = Files::from([
        file(
            PACKAGE_JSON,
            &json!({
                "name": "ug.moh.anc.lg", "version": "1.0.0", "fhirVersions": ["5.0.0"],
                "kind": "language", "platform": { "minSoftware": "0.3.0", "formatVersion": 1 },
                "translates": { "package": "ug.moh.anc", "version": "2.x", "language": "lg" }
            }),
        ),
        file(
            TRANSLATIONS,
            &json!({ "$schema": crate::LANGUAGE_PACK, "language": "lg", "entries": entries }),
        ),
    ]);
    crate::pack(&files).expect("packed")
}

#[test]
fn after_the_target_rewords_an_item_its_entry_is_stale() {
    let pack = pack(vec![
        entry(item("bp", Part::Text), "Blood pressure", "Puleesa"),
        entry(concept("bleeding"), "Bleeding", "Okuvaamu omusaayi"),
        entry(item("gone", Part::Text), "Removed question", "Ekibuuzo"),
    ]);
    let before = report(&pack, &[&target("2.0.0", "Blood pressure"), &core()]).expect("report");
    assert_eq!(before.stale().count(), 0);
    assert_eq!(before.served().len(), 2);

    let after =
        report(&pack, &[&target("2.1.0", "Blood pressure (mmHg)"), &core()]).expect("report");
    let stale: Vec<_> = after.stale().collect();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].entry.key, item("bp", Part::Text));
    assert_eq!(
        stale[0].status,
        Status::Stale {
            now: "Blood pressure (mmHg)".into()
        }
    );
    let gone: Vec<_> = after.gone().map(|checked| &checked.entry.key).collect();
    assert_eq!(gone, [&item("gone", Part::Text)]);
    assert_eq!(
        after.served().len(),
        1,
        "only the code-list display is served"
    );
}

#[test]
fn a_packs_entries_must_be_in_its_language_once_each() {
    let pack = pack(vec![
        entry(concept("bleeding"), "Bleeding", "Okuvaamu omusaayi"),
        entry(concept("bleeding"), "Bleeding", "Omusaayi"),
    ]);
    let mut files = pack.files.clone();
    let mut read: Value = serde_json::from_slice(&files[TRANSLATIONS]).expect("json");
    read["language"] = json!("sw");
    files.insert(TRANSLATIONS.into(), read.to_string().into_bytes());
    assert_eq!(
        problems(&pack.contents, &files).expect("readable"),
        [
            PackProblem::OtherLanguage {
                pack: "lg".into(),
                entries: "sw".into()
            },
            PackProblem::Duplicate(concept("bleeding")),
        ]
    );
    files.insert(TRANSLATIONS.into(), b"not json".to_vec());
    assert!(problems(&pack.contents, &files).is_err());
}
