//! Central writes the active packages' definitions into its store (#89), each version once,
//! under an id derived from its `url|version`.

use std::sync::Arc;

use health_activation::definitions::{definition_id, definitions, write_definitions};
use health_configuration::activation::ActiveSet;
use health_persistence::{Store as _, Transaction as _};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use serde_json::{Value, json};

fn package(version: &str) -> health_packages::Files {
    let file = |path: &str, json: Value| (path.to_owned(), json.to_string().into_bytes());
    health_packages::Files::from([
        file(
            "package.json",
            json!({
                "name": "ug.moh.anc", "version": version, "fhirVersions": ["5.0.0"],
                "dependencies": {},
                "kind": "program", "platform": { "minSoftware": "0.1.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json",
            json!({
                "resourceType": "Library", "id": "manifest",
                "url": "http://health.go.ug/fhir/Library/ug.moh.anc-manifest", "version": version,
                "status": "active", "type": { "text": "asset-collection" }
            }),
        ),
        file(
            "Questionnaire-anc-visit.json",
            json!({
                "resourceType": "Questionnaire", "id": "anc-visit",
                "url": "http://health.go.ug/fhir/Questionnaire/anc-visit", "version": version,
                "status": "active",
                "item": [{ "linkId": "bp", "text": "Blood pressure", "type": "string" }]
            }),
        ),
    ])
}

#[tokio::test]
async fn each_active_version_s_definitions_are_written_once() {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    let dir = tempfile::tempdir().expect("blobs");
    let blobs = health_files::FsBlobStore::open(dir.path()).expect("blob store");

    let mut active = ActiveSet::new();
    for version in ["1.0.0", "1.1.0"] {
        let packed = health_packages::pack(&package(version)).expect("pack");
        assert_eq!(
            definitions(&packed.files).len(),
            2,
            "the manifest and the form; package.json and .index.json are not resources"
        );
        blobs.put(packed.tgz.as_slice()).expect("blob");
        let mut tx = store.begin().await.expect("begin");
        tx.put_staged_package(
            &format!("ug.moh.anc@{version}"),
            &packed.digest.to_string(),
            "{}",
        )
        .await
        .expect("staged");
        tx.commit().await.expect("commit");
        active.insert("ug.moh.anc".into(), version.parse().expect("version"));
        assert_eq!(
            write_definitions(&store, &blobs, &active)
                .await
                .expect("write"),
            2
        );
        assert_eq!(
            write_definitions(&store, &blobs, &active)
                .await
                .expect("again"),
            0,
            "written once"
        );
    }
    // Both versions of the form are there, each under its own id: records keep theirs.
    let url = "http://health.go.ug/fhir/Questionnaire/anc-visit";
    let mut tx = store.begin().await.expect("begin");
    for version in ["1.0.0", "1.1.0"] {
        let id = definition_id(url, version).expect("id");
        let head = tx
            .read(&"Questionnaire".parse().expect("type"), id)
            .await
            .expect("read")
            .expect("written");
        let body: Value = serde_json::from_str(head.json.as_str()).expect("json");
        assert_eq!(body["version"], version);
        assert_eq!(body["id"], id.to_string());
    }
    assert_ne!(definition_id(url, "1.0.0"), definition_id(url, "1.1.0"));

    // A set naming a package that is not staged is refused, not half-written.
    active.insert("ug.moh.tb".into(), "1.0.0".parse().expect("version"));
    assert!(
        write_definitions(&store, &blobs, &active)
            .await
            .expect_err("not staged")
            .contains("ug.moh.tb@1.0.0 is active but not staged")
    );
}
