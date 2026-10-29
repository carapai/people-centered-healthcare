//! Times tier 1 over a directory of resources: `cargo run --release -p health-fhir-validation
//! --example time_corpus -- <dir>`. The budget for tiers 1–3 is p95 ≤ 2 ms per resource on
//! reference edge hardware (ADR 0053). Set `EXCLUDE_TYPES=Bundle,StructureDefinition,…` to
//! leave out resource types, e.g. large conformance resources that are not clinical writes.

use std::time::Instant;

fn main() {
    let Some(dir) = std::env::args().nth(1) else {
        eprintln!("usage: time_corpus <dir>");
        return;
    };
    let exclude: Vec<String> = std::env::var("EXCLUDE_TYPES")
        .map(|v| v.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    let mut texts = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if entry.path().extension().is_some_and(|e| e == "json")
            && let Ok(text) = std::fs::read_to_string(entry.path())
        {
            let resource_type = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v["resourceType"].as_str().map(str::to_owned))
                .unwrap_or_default();
            if !exclude.contains(&resource_type) {
                texts.push(text);
            }
        }
    }
    let _ = health_fhir_validation::validate_structure("{}");
    let mut durations: Vec<f64> = texts
        .iter()
        .map(|text| {
            let started = Instant::now();
            let _ = health_fhir_validation::validate_structure(text);
            started.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    durations.sort_by(f64::total_cmp);
    let at = |q: f64| durations[((durations.len() as f64 - 1.0) * q) as usize];
    eprintln!(
        "{} resources: p50 {:.3} ms, p95 {:.3} ms, p99 {:.3} ms, max {:.3} ms",
        durations.len(),
        at(0.50),
        at(0.95),
        at(0.99),
        at(1.0)
    );
}
