//! The CapabilityStatement, generated from what this server implements so that it always
//! matches behaviour (the acceptance test exercises every interaction it declares).

use health_fhir_search::{ParamState, ParamType, Registry};
use health_surrealdb::ownership;
use health_time::Timestamp;
use serde_json::{Value, json};

/// The type-level interactions every stored resource type supports.
pub const TYPE_INTERACTIONS: &[&str] = &[
    "read",
    "vread",
    "update",
    "delete",
    "history-instance",
    "history-type",
    "create",
    "search-type",
];

fn type_code(param_type: ParamType) -> &'static str {
    match param_type {
        ParamType::Number => "number",
        ParamType::Date => "date",
        ParamType::String => "string",
        ParamType::Token => "token",
        ParamType::Reference => "reference",
        ParamType::Composite => "composite",
        ParamType::Quantity => "quantity",
        ParamType::Uri => "uri",
    }
}

/// The CapabilityStatement for a server at `base` started at `date`.
pub fn capability_statement(registry: &Registry, base: &str, date: Timestamp) -> Value {
    let resources: Vec<Value> = ownership::resources()
        .iter()
        .map(|r| {
            let search_params: Vec<Value> = registry
                .indexed(r.resource_type)
                .filter(|s| s.state == ParamState::Active)
                .map(|s| {
                    json!({
                        "name": s.param.code,
                        "definition": s.param.url,
                        "type": type_code(s.param.param_type),
                    })
                })
                .collect();
            json!({
                "type": r.resource_type,
                "interaction": TYPE_INTERACTIONS.iter().map(|c| json!({ "code": c })).collect::<Vec<_>>(),
                "versioning": "versioned-update",
                "readHistory": true,
                "updateCreate": true,
                "conditionalCreate": true,
                "conditionalRead": "not-supported",
                "conditionalUpdate": true,
                "conditionalDelete": "single",
                "searchParam": search_params,
            })
        })
        .collect();
    json!({
        "resourceType": "CapabilityStatement",
        "status": "active",
        "date": format!("{date:.3}"),
        "kind": "instance",
        "software": { "name": "health-fhir-server", "version": env!("CARGO_PKG_VERSION") },
        "implementation": { "description": "People-centered healthcare FHIR R5 server", "url": base },
        "fhirVersion": "5.0.0",
        "format": ["application/fhir+json", "json"],
        "rest": [{
            "mode": "server",
            "resource": resources,
            "interaction": [{ "code": "history-system" }],
        }],
    })
}
