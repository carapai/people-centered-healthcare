//! Completing a QuestionnaireResponse (#93): the mapping chosen, the Owners that accept each
//! planned resource, reconciling with the last run (managed written or retracted, seed reviewed),
//! and the run's Provenance.

use std::collections::BTreeMap;

use health_clinical_documentation_app::{
    CompletionError, MAPPING_EXTENSION, Run, Step, admit, choose, in_range, key, provenance,
    provenance_id, reconcile, record_mapping, recorded_mapping, targets,
};
use health_fhir_extraction::{Mapping, Planned, extract};
use health_fhir_validation::{OctofhirEngine, validate_structure};
use serde_json::{Value, json};

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("json")
}

fn library() -> Value {
    parse(include_str!(
        "../../../contracts/health-fhir-extraction/tests/fixtures/anc-mapping.json"
    ))
}

fn mapping() -> Mapping {
    Mapping::from_library(&library()).expect("mapping")
}

fn response() -> Value {
    parse(include_str!(
        "../../../contracts/health-fhir-extraction/tests/fixtures/anc-response.json"
    ))
}

async fn plan(response: &Value) -> Vec<Planned> {
    extract(&OctofhirEngine::new(), response, &mapping())
        .await
        .expect("extracted")
}

/// What the store holds after `plan` was written: each resource with the store's own meta.
fn stored(plan: &[Planned]) -> BTreeMap<String, Value> {
    plan.iter()
        .map(|p| {
            let mut resource = p.resource.clone();
            resource["meta"]["versionId"] = json!("v1");
            resource["meta"]["tag"]
                .as_array_mut()
                .expect("tags")
                .push(json!({ "system": "urn:health:facility", "code": "KAM013" }));
            (key(&p.resource_type, p.id), resource)
        })
        .collect()
}

#[tokio::test]
async fn owners_accept_only_what_they_opted_into() {
    let planned = plan(&response()).await;
    for p in &planned {
        admit(p).expect("the ANC targets are accepted");
    }
    let mut lab = planned[0].clone();
    lab.resource["category"] = json!([{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "laboratory" }] }]);
    assert!(
        matches!(admit(&lab), Err(CompletionError::NotAccepted { .. })),
        "clinical-data takes no laboratory results"
    );
    lab.owner_use_case = "laboratory.record-point-of-care".into();
    admit(&lab).expect("laboratory takes point-of-care results");
    let mut weight = planned[0].clone();
    weight.owner_use_case = "laboratory.record-point-of-care".into();
    assert!(
        admit(&weight).is_err(),
        "laboratory takes laboratory results only"
    );

    let mut wrong = planned[1].clone();
    wrong.owner_use_case = "clinical-data.record-observation".into();
    assert_eq!(
        admit(&wrong),
        Err(CompletionError::NotAccepted {
            template: "medication".into(),
            use_case: "clinical-data.record-observation".into(),
            reason: "a MedicationRequest".into()
        })
    );
    wrong.owner_use_case = "pharmacy.dispense".into();
    assert!(
        admit(&wrong).is_err(),
        "an unknown use case accepts nothing"
    );
}

#[test]
fn version_ranges() {
    assert!(in_range(">=1.0.0 <2.0.0", "1.4.0"));
    assert!(!in_range(">=1.0.0 <2.0.0", "2.0.0"));
    assert!(in_range(">1.0.0", "1.0.1"));
    assert!(!in_range(">1.0.0", "1.0.0"));
    assert!(in_range("<=1.0.0", "1.0.0"));
    assert!(in_range("=1.4.0", "1.4.0"));
    assert!(in_range("1.4.0", "1.4.0"));
    assert!(!in_range("1.4.0", "1.4.1"));
    assert!(!in_range("", "1.4.0"), "an empty range takes nothing");
    assert!(!in_range(">=1.x", "1.4.0"));
    assert!(!in_range(">=1.0.0", "one"));
}

#[test]
fn the_newest_mapping_whose_range_takes_the_version_is_chosen() {
    let versioned = |version: &str, range: &str| {
        let mut l = library();
        l["version"] = json!(version);
        l["extension"][1]["valueString"] = json!(range);
        Mapping::from_library(&l).expect("mapping")
    };
    let mappings = [
        versioned("1.0.0", ">=1.0.0 <2.0.0"),
        versioned("1.2.0", ">=1.0.0 <2.0.0"),
        versioned("2.0.0", ">=2.0.0"),
    ];
    let url = "https://health.go.ug/fhir/Questionnaire/anc-visit";
    assert_eq!(
        choose(&mappings, &format!("{url}|1.4.0")).map(|m| m.version.as_str()),
        Some("1.2.0")
    );
    assert_eq!(
        choose(&mappings, &format!("{url}|2.1.0")).map(|m| m.version.as_str()),
        Some("2.0.0")
    );
    assert_eq!(choose(&mappings, &format!("{url}|0.9.0")), None);
    assert_eq!(
        choose(&mappings, url),
        None,
        "a response names its Questionnaire's version"
    );
    assert_eq!(
        choose(&mappings, "https://other/Questionnaire/x|1.4.0"),
        None
    );
}

#[test]
fn the_response_records_its_mapping() {
    let mut response = response();
    assert_eq!(recorded_mapping(&response), None);
    record_mapping(
        &mut response,
        "https://health.go.ug/fhir/Library/anc-visit-mapping|1.0.0",
    );
    record_mapping(
        &mut response,
        "https://health.go.ug/fhir/Library/anc-visit-mapping|1.1.0",
    );
    assert_eq!(
        recorded_mapping(&response),
        Some("https://health.go.ug/fhir/Library/anc-visit-mapping|1.1.0")
    );
    assert_eq!(response["extension"].as_array().map(Vec::len), Some(1));
    assert_eq!(response["extension"][0]["url"], MAPPING_EXTENSION);
    response["extension"] = json!([{ "url": "http://other", "valueString": "kept" }]);
    record_mapping(&mut response, "https://x|1.0.0");
    assert_eq!(response["extension"].as_array().map(Vec::len), Some(2));
}

