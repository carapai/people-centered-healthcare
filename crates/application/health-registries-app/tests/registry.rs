//! The client registry over SurrealDB (memory engine): links, survivorship, unlink and merge,
//! and clinical references that stay valid through all of them.

use std::sync::{Arc, Mutex};

use health_client_registry::{Field, LinkRefused, MergeRefused};
use health_fhir_search::Registry;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, Transaction, WriteError, WriteRequest,
};
use health_registries_app::fhir::{ECID_SYSTEM, RegistryConfig, linked};
use health_registries_app::{AuditTrail, ClientRegistry, RegistryError};
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealStore, SurrealTx, Target, migrate,
};
use health_time::SystemClock;
use serde_json::{Value, json};

const NIN: &str = "urn:health:synthetic-data/nin";

#[derive(Default, Clone)]
struct Recorded(Arc<Mutex<Vec<(String, String, String)>>>);

impl AuditTrail<SurrealTx> for Recorded {
    async fn written(
        &self,
        _tx: &mut SurrealTx,
        actor: &str,
        reason: &str,
        json: &RawJson,
    ) -> Result<(), WriteError> {
        let id = serde_json::from_str::<Value>(json.as_str())
            .ok()
            .and_then(|v| v["id"].as_str().map(str::to_owned))
            .unwrap_or_default();
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((actor.into(), reason.into(), id));
        Ok(())
    }

    async fn recorded(
        &self,
        _tx: &mut SurrealTx,
        actor: &str,
        reason: &str,
        what: &str,
    ) -> Result<(), WriteError> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((actor.into(), reason.into(), what.into()));
        Ok(())
    }
}

async fn store() -> SurrealStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "registry".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(Registry::new(jiff::tz::Offset::UTC))
}

async fn create(store: &SurrealStore, resource: Value) -> ResourceId {
    let id = ResourceId::new();
    let resource_type: ResourceType = resource["resourceType"]
        .as_str()
        .expect("type")
        .parse()
        .expect("type");
    let mut tx = store.begin().await.expect("tx");
    tx.write(WriteRequest {
        resource_type,
        id,
        json: RawJson::from_validated(resource.to_string()),
        op_id: OpId::new(),
        precondition: Precondition::MustNotExist,
    })
    .await
    .expect("write");
    tx.commit().await.expect("commit");
    id
}

async fn read(store: &SurrealStore, resource_type: &str, id: ResourceId) -> Value {
    let mut tx = store.begin().await.expect("tx");
    let stored = tx
        .read(&resource_type.parse().expect("type"), id)
        .await
        .expect("read")
        .expect("exists");
    serde_json::from_str(stored.json.as_str()).expect("json")
}

fn patient(
    facility: &str,
    trust: &str,
    family: &str,
    nin: Option<(&str, bool)>,
    provisional: bool,
) -> Value {
    let mut tags = vec![json!({ "system": "urn:health:registry:trust", "code": trust })];
    if provisional {
        tags.push(json!({ "system": "urn:health:registry", "code": "provisional" }));
    }
    let mut p = json!({
        "resourceType": "Patient",
        "meta": { "tag": tags },
        "name": [{ "family": family, "given": ["Grace"] }],
        "gender": "female",
        "managingOrganization": { "reference": format!("Organization/{facility}") },
    });
    if let Some((value, verified)) = nin {
        let mut identifier = json!({ "system": NIN, "value": value });
        if verified {
            identifier["extension"] = json!([{ "url": "urn:health:registry:verification", "valueCode": "nira-verified" }]);
        }
        p["identifier"] = json!([identifier]);
    }
    p
}

fn registry(store: &SurrealStore, audit: &Recorded) -> ClientRegistry<SurrealStore, Recorded> {
    ClientRegistry::new(
        store.clone(),
        audit.clone(),
        RegistryConfig {
            nin_system: NIN.into(),
        },
    )
}

