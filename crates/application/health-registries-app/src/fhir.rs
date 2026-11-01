//! Source and golden Patients as FHIR R5, and their registry domain view.

use std::collections::{BTreeMap, BTreeSet};

use health_client_registry::{Claim, Ecid, Field, GoldenRecord, SourceRecord, Trust};
use health_time::Timestamp;
use serde_json::{Map, Value, json};

/// The ECID's identifier system.
pub const ECID_SYSTEM: &str = "urn:health:ecid";
/// The registry's tag system: `golden` on golden Patients, `provisional` on sources not yet
/// reconciled (identifiers.md).
pub const REGISTRY_TAG: &str = "urn:health:registry";

/// The tag system for where a record came from: `patient-reported` marks what a patient
/// submitted through the portal (ADR 0029).
pub const ORIGIN_TAG: &str = "urn:health:origin";
/// The tag system stating how far a source's values are trusted.
pub const TRUST_TAG: &str = "urn:health:registry:trust";
/// An Identifier extension: `nira-verified` once central verified the NIN with NIRA.
pub const VERIFICATION_EXT: &str = "urn:health:registry:verification";
/// On golden Patients: where a field's surviving value came from.
pub const ORIGIN_EXT: &str = "urn:health:registry:origin";
/// On golden Patients: a source a reviewer separated (the adjudication marker).
pub const SEPARATED_EXT: &str = "urn:health:registry:separated";

/// How the registry reads Patients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryConfig {
    /// The NIN identifier system.
    pub nin_system: String,
}

/// A Patient the registry cannot read as it expects.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct MappingError(pub String);

fn trust_code(trust: Trust) -> &'static str {
    match trust {
        Trust::NiraVerified => "nira-verified",
        Trust::StaffVerified => "staff-verified",
        Trust::StaffEntered => "staff-entered",
        Trust::SelfReported => "self-reported",
    }
}

fn trust_of(code: &str) -> Option<Trust> {
    [
        Trust::NiraVerified,
        Trust::StaffVerified,
        Trust::StaffEntered,
        Trust::SelfReported,
    ]
    .into_iter()
    .find(|t| trust_code(*t) == code)
}

fn tags<'a>(json: &'a Value, system: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    json["meta"]["tag"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(move |t| t["system"] == system)
        .filter_map(|t| t["code"].as_str())
}

/// Whether `json` is a golden Patient.
pub fn is_golden(json: &Value) -> bool {
    tags(json, REGISTRY_TAG).any(|c| c == "golden")
}

/// The Patient a link of `kind` points at (`Patient/{id}` → `{id}`).
pub fn linked(json: &Value, kind: &str) -> Vec<String> {
    json["link"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|l| l["type"] == kind)
        .filter_map(|l| {
            l["other"]["reference"]
                .as_str()?
                .strip_prefix("Patient/")
                .map(str::to_owned)
        })
        .collect()
}

