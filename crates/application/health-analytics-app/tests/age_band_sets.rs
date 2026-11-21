//! Age Band Sets read from their `Library` (#551; ADR 0071): MoH's ANC set in the national core
//! reference package, and content the set cannot be read from.

use std::path::Path;

use health_analytics_app::{age_band_set, is_age_band_set};
use serde_json::{Value, json};

fn library(path: &str) -> Value {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/reference-packages/core")
        .join(path);
    serde_json::from_slice(&std::fs::read(file).expect("read")).expect("json")
}

#[test]
fn moh_anc_age_bands_read_from_the_national_core() {
    let resource = library("Library-age-bands-anc.json");
    assert!(is_age_band_set(&resource));
    let set = age_band_set(&resource).age_band_set().expect("a set");
    assert_eq!(
        set.canonical(),
        "https://health.go.ug/fhir/Library/age-bands-anc|1.0.0"
    );
    assert_eq!(set.title, "MoH ANC age bands");
    let labels: Vec<String> = set.bands.iter().map(|b| b.label(set.unit)).collect();
    assert_eq!(
        labels,
        ["10–14 years", "15–19 years", "20–24 years", "25+ years"]
    );
    let outpatient = age_band_set(&library("Library-age-bands-outpatient.json"))
        .age_band_set()
        .expect("a set");
    assert_eq!(
        outpatient
            .band(0)
            .map(|b| b.label(outpatient.unit))
            .as_deref(),
        Some("0 years")
    );
}

#[test]
fn unreadable_content_is_named() {
    let mut resource = library("Library-age-bands-anc.json");
    resource["content"][0]["data"] = json!("not base64!");
    assert_eq!(
        age_band_set(&resource).problems(),
        ["the Age Band Set's content is not base64: Invalid symbol 32, offset 3."]
    );
    resource["content"][0]["contentType"] = json!("application/json");
    assert_eq!(
        age_band_set(&resource).problems(),
        ["the Age Band Set's content is not application/vnd.health.age-band-set+json"]
    );
    let workflow =
        json!({"resourceType": "Library", "type": {"coding": [{"code": "workflow-definition"}]}});
    assert!(!is_age_band_set(&workflow));
}
