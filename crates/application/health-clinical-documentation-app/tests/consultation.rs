//! The consultation's commands (#104): notes, diagnoses, test orders and prescriptions as valid
//! FHIR R5, in their Encounter, tagged with its facility; incomplete commands are refused.

use health_clinical_documentation_app::consultation::{
    CommandRefused, Concept, Context, diagnosis, note, prescription, service_order,
};
use health_fhir_validation::validate_structure;
use serde_json::{Value, json};

fn context() -> Context {
    Context {
        encounter: "Encounter/0190f1a2-7c3d-7e4f-8a5b-000000000201".into(),
        patient: "Patient/0190f1a2-7c3d-7e4f-8a5b-0000000000aa".into(),
        facility: "KAM013".into(),
        author: "clinician-1".into(),
        at: "2026-11-05T10:00:00Z".into(),
    }
}

const ID: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000301";

fn valid(resource: &Value) {
    let issues = validate_structure(&resource.to_string());
    assert!(issues.is_empty(), "{resource}: {issues:?}");
    assert_eq!(resource["id"], ID);
    assert_eq!(
        resource["meta"]["tag"][0],
        json!({ "system": "urn:health:facility", "code": "KAM013" })
    );
}

fn malaria() -> Concept {
    Concept {
        display: "Malaria".into(),
        system: Some("http://snomed.info/sct".into()),
        code: Some("61462000".into()),
    }
}

#[test]
fn a_note_is_a_final_composition_with_its_text_escaped() {
    let composition = note(ID, &context(), "  Fever 3 days <38.5>\n& cough ").expect("note");
    valid(&composition);
    assert_eq!(composition["status"], "final");
    assert_eq!(composition["subject"][0]["reference"], context().patient);
    assert_eq!(composition["encounter"]["reference"], context().encounter);
    assert_eq!(
        composition["section"][0]["text"]["div"],
        "<div xmlns=\"http://www.w3.org/1999/xhtml\"><p>Fever 3 days &lt;38.5&gt;<br/>&amp; cough</p></div>"
    );
    assert_eq!(
        note(ID, &context(), "  "),
        Err(CommandRefused::Missing("the note's text"))
    );
}

#[test]
fn a_diagnosis_is_an_encounter_diagnosis() {
    let condition = diagnosis(ID, &context(), &malaria(), false).expect("diagnosis");
    valid(&condition);
    assert_eq!(
        condition["verificationStatus"]["coding"][0]["code"],
        "confirmed"
    );
    assert_eq!(condition["clinicalStatus"]["coding"][0]["code"], "active");
    assert_eq!(condition["code"]["coding"][0]["code"], "61462000");
    assert_eq!(
        condition["category"][0]["coding"][0]["code"],
        "encounter-diagnosis"
    );
    let provisional = diagnosis(
        ID,
        &context(),
        &Concept {
            display: "Suspected typhoid".into(),
            ..Concept::default()
        },
        true,
    )
    .expect("diagnosis");
    valid(&provisional);
    assert_eq!(
        provisional["verificationStatus"]["coding"][0]["code"],
        "provisional"
    );
    assert_eq!(provisional["code"], json!({ "text": "Suspected typhoid" }));

    assert_eq!(
        diagnosis(ID, &context(), &Concept::default(), false),
        Err(CommandRefused::Missing("what it is"))
    );
    let half = Concept {
        display: "Malaria".into(),
        system: Some("http://snomed.info/sct".into()),
        code: None,
    };
    assert!(matches!(
        diagnosis(ID, &context(), &half, false),
        Err(CommandRefused::Invalid(_))
    ));
}

#[test]
fn orders_name_what_is_ordered() {
    let test = service_order(
        ID,
        &context(),
        &Concept {
            display: "Malaria RDT".into(),
            ..Concept::default()
        },
        Some(" urgent "),
    )
    .expect("order");
    valid(&test);
    assert_eq!(
        (test["status"].as_str(), test["intent"].as_str()),
        (Some("active"), Some("order"))
    );
    assert_eq!(test["code"]["concept"]["text"], "Malaria RDT");
    assert_eq!(test["note"][0]["text"], "urgent");
    let plain = service_order(ID, &context(), &malaria(), Some("")).expect("order");
    assert!(plain.get("note").is_none());

    let artemether = Concept {
        display: "Artemether/lumefantrine 20/120 mg".into(),
        ..Concept::default()
    };
    let request = prescription(
        ID,
        &context(),
        &artemether,
        "4 tablets twice daily for 3 days",
    )
    .expect("prescription");
    valid(&request);
    assert_eq!(
        request["medication"]["concept"]["text"],
        "Artemether/lumefantrine 20/120 mg"
    );
    assert_eq!(
        request["dosageInstruction"][0]["text"],
        "4 tablets twice daily for 3 days"
    );
    assert_eq!(
        prescription(ID, &context(), &artemether, " "),
        Err(CommandRefused::Missing("the dosage"))
    );
}