fn tier1(json: &Value) {
    let issues = health_fhir_validation::validate_structure(&json.to_string());
    assert!(issues.is_empty(), "{issues:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn links_build_one_golden_record_and_lower_trust_never_overwrites() {
    let store = store().await;
    let audit = Recorded::default();
    let registry = registry(&store, &audit);
    // Facility A: staff-entered, NIN verified with NIRA. Facility B, offline: provisional.
    let a = create(
        &store,
        patient(
            "A",
            "staff-entered",
            "Okello",
            Some(("CM8601234567AB", true)),
            false,
        ),
    )
    .await;
    let b = create(
        &store,
        patient(
            "B",
            "staff-verified",
            "Akello",
            Some(("CM8601234567XX", false)),
            true,
        ),
    )
    .await;
    // A self-registration that disagrees on the family name.
    let s = create(&store, patient("A", "self-reported", "Okelo", None, false)).await;

    let first = registry.link(a, None, "matching").await.expect("link a");
    assert!(first.created);
    let second = registry
        .link(b, Some(first.golden), "reviewer-1")
        .await
        .expect("link b");
    assert!(!second.created && second.golden == first.golden);
    let third = registry
        .link(s, Some(first.golden), "reviewer-1")
        .await
        .expect("link s");

    let golden = read(&store, "Patient", first.golden).await;
    tier1(&golden);
    assert_eq!(golden["identifier"][0]["system"], ECID_SYSTEM);
    assert_eq!(golden["identifier"][0]["value"], first.ecid.as_str());
    assert_eq!(linked(&golden, "seealso").len(), 3);
    // Staff-verified (B) beats staff-entered (A) for the name...
    assert_eq!(golden["name"][0]["family"], "Akello");
    // ...the NIRA-verified NIN survives B's unverified one, and goes to review instead.
    assert_eq!(golden["identifier"][1]["value"], "CM8601234567AB");
    assert_eq!(
        second.review.iter().map(|r| r.field).collect::<Vec<_>>(),
        vec![Field::Nin]
    );
    // A self-reported name never overwrites the staff-verified one.
    assert_eq!(
        third
            .review
            .iter()
            .map(|r| (r.field, r.incoming.value.clone()))
            .collect::<Vec<_>>(),
        vec![(Field::Family, "Okelo".to_owned())]
    );

    // Each source now refers to the golden record, and B is no longer provisional.
    for source in [a, b, s] {
        let json = read(&store, "Patient", source).await;
        assert_eq!(linked(&json, "refer"), vec![first.golden.to_string()]);
    }
    let b_json = read(&store, "Patient", b).await;
    assert!(
        b_json["meta"]["tag"]
            .as_array()
            .expect("tags")
            .iter()
            .all(|t| t["code"] != "provisional")
    );
    assert_eq!(
        registry.link(a, Some(first.golden), "x").await,
        Err(RegistryError::AlreadyLinked {
            source_id: a.to_string(),
            golden: first.golden.to_string()
        })
    );
    // Every version written was audited, with who and why: six Patient versions, and the
    // Provenance of each surviving value that changed.
    let provenance = provenance(&store).await;
    let audited = audit.0.lock().expect("audit").clone();
    assert_eq!(audited.len(), 6 + provenance.len());
    assert!(audited.iter().all(|(_, reason, _)| reason == "link"));

    // Each surviving value names its source Patient and trust, against a golden version (#196).
    let latest = |field: &str| {
        provenance
            .iter()
            .filter(|p| p["activity"]["coding"][0]["code"] == field)
            .max_by_key(|p| p["recorded"].as_str().map(str::to_owned))
            .cloned()
            .expect("a Provenance for the field")
    };
    let family = latest("family");
    assert_eq!(
        family["entity"][0]["what"]["reference"],
        format!("Patient/{b}")
    );
    assert_eq!(
        family["entity"][0]["extension"][0]["valueCode"],
        "staff-verified"
    );
    let nin = latest("nin");
    assert_eq!(
        nin["entity"][0]["what"]["reference"],
        format!("Patient/{a}")
    );
    assert_eq!(
        nin["entity"][0]["extension"][0]["valueCode"],
        "nira-verified"
    );
    for p in &provenance {
        tier1(p);
        assert!(
            p["target"][0]["reference"]
                .as_str()
                .expect("target")
                .starts_with(&format!("Patient/{}/_history/", first.golden))
        );
    }
}

/// Every Provenance in the store, all owned by the client registry.
async fn provenance(store: &SurrealStore) -> Vec<Value> {
    let mut tx = store.begin().await.expect("tx");
    let ids = tx
        .provenance_of_owner("client-registry")
        .await
        .expect("ids");
    let mut out = Vec::new();
    for id in ids {
        let stored = tx
            .read(
                &"Provenance".parse().expect("type"),
                id.parse().expect("id"),
            )
            .await
            .expect("read")
            .expect("exists");
        out.push(serde_json::from_str(stored.json.as_str()).expect("json"));
    }
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unlinking_splits_and_the_pair_is_never_relinked() {
    let store = store().await;
    let audit = Recorded::default();
    let registry = registry(&store, &audit);
    let a = create(&store, patient("A", "staff-entered", "Okello", None, false)).await;
    let b = create(
        &store,
        patient("B", "staff-verified", "Akello", None, false),
    )
    .await;
    let golden = registry.link(a, None, "matching").await.expect("a").golden;
    registry.link(b, Some(golden), "matching").await.expect("b");

    assert_eq!(registry.unlink(b, "reviewer-2").await, Ok(golden));
    let json = read(&store, "Patient", golden).await;
    tier1(&json);
    assert_eq!(linked(&json, "seealso"), vec![a.to_string()]);
    assert_eq!(
        json["name"][0]["family"], "Okello",
        "B's values left with it"
    );
    assert!(linked(&read(&store, "Patient", b).await, "refer").is_empty());
    assert_eq!(
        registry.link(b, Some(golden), "matching").await,
        Err(RegistryError::Link(LinkRefused::Adjudicated))
    );
    // B can still get a golden record of its own.
    assert!(
        registry
            .link(b, None, "matching")
            .await
            .expect("new")
            .created
    );
    // The last source leaving makes the golden record inactive.
    registry.unlink(a, "reviewer-2").await.expect("unlink a");
    assert_eq!(read(&store, "Patient", golden).await["active"], false);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn merging_keeps_every_clinical_reference_valid() {
    let store = store().await;
    let audit = Recorded::default();
    let registry = registry(&store, &audit);
    let duplicate = create(&store, patient("A", "staff-entered", "Okello", None, false)).await;
    let keeper = create(&store, patient("A", "staff-entered", "Okello", None, false)).await;
    let elsewhere = create(&store, patient("B", "staff-entered", "Okello", None, false)).await;
    let observation = create(&store, json!({ "resourceType": "Observation", "status": "final", "code": { "text": "Hb" }, "subject": { "reference": format!("Patient/{duplicate}") } })).await;
    let before = read(&store, "Observation", observation).await["meta"]["versionId"].clone();
    let golden = registry
        .link(duplicate, None, "matching")
        .await
        .expect("link")
        .golden;

    // Across facilities: link, never merge. Without approval: nothing happens.
    assert_eq!(
        registry.merge_preview(duplicate, elsewhere).await,
        Err(RegistryError::Merge(MergeRefused::DifferentFacility))
    );
    let plan = registry
        .merge_preview(duplicate, keeper)
        .await
        .expect("preview");
    assert_eq!(
        (
            plan.source.as_str(),
            plan.target.as_str(),
            plan.facility.as_str()
        ),
        (
            duplicate.to_string().as_str(),
            keeper.to_string().as_str(),
            "Organization/A"
        )
    );
    assert_eq!(
        registry.merge(&plan).await,
        Err(RegistryError::Merge(MergeRefused::NotApproved))
    );
    assert_eq!(
        read(&store, "Patient", duplicate).await["active"],
        Value::Null
    );

    registry
        .merge(&plan.clone().approve("reviewer-3"))
        .await
        .expect("merge");
    let retired = read(&store, "Patient", duplicate).await;
    let survivor = read(&store, "Patient", keeper).await;
    tier1(&retired);
    tier1(&survivor);
    assert_eq!(retired["active"], false);
    assert_eq!(linked(&retired, "replaced-by"), vec![keeper.to_string()]);
    assert_eq!(linked(&survivor, "replaces"), vec![duplicate.to_string()]);
    // The duplicate keeps its id and its link to the golden record; the Observation still
    // points at it, untouched.
    assert_eq!(linked(&retired, "refer"), vec![golden.to_string()]);
    let obs = read(&store, "Observation", observation).await;
    assert_eq!(obs["subject"]["reference"], format!("Patient/{duplicate}"));
    assert_eq!(
        obs["meta"]["versionId"], before,
        "the Observation was never rewritten"
    );
    // Merged away: it cannot be merged or linked again.
    assert_eq!(
        registry.merge(&plan.approve("reviewer-3")).await,
        Err(RegistryError::Merge(MergeRefused::Inactive))
    );
}
