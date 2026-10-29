//! The registry, extraction and query parser over R5 core SearchParameters
//! (`tests/fixtures/search-parameters.json`, a subset of `hl7.fhir.r5.core#5.0.0`).

use health_fhir_search::normalize::date_range;
use health_fhir_search::{
    Catalog, Handling, IndexValue, ParamState, ParamType, Prefix, Registry, SearchError,
    SearchValue, StringMatch, Test, extract, parse_query,
};
use jiff::tz::Offset;
use serde_json::{Value, json};

fn catalog() -> Catalog {
    let bundle: Value =
        serde_json::from_str(include_str!("fixtures/search-parameters.json")).expect("fixture");
    let entries = bundle["entry"].as_array().expect("entries");
    Catalog::from_definitions(entries.iter().map(|e| &e["resource"]))
}

fn registry() -> Registry {
    let catalog = catalog();
    let mut registry = Registry::new(Offset::from_hours(3).expect("offset"));
    for (t, codes) in [
        (
            "Patient",
            &[
                "_id",
                "name",
                "family",
                "birthdate",
                "gender",
                "identifier",
                "phone",
                "general-practitioner",
            ][..],
        ),
        (
            "Observation",
            &[
                "_id",
                "_lastUpdated",
                "code",
                "subject",
                "patient",
                "date",
                "value-quantity",
                "code-value-quantity",
                "status",
            ][..],
        ),
        ("RiskAssessment", &["probability"][..]),
    ] {
        for code in codes {
            registry
                .support(&catalog, t, code, ParamState::Active)
                .expect(code);
        }
    }
    registry
}

