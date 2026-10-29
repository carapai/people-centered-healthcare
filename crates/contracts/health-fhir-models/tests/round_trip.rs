//! FHIR conformance: the model round-trips R5 JSON losslessly (ADR 0050; testing.md).
//!
//! - `tests/fixtures/examples/` holds official R5 examples chosen for their edge cases
//!   (primitive `_field` extensions, nulls in arrays, contained resources, decimals with
//!   trailing zeros), plus the one known-unparseable example.
//! - `tests/fixtures/edge/` holds hand-written edge cases (decimal text, `integer64`, unknown
//!   codes).
//! - Set `HEALTH_FHIR_R5_EXAMPLES` to the unpacked official `examples-json.zip` to run the whole
//!   corpus; `tools/fhir/fetch-r5-examples.sh` downloads and checks it. CI runs it on every bump
//!   of the model crate.

use std::path::{Path, PathBuf};

use health_fhir_models::{
    KNOWN_UNPARSEABLE_EXAMPLES, Resource, RoundTrip, parse_resource, round_trip, to_json,
};

fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "json") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

struct Report {
    exact: usize,
    known_invalid: usize,
    failures: Vec<String>,
}

fn run(files: &[PathBuf]) -> Report {
    let mut report = Report {
        exact: 0,
        known_invalid: 0,
        failures: Vec::new(),
    };
    for file in files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let text = std::fs::read_to_string(file).expect("read");
        let known_invalid = KNOWN_UNPARSEABLE_EXAMPLES.contains(&name);
        match (round_trip(&text), known_invalid) {
            (Ok(RoundTrip::Exact), false) => report.exact += 1,
            (Err(_), true) => report.known_invalid += 1,
            (Ok(RoundTrip::Exact), true) => {
                report.failures.push(format!(
                    "{name}: listed as known-unparseable but parses; remove it from the list"
                ));
            }
            (Ok(RoundTrip::Differs { pointer }), _) => report
                .failures
                .push(format!("{name}: differs at {pointer}")),
            (Err(error), false) => report.failures.push(format!("{name}: {error}")),
        }
    }
    report
}

fn fixtures(kind: &str) -> Vec<PathBuf> {
    json_files(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(kind),
    )
}

#[test]
fn selected_official_examples_round_trip_exactly() {
    let files = fixtures("examples");
    assert_eq!(files.len(), 18);
    let report = run(&files);
    assert_eq!(report.failures, Vec::<String>::new());
    assert_eq!((report.exact, report.known_invalid), (17, 1));
}

#[test]
fn edge_cases_keep_decimal_text_integer64_and_extensions() {
    let report = run(&fixtures("edge"));
    assert_eq!(report.failures, Vec::<String>::new());
    assert_eq!(report.exact, 2);

    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/edge/decimals-and-integer64.json"),
    )
    .expect("read");
    let written = to_json(&parse_resource(&text).expect("parse")).expect("write");
    for kept in [
        r#""value":1.50"#,
        r#""value":0.010"#,
        r#""value":123456789012345678901234567890"#,
        r#""value":123456789012.345678901234567890123456"#,
        // The exponent is respelled (`1.0e3` → `1.0e+3`): same value and precision, and
        // serde_json compares the two as equal. Stored bytes are never re-serialized (ADR 0051).
        r#""value":1.0e+3"#,
        r#""value":-0.0"#,
        r#""valueInteger64":"9007199254740993""#, // a string beyond 2^53
    ] {
        assert!(written.contains(kept), "{kept} not kept in {written}");
    }
}

#[test]
fn typed_access_and_strictness() {
    let patient =
        parse_resource(r#"{"resourceType":"Patient","id":"p1","name":[{"family":"Akello"}]}"#)
            .expect("parse");
    let Resource::Patient(patient) = patient else {
        panic!("expected a Patient");
    };
    assert_eq!(
        patient.name[0].family.as_ref().map(|f| f.0.as_str()),
        Some("Akello")
    );
    assert!(
        parse_resource(r#"{"resourceType":"Observation","status":"final"}"#).is_err(),
        "code is required"
    );
    assert!(parse_resource(r#"{"resourceType":"NotAType"}"#).is_err());
    // Unknown elements are dropped, so typed parsing is never the validator (ADR 0051).
    let dropped = parse_resource(
        r#"{"resourceType":"Observation","status":"final","code":{"text":"x"},"valueInteger64":"1"}"#,
    )
    .expect("parse");
    assert!(!to_json(&dropped).expect("write").contains("valueInteger64"));
    assert!(parse_resource("{not json").is_err());
}

/// The whole official corpus, when `HEALTH_FHIR_R5_EXAMPLES` points at it.
#[test]
fn the_official_corpus_round_trips_when_available() {
    let Some(dir) = std::env::var_os("HEALTH_FHIR_R5_EXAMPLES") else {
        eprintln!("skipped: set HEALTH_FHIR_R5_EXAMPLES (tools/fhir/fetch-r5-examples.sh)");
        return;
    };
    let files = json_files(Path::new(&dir));
    let report = run(&files);
    eprintln!(
        "{} files: {} exact, {} known-unparseable, {} failures",
        files.len(),
        report.exact,
        report.known_invalid,
        report.failures.len()
    );
    assert!(
        files.len() >= 2824,
        "{} files; is the corpus complete?",
        files.len()
    );
    assert_eq!(report.failures, Vec::<String>::new());
    assert_eq!(report.known_invalid, KNOWN_UNPARSEABLE_EXAMPLES.len());
}
