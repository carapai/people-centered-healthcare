//! Repository and EXPLAIN tests for search on SurrealDB 3.3.5 (in-memory engine), with R5 core
//! SearchParameters from `health-fhir-search`'s fixture.

use std::collections::BTreeSet;
use std::sync::Arc;

use health_fhir_search::{
    Catalog, Clause, Handling, ParamState, ParamType, Registry, SearchPolicy, SearchQuery,
    SearchValue, Test, Unrestricted, parse_query,
};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, Store, Transaction, WriteRequest};
use health_surrealdb::{
    Connection, MIGRATIONS, ReindexJob, SearchPage, Settings, SurrealStore, Target, migrate,
};
use health_time::{FixedClock, Timestamp};
use jiff::tz::Offset;
use serde_json::Value;

fn catalog() -> Catalog {
    let bundle: Value = serde_json::from_str(include_str!(
        "../../../contracts/health-fhir-search/tests/fixtures/search-parameters.json"
    ))
    .expect("fixture");
    let entries = bundle["entry"].as_array().expect("entries");
    Catalog::from_definitions(entries.iter().map(|e| &e["resource"]))
}

const PATIENT: &[&str] = &[
    "name",
    "family",
    "birthdate",
    "gender",
    "identifier",
    "general-practitioner",
];
const OBSERVATION: &[&str] = &[
    "code",
    "subject",
    "patient",
    "date",
    "value-quantity",
    "code-value-quantity",
    "status",
    "_lastUpdated",
];

fn registry(patient: &[&str], state: ParamState) -> Registry {
    let catalog = catalog();
    let mut registry = Registry::new(Offset::from_hours(3).expect("offset"));
    for code in patient {
        registry
            .support(&catalog, "Patient", code, state)
            .expect(code);
    }
    for code in OBSERVATION {
        registry
            .support(&catalog, "Observation", code, ParamState::Active)
            .expect(code);
    }
    registry
        .support(
            &catalog,
            "RiskAssessment",
            "probability",
            ParamState::Active,
        )
        .expect("probability");
    registry
}

async fn connection() -> Connection {
    let settings = Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    connection
}

fn store(connection: Connection, registry: Registry) -> SurrealStore {
    let at: Timestamp = "2026-10-29T08:00:00Z".parse().expect("timestamp");
    SurrealStore::new(connection, Arc::new(FixedClock::new(at))).with_registry(registry)
}

async fn put(store: &SurrealStore, resource_type: &str, id: ResourceId, json: &str) -> OpId {
    let mut tx = store.begin().await.expect("begin");
    let current = tx
        .read(&resource_type.parse().expect("type"), id)
        .await
        .expect("read")
        .map(|h| h.version.version_id);
    let op = OpId::new();
    tx.write(WriteRequest {
        resource_type: resource_type.parse().expect("type"),
        id,
        json: RawJson::from_validated(json),
        op_id: op,
        precondition: current.map_or(Precondition::MustNotExist, Precondition::IfMatch),
    })
    .await
    .expect("write");
    tx.commit().await.expect("commit");
    op
}

