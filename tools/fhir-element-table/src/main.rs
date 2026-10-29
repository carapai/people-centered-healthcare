//! Generates the R5 element table that tier 1 validation walks (ADR 0051), from the
//! StructureDefinitions in an unpacked `hl7.fhir.r5.core#5.0.0` package:
//!
//! ```sh
//! cargo run -p fhir-element-table -- ~/.fhir/packages/hl7.fhir.r5.core#5.0.0/package \
//!   > crates/contracts/health-fhir-validation/data/r5-elements.json
//! ```
//!
//! It keeps the base definitions only (`derivation = specialization`, not profiles). Each
//! element is listed by its path relative to its type, with its cardinality, its type codes,
//! and the target of a `contentReference`. A primitive gets its JSON representation and the
//! `regex` of its `value`. The output is deterministic, so the committed table can be checked
//! for drift.

use std::collections::BTreeMap;
use std::io::Write;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

const FHIR_TYPE: &str = "http://hl7.org/fhir/StructureDefinition/structuredefinition-fhir-type";
const REGEX: &str = "http://hl7.org/fhir/StructureDefinition/regex";

fn extension<'a>(value: &'a Value, url: &str) -> Option<&'a Value> {
    value
        .get("extension")?
        .as_array()?
        .iter()
        .find(|e| e.get("url").and_then(Value::as_str) == Some(url))
}

/// An element's type codes, with `System.*` codes replaced by their FHIR type.
fn type_codes(element: &Value) -> Vec<String> {
    element
        .get("type")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| {
            let code = t.get("code")?.as_str()?;
            if code.starts_with("http://hl7.org/fhirpath/System.") {
                let fhir = extension(t, FHIR_TYPE)
                    .and_then(|e| e.get("valueUrl").or_else(|| e.get("valueUri")))
                    .and_then(Value::as_str)
                    .unwrap_or("string");
                Some(fhir.rsplit('/').next().unwrap_or(fhir).to_owned())
            } else {
                Some(code.to_owned())
            }
        })
        .collect()
}

fn json_kind(primitive: &str) -> &'static str {
    match primitive {
        "boolean" => "boolean",
        "integer" | "positiveInt" | "unsignedInt" | "decimal" => "number",
        _ => "string",
    }
}

fn main() -> Result<()> {
    let dir = std::env::args()
        .nth(1)
        .context("usage: fhir-element-table <package dir>")?;
    let mut types = BTreeMap::new();
    let mut primitives = BTreeMap::new();
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .with_context(|| format!("reading {dir}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("StructureDefinition-") && n.ends_with(".json"))
        })
        .collect();
    files.sort();
    for file in files {
        let sd: Value = serde_json::from_str(&std::fs::read_to_string(&file)?)
            .with_context(|| format!("parsing {}", file.display()))?;
        let kind = sd.get("kind").and_then(Value::as_str).unwrap_or_default();
        let derivation = sd.get("derivation").and_then(Value::as_str);
        let base = matches!(derivation, Some("specialization") | None);
        if !base || !matches!(kind, "resource" | "complex-type" | "primitive-type") {
            continue;
        }
        let name = sd["type"].as_str().context("type")?.to_owned();
        if sd["id"].as_str() != Some(name.as_str()) {
            continue;
        }
        let elements = sd["snapshot"]["element"].as_array().context("snapshot")?;
        if kind == "primitive-type" {
            let value = elements
                .iter()
                .find(|e| e["path"].as_str() == Some(&format!("{name}.value")))
                .with_context(|| format!("{name}.value"))?;
            let regex = value["type"][0]
                .as_object()
                .and_then(|t| extension(&Value::Object(t.clone()), REGEX).cloned())
                .and_then(|e| e.get("valueString").cloned());
            let mut primitive = Map::new();
            primitive.insert("json".into(), json!(json_kind(&name)));
            // The core decimal regex allows 18 digits (and has a stray `}`), but R5 decimals
            // are arbitrary precision: the official examples hold 30- and 36-digit values, and
            // tier 1 accepts and stores them (ADR 0052). JSON number syntax is the check.
            if let Some(regex) = regex.filter(|_| name != "decimal") {
                primitive.insert("regex".into(), regex);
            }
            primitives.insert(name.clone(), Value::Object(primitive));
        }
        let mut table = BTreeMap::new();
        for element in elements {
            let path = element["path"].as_str().context("path")?;
            let Some(relative) = path.strip_prefix(&format!("{name}.")) else {
                continue;
            };
            let max = element["max"].as_str().context("max")?;
            let mut codes = type_codes(element);
            // `Element.id` is a `string`; the snapshots carry an `id` type onto every element's
            // `id`, which neither the base definition nor the official examples follow
            // (ElementDefinition ids hold `:` and exceed 64 characters). Only a resource's own
            // `id` is an `id`.
            let element_id = relative == "id" && kind != "resource" || relative.ends_with(".id");
            if element_id && codes == ["id"] {
                codes = vec!["string".to_owned()];
            }
            let mut entry = vec![
                json!(element["min"].as_u64().context("min")?),
                json!(max),
                json!(codes),
            ];
            if let Some(reference) = element.get("contentReference").and_then(Value::as_str) {
                let target = reference
                    .strip_prefix(&format!("#{name}."))
                    .with_context(|| format!("{path}: contentReference {reference}"))?;
                entry.push(json!(target));
            } else if entry[2].as_array().is_some_and(Vec::is_empty) {
                bail!("{path} has neither a type nor a contentReference");
            }
            table.insert(relative.to_owned(), Value::Array(entry));
        }
        types.insert(
            name,
            json!({
                "kind": kind,
                "abstract": sd["abstract"].as_bool().unwrap_or(false),
                "elements": table,
            }),
        );
    }
    let out = json!({
        "source": "hl7.fhir.r5.core#5.0.0",
        "generator": "tools/fhir-element-table",
        "primitives": primitives,
        "types": types,
    });
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{}", serde_json::to_string_pretty(&out)?)?;
    Ok(())
}
