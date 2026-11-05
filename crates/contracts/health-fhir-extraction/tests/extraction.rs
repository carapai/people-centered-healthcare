//! Template extraction (#93): a fixture response gives exactly its expected Bundle; ids are stable
//! across amendments, repeats and mapping versions; decimals keep their scale; malformed
//! mappings and outputs are refused.

use health_fhir_extraction::{
    ExtractionError, Mapping, MappingError, Mode, Planned, bundle, extract,
};
use health_fhir_validation::{OctofhirEngine, validate_structure};
use serde_json::{Value, json};

// Parsed from text: `json!` would lose a decimal's scale.
fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("json")
}

fn mapping_json() -> Value {
    parse(include_str!("fixtures/anc-mapping.json"))
}

fn response() -> Value {
    parse(include_str!("fixtures/anc-response.json"))
}

fn mapping() -> Mapping {
    Mapping::from_library(&mapping_json()).expect("mapping")
}

async fn plan(response: &Value, mapping: &Mapping) -> Result<Vec<Planned>, ExtractionError> {
    extract(&OctofhirEngine::new(), response, mapping).await
}

fn ids(plan: &[Planned]) -> Vec<String> {
    plan.iter()
        .map(|p| format!("{}/{}", p.resource_type, p.id))
        .collect()
}