fn pairs(query: &[(&str, &str)]) -> Vec<(String, String)> {
    query
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn parse(store: &SurrealStore, resource_type: &str, query: &[(&str, &str)]) -> SearchQuery {
    parse_query(
        &store.registry(),
        resource_type,
        &pairs(query),
        Handling::Strict,
    )
    .expect("query")
}

async fn page(
    store: &SurrealStore,
    resource_type: &str,
    query: &[(&str, &str)],
    policy: &dyn SearchPolicy,
    after: Option<ResourceId>,
) -> SearchPage {
    let query = parse(store, resource_type, query);
    let mut tx = store.begin().await.expect("begin");
    tx.search(&query, policy, after).await.expect("search")
}

async fn ids(
    store: &SurrealStore,
    resource_type: &str,
    query: &[(&str, &str)],
) -> BTreeSet<ResourceId> {
    page(store, resource_type, query, &Unrestricted, None)
        .await
        .resources
        .into_iter()
        .map(|r| r.id)
        .collect()
}

fn set(ids: &[ResourceId]) -> BTreeSet<ResourceId> {
    ids.iter().copied().collect()
}

struct Fixture {
    store: SurrealStore,
    patients: [ResourceId; 3],
    observations: [ResourceId; 4],
}

async fn fixture() -> Fixture {
    let store = store(connection().await, registry(PATIENT, ParamState::Active));
    let patients = [ResourceId::new(), ResourceId::new(), ResourceId::new()];
    let [akello, okello, nandu] = patients;
    put(&store, "Patient", akello, r#"{"resourceType":"Patient","name":[{"family":"Akello","given":["Grace"]}],"gender":"female","birthDate":"1990-05-17","identifier":[{"system":"urn:ug:nin","value":"CF900"}],"generalPractitioner":[{"reference":"Practitioner/9"}]}"#).await;
    put(&store, "Patient", okello, r#"{"resourceType":"Patient","name":[{"family":"Okello","given":["John"]}],"gender":"male","birthDate":"1985","identifier":[{"system":"urn:ug:nin","value":"CM800"}]}"#).await;
    put(
        &store,
        "Patient",
        nandu,
        r#"{"resourceType":"Patient","name":[{"family":"Ñandú"}],"gender":"female"}"#,
    )
    .await;

    let observations = [
        ResourceId::new(),
        ResourceId::new(),
        ResourceId::new(),
        ResourceId::new(),
    ];
    let systolic = |id: ResourceId, patient: ResourceId, value: &str, when: &str, status: &str| {
        format!(
            r#"{{"resourceType":"Observation","id":"{id}","status":"{status}","code":{{"coding":[{{"system":"http://loinc.org","code":"8480-6"}}]}},"subject":{{"reference":"Patient/{patient}"}},"effectiveDateTime":"{when}","valueQuantity":{{"value":{value},"system":"http://unitsofmeasure.org","code":"mm[Hg]"}}}}"#
        )
    };
    let [o1, o2, o3, o4] = observations;
    put(
        &store,
        "Observation",
        o1,
        &systolic(o1, akello, "120.50", "2026-10-01T08:00:00+03:00", "final"),
    )
    .await;
    put(
        &store,
        "Observation",
        o2,
        &systolic(o2, akello, "145", "2026-10-15", "final"),
    )
    .await;
    put(
        &store,
        "Observation",
        o3,
        &systolic(o3, okello, "99.9", "2026-09-30T23:30:00Z", "amended"),
    )
    .await;
    put(
        &store,
        "Observation",
        o4,
        r#"{"resourceType":"Observation","status":"cancelled","code":{"coding":[{"system":"http://snomed.info/sct","code":"8480-6"}]},"subject":{"reference":"Group/g1"},"effectivePeriod":{"start":"2026-10-20"},"valueQuantity":{"value":5.4,"unit":"mmol/L"}}"#,
    )
    .await;
    Fixture {
        store,
        patients,
        observations,
    }
}

#[tokio::test]
async fn each_search_type_and_modifier_finds_the_right_resources() {
    let Fixture {
        store,
        patients: [akello, okello, nandu],
        observations: [o1, o2, o3, o4],
    } = fixture().await;
    let subject = format!("Patient/{akello}");
    let okello_id = okello.to_string();
    type Case<'a> = (&'a str, Vec<(&'a str, &'a str)>, BTreeSet<ResourceId>);
    let cases: Vec<Case<'_>> = vec![
        // string
        ("Patient", vec![("name", "akel")], set(&[akello])),
        ("Patient", vec![("name", "kello")], set(&[])),
        (
            "Patient",
            vec![("name:contains", "kello")],
            set(&[akello, okello]),
        ),
        ("Patient", vec![("family", "NANDU")], set(&[nandu])),
        ("Patient", vec![("family:exact", "Ñandú")], set(&[nandu])),
        ("Patient", vec![("family:exact", "nandu")], set(&[])),
        // token
        ("Patient", vec![("gender", "female")], set(&[akello, nandu])),
        (
            "Patient",
            vec![("identifier", "urn:ug:nin|CM800")],
            set(&[okello]),
        ),
        (
            "Patient",
            vec![("identifier", "urn:ug:nin|")],
            set(&[akello, okello]),
        ),
        ("Patient", vec![("identifier", "|CM800")], set(&[])),
        (
            "Observation",
            vec![("code", "8480-6")],
            set(&[o1, o2, o3, o4]),
        ),
        (
            "Observation",
            vec![("code", "http://loinc.org|8480-6")],
            set(&[o1, o2, o3]),
        ),
        ("Observation", vec![("status:not", "final")], set(&[o3, o4])),
        (
            "Observation",
            vec![("status", "amended,cancelled")],
            set(&[o3, o4]),
        ),
        // date (Uganda offset for values without one)
        ("Patient", vec![("birthdate", "1990")], set(&[akello])),
        ("Patient", vec![("birthdate", "lt1990")], set(&[okello])),
        (
            "Patient",
            vec![("birthdate:missing", "true")],
            set(&[nandu]),
        ),
        ("Observation", vec![("date", "2026-10-01")], set(&[o1, o3])),
        ("Observation", vec![("date", "gt2026-10-15")], set(&[o4])),
        (
            "Observation",
            vec![("date", "ge2026-10-15")],
            set(&[o2, o4]),
        ),
        (
            "Observation",
            vec![("date", "le2026-10-01")],
            set(&[o1, o3]),
        ),
        (
            "Observation",
            vec![("date", "sa2026-10-01")],
            set(&[o2, o4]),
        ),
        (
            "Observation",
            vec![("date", "eb2026-10-15")],
            set(&[o1, o3]),
        ),
        (
            "Observation",
            vec![("date", "ne2026-10-15")],
            set(&[o1, o3, o4]),
        ),
        ("Observation", vec![("date", "ap2026-10-20")], set(&[o4])),
        // quantity
        (
            "Observation",
            vec![("value-quantity", "120.5|http://unitsofmeasure.org|mm[Hg]")],
            set(&[o1]),
        ),
        (
            "Observation",
            vec![("value-quantity", "gt100|http://unitsofmeasure.org|mm[Hg]")],
            set(&[o1, o2]),
        ),
        (
            "Observation",
            vec![("value-quantity", "lt100")],
            set(&[o3, o4]),
        ),
        (
            "Observation",
            vec![("value-quantity", "5.4||mmol/L")],
            set(&[o4]),
        ),
        (
            "Observation",
            vec![("value-quantity", "ne145|http://unitsofmeasure.org|mm[Hg]")],
            set(&[o1, o3]),
        ),
        // reference
        ("Observation", vec![("subject", &subject)], set(&[o1, o2])),
        ("Observation", vec![("patient", &okello_id)], set(&[o3])),
        ("Observation", vec![("subject", "g1")], set(&[o4])),
        (
            "Patient",
            vec![("general-practitioner", "Practitioner/9")],
            set(&[akello]),
        ),
        // composite
        (
            "Observation",
            vec![(
                "code-value-quantity",
                "http://loinc.org|8480-6$gt140|http://unitsofmeasure.org|mm[Hg]",
            )],
            set(&[o2]),
        ),
        (
            "Observation",
            vec![("code-value-quantity", "http://loinc.org|8480-6$lt130")],
            set(&[o1, o3]),
        ),
        // clauses intersect
        (
            "Observation",
            vec![("subject", &subject), ("date", "lt2026-10-10")],
            set(&[o1]),
        ),
        (
            "Observation",
            vec![("patient:missing", "true"), ("code", "8480-6")],
            set(&[o4]),
        ),
    ];
    for (resource_type, query, expected) in cases {
        assert_eq!(
            ids(&store, resource_type, &query).await,
            expected,
            "{resource_type}?{query:?}"
        );
    }
}

#[tokio::test]
async fn updates_and_deletes_replace_the_entries() {
    let Fixture {
        store,
        patients: [akello, ..],
        ..
    } = fixture().await;
    assert_eq!(
        ids(&store, "Patient", &[("family", "akello")]).await,
        set(&[akello])
    );
    put(
        &store,
        "Patient",
        akello,
        r#"{"resourceType":"Patient","name":[{"family":"Atim"}],"gender":"female"}"#,
    )
    .await;
    assert_eq!(
        ids(&store, "Patient", &[("family", "akello")]).await,
        set(&[])
    );
    assert_eq!(
        ids(&store, "Patient", &[("family", "atim")]).await,
        set(&[akello])
    );
    assert_eq!(
        ids(&store, "Patient", &[("birthdate:missing", "true")])
            .await
            .len(),
        2
    );

    let mut tx = store.begin().await.expect("begin");
    let head = tx
        .read(&"Patient".parse().expect("type"), akello)
        .await
        .expect("read")
        .expect("head");
    tx.delete(
        &"Patient".parse().expect("type"),
        akello,
        OpId::new(),
        Some(head.version.version_id),
    )
    .await
    .expect("delete");
    tx.commit().await.expect("commit");
    assert_eq!(
        ids(&store, "Patient", &[("family", "atim")]).await,
        set(&[])
    );
    assert_eq!(
        ids(&store, "Patient", &[("gender", "female")]).await.len(),
        1
    );
}

#[tokio::test]
async fn pages_follow_the_cursor_and_report_totals() {
    let Fixture {
        store,
        observations,
        ..
    } = fixture().await;
    let mut seen = Vec::new();
    let mut after = None;
    loop {
        let result = page(
            &store,
            "Observation",
            &[("code", "8480-6"), ("_count", "3")],
            &Unrestricted,
            after,
        )
        .await;
        assert_eq!(result.total, Some(4));
        seen.extend(result.resources.iter().map(|r| r.id));
        match result.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    assert_eq!(seen.len(), 4);
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "id order");
    assert_eq!(set(&seen), set(&observations));

    let listing = page(
        &store,
        "Observation",
        &[("_count", "2")],
        &Unrestricted,
        None,
    )
    .await;
    assert_eq!((listing.resources.len(), listing.total), (2, None));
    let rest = page(
        &store,
        "Observation",
        &[("_count", "2")],
        &Unrestricted,
        listing.next,
    )
    .await;
    assert_eq!((rest.resources.len(), rest.next), (2, None));
}

/// A policy restricting every Observation search to one patient's compartment.
struct OnlyPatient(ResourceId);

impl SearchPolicy for OnlyPatient {
    fn clauses(&self, resource_type: &str) -> Vec<Clause> {
        if resource_type != "Observation" {
            return Vec::new();
        }
        vec![Clause {
            param: "patient".into(),
            param_type: ParamType::Reference,
            test: Test::Any(vec![SearchValue::Reference {
                target_type: Some("Patient".into()),
                target_id: self.0.to_string(),
            }]),
        }]
    }
}

#[tokio::test]
async fn policy_clauses_restrict_results_and_totals() {
    let Fixture {
        store,
        patients: [akello, ..],
        observations: [o1, o2, ..],
    } = fixture().await;
    let policy = OnlyPatient(akello);
    let result = page(&store, "Observation", &[("code", "8480-6")], &policy, None).await;
    assert_eq!(
        result.total,
        Some(2),
        "the total never counts hidden resources"
    );
    assert_eq!(
        result
            .resources
            .iter()
            .map(|r| r.id)
            .collect::<BTreeSet<_>>(),
        set(&[o1, o2])
    );
    let unfiltered = page(&store, "Observation", &[], &policy, None).await;
    assert_eq!(
        unfiltered.total,
        Some(2),
        "the policy applies to the unfiltered listing too"
    );
}

#[tokio::test]
async fn explain_shows_no_full_scan_for_any_query_shape() {
    let Fixture {
        store,
        patients: [akello, ..],
        ..
    } = fixture().await;
    let subject = format!("Patient/{akello}");
    let shapes: Vec<Vec<(&str, &str)>> = vec![
        vec![("name", "akel")],
        vec![("name:exact", "Akello")],
        vec![("name:contains", "kel")],
        vec![("gender", "female")],
        vec![("identifier", "urn:ug:nin|CM800")],
        vec![("identifier", "urn:ug:nin|")],
        vec![("birthdate", "1990")],
        vec![("birthdate", "gt1990")],
        vec![("birthdate", "lt1990")],
        vec![("birthdate", "eb1990")],
        vec![("birthdate", "ne1990")],
        vec![("birthdate", "ap1990")],
        vec![("birthdate:missing", "true")],
        vec![("gender:not", "male")],
        vec![("general-practitioner", "9")],
    ];
    let observation_shapes: Vec<Vec<(&str, &str)>> = vec![
        vec![("code", "8480-6")],
        vec![("code", "http://loinc.org|8480-6")],
        vec![("subject", &subject)],
        vec![("value-quantity", "gt100|http://unitsofmeasure.org|mm[Hg]")],
        vec![("value-quantity", "5.4||mmol/L")],
        vec![("value-quantity", "lt100")],
        vec![("value-quantity", "ne145|http://unitsofmeasure.org|mm[Hg]")],
        vec![(
            "code-value-quantity",
            "http://loinc.org|8480-6$gt140|http://unitsofmeasure.org|mm[Hg]",
        )],
        vec![("code-value-quantity", "http://loinc.org|8480-6$lt130")],
        vec![("_lastUpdated", "ge2026")],
    ];
    let mut tx = store.begin().await.expect("begin");
    let mut checked = 0;
    for (resource_type, shapes) in [("Patient", shapes), ("Observation", observation_shapes)] {
        for shape in shapes {
            let query = parse(&store, resource_type, &shape);
            let plans = tx.explain(&query, &Unrestricted).await.expect("explain");
            assert!(!plans.is_empty(), "{shape:?}");
            for plan in plans {
                assert!(
                    !plan.contains("TableScan"),
                    "{resource_type}?{shape:?} scans a table: {plan}"
                );
                assert!(
                    plan.contains("RecordIdScan") || plan.contains("IndexScan"),
                    "{resource_type}?{shape:?}: {plan}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 25, "{checked}");
}

#[tokio::test]
async fn unsupported_composites_are_refused() {
    let Fixture { store, .. } = fixture().await;
    let query = parse(
        &store,
        "Observation",
        &[("code-value-quantity", "8480-6$gt140")],
    );
    let mut tx = store.begin().await.expect("begin");
    assert!(matches!(
        tx.search(&query, &Unrestricted, None).await,
        Err(health_surrealdb::SearchFailure::Unsupported(_))
    ));
}

#[tokio::test]
async fn reindexing_is_resumable_and_gates_the_parameter() {
    let connection = connection().await;
    let without_gender: Vec<&str> = PATIENT.iter().copied().filter(|c| *c != "gender").collect();
    let before = store(
        connection.clone(),
        registry(&without_gender, ParamState::Active),
    );
    let mut female = Vec::new();
    for i in 0..5 {
        let id = ResourceId::new();
        let gender = if i % 2 == 0 { "female" } else { "male" };
        if gender == "female" {
            female.push(id);
        }
        put(
            &before,
            "Patient",
            id,
            &format!(r#"{{"resourceType":"Patient","gender":"{gender}"}}"#),
        )
        .await;
    }

    // Activation: gender is indexed on new writes while the job backfills.
    let mut reindexing = registry(&without_gender, ParamState::Active);
    reindexing
        .support(&catalog(), "Patient", "gender", ParamState::Reindexing)
        .expect("gender");
    let during = store(connection.clone(), reindexing.clone());
    assert_eq!(
        parse_query(
            &during.registry(),
            "Patient",
            &pairs(&[("gender", "female")]),
            Handling::Strict
        ),
        Err(health_fhir_search::SearchError::NotReady("gender".into()))
    );
    let written_during = ResourceId::new();
    put(
        &during,
        "Patient",
        written_during,
        r#"{"resourceType":"Patient","gender":"female"}"#,
    )
    .await;
    female.push(written_during);

    let job = ReindexJob {
        id: "activation-1".into(),
        resource_type: "Patient".parse().expect("type"),
        params: vec!["gender".into()],
    };
    during.start_reindex(&job).await.expect("start");
    let first = during.reindex_batch(&job.id, 2).await.expect("batch");
    assert_eq!((first.processed, first.done), (2, false));
    drop(during);

    // A restart: a new store, the same job, starting again is a no-op.
    let restarted = store(connection.clone(), reindexing);
    restarted.start_reindex(&job).await.expect("restart");
    let mut progress = restarted.reindex_batch(&job.id, 2).await.expect("batch");
    assert_eq!(progress.processed, 4, "resumed after the first batch");
    while !progress.done {
        progress = restarted.reindex_batch(&job.id, 2).await.expect("batch");
    }
    assert_eq!(progress.processed, 6);

    let mut active = restarted.registry().as_ref().clone();
    assert!(active.activate("Patient", "gender"));
    restarted.set_registry(active);
    assert_eq!(
        ids(&restarted, "Patient", &[("gender", "female")]).await,
        set(&female)
    );
    assert_eq!(
        ids(&restarted, "Patient", &[("gender", "male")])
            .await
            .len(),
        2
    );
    assert_eq!(
        ids(&restarted, "Patient", &[("gender:missing", "true")])
            .await
            .len(),
        0
    );
}
