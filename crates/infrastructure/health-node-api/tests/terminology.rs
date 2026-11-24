//! The terminology a node serves (#843), on the in-memory engine: the active packages'
//! CodeSystems and ValueSets, read from their tarballs, answer `ValueSet/$expand` for a form's
//! bound question with no central to reach.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use health_fhir_search::Unrestricted;
use health_fhir_server::FhirServer;
use health_files::FsBlobStore;
use health_node_api::ActiveTerminology;
use health_node_api::terminology::reload;
use health_persistence::{Store as _, Transaction as _};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const COMPLAINTS: &str = "https://health.go.ug/fhir/ValueSet/presenting-complaint";

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

/// A package whose form binds its choice question to a ValueSet of the package's own
/// CodeSystem, one of whose codes is retired.
fn package() -> health_packages::Files {
    let code = |code: &str, display: &str| json!({ "code": code, "display": display });
    let resources = [
        (
            "package.json",
            json!({
                "name": "ug.moh.complaints", "version": "1.0.0", "fhirVersions": ["5.0.0"],
                "kind": "program", "platform": { "minSoftware": "0.1.0", "formatVersion": 1 }
            }),
        ),
        (
            "Library-manifest.json",
            json!({
                "resourceType": "Library", "id": "manifest", "status": "active",
                "url": "https://health.go.ug/fhir/Library/ug.moh.complaints-manifest",
                "version": "1.0.0", "type": { "text": "asset-collection" }
            }),
        ),
        (
            "CodeSystem-presenting-complaint.json",
            json!({
                "resourceType": "CodeSystem", "id": "presenting-complaint",
                "url": "https://health.go.ug/fhir/CodeSystem/presenting-complaint",
                "version": "1.0.0", "status": "active", "content": "complete",
                "concept": [
                    code("fever", "Fever"),
                    code("cough", "Cough"),
                    code("diarrhoea", "Diarrhoea"),
                    {
                        "code": "malaise", "display": "Malaise",
                        "property": [{ "code": "status", "valueCode": "retired" }]
                    }
                ]
            }),
        ),
        (
            "ValueSet-presenting-complaint.json",
            json!({
                "resourceType": "ValueSet", "id": "presenting-complaint",
                "url": COMPLAINTS, "version": "1.0.0", "status": "active",
                "compose": { "include": [{
                    "system": "https://health.go.ug/fhir/CodeSystem/presenting-complaint"
                }] }
            }),
        ),
        (
            "Questionnaire-complaint.json",
            json!({
                "resourceType": "Questionnaire", "id": "complaint",
                "url": "https://health.go.ug/fhir/Questionnaire/complaint",
                "version": "1.0.0", "status": "active", "title": "Presenting complaint",
                "item": [{
                    "linkId": "complaint", "text": "Main complaint", "type": "coding",
                    "answerValueSet": format!("{COMPLAINTS}|1.0.0")
                }]
            }),
        ),
    ];
    resources
        .into_iter()
        .map(|(path, value)| (path.to_owned(), value.to_string().into_bytes()))
        .collect()
}

/// Stages `files` (its tarball in `blobs`) and makes it the node's active set, as central's
/// activation leaves an edge.
async fn activate(store: &SurrealStore, blobs: &FsBlobStore, files: &health_packages::Files) {
    let packed = health_packages::pack(files).expect("pack");
    blobs.put(packed.tgz.as_slice()).expect("blob");
    let mut tx = store.begin().await.expect("begin");
    tx.put_staged_package(&packed.id, &packed.digest.to_string(), "{}")
        .await
        .expect("staged");
    let (name, version) = packed.id.rsplit_once('@').expect("id");
    tx.set_active_packages(&json!({ name: version }).to_string())
        .await
        .expect("active");
    tx.commit().await.expect("commit");
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// #843: a facility fills a form with a bound question offline. The edge serves the active
/// packages' terminology, so its FHIR base expands the bound ValueSet (retired codes left out,
/// as forms ask), and a set read once is not read again until it changes.
#[tokio::test]
async fn an_edge_expands_a_bound_value_set_from_its_active_packages_offline() {
    let store = store().await;
    let dir = std::env::temp_dir().join(format!("edge-terminology-{}", uuid::Uuid::new_v4()));
    let blobs = FsBlobStore::open(&dir).expect("blob store");
    activate(&store, &blobs, &package()).await;

    let served = ActiveTerminology::default();
    let app = FhirServer::new(
        store.clone(),
        "http://edge/api/fhir",
        Arc::new(Unrestricted),
    )
    .with_terminology(served.clone())
    .router();
    let ask = format!("/ValueSet/$expand?url={COMPLAINTS}&valueSetVersion=1.0.0&activeOnly=true");
    let (status, _) = get(&app, &ask).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "nothing is served before");

    let mut last = None;
    let read = reload(&served, &store, &blobs, &mut last).await;
    let again = reload(&served, &store, &blobs, &mut last).await;
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(read, Ok(true));
    assert_eq!(again, Ok(false), "the same set is not read again");

    let (status, expanded) = get(&app, &ask).await;
    assert_eq!(status, StatusCode::OK, "{expanded}");
    assert_eq!(expanded["resourceType"], "ValueSet");
    let codes: Vec<(&str, &str)> = expanded["expansion"]["contains"]
        .as_array()
        .expect("contains")
        .iter()
        .map(|c| {
            (
                c["code"].as_str().unwrap_or_default(),
                c["display"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        codes,
        [
            ("cough", "Cough"),
            ("diarrhoea", "Diarrhoea"),
            ("fever", "Fever")
        ],
        "the retired code is not offered"
    );
}
