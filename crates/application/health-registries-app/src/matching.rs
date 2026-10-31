//! Matching's adapters: the Double Metaphone encoder, and Patients as matching people.

use health_client_registry::matching::{Identifier, Person, Phonetic};
use rphonetic::Encoder;
use serde_json::Value;

use crate::fhir::{ECID_SYSTEM, RegistryConfig, VERIFICATION_EXT};

/// Double Metaphone primary codes (ADR 0055's default), from `rphonetic`, a port of Apache
/// commons-codec.
#[derive(Debug, Default, Clone, Copy)]
pub struct DoubleMetaphone;

impl Phonetic for DoubleMetaphone {
    fn encode(&self, word: &str, length: usize) -> String {
        rphonetic::DoubleMetaphone::new(Some(length)).encode(word)
    }
}

/// A Patient as matching sees it. A year-only `birthDate` is an estimate (ages are often
/// rounded); the ECID is always verified, a NIN only once NIRA verified it.
pub fn person(json: &Value, config: &RegistryConfig) -> Person {
    let name = &json["name"][0];
    let birth = json["birthDate"].as_str().unwrap_or_default();
    let mut identifiers = Vec::new();
    for identifier in json["identifier"].as_array().into_iter().flatten() {
        let (Some(system), Some(value)) =
            (identifier["system"].as_str(), identifier["value"].as_str())
        else {
            continue;
        };
        let verified = system == ECID_SYSTEM
            || identifier["extension"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|e| e["url"] == VERIFICATION_EXT && e["valueCode"] == "nira-verified");
        if system == ECID_SYSTEM || system == config.nin_system {
            identifiers.push(Identifier {
                system: system.to_owned(),
                value: value.to_owned(),
                verified,
            });
        }
    }
    Person {
        id: json["id"].as_str().map(str::to_owned),
        family: name["family"].as_str().map(str::to_owned),
        given: name["given"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| g.as_str().map(str::to_owned))
            .collect(),
        birth_year: birth.get(..4).and_then(|y| y.parse().ok()),
        birth_estimated: birth.len() == 4,
        sex: json["gender"].as_str().map(str::to_owned),
        district: json["address"][0]["district"].as_str().map(str::to_owned),
        phone: json["telecom"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|t| t["system"] == "phone")
            .and_then(|t| t["value"].as_str())
            .map(str::to_owned),
        identifiers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_metaphone_merges_spelling_variants() {
        let dm = DoubleMetaphone;
        assert_eq!(dm.encode("OKELLO", 6), dm.encode("OKELO", 6));
        assert_eq!(dm.encode("WASSWA", 6), dm.encode("WASWA", 6));
        assert_ne!(dm.encode("OKELLO", 6), dm.encode("MUKASA", 6));
        assert!(dm.encode("NAMUKASA", 6).len() <= 6);
    }
}
