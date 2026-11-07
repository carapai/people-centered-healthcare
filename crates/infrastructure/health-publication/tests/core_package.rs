//! The platform core package (#87; ADR 0049) publishes through the lifecycle (#88): the
//! committed `packages/fhir-core-package/package/` is drafted, passes validation, is reviewed and
//! approved by two people as national core content, and is signed into a package that verifies
//! under the root as a node checks it.

use std::path::Path;
use std::sync::Arc;

use health_configuration::lifecycle::Stage;
use health_files::FsBlobStore;
use health_packages::{Digest, Limits, Schemas, verify};
use health_pki::{Authority, Trust, new_key, request};
use health_publication::{Files, LocalSigner, NoSandbox, Publications};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{Clock as _, FixedClock, Timestamp};

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn core_package() -> Files {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/fhir-core-package/package");
    std::fs::read_dir(&dir)
        .expect("the core package")
        .map(|entry| {
            let path = entry.expect("entry").path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .expect("name")
                .to_owned();
            (name, std::fs::read(&path).expect("read"))
        })
        .collect()
}

#[tokio::test]
async fn the_core_package_publishes_through_the_lifecycle() {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock = Arc::new(FixedClock::new(at("2026-11-04T09:00:00Z")));
    let root = Authority::root("Uganda Health Root", at("2026-01-01T00:00:00Z")).expect("root");
    let key = new_key().expect("key");
    let issued = root
        .issue_publication(
            &request(&key, "Platform publication signing").expect("request"),
            "Platform publication signing",
            at("2026-06-01T00:00:00Z"),
        )
        .expect("publication certificate");
    let signer = LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer");
    let dir = tempfile::tempdir().expect("tempdir");
    let publications = Publications::new(
        SurrealStore::new(connection, clock.clone()),
        FsBlobStore::open(dir.path()).expect("blobs"),
        clock.clone(),
        Schemas::platform(),
        Arc::new(NoSandbox),
        "central",
    );

    let draft = publications
        .draft(&core_package(), "platform-author")
        .await
        .expect("draft");
    let id = format!("{}@{}", draft.package, draft.version);
    assert_eq!(id, "health.platform.core@0.1.0");
    assert_eq!(
        draft.approvals_required(),
        2,
        "national core is clinical content"
    );

    let submitted = publications
        .submit(&id, "platform-author")
        .await
        .expect("submit");
    let (_, records) = publications.lifecycle(&id).await.expect("records");
    assert_eq!(submitted.stage, Stage::Review, "{records:?}");
    publications.review(&id, "reviewer").await.expect("review");
    publications
        .approve(&id, "approver-1")
        .await
        .expect("approve");
    publications
        .approve(&id, "approver-2")
        .await
        .expect("approve");
    let published = publications
        .publish(&id, "operator", &signer)
        .await
        .expect("publish");

    let (_, tgz) = publications.package(&id).await.expect("package");
    let verified = verify(
        &tgz,
        &published.signature,
        Some(&Digest::of(&tgz)),
        &Trust::new(&root.certificate_pem()).expect("trust"),
        &Schemas::platform(),
        Limits::default(),
        clock.now(),
    )
    .expect("the core package verifies under the root");
    let profiles = verified
        .contents
        .artefacts
        .iter()
        .filter(|artefact| artefact.resource_type == "StructureDefinition")
        .count();
    assert_eq!(profiles, 33);
}
