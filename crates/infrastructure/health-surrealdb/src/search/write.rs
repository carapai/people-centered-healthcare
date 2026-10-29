//! Index-entry maintenance inside the write transaction (persistence.md § Search index tables:
//! a write deletes the entries listed in the head's `index_keys` and writes the new ones).

use health_fhir_search::normalize::ReferenceKey;
use health_fhir_search::{Extraction, IndexValue, ParamType};

use super::sql::{Bind, Sql};

/// The index table for a parameter type.
pub(crate) fn table(param_type: ParamType) -> &'static str {
    match param_type {
        ParamType::Number => "idx_number",
        ParamType::Date => "idx_date",
        ParamType::String => "idx_string",
        ParamType::Token => "idx_token",
        ParamType::Reference => "idx_reference",
        ParamType::Composite => "idx_composite",
        ParamType::Quantity => "idx_quantity",
        ParamType::Uri => "idx_uri",
    }
}

/// The key elements a value contributes, in record-id order.
pub(crate) fn key_parts(value: &IndexValue) -> Vec<Bind> {
    match value {
        IndexValue::Token { system, code } => {
            vec![Bind::Str(system.clone()), Bind::Str(code.clone())]
        }
        IndexValue::String { folded, .. } => vec![Bind::Str(folded.clone())],
        IndexValue::Date(range) => vec![
            Bind::Datetime(range.start.to_string()),
            Bind::Datetime(range.end.to_string()),
        ],
        IndexValue::Reference(key) => vec![
            Bind::Str(key.target_type.clone()),
            Bind::Str(key.target_id.clone()),
        ],
        IndexValue::Quantity {
            system,
            code,
            value,
        } => vec![
            Bind::Str(system.clone()),
            Bind::Str(code.clone()),
            Bind::Decimal(value.to_string()),
        ],
        IndexValue::Number(value) => vec![Bind::Decimal(value.to_string())],
        IndexValue::Uri(value) => vec![Bind::Str(value.clone())],
    }
}

/// The secondary-index fields of a single-value entry (see migration 0002).
fn fields(value: &IndexValue) -> Vec<(&'static str, Bind)> {
    match value {
        IndexValue::Token { code, .. } => vec![("code", Bind::Str(code.clone()))],
        IndexValue::String { exact, .. } => vec![("exact", Bind::Str(exact.clone()))],
        IndexValue::Date(range) => vec![("end", Bind::Datetime(range.end.to_string()))],
        IndexValue::Reference(key) => vec![("target_id", Bind::Str(key.target_id.clone()))],
        IndexValue::Quantity { code, value, .. } => vec![
            ("code", Bind::Str(code.clone())),
            ("value", Bind::Decimal(value.to_string())),
        ],
        IndexValue::Number(_) | IndexValue::Uri(_) => Vec::new(),
    }
}

/// The literal `Type/id` references in a resource, deduplicated: what `idx_ref_in` holds.
/// Contained (`#id`), `urn:` and other references that name no type are not included.
pub(crate) fn references(resource: &serde_json::Value) -> Vec<ReferenceKey> {
    fn walk(value: &serde_json::Value, out: &mut Vec<ReferenceKey>) {
        match value {
            serde_json::Value::Object(object) => {
                for (key, child) in object {
                    match (key.as_str(), child) {
                        ("reference", serde_json::Value::String(reference)) => {
                            let key = ReferenceKey::parse(reference);
                            if !key.target_type.is_empty() {
                                out.push(key);
                            }
                        }
                        _ => walk(child, out),
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(resource, &mut out);
    out.sort();
    out.dedup();
    out
}

/// Appends statements that replace the head's index entries with `extraction`'s and its
/// reverse `references`, and returns the expression for its new `index_keys`. With `only`,
/// just those parameters' entries are replaced (a reindex job) and references are kept;
/// otherwise every entry is replaced.
pub(crate) fn replace_entries(
    sql: &mut Sql,
    head_table: &str,
    resource_type: &str,
    resource_id: &str,
    extraction: &Extraction,
    references: &[ReferenceKey],
    only: Option<&[String]>,
) -> String {
    let head = sql.str(head_table);
    let id = sql.str(resource_id);
    let old = sql.variable("old");
    sql.push(format!(
        "LET {old} = (SELECT VALUE index_keys FROM ONLY type::record({head}, {id})) ?? []"
    ));
    let drop = sql.variable("drop");
    match only {
        Some(params) => {
            let params = sql.value(Bind::Strs(params.to_vec()));
            sql.push(format!(
                "LET {drop} = array::filter({old}, |$k| record::id($k)[1] IN {params})"
            ));
        }
        None => {
            sql.push(format!("LET {drop} = {old}"));
        }
    }
    sql.push(format!("DELETE {drop}"));

    let rt = sql.str(resource_type);
    let rid = sql.str(resource_id);
    let mut keys = Vec::new();
    let mut write = |sql: &mut Sql, index_table: &str, param: &str, parts: Vec<Bind>, extra| {
        let param = sql.str(param);
        let mut elements = vec![rt.clone(), param.clone()];
        elements.extend(parts.into_iter().map(|p| sql.value(p)));
        elements.push(rid.clone());
        let mut set = vec![format!("rt = {rt}"), format!("param = {param}")];
        for (field, value) in extra {
            let value: Bind = value;
            set.push(format!("{field} = {}", sql.value(value)));
        }
        let key = sql.variable("k");
        sql.push(format!(
            "LET {key} = type::record('{index_table}', [{}])",
            elements.join(", ")
        ));
        sql.push(format!("UPSERT {key} SET {}", set.join(", ")));
        keys.push(key);
    };
    for entry in &extraction.entries {
        let parts = entry.key.iter().flat_map(key_parts).collect();
        let extra = match entry.key.as_slice() {
            [single] if entry.param_type != ParamType::Composite => fields(single),
            _ => Vec::new(),
        };
        write(sql, table(entry.param_type), &entry.param, parts, extra);
    }
    for param in &extraction.missing {
        write(sql, "idx_missing", param, Vec::new(), Vec::new());
    }
    for target in references {
        let (target_type, target_id) = (
            sql.str(target.target_type.as_str()),
            sql.str(target.target_id.as_str()),
        );
        let key = sql.variable("k");
        sql.push(format!(
            "LET {key} = type::record('idx_ref_in', [{target_type}, {target_id}, {rt}, {rid}])"
        ));
        sql.push(format!("UPSERT {key}"));
        keys.push(key);
    }
    format!(
        "array::distinct(array::concat(array::complement({old}, {drop}), [{}]))",
        keys.join(", ")
    )
}