fn written(steps: &[Step]) -> Vec<&str> {
    steps
        .iter()
        .map(|s| match s {
            Step::Write { resource_type, .. } => resource_type.as_str(),
            Step::Retract { .. } => "retract",
            Step::Review { .. } => "review",
        })
        .collect()
}

#[tokio::test]
async fn managed_targets_follow_the_form_and_seed_targets_are_reviewed() {
    let mapping = mapping();
    let first = plan(&response()).await;

    // The first run writes everything.
    let steps = reconcile(&first, &BTreeMap::new(), &[], &mapping);
    assert_eq!(
        written(&steps),
        ["Observation", "MedicationRequest", "MedicationRequest"]
    );

    // The same answers again change nothing, whatever the store stamped on them.
    let current = stored(&first);
    let previous: Vec<String> = current.keys().cloned().collect();
    assert_eq!(reconcile(&first, &current, &previous, &mapping), []);

    // An amended weight versions the managed Observation; an amended drug is reviewed, not rewritten.
    let mut amended = response();
    amended["item"][0]["item"][0]["answer"][0] = parse(r#"{ "valueDecimal": 62.00 }"#);
    amended["item"][1]["item"][0]["item"][0]["answer"][0]["valueCoding"]["code"] =
        json!("322236009");
    let steps = reconcile(&plan(&amended).await, &current, &previous, &mapping);
    assert_eq!(written(&steps), ["Observation", "review"]);

    // A weight no longer answered retracts the managed Observation; a removed seed is left alone.
    let mut emptied = response();
    emptied["item"][0]["item"] = json!([]);
    emptied["item"][1]["item"]
        .as_array_mut()
        .expect("repeats")
        .remove(1);
    let steps = reconcile(&plan(&emptied).await, &current, &previous, &mapping);
    assert_eq!(written(&steps), ["retract"]);
    let Step::Retract { resource, .. } = &steps[0] else {
        unreachable!()
    };
    assert_eq!(resource["status"], "entered-in-error");
    assert!(validate_structure(&resource.to_string()).is_empty());

    // An Observation already retracted is not retracted again.
    let mut current = current;
    let weight = key("Observation", first[0].id);
    current.insert(weight.clone(), resource.clone());
    assert_eq!(
        reconcile(&plan(&emptied).await, &current, &previous, &mapping),
        []
    );
    // And answering the weight again revives it.
    assert_eq!(
        written(&reconcile(&first, &current, &previous, &mapping)),
        ["Observation"]
    );
}

#[tokio::test]
async fn a_retracted_condition_is_entered_in_error() {
    let mut library = library();
    library["contained"][2]["extension"][0]["extension"] = json!([
        { "url": "mode", "valueCode": "managed" },
        { "url": "retraction", "valueCode": "entered-in-error" },
        { "url": "ownerUseCase", "valueString": "clinical-data.record-condition" }
    ]);
    let mapping = Mapping::from_library(&library).expect("mapping");
    let mut yes = response();
    yes["item"][2]["answer"][0] = json!({ "valueBoolean": true });
    let engine = OctofhirEngine::new();
    let first = extract(&engine, &yes, &mapping).await.expect("extracted");
    let current = stored(&first);
    let previous: Vec<String> = current.keys().cloned().collect();
    let none = extract(&engine, &response(), &mapping)
        .await
        .expect("extracted");
    let steps = reconcile(&none, &current, &previous, &mapping);
    let Some(Step::Retract { resource, .. }) =
        steps.iter().find(|s| matches!(s, Step::Retract { .. }))
    else {
        panic!("{steps:?}");
    };
    assert_eq!(
        resource["verificationStatus"]["coding"][0]["code"],
        "entered-in-error"
    );
}

#[tokio::test]
async fn one_provenance_per_response_names_each_run() {
    let response = response();
    let id = response["id"].as_str().expect("id").parse().expect("uuid");
    let planned = plan(&response).await;
    let names: Vec<String> = planned
        .iter()
        .map(|p| format!("{}/_history/v1", key(&p.resource_type, p.id)))
        .collect();
    let run = Run {
        response: &response,
        response_version: "v7",
        mapping: "https://health.go.ug/fhir/Library/anc-visit-mapping|1.0.0",
        targets: &names,
        recorded: "2026-11-05T09:31:00Z",
    };
    let provenance = provenance(&run, id);
    assert_eq!(provenance["id"], provenance_id(id).to_string());
    assert_eq!(
        provenance_id(id),
        provenance_id(id),
        "derived from the response"
    );
    let issues = validate_structure(&provenance.to_string());
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        targets(&provenance),
        planned
            .iter()
            .map(|p| key(&p.resource_type, p.id))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        provenance["entity"][0]["what"]["reference"],
        format!("QuestionnaireResponse/{id}/_history/v7")
    );
    assert_eq!(
        provenance["agent"].as_array().map(Vec::len),
        Some(1),
        "no author on the fixture"
    );

    let mut authored = response.clone();
    authored["author"] =
        json!({ "reference": "Practitioner/0190f1a2-7c3d-7e4f-8a5b-0000000000f1" });
    let with_author = health_clinical_documentation_app::provenance(
        &Run {
            response: &authored,
            ..run
        },
        id,
    );
    assert_eq!(with_author["agent"][0]["who"], authored["author"]);
}
