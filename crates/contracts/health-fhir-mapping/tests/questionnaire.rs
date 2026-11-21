//! Reading a pasted Questionnaire as R5, or as R4 converted to R5, with its conversion report
//! (#497, #853): anything that cannot be converted is reported, and blocks the import.

use health_fhir_mapping::questionnaire::{Finding, Refusal, Version, blocks, read};
use serde_json::{Value, json};

fn r4() -> Value {
    json!({
        "resourceType": "Questionnaire",
        "url": "http://smart.who.int/anc/Questionnaire/ANCDT01",
        "version": "1.0.0",
        "status": "active",
        "title": "ANC contact",
        "item": [{
            "linkId": "visit", "type": "group", "text": "Visit",
            "item": [
                { "linkId": "danger", "type": "choice", "text": "Danger signs",
                  "answerOption": [{ "valueCoding": { "code": "bleeding", "display": "Bleeding" } }] },
                { "linkId": "other", "type": "open-choice", "text": "Other",
                  "answerValueSet": "http://example.org/ValueSet/x" }
            ]
        }]
    })
}

#[test]
fn r4_choice_and_open_choice_become_coding_with_an_answer_constraint() {
    let read = read(&r4().to_string(), Version::R4).expect("read");
    let items = &read.questionnaire["item"][0]["item"];
    assert_eq!(items[0]["type"], "coding");
    assert_eq!(items[0]["answerConstraint"], "optionsOnly");
    assert_eq!(items[1]["type"], "coding");
    assert_eq!(items[1]["answerConstraint"], "optionsOrString");
    // Everything else is carried as it was.
    assert_eq!(read.questionnaire["title"], "ANC contact");
    assert_eq!(
        items[0]["answerOption"],
        r4()["item"][0]["item"][0]["answerOption"]
    );
    assert_eq!(
        read.report,
        vec![
            Finding::ChoiceToCoding {
                at: "danger".into()
            },
            Finding::OpenChoiceToCoding { at: "other".into() },
        ]
    );
    assert!(!blocks(&read.report));
}

#[test]
fn an_answer_constraint_already_there_is_left_as_it_says() {
    let mut q = r4();
    q["item"][0]["item"][1]["answerConstraint"] = json!("optionsOrType");
    let read = read(&q.to_string(), Version::R4).expect("read");
    assert_eq!(
        read.questionnaire["item"][0]["item"][1]["answerConstraint"],
        "optionsOrType"
    );
}

#[test]
fn an_r5_paste_holding_r4_choice_types_is_refused_as_looking_like_r4() {
    assert_eq!(
        read(&r4().to_string(), Version::R5).err(),
        Some(Refusal::LooksR4)
    );
}

#[test]
fn what_cannot_be_converted_is_reported_and_blocks_the_import() {
    let mut odd = r4();
    odd["contained"] = json!([{ "resourceType": "Observation", "id": "t" }]);
    odd["somethingElse"] = json!(1);
    odd["item"] = json!([{
        "linkId": "a", "type": "question", "text": "A", "mystery": true,
        "modifierExtension": [{ "url": "x" }]
    }]);
    let read = read(&odd.to_string(), Version::R4).expect("read");
    for expected in [
        Finding::UnknownElement {
            at: "Questionnaire".into(),
            element: "somethingElse".into(),
        },
        Finding::Contained {
            at: "Questionnaire".into(),
        },
        Finding::UnknownElement {
            at: "a".into(),
            element: "mystery".into(),
        },
        Finding::ModifierExtension { at: "a".into() },
        Finding::UnknownType {
            at: "a".into(),
            item_type: "question".into(),
        },
    ] {
        assert!(
            read.report.contains(&expected),
            "{expected:?} in {:?}",
            read.report
        );
    }
    assert!(blocks(&read.report));
}

#[test]
fn a_questionnaire_modifier_extension_blocks_too() {
    let mut q = r4();
    q["modifierExtension"] = json!([{ "url": "x" }]);
    let read = read(&q.to_string(), Version::R4).expect("read");
    assert!(read.report.contains(&Finding::ModifierExtension {
        at: "Questionnaire".into()
    }));
    assert!(blocks(&read.report));
}

#[test]
fn an_item_without_a_link_id_is_reported_at_a_question_mark() {
    let q = json!({ "resourceType": "Questionnaire", "status": "draft",
        "item": [{ "type": "question" }] });
    let read = read(&q.to_string(), Version::R4).expect("read");
    assert_eq!(
        read.report,
        vec![Finding::UnknownType {
            at: "?".into(),
            item_type: "question".into()
        }]
    );
}

#[test]
fn primitive_extensions_are_known_elements_and_an_r5_paste_converts_nothing() {
    let r5 = json!({
        "resourceType": "Questionnaire", "status": "active", "_title": { "extension": [] },
        "item": [{ "linkId": "a", "type": "coding", "_text": { "extension": [] }, "answerOption": [] }]
    });
    let read = read(&r5.to_string(), Version::R5).expect("read");
    assert_eq!(read.report, vec![]);
    assert_eq!(read.questionnaire, r5);
}

#[test]
fn an_empty_item_list_is_dropped() {
    let q = json!({ "resourceType": "Questionnaire", "status": "draft", "item": [] });
    let read = read(&q.to_string(), Version::R4).expect("read");
    assert!(read.questionnaire.get("item").is_none());
}

#[test]
fn not_json_or_not_a_questionnaire_is_refused() {
    assert_eq!(read("{", Version::R5).err(), Some(Refusal::NotJson));
    assert_eq!(
        read(r#"{"resourceType":"Patient"}"#, Version::R5).err(),
        Some(Refusal::NotQuestionnaire)
    );
    assert_eq!(
        read("[]", Version::R4).err(),
        Some(Refusal::NotQuestionnaire)
    );
}

#[test]
fn each_finding_has_a_wire_code_and_an_outcome() {
    let converted = Finding::OpenChoiceToCoding { at: "x".into() };
    assert_eq!(converted.code(), "open-choice-to-coding");
    assert!(converted.converted());
    let unconverted = Finding::UnknownType {
        at: "x".into(),
        item_type: "question".into(),
    };
    assert_eq!(unconverted.code(), "unknown-type");
    assert!(!unconverted.converted());
    assert_eq!(unconverted.at(), "x");
    assert_eq!(Refusal::LooksR4.code(), "looks-r4");
}
