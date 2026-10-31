//! `$match` over SurrealDB (memory engine): the stored blocking index gives the same
//! candidates and grades as the in-memory pipeline, and the stop-list skips blocks by their
//! stored counts.

use std::collections::BTreeSet;
use std::sync::Arc;

use health_client_registry::matching::{
    MatchGrade, MatchingRules, evaluate, index_keys, plan, query_keys,
};
use health_fhir_search::Registry;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, Store, Transaction, WriteRequest};
use health_registries_app::fhir::RegistryConfig;
use health_registries_app::matching::{DoubleMetaphone, Matcher, RegistryBlocking, bundle, person};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use serde_json::{Value, json};
use synthetic_data::population::{Population, systems};
use synthetic_data::world::World;

fn config() -> RegistryConfig {
    RegistryConfig {
        nin_system: systems::NIN.into(),
    }
}

fn rules() -> MatchingRules {
    MatchingRules::platform_default(systems::NIN, &[systems::NIN, "urn:health:ecid"])
}

async fn store(rules: &MatchingRules) -> SurrealStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "match".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(Registry::new(jiff::tz::Offset::UTC))
        .with_blocking(Arc::new(RegistryBlocking::new(rules.clone(), config())))
}

/// Loads `n` synthetic patients, returning their stored JSON.
async fn load(store: &SurrealStore, n: usize) -> Vec<Value> {
    let world = World::new(21);
    let mut stored = Vec::new();
    let mut tx = store.begin().await.expect("tx");
    for (i, mut patient) in Population::new(21, &world, 2).take(n).enumerate() {
        // The synthetic references point at facilities this test does not load.
        if let Some(object) = patient.as_object_mut() {
            object.remove("managingOrganization");
        }
        let id: ResourceId = patient["id"].as_str().expect("id").parse().expect("uuid");
        tx.write(WriteRequest {
            resource_type: "Patient".parse().expect("type"),
            id,
            json: RawJson::from_validated(patient.to_string()),
            op_id: OpId::new(),
            precondition: Precondition::MustNotExist,
        })
        .await
        .expect("write");
        stored.push(patient);
        if i % 500 == 499 {
            tx.commit().await.expect("commit");
            tx = store.begin().await.expect("tx");
        }
    }
    tx.commit().await.expect("commit");
    stored
}

/// The query a clerk might type for `patient`: a typo in the family name, and no NIN.
fn query_for(patient: &Value) -> Value {
    let mut q = patient.clone();
    if let Some(object) = q.as_object_mut() {
        object.remove("id");
        object.remove("identifier");
    }
    if let Some(family) = patient["name"][0]["family"].as_str() {
        let mut chars: Vec<char> = family.chars().collect();
        if chars.len() > 3 {
            chars.swap(2, 3);
        }
        q["name"][0]["family"] = json!(chars.into_iter().collect::<String>());
    }
    q
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn match_over_the_store_agrees_with_the_in_memory_pipeline() {
    let rules = rules();
    let store = store(&rules).await;
    let people = load(&store, 2_000).await;
    assert!(store.recount_blocks().await.expect("recount") > 700);
    let matcher = Matcher::new(store.clone(), rules.clone(), config());

    // The in-memory pipeline over the same people.
    let persons: Vec<_> = people.iter().map(|p| person(p, &config())).collect();
    let index: Vec<BTreeSet<_>> = persons
        .iter()
        .map(|p| {
            index_keys(p, &DoubleMetaphone, &rules)
                .into_iter()
                .collect()
        })
        .collect();
    let block_size = |k: &health_client_registry::matching::BlockKey| {
        index.iter().filter(|keys| keys.contains(k)).count() as u64
    };

    let mut found = 0;
    for (i, patient) in people.iter().enumerate().step_by(40) {
        let query = query_for(patient);
        let matched = matcher.find(&query, 50, false).await.expect("match");

        let q = person(&query, &config());
        let planned = plan(
            query_keys(&q, &DoubleMetaphone, &rules),
            &block_size,
            &rules,
        );
        let expected: BTreeSet<String> = (0..persons.len())
            .filter(|c| planned.used.iter().any(|k| index[*c].contains(k)))
            .filter(|c| {
                evaluate(&q, &persons[*c], &rules, &DoubleMetaphone, &|_, _| None).grade
                    != MatchGrade::CertainlyNot
            })
            .map(|c| people[c]["id"].as_str().expect("id").to_owned())
            .collect();
        let got: BTreeSet<String> = matched
            .candidates
            .iter()
            .map(|c| c.id.to_string())
            .collect();
        assert_eq!(got, expected, "query {i}");
        assert_eq!(
            (matched.keys_used, matched.keys_skipped),
            (planned.used.len(), planned.skipped.len())
        );
        if matched
            .candidates
            .first()
            .is_some_and(|c| c.resource["id"] == patient["id"])
        {
            found += 1;
        }

        let answer = bundle(&matched);
        assert_eq!(answer["type"], "searchset");
        if let Some(first) = answer["entry"].as_array().and_then(|e| e.first()) {
            let score = first["search"]["score"].as_f64().expect("score");
            assert!((0.0..=1.0).contains(&score));
            assert_eq!(
                first["search"]["extension"][0]["url"],
                "http://hl7.org/fhir/StructureDefinition/match-grade"
            );
        }
    }
    assert!(
        found >= 45,
        "the right person ranked first in {found} of 50 queries"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_blocks_are_skipped_by_stored_counts() {
    // A tiny stop-list: any block of more than two people is skipped.
    let rules = MatchingRules {
        max_block: 2,
        ..rules()
    };
    let store = store(&rules).await;
    let household = |given: &str| json!({ "resourceType": "Patient", "name": [{ "family": "Okello", "given": [given] }], "gender": "male", "birthDate": "1990-01-01", "address": [{ "district": "Gulu" }], "telecom": [{ "system": "phone", "value": "+256772123456" }] });
    let mut tx = store.begin().await.expect("tx");
    for given in ["Peter", "Paul", "John", "James"] {
        tx.write(WriteRequest {
            resource_type: "Patient".parse().expect("type"),
            id: ResourceId::new(),
            json: RawJson::from_validated(household(given).to_string()),
            op_id: OpId::new(),
            precondition: Precondition::MustNotExist,
        })
        .await
        .expect("write");
    }
    tx.commit().await.expect("commit");
    let matcher = Matcher::new(store.clone(), rules, config());
    let query = household("Peter");

    // Before any recount nothing is known to be large, so every key is used.
    let before = matcher.find(&query, 10, false).await.expect("match");
    assert_eq!(before.keys_skipped, 0);
    store.recount_blocks().await.expect("recount");
    // Now the shared phone and family+year+district blocks (all four people) are skipped by their
    // counts; the keys with the given name still find Peter alone.
    let after = matcher.find(&query, 10, false).await.expect("match");
    assert_eq!(after.keys_skipped, 2, "{after:?}");
    assert_eq!(after.candidates.len(), 1);
    assert_eq!(after.candidates[0].resource["name"][0]["given"][0], "Peter");

    // With every key skipped, $match reports too little to match on.
    let sparse = json!({ "resourceType": "Patient", "telecom": [{ "system": "phone", "value": "+256772123456" }] });
    let nothing = matcher.find(&sparse, 10, false).await.expect("match");
    assert!(nothing.insufficient);
    assert_eq!(bundle(&nothing)["issue"][0]["code"], "incomplete");
}