fn pairs(query: &[(&str, &str)]) -> Vec<(String, String)> {
    query
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn the_catalog_compiles_the_subset_and_rejects_the_rest() {
    let catalog = catalog();
    assert_eq!(catalog.len(), 21);
    let rejected: Vec<_> = catalog.rejected().iter().map(|r| r.url.as_str()).collect();
    assert_eq!(
        rejected,
        ["http://hl7.org/fhir/SearchParameter/Patient-deceased"]
    );
    let composite = catalog
        .find("Observation", "code-value-quantity")
        .expect("composite");
    assert_eq!(
        composite
            .components
            .iter()
            .map(|c| c.param_type)
            .collect::<Vec<_>>(),
        [ParamType::Token, ParamType::Quantity]
    );
    assert!(
        catalog.find("Patient", "_id").is_some(),
        "Resource base applies to every type"
    );
}

/// Set `HEALTH_FHIR_R5_PACKAGE` to an unpacked `hl7.fhir.r5.core#5.0.0/package` directory to
/// compile every core SearchParameter.
#[test]
fn the_full_core_package_compiles_when_available() {
    let Ok(dir) = std::env::var("HEALTH_FHIR_R5_PACKAGE") else {
        return;
    };
    let mut definitions = Vec::new();
    for entry in std::fs::read_dir(dir).expect("package dir") {
        let path = entry.expect("entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name.starts_with("SearchParameter-") && name.ends_with(".json") {
            let text = std::fs::read_to_string(&path).expect("read");
            definitions.push(serde_json::from_str::<Value>(&text).expect("json"));
        }
    }
    let catalog = Catalog::from_definitions(&definitions);
    eprintln!(
        "{} compiled, {} rejected",
        catalog.len(),
        catalog.rejected().len()
    );
    assert!(catalog.len() >= 1190, "{}", catalog.len());
}

#[test]
fn extraction_normalizes_each_type() {
    let registry = registry();
    let patient = json!({
        "resourceType": "Patient", "id": "p1",
        "name": [{"family": "Ñandú", "given": ["Akello", "Grace"]}],
        "birthDate": "1990-05",
        "gender": "female",
        "identifier": [{"system": "urn:ug:nin", "value": "CM900"}],
        "telecom": [{"system": "phone", "value": "+256700"}, {"system": "email", "value": "a@b"}],
        "generalPractitioner": [{"reference": "Practitioner/9"}]
    });
    let extraction = extract(&registry, "Patient", &patient, None);
    let key = |param: &str| -> Vec<IndexValue> {
        extraction
            .entries
            .iter()
            .filter(|e| e.param == param)
            .flat_map(|e| e.key.clone())
            .collect()
    };
    let folded: Vec<_> = key("name")
        .into_iter()
        .map(|v| match v {
            IndexValue::String { folded, .. } => folded,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(folded, ["nandu", "akello", "grace"]);
    assert_eq!(
        key("phone"),
        [IndexValue::Token {
            system: String::new(),
            code: "+256700".into()
        }]
    );
    assert_eq!(
        key("identifier"),
        [IndexValue::Token {
            system: "urn:ug:nin".into(),
            code: "CM900".into()
        }]
    );
    assert_eq!(
        key("birthdate"),
        [IndexValue::Date(
            date_range("1990-05", Offset::from_hours(3).expect("offset")).expect("date")
        )]
    );
    assert!(
        matches!(&key("general-practitioner")[..], [IndexValue::Reference(r)] if r.target_type == "Practitioner" && r.target_id == "9")
    );
    assert_eq!(extraction.missing, Vec::<String>::new());

    let bare = extract(
        &registry,
        "Patient",
        &json!({"resourceType":"Patient","id":"p2"}),
        None,
    );
    assert_eq!(bare.entries.len(), 1, "only _id");
    assert_eq!(bare.missing.len(), 7);
}

#[test]
fn composites_pair_components_from_the_same_element() {
    let registry = registry();
    // Parsed from text: the `json!` macro would read 120.50 as an f64 and lose its scale.
    let observation: Value = serde_json::from_str(
        r#"{"resourceType": "Observation", "id": "o1", "status": "final",
        "code": {"coding": [{"system": "http://loinc.org", "code": "8480-6"}]},
        "subject": {"reference": "Patient/p1"},
        "valueQuantity": {"value": 120.50, "system": "http://unitsofmeasure.org", "code": "mm[Hg]"},
        "effectiveDateTime": "2026-10-29T08:00:00+03:00",
        "meta": {"lastUpdated": "2026-10-29T05:00:00.25Z"}}"#,
    )
    .expect("json");
    let extraction = extract(&registry, "Observation", &observation, None);
    let composite: Vec<_> = extraction
        .entries
        .iter()
        .filter(|e| e.param == "code-value-quantity")
        .collect();
    assert_eq!(composite.len(), 1);
    assert!(
        matches!(&composite[0].key[..], [IndexValue::Token { code, .. }, IndexValue::Quantity { value, .. }] if code == "8480-6" && value.to_string() == "120.50")
    );
    let patient: Vec<_> = extraction
        .entries
        .iter()
        .filter(|e| e.param == "patient")
        .collect();
    assert_eq!(patient.len(), 1, "subject is a Patient");
    let big: Value = serde_json::from_str(
        r#"{"resourceType":"RiskAssessment","prediction":[{"probabilityDecimal": 0.12345678901234567890123456789}]}"#,
    )
    .expect("json");
    let extraction = extract(&registry, "RiskAssessment", &big, None);
    assert_eq!(
        extraction.unindexable.len(),
        1,
        "fails closed beyond 28 digits"
    );
}

#[test]
fn queries_parse_into_typed_clauses() {
    let registry = registry();
    let query = parse_query(
        &registry,
        "Observation",
        &pairs(&[
            ("code", "http://loinc.org|8480-6,|x"),
            ("date", "ge2026-10"),
            ("value-quantity", "lt5.4|http://unitsofmeasure.org|mmol/L"),
            ("code-value-quantity", "http://loinc.org|8480-6$gt100"),
            ("subject", "Patient/p1"),
            ("status:not", "cancelled"),
            ("_count", "5000"),
            ("_sort", "-date"),
        ]),
        Handling::Strict,
    )
    .expect("parses");
    assert_eq!(query.count, Some(1000));
    assert_eq!(query.control, pairs(&[("_sort", "-date")]));
    assert_eq!(query.clauses.len(), 6);
    assert_eq!(
        query.clauses[0].test,
        Test::Any(vec![
            SearchValue::Token {
                system: Some("http://loinc.org".into()),
                code: Some("8480-6".into())
            },
            SearchValue::Token {
                system: Some(String::new()),
                code: Some("x".into())
            },
        ])
    );
    assert!(
        matches!(&query.clauses[1].test, Test::Any(v) if matches!(v[..], [SearchValue::Date { prefix: Prefix::Ge, .. }]))
    );
    assert!(matches!(&query.clauses[5].test, Test::NoneOf(_)));

    let names = parse_query(
        &registry,
        "Patient",
        &pairs(&[
            ("name:exact", "Ñandú"),
            ("family", "NAN"),
            ("general-practitioner", "9"),
        ]),
        Handling::Strict,
    )
    .expect("parses");
    assert_eq!(
        names.clauses[1].test,
        Test::Any(vec![SearchValue::String {
            text: "nan".into(),
            mode: StringMatch::StartsWith
        }])
    );
    assert!(
        matches!(&names.clauses[2].test, Test::Any(v) if matches!(&v[..], [SearchValue::Reference { target_type: None, .. }])),
        "general-practitioner has several target types"
    );
}

#[test]
fn strict_handling_rejects_what_is_not_supported() {
    let mut registry = registry();
    let parse = |registry: &Registry, query: &[(&str, &str)], handling| {
        parse_query(registry, "Patient", &pairs(query), handling)
    };
    assert_eq!(
        parse(&registry, &[("nickname", "x")], Handling::Strict),
        Err(SearchError::UnknownParameter("nickname".into()))
    );
    let lenient = parse(&registry, &[("nickname", "x")], Handling::Lenient).expect("lenient");
    assert_eq!(lenient.ignored, ["nickname"]);
    for (query, modifier) in [
        (("gender:text", "f"), "text"),
        (("gender:in", "http://x"), "in"),
        (("name:phonetic", "x"), "phonetic"),
        (("birthdate:exact", "2026"), "exact"),
    ] {
        let error = parse(&registry, &[query], Handling::Lenient).expect_err(query.0);
        assert!(
            matches!(&error, SearchError::UnsupportedModifier { modifier: m, .. } if m == modifier),
            "{error}"
        );
    }
    assert!(matches!(
        parse(
            &registry,
            &[("general-practitioner.name", "x")],
            Handling::Strict
        ),
        Err(SearchError::UnsupportedFeature(_))
    ));
    assert!(matches!(
        parse(&registry, &[("birthdate", "1990-13")], Handling::Strict),
        Err(SearchError::InvalidValue { .. })
    ));
    assert!(matches!(
        parse(&registry, &[("name:missing", "maybe")], Handling::Strict),
        Err(SearchError::InvalidValue { .. })
    ));

    registry
        .support(&catalog(), "Patient", "address", ParamState::Reindexing)
        .expect("address");
    assert_eq!(
        parse(&registry, &[("address", "Gulu")], Handling::Strict),
        Err(SearchError::NotReady("address".into()))
    );
    registry.activate("Patient", "address");
    assert!(parse(&registry, &[("address", "Gulu")], Handling::Strict).is_ok());
}
