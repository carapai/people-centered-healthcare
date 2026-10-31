//! The generated output is reproducible and loads through the FHIR API: a real `FhirServer` on
//! the SurrealDB memory engine, tiers 1 and 2 validating every resource.

use std::sync::Arc;

use axum::Router;
use health_fhir_search::{Registry, Unrestricted};
use health_fhir_server::FhirServer;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use synthetic_data::load::{Target as LoadTarget, load};
use synthetic_data::output::{generate, read_manifest};

async fn server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let base = format!("http://{}/fhir", listener.local_addr().expect("address"));
    let settings = Settings {
        target: Target::Memory,
        namespace: "synthetic".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(Registry::new(jiff::tz::Offset::UTC));
    let router = Router::new().nest(
        "/fhir",
        FhirServer::new(store, base.as_str(), Arc::new(Unrestricted)).router(),
    );
    tokio::spawn(async move { axum::serve(listener, router).await });
    base
}

#[test]
fn the_same_seed_gives_the_same_bytes() {
    let (a, b, c) = (
        tempfile::tempdir().expect("tmp"),
        tempfile::tempdir().expect("tmp"),
        tempfile::tempdir().expect("tmp"),
    );
    let first = generate(5, 2_000, 2, a.path()).expect("generate");
    assert_eq!(first, generate(5, 2_000, 2, b.path()).expect("generate"));
    assert_eq!(first, read_manifest(a.path()).expect("manifest"));
    let other = generate(6, 2_000, 2, c.path()).expect("generate");
    let patients = |m: &synthetic_data::output::Manifest| m.files.last().map(|f| f.sha256.clone());
    assert_ne!(patients(&first), patients(&other));
    assert_eq!(first.files.last().map(|f| f.count), Some(2_000));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_output_loads_through_the_fhir_api_and_never_twice() {
    let dir = tempfile::tempdir().expect("tmp");
    let manifest = generate(3, 300, 2, dir.path()).expect("generate");
    let base = server().await;
    let target = LoadTarget {
        base: base.clone(),
        bearer: None,
        batch: 50,
        concurrency: 4,
    };

    let loaded = load(dir.path(), &target).await.expect("load");
    assert_eq!(
        loaded
            .iter()
            .map(|l| (l.path.clone(), l.count))
            .collect::<Vec<_>>(),
        manifest
            .files
            .iter()
            .map(|f| (f.path.clone(), f.count))
            .collect::<Vec<_>>()
    );
    // The ids are deterministic, so a second load cannot duplicate anything: the server refuses
    // the updates, which need If-Match.
    let again = load(dir.path(), &target)
        .await
        .expect_err("a second load is refused");
    assert!(again.to_string().contains("428"), "{again}");

    let line = std::fs::read_to_string(dir.path().join("Patient.ndjson")).expect("patients");
    let first: serde_json::Value =
        serde_json::from_str(line.lines().next().expect("a patient")).expect("json");
    let id = first["id"].as_str().expect("id");
    let read: serde_json::Value = reqwest::get(format!("{base}/Patient/{id}"))
        .await
        .expect("read")
        .json()
        .await
        .expect("json");
    assert_eq!(read["name"], first["name"]);
    assert_eq!(
        read["meta"]["versionId"].as_str().map(str::is_empty),
        Some(false)
    );
    let history: serde_json::Value = reqwest::get(format!("{base}/Patient/{id}/_history"))
        .await
        .expect("history")
        .json()
        .await
        .expect("json");
    assert_eq!(history["entry"].as_array().map(Vec::len), Some(1));
}