/// A source Patient's registry view. Its trust comes from its trust tag (staff-entered by
/// default, self-reported for self-registration); a NIN central verified with NIRA is trusted
/// at level 1 whatever the record's level.
pub fn source_record(json: &Value, config: &RegistryConfig) -> Result<SourceRecord, MappingError> {
    let id = json["id"]
        .as_str()
        .ok_or_else(|| MappingError("a Patient without an id".into()))?
        .to_owned();
    let trust = tags(json, TRUST_TAG)
        .find_map(trust_of)
        .unwrap_or(Trust::StaffEntered);
    let recorded: Timestamp = json["meta"]["lastUpdated"]
        .as_str()
        .and_then(|t| t.parse().ok())
        .ok_or_else(|| MappingError("a stored Patient without meta.lastUpdated".into()))?;
    let claim = |value: &str, trust: Trust| Claim {
        value: value.to_owned(),
        trust,
        recorded,
        source: id.clone(),
    };
    let mut claims = Vec::new();
    let name = &json["name"][0];
    if let Some(family) = name["family"].as_str() {
        claims.push((Field::Family, claim(family, trust)));
    }
    let given: Vec<&str> = name["given"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !given.is_empty() {
        claims.push((Field::Given, claim(&given.join(" "), trust)));
    }
    if let Some(gender) = json["gender"].as_str() {
        claims.push((Field::Gender, claim(gender, trust)));
    }
    if let Some(date) = json["birthDate"].as_str() {
        claims.push((Field::BirthDate, claim(date, trust)));
    }
    let nin = json["identifier"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|i| i["system"] == config.nin_system.as_str());
    if let Some(nin) = nin
        && let Some(value) = nin["value"].as_str()
    {
        let verified = nin["extension"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|e| e["url"] == VERIFICATION_EXT && e["valueCode"] == "nira-verified");
        claims.push((
            Field::Nin,
            claim(value, if verified { Trust::NiraVerified } else { trust }),
        ));
    }
    let phone = json["telecom"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["system"] == "phone");
    if let Some(phone) = phone.and_then(|t| t["value"].as_str()) {
        claims.push((Field::Phone, claim(phone, trust)));
    }
    if let Some(district) = json["address"][0]["district"].as_str() {
        claims.push((Field::District, claim(district, trust)));
    }
    Ok(SourceRecord {
        id,
        facility: json["managingOrganization"]["reference"]
            .as_str()
            .map(str::to_owned),
        active: json["active"].as_bool().unwrap_or(true),
        claims,
    })
}

/// The golden Patient for `record`, with id `id`.
pub fn golden_json(id: &str, record: &GoldenRecord, config: &RegistryConfig) -> Value {
    let value = |field: Field| record.claims.get(&field).map(|c| c.value.clone());
    let mut patient = json!({
        "resourceType": "Patient",
        "id": id,
        "meta": { "tag": [{ "system": REGISTRY_TAG, "code": "golden" }] },
        "identifier": [{ "use": "official", "system": ECID_SYSTEM, "value": record.ecid.as_str() }],
        "active": !record.sources.is_empty(),
    });
    let mut name = Map::new();
    if let Some(family) = value(Field::Family) {
        name.insert("family".into(), json!(family));
    }
    if let Some(given) = value(Field::Given) {
        name.insert("given".into(), json!(given.split(' ').collect::<Vec<_>>()));
    }
    if !name.is_empty() {
        patient["name"] = json!([Value::Object(name)]);
    }
    if let Some(gender) = value(Field::Gender) {
        patient["gender"] = json!(gender);
    }
    if let Some(date) = value(Field::BirthDate) {
        patient["birthDate"] = json!(date);
    }
    if let Some(nin) = record.claims.get(&Field::Nin) {
        let mut identifier = json!({ "system": config.nin_system, "value": nin.value });
        if nin.trust == Trust::NiraVerified {
            identifier["extension"] =
                json!([{ "url": VERIFICATION_EXT, "valueCode": "nira-verified" }]);
        }
        if let Some(list) = patient["identifier"].as_array_mut() {
            list.push(identifier);
        }
    }
    if let Some(phone) = value(Field::Phone) {
        patient["telecom"] = json!([{ "system": "phone", "value": phone }]);
    }
    if let Some(district) = value(Field::District) {
        patient["address"] = json!([{ "district": district }]);
    }
    let mut extensions: Vec<Value> = record
        .claims
        .iter()
        .map(|(field, claim)| {
            json!({
                "url": ORIGIN_EXT,
                "extension": [
                    { "url": "field", "valueCode": field.code() },
                    { "url": "source", "valueReference": { "reference": format!("Patient/{}", claim.source) } },
                    { "url": "trust", "valueCode": trust_code(claim.trust) },
                    { "url": "recorded", "valueInstant": claim.recorded.to_string() },
                    { "url": "value", "valueString": claim.value },
                ],
            })
        })
        .collect();
    extensions.extend(record.separated.iter().map(|s| json!({ "url": SEPARATED_EXT, "valueReference": { "reference": format!("Patient/{s}") } })));
    if !extensions.is_empty() {
        patient["extension"] = json!(extensions);
    }
    if !record.sources.is_empty() {
        patient["link"] = json!(record
            .sources
            .iter()
            .map(|s| json!({ "other": { "reference": format!("Patient/{s}") }, "type": "seealso" }))
            .collect::<Vec<_>>());
    }
    patient
}

/// The registry view of a stored golden Patient.
pub fn golden_record(json: &Value) -> Result<GoldenRecord, MappingError> {
    if !is_golden(json) {
        return Err(MappingError("not a golden Patient".into()));
    }
    let ecid: Ecid = json["identifier"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|i| i["system"] == ECID_SYSTEM)
        .and_then(|i| i["value"].as_str())
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| MappingError("a golden Patient without a valid ECID".into()))?;
    let mut claims = BTreeMap::new();
    let mut separated = BTreeSet::new();
    for extension in json["extension"].as_array().into_iter().flatten() {
        if extension["url"] == SEPARATED_EXT {
            if let Some(id) = extension["valueReference"]["reference"]
                .as_str()
                .and_then(|r| r.strip_prefix("Patient/"))
            {
                separated.insert(id.to_owned());
            }
            continue;
        }
        if extension["url"] != ORIGIN_EXT {
            continue;
        }
        let part = |url: &str| {
            extension["extension"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|e| e["url"] == url)
        };
        let field = part("field")
            .and_then(|e| e["valueCode"].as_str())
            .and_then(Field::from_code);
        let source = part("source")
            .and_then(|e| e["valueReference"]["reference"].as_str())
            .and_then(|r| r.strip_prefix("Patient/"));
        let trust = part("trust")
            .and_then(|e| e["valueCode"].as_str())
            .and_then(trust_of);
        let recorded = part("recorded")
            .and_then(|e| e["valueInstant"].as_str())
            .and_then(|t| t.parse().ok());
        let value = part("value").and_then(|e| e["valueString"].as_str());
        let (Some(field), Some(source), Some(trust), Some(recorded), Some(value)) =
            (field, source, trust, recorded, value)
        else {
            return Err(MappingError("an incomplete origin extension".into()));
        };
        claims.insert(
            field,
            Claim {
                value: value.to_owned(),
                trust,
                recorded,
                source: source.to_owned(),
            },
        );
    }
    Ok(GoldenRecord {
        ecid,
        sources: linked(json, "seealso").into_iter().collect(),
        claims,
        separated,
    })
}

/// `json` with its links of `kind` replaced by `targets`, leaving other links alone.
pub fn with_links(json: &Value, kind: &str, targets: &[String]) -> Value {
    let mut patient = json.clone();
    let mut links: Vec<Value> = json["link"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|l| l["type"] != kind)
        .cloned()
        .collect();
    links.extend(
        targets
            .iter()
            .map(|t| json!({ "other": { "reference": format!("Patient/{t}") }, "type": kind })),
    );
    match patient.as_object_mut() {
        Some(object) if links.is_empty() => {
            object.remove("link");
        }
        Some(object) => {
            object.insert("link".into(), json!(links));
        }
        None => {}
    }
    patient
}

/// `json` without the `provisional` registry tag (central reconciled it).
pub fn reconciled(json: &Value) -> Value {
    let mut patient = json.clone();
    if let Some(list) = patient["meta"]["tag"].as_array_mut() {
        list.retain(|t| !(t["system"] == REGISTRY_TAG && t["code"] == "provisional"));
    }
    patient
}
