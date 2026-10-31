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

// ---------------------------------------------------------------------------------------------
// Blocking keys and $match

use std::collections::BTreeSet;

use health_client_registry::matching::{
    BlockKey, Evaluation, MatchGrade, MatchingRules, evaluate, index_keys, plan, query_keys,
};
use health_identifiers::ResourceId;
use health_persistence::{
    BlockingIndex, DerivedKeys, ResourceType, Store, StoreError, Transaction,
};
use serde_json::json;

use crate::fhir::is_golden;

/// The blocking keys the store writes with each version of a source Patient (ADR 0055).
/// Golden records and merged-away sources are never candidates, so they have none.
pub struct RegistryBlocking {
    rules: MatchingRules,
    config: RegistryConfig,
}

impl RegistryBlocking {
    /// Keys under `rules`.
    pub fn new(rules: MatchingRules, config: RegistryConfig) -> Self {
        Self { rules, config }
    }
}

impl DerivedKeys for RegistryBlocking {
    fn keys(&self, resource_type: &str, resource: &Value) -> Vec<(String, String)> {
        if resource_type != "Patient"
            || is_golden(resource)
            || resource["active"] == Value::Bool(false)
        {
            return Vec::new();
        }
        index_keys(
            &person(resource, &self.config),
            &DoubleMetaphone,
            &self.rules,
        )
        .into_iter()
        .map(|k| (k.kind.code().to_owned(), k.value))
        .collect()
    }
}

/// One candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// The source Patient.
    pub id: ResourceId,
    /// Its stored JSON.
    pub resource: Value,
    /// Its score and grade.
    pub evaluation: Evaluation,
}

/// What `$match` found.
#[derive(Debug, Clone, PartialEq)]
pub struct Matched {
    /// Candidates graded possible or better, best first.
    pub candidates: Vec<Candidate>,
    /// Keys looked up.
    pub keys_used: usize,
    /// Keys skipped by the stop-list (a quality metric).
    pub keys_skipped: usize,
    /// No key could be used: too little identifying data.
    pub insufficient: bool,
}

/// `Patient/$match` (patient-identity.md § Matching): candidates from the blocking index,
/// scored by the Matching Rules.
pub struct Matcher<S> {
    store: S,
    rules: MatchingRules,
    config: RegistryConfig,
}

fn patient_type() -> ResourceType {
    "Patient"
        .parse()
        .unwrap_or_else(|_| unreachable!("Patient is a resource type"))
}

impl<S> Matcher<S>
where
    S: Store,
    S::Tx: BlockingIndex,
{
    /// A matcher over `store` (whose blocking keys come from [`RegistryBlocking`] with the same
    /// rules).
    pub fn new(store: S, rules: MatchingRules, config: RegistryConfig) -> Self {
        Self {
            store,
            rules,
            config,
        }
    }

    /// Finds candidates for the Patient `query`. Oversized blocks are skipped by their stored
    /// counts; a block is never scanned with a limit instead.
    pub async fn find(
        &self,
        query: &Value,
        count: usize,
        only_certain: bool,
    ) -> Result<Matched, StoreError> {
        let query = person(query, &self.config);
        let keys = query_keys(&query, &DoubleMetaphone, &self.rules);
        let mut tx = self.store.begin().await?;
        let pairs: Vec<(String, String)> = keys
            .iter()
            .map(|k| (k.kind.code().to_owned(), k.value.clone()))
            .collect();
        let counts = tx.counts(&pairs).await?;
        let sized: Vec<(BlockKey, u64)> = keys
            .into_iter()
            .zip(counts)
            .map(|(k, c)| (k, c.unwrap_or(0)))
            .collect();
        let lookup = |k: &BlockKey| sized.iter().find(|(s, _)| s == k).map_or(0, |(_, c)| *c);
        let planned = plan(
            sized.iter().map(|(k, _)| k.clone()).collect(),
            &lookup,
            &self.rules,
        );
        let mut matched = Matched {
            candidates: Vec::new(),
            keys_used: planned.used.len(),
            keys_skipped: planned.skipped.len(),
            insufficient: planned.insufficient(),
        };
        if matched.insufficient {
            return Ok(matched);
        }
        // Counts lag writes, so a block may have grown past max_block since the recount; the
        // limit only stops one unexpectedly large block from being read whole.
        let limit = usize::try_from(self.rules.max_block)
            .unwrap_or(usize::MAX)
            .saturating_add(1);
        let mut ids = BTreeSet::new();
        for key in &planned.used {
            ids.extend(tx.members(key.kind.code(), &key.value, limit).await?);
        }
        for id in ids {
            let Some(head) = tx.read(&patient_type(), id).await? else {
                continue;
            };
            if head.version.deleted {
                continue;
            }
            let resource: Value = serde_json::from_str(head.json.as_str())
                .map_err(|e| StoreError::Unavailable(e.to_string()))?;
            let evaluation = evaluate(
                &query,
                &person(&resource, &self.config),
                &self.rules,
                &DoubleMetaphone,
                &|_, _| None,
            );
            let keep = if only_certain {
                evaluation.grade.auto_links()
            } else {
                evaluation.grade != MatchGrade::CertainlyNot
            };
            if keep {
                matched.candidates.push(Candidate {
                    id,
                    resource,
                    evaluation,
                });
            }
        }
        matched
            .candidates
            .sort_by(|a, b| b.evaluation.score.total_cmp(&a.evaluation.score));
        matched.candidates.truncate(count);
        Ok(matched)
    }
}

/// The FHIR answer for `matched`: a searchset Bundle with `search.score` (the match weight as a
/// probability) and the `match-grade` extension, or an OperationOutcome when there was too
/// little to match on.
pub fn bundle(matched: &Matched) -> Value {
    if matched.insufficient {
        return json!({
            "resourceType": "OperationOutcome",
            "issue": [{
                "severity": "warning",
                "code": "incomplete",
                "diagnostics": "too little identifying data to match on; search the registry by name, birth year and village or parish",
            }],
        });
    }
    let entries: Vec<Value> = matched
        .candidates
        .iter()
        .map(|c| {
            let probability = if c.evaluation.score.is_infinite() {
                if c.evaluation.score > 0.0 { 1.0 } else { 0.0 }
            } else {
                1.0 / (1.0 + (-c.evaluation.score).exp2())
            };
            json!({
                "fullUrl": format!("Patient/{}", c.id),
                "resource": c.resource,
                "search": {
                    "mode": "match",
                    "score": (probability * 10_000.0).round() / 10_000.0,
                    "extension": [{ "url": "http://hl7.org/fhir/StructureDefinition/match-grade", "valueCode": c.evaluation.grade.code() }],
                },
            })
        })
        .collect();
    json!({ "resourceType": "Bundle", "type": "searchset", "total": entries.len(), "entry": entries })
}
