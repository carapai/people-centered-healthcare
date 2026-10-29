//! The HL7 Java validator differential (#166; ADR 0051, ADR 0053). The Java validator is a CI
//! oracle only, never shipped (fhir-r5-model.md § Validation).
//!
//! With `HEALTH_JAVA_VALIDATOR_RESULTS` set to a directory of the validator's `-output` files
//! (run with absolute input paths), this compares its verdict on each file with ours:
//!
//! - **Java:** the file fails when any issue has severity `error` or `fatal`. Terminology is
//!   off (`-tx n/a`); bindings are tier 4.
//! - **Ours:** tiers 1–3 as the write path runs them, with the profiles of
//!   `tests/fixtures/profiles/` and `tests/fixtures/custom-profiles/` active, applied to the
//!   ones a file claims in `meta.profile`.
//!
//! Every disagreement must be explained, with its reason, in
//! `tests/fixtures/java-differential/known-disagreements.json`, and every entry must still
//! explain one:
//!
//! - `files`: a case named by file, in either direction;
//! - `javaMessageIds`: rules outside our tiers (terminology, URL resolution, Java policy). A file
//!   that only Java fails is explained when every one of its Java errors has a listed id. The cases come from `examples/differential_cases.rs` and the official examples.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use health_fhir_validation::{
    OctofhirEngine, ProfileSet, ProfileValidator, SnapshotValidator, validate_invariants,
    validate_value,
};
use serde_json::Value;

fn fixtures(dir: &str) -> Vec<Value> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir);
    std::fs::read_dir(dir)
        .expect("fixtures")
        .map(|e| {
            let text = std::fs::read_to_string(e.expect("entry").path()).expect("read");
            serde_json::from_str(&text).expect("json")
        })
        .collect()
}

