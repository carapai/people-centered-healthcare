use serde_json::{Value, json};

use super::check_response;
use crate::fhirpath::OctofhirEngine;
use crate::structure::IssueCode;

const SDC: &str = "http://hl7.org/fhir/uv/sdc/StructureDefinition";

fn anc() -> Value {
    json!({
        "resourceType": "Questionnaire", "url": "urn:anc", "version": "2.0.0", "status": "active",
        "item": [
            { "linkId": "age", "type": "integer", "required": true, "extension": [{
                "url": "http://hl7.org/fhir/StructureDefinition/questionnaire-constraint",
                "extension": [
                    { "url": "key", "valueId": "age-plausible" },
                    { "url": "severity", "valueCode": "error" },
                    { "url": "expression", "valueString": "%resource.item.where(linkId = 'age').answer.value.empty() or %resource.item.where(linkId = 'age').answer.value >= 10" },
                    { "url": "human", "valueString": "At least 10 years old" }
                ]
            }] },
            { "linkId": "counselling", "type": "boolean", "extension": [{
                "url": format!("{SDC}/sdc-questionnaire-enableWhenExpression"),
                "valueExpression": { "language": "text/fhirpath", "expression": "%resource.item.where(linkId = 'age').answer.value < 18" }
            }] },
            { "linkId": "pregnant", "type": "boolean" },
            { "linkId": "lmp", "type": "date", "required": true,
              "enableWhen": [{ "question": "pregnant", "operator": "=", "answerBoolean": true }] },
            { "linkId": "sign", "type": "coding", "answerOption": [
                { "valueCoding": { "system": "urn:signs", "code": "bleeding" } },
                { "valueCoding": { "system": "urn:signs", "code": "fits" } }
            ] },
            { "linkId": "start", "type": "decimal" },
            { "linkId": "gain", "type": "decimal" },
            { "linkId": "now", "type": "decimal", "readOnly": true, "extension": [{
                "url": format!("{SDC}/sdc-questionnaire-calculatedExpression"),
                "valueExpression": { "language": "text/fhirpath", "expression": "%resource.item.where(linkId = 'start').answer.value + %resource.item.where(linkId = 'gain').answer.value" }
            }] }
        ]
    })
}

fn response(status: &str, items: Value) -> Value {
    json!({ "resourceType": "QuestionnaireResponse", "status": status, "questionnaire": "urn:anc|2.0.0", "item": items })
}

fn codes(issues: &[crate::structure::Issue]) -> Vec<(IssueCode, String)> {
    issues
        .iter()
        .map(|i| (i.code, i.location.clone()))
        .collect()
}

#[tokio::test]
async fn a_valid_response_is_stored_with_the_servers_calculated_value() {
    let engine = OctofhirEngine::new();
    // As the server receives it: JSON text, so decimals keep their scale (json! would not).
    let submitted: Value = serde_json::from_str(
        r#"{ "resourceType": "QuestionnaireResponse", "status": "completed",
             "questionnaire": "urn:anc|2.0.0", "item": [
          { "linkId": "age", "answer": [{ "valueInteger": 25 }] },
          { "linkId": "pregnant", "answer": [{ "valueBoolean": true }] },
          { "linkId": "lmp", "answer": [{ "valueDate": "2026-07-01" }] },
          { "linkId": "sign", "answer": [{ "valueCoding": { "system": "urn:signs", "code": "fits" } }] },
          { "linkId": "start", "answer": [{ "valueDecimal": 61.50 }] },
          { "linkId": "gain", "answer": [{ "valueDecimal": 0.10 }] },
          { "linkId": "now", "answer": [{ "valueDecimal": 99 }] }
        ] }"#,
    )
    .expect("json");
    let (stored, warnings) = check_response(&engine, &anc(), &submitted)
        .await
        .expect("valid");
    assert!(warnings.is_empty());
    let now = stored["item"]
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["linkId"] == "now")
        .expect("now");
    assert_eq!(
        now["answer"][0]["valueDecimal"].to_string(),
        "61.60",
        "the server's value wins, exactly"
    );
}

#[tokio::test]
async fn what_the_browser_should_have_stopped_is_refused() {
    let engine = OctofhirEngine::new();
    let refused = |items: Value, status: &'static str| {
        let engine = &engine;
        async move {
            check_response(engine, &anc(), &response(status, items))
                .await
                .err()
                .map(|issues| codes(&issues))
                .unwrap_or_default()
        }
    };
    assert_eq!(
        refused(
            json!([{ "linkId": "pregnant", "answer": [{ "valueBoolean": true }] }]),
            "completed"
        )
        .await,
        [
            (IssueCode::Required, "age".to_owned()),
            (IssueCode::Required, "lmp".to_owned())
        ]
    );
    assert!(
        refused(
            json!([{ "linkId": "pregnant", "answer": [{ "valueBoolean": true }] }]),
            "in-progress"
        )
        .await
        .is_empty(),
        "a draft may be incomplete"
    );
    assert_eq!(
        refused(json!([{ "linkId": "age", "answer": [{ "valueInteger": 25 }] }, { "linkId": "counselling", "answer": [{ "valueBoolean": true }] }]), "completed").await,
        [(IssueCode::Value, "counselling".to_owned())],
        "a question disabled by its enableWhenExpression takes no answer"
    );
    assert_eq!(
        refused(
            json!([{ "linkId": "age", "answer": [{ "valueInteger": 8 }] }]),
            "completed"
        )
        .await,
        [(IssueCode::Invariant, "age".to_owned())]
    );
    assert_eq!(
        refused(
            json!([{ "linkId": "age", "answer": [{ "valueString": "twenty" }] }]),
            "in-progress"
        )
        .await,
        [(IssueCode::Value, "QuestionnaireResponse.item[0]".to_owned())]
    );
    assert_eq!(
        refused(json!([{ "linkId": "sign", "answer": [{ "valueCoding": { "system": "urn:signs", "code": "rash" } }] }]), "in-progress").await,
        [(IssueCode::CodeInvalid, "QuestionnaireResponse.item[0]".to_owned())]
    );
    assert_eq!(
        refused(json!([{ "linkId": "age", "answer": [{ "valueInteger": 25 }, { "valueInteger": 26 }] }]), "in-progress").await,
        [(IssueCode::Structure, "QuestionnaireResponse.item[0]".to_owned())]
    );
    assert_eq!(
        refused(
            json!([{ "linkId": "weight", "answer": [{ "valueDecimal": 1 }] }]),
            "in-progress"
        )
        .await,
        [(
            IssueCode::Structure,
            "QuestionnaireResponse.item[0]".to_owned()
        )]
    );
}