#[tokio::test]
async fn the_fixture_response_gives_exactly_its_expected_bundle() {
    let planned = plan(&response(), &mapping()).await.expect("extracted");
    let expected = parse(include_str!("fixtures/anc-expected.json"));
    assert_eq!(bundle(&planned), expected);

    // An extracted 1.50 is written as 1.50 (ADR 0052).
    let text = serde_json::to_string(&planned[0].resource).expect("text");
    assert!(text.contains(r#""value":61.50"#), "{text}");

    for p in &planned {
        let issues = validate_structure(&p.resource.to_string());
        assert!(issues.is_empty(), "{}: {issues:?}", p.template);
    }
    assert_eq!(planned[0].mode, Mode::Managed);
    assert_eq!(planned[0].retraction.as_deref(), Some("entered-in-error"));
    assert_eq!(planned[1].mode, Mode::Seed);
    assert_eq!(planned[1].owner_use_case, "medications.prescribe");
    assert_eq!(
        plan(&response(), &mapping()).await.expect("again"),
        planned,
        "a pure function"
    );
}

#[tokio::test]
async fn ids_survive_amendments_removed_repeats_and_newer_mappings() {
    let first = plan(&response(), &mapping()).await.expect("extracted");

    // Amending the weight versions the same Observation.
    let mut amended = response();
    amended["item"][0]["item"][0]["answer"][0] = parse(r#"{ "valueDecimal": 62.00 }"#);
    let again = plan(&amended, &mapping()).await.expect("amended");
    assert_eq!(ids(&again), ids(&first));
    assert_eq!(
        again[0].resource["valueQuantity"]["value"].to_string(),
        "62.00"
    );

    // Removing the first medication never shifts the second's identity.
    let mut removed = response();
    removed["item"][1]["item"]
        .as_array_mut()
        .expect("repeats")
        .remove(0);
    let fewer = plan(&removed, &mapping()).await.expect("removed");
    assert_eq!(fewer.len(), 2);
    assert_eq!(fewer[1].id, first[2].id, "the ferrous sulfate keeps its id");

    // A newer version of the same mapping gives new versions of the same resources.
    let mut newer = mapping_json();
    newer["version"] = json!("1.1.0");
    assert_eq!(
        ids(&plan(
            &response(),
            &Mapping::from_library(&newer).expect("mapping")
        )
        .await
        .expect("extracted")),
        ids(&first)
    );

    // Another response gives other resources.
    let mut other = response();
    other["id"] = json!("0190f1a2-7c3d-7e4f-8a5b-0000000000c2");
    let theirs = plan(&other, &mapping()).await.expect("extracted");
    assert!(ids(&theirs).iter().all(|id| !ids(&first).contains(id)));
}

#[tokio::test]
async fn answers_decide_what_is_produced() {
    // A danger sign answered yes gives a Condition; none answered gives no weight.
    let mut response = response();
    response["item"][2]["answer"][0] = json!({ "valueBoolean": true });
    response["item"][0]["item"] = json!([]);
    let planned = plan(&response, &mapping()).await.expect("extracted");
    let templates: Vec<&str> = planned.iter().map(|p| p.template.as_str()).collect();
    assert_eq!(templates, ["medication", "medication", "danger-signs"]);
    let condition = &planned[2].resource;
    assert_eq!(
        condition["subject"]["reference"],
        "Patient/0190f1a2-7c3d-7e4f-8a5b-0000000000aa"
    );
    assert!(
        condition.get("derivedFrom").is_none(),
        "Condition has no derivedFrom"
    );
    assert_eq!(
        condition["meta"]["tag"][0],
        json!({ "system": "urn:health:extraction", "code": "seed" })
    );

    // An unanswered element is removed, not written empty.
    let second = &planned[1].resource;
    assert!(second.get("dosageInstruction").is_none(), "{second}");
}

fn with_template_value(path: &str, expression: &str) -> Mapping {
    let mut library = mapping_json();
    library["contained"][0].pointer_mut(path).expect("element")["extension"][0]["valueString"] =
        json!(expression);
    Mapping::from_library(&library).expect("mapping")
}

#[tokio::test]
async fn outputs_that_do_not_fit_are_refused() {
    // Several values for an element that does not repeat.
    let refused = with_template_value("/subject", "%resource.item");
    assert!(matches!(
        plan(&response(), &refused).await,
        Err(ExtractionError::Shape { template, .. }) if template == "weight"
    ));
    // An expression that cannot be evaluated.
    let refused = with_template_value("/subject", "%resource.subject.(");
    assert!(matches!(
        plan(&response(), &refused).await,
        Err(ExtractionError::Expression { template, .. }) if template == "weight"
    ));
    // A complex value written to a primitive.
    let refused = with_template_value("/_effectiveDateTime", "%resource.subject");
    assert!(matches!(
        plan(&response(), &refused).await,
        Err(ExtractionError::Shape { .. })
    ));
    // A response without a resource id.
    let mut anonymous = response();
    anonymous.as_object_mut().expect("object").remove("id");
    assert_eq!(
        plan(&anonymous, &mapping()).await,
        Err(ExtractionError::NoResponseId)
    );
    // Two templates naming the same resource.
    let mut library = mapping_json();
    let mut twin = library["contained"][0].clone();
    twin["id"] = json!("weight-again");
    library["contained"]
        .as_array_mut()
        .expect("contained")
        .push(twin);
    let twins = Mapping::from_library(&library).expect("mapping");
    // Keyed by the same item and type, the second collides.
    assert!(matches!(
        plan(&response(), &twins).await,
        Err(ExtractionError::Collision(_))
    ));
}

#[test]
fn malformed_mappings_are_refused() {
    let mut library = mapping_json();
    library["type"]["coding"][0]["code"] = json!("matching-rules");
    assert_eq!(
        Mapping::from_library(&library),
        Err(MappingError::NotAMapping)
    );

    let mut library = mapping_json();
    library["contained"][0]["extension"][0]["extension"]
        .as_array_mut()
        .expect("governing")
        .remove(1);
    assert!(matches!(
        Mapping::from_library(&library),
        Err(MappingError::Template { template, reason }) if template == "weight" && reason.contains("retraction")
    ));

    let mut library = mapping_json();
    library["contained"][1]["extension"][0]["extension"]
        .as_array_mut()
        .expect("governing")
        .remove(1);
    assert!(matches!(
        Mapping::from_library(&library),
        Err(MappingError::Template { .. })
    ));

    let mut library = mapping_json();
    library["contained"][2]["id"] = json!("weight");
    assert!(matches!(
        Mapping::from_library(&library),
        Err(MappingError::Template { .. })
    ));

    let mut library = mapping_json();
    library.as_object_mut().expect("object").remove("extension");
    assert_eq!(
        Mapping::from_library(&library),
        Err(MappingError::Missing("target Questionnaire"))
    );
}

#[test]
fn the_mapping_names_its_target_and_expressions() {
    let mapping = mapping();
    assert_eq!(
        mapping.canonical(),
        "https://health.go.ug/fhir/Library/anc-visit-mapping|1.0.0"
    );
    assert_eq!(
        mapping.questionnaire,
        "https://health.go.ug/fhir/Questionnaire/anc-visit"
    );
    assert_eq!(mapping.versions, ">=1.0.0 <2.0.0");
    let expressions = mapping.expressions();
    assert!(expressions.contains(&"answer.value".to_owned()));
    assert!(
        expressions
            .contains(&"item.where(linkId = 'danger-signs' and answer.value = true)".to_owned())
    );
    assert_eq!(expressions.len(), 13);
}