/// The Java validator's error messages per validated file.
fn java_verdicts(dir: &Path) -> BTreeMap<PathBuf, Vec<String>> {
    fn outcome(resource: &Value, out: &mut BTreeMap<PathBuf, Vec<String>>) {
        let file = resource["extension"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|e| e["url"] == "http://hl7.org/fhir/StructureDefinition/operationoutcome-file")
            .and_then(|e| e["valueString"].as_str());
        let Some(file) = file else { return };
        let errors = resource["issue"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|i| matches!(i["severity"].as_str(), Some("error" | "fatal")))
            .map(|i| {
                let id = i["extension"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|e| {
                        e["url"]
                            .as_str()
                            .is_some_and(|u| u.ends_with("operationoutcome-message-id"))
                    })
                    .and_then(|e| e["valueCode"].as_str())
                    .unwrap_or("?");
                let at = i["expression"][0].as_str().unwrap_or("");
                format!(
                    "{id} at {at}: {}",
                    i["details"]["text"].as_str().unwrap_or("")
                )
            })
            .collect();
        out.insert(PathBuf::from(file), errors);
    }
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(dir).expect("results").flatten() {
        let text = std::fs::read_to_string(entry.path()).expect("read");
        let result: Value = serde_json::from_str(&text).expect("json");
        match result["resourceType"].as_str() {
            Some("Bundle") => result["entry"]
                .as_array()
                .into_iter()
                .flatten()
                .for_each(|e| outcome(&e["resource"], &mut out)),
            _ => outcome(&result, &mut out),
        }
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn the_java_validator_agrees_except_where_recorded_when_available() {
    let Some(results) = std::env::var_os("HEALTH_JAVA_VALIDATOR_RESULTS") else {
        eprintln!("skipped: set HEALTH_JAVA_VALIDATOR_RESULTS (see .github/workflows/rust-ci.yml)");
        return;
    };
    let java = java_verdicts(Path::new(&results));
    assert!(
        !java.is_empty(),
        "no validator results in {}",
        results.to_string_lossy()
    );

    let mut definitions = fixtures("profiles");
    definitions.extend(fixtures("custom-profiles"));
    let set = Arc::new(ProfileSet::compile(&definitions).expect("the profiles compile"));
    let engine = Arc::new(OctofhirEngine::new());
    let validator = SnapshotValidator::new(set.clone(), engine.clone());

    // file name → (ours fails, Java fails, our issues, Java's errors)
    let mut disagreements: BTreeMap<String, (bool, bool, Vec<String>, Vec<String>)> =
        BTreeMap::new();
    for (file, java_errors) in &java {
        let text = std::fs::read_to_string(file).expect("a validated file");
        let resource: Value = serde_json::from_str(&text).expect("json");
        let mut ours = validate_value(&resource);
        if ours.is_empty() {
            ours.extend(validate_invariants(engine.as_ref(), &resource).await);
            let claimed: Vec<String> = resource["meta"]["profile"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|url| set.urls().any(|u| u == *url))
                .map(str::to_owned)
                .collect();
            if !claimed.is_empty() {
                ours.extend(validator.validate(&resource, &claimed).await);
            }
        }
        let (ours_fails, java_fails) = (!ours.is_empty(), !java_errors.is_empty());
        if ours_fails != java_fails {
            let name = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_owned();
            let ours = ours
                .iter()
                .map(|i| format!("{} at {}: {}", i.code.as_str(), i.location, i.message))
                .collect();
            disagreements.insert(name, (ours_fails, java_fails, ours, java_errors.clone()));
        }
    }

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/java-differential/known-disagreements.json");
    // [{ "reason": "…", "files": ["…"] } | { "reason": "…", "javaMessageIds": ["…"] }]
    let known: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json");
    let strings = |group: &Value, key: &str| -> Vec<String> {
        group[key]
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| v.as_str().expect("a string").to_owned())
            .collect()
    };
    let mut recorded: BTreeMap<String, String> = BTreeMap::new();
    let mut java_only: BTreeMap<String, String> = BTreeMap::new();
    for group in known.as_array().expect("a list of reasons") {
        let reason = group["reason"].as_str().expect("a reason").to_owned();
        assert!(!reason.is_empty(), "every entry needs a reason");
        for file in strings(group, "files") {
            recorded.insert(file, reason.clone());
        }
        for id in strings(group, "javaMessageIds") {
            java_only.insert(id, reason.clone());
        }
    }
    let message_id = |error: &str| error.split(' ').next().unwrap_or_default().to_owned();
    let mut ids_used = std::collections::BTreeSet::new();
    let explained = |name: &str,
                     ours: bool,
                     java_errors: &[String],
                     used: &mut std::collections::BTreeSet<String>| {
        if recorded.contains_key(name) {
            return true;
        }
        let ids: Vec<String> = java_errors.iter().map(|e| message_id(e)).collect();
        let all_java_only =
            !ours && !ids.is_empty() && ids.iter().all(|id| java_only.contains_key(id));
        if all_java_only {
            used.extend(ids);
        }
        all_java_only
    };
    let unexplained: Vec<String> = disagreements
        .iter()
        .filter(|(name, (ours, _, _, java_errors))| {
            !explained(name, *ours, java_errors, &mut ids_used)
        })
        .map(|(name, (ours, java, our_issues, java_errors))| {
            format!(
                "{name}: ours {}, Java {}\n  ours: {our_issues:?}\n  Java: {java_errors:?}",
                if *ours { "fails" } else { "passes" },
                if *java { "fails" } else { "passes" },
            )
        })
        .collect();
    let mut stale: Vec<String> = recorded
        .keys()
        .filter(|name| {
            java.keys().any(|f| f.ends_with(name.as_str()))
                && !disagreements.contains_key(name.as_str())
        })
        .cloned()
        .collect();
    stale.extend(
        java_only
            .keys()
            .filter(|id| !ids_used.contains(*id))
            .cloned(),
    );
    eprintln!(
        "{} files compared; {} disagreements ({} explained by recorded reasons)",
        java.len(),
        disagreements.len(),
        disagreements.len() - unexplained.len()
    );
    assert!(
        unexplained.is_empty(),
        "{} unexplained disagreements:\n{}",
        unexplained.len(),
        unexplained.join("\n")
    );
    assert!(
        stale.is_empty(),
        "recorded files or message ids that no longer explain a disagreement: {stale:?}"
    );
}
