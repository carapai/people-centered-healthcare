//! Language Packs through central's lifecycle (ADR 0066; #502): a pack for `ug.moh.anc@2.x`
//! publishes with one approval by someone who never held its draft, and once the target rewords
//! an item, the pack's entry for it is reported stale.

use std::sync::Arc;

use health_configuration::language::{Key, Part, Status};
use health_configuration::lifecycle::{Refused, Stage};
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_pki::{Authority, new_key, request};
use health_publication::{Error, Files, LocalSigner, NoSandbox, Publications};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use serde_json::{Value, json};

const ANC: &str = "http://health.go.ug/fhir/Questionnaire/anc-visit";

struct Central {
    publications: Publications,
    signer: LocalSigner,
    _blobs: tempfile::TempDir,
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

async fn central() -> Central {
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
            &request(&key, "MoH publication signing").expect("request"),
            "MoH publication signing",
            at("2026-06-01T00:00:00Z"),
        )
        .expect("publication certificate");
    let dir = tempfile::tempdir().expect("tempdir");
    let blobs = FsBlobStore::open(dir.path()).expect("blobs");
    let store = SurrealStore::new(connection, clock.clone());
    Central {
        publications: Publications::new(
            store,
            blobs,
            clock,
            Schemas::platform(),
            Arc::new(NoSandbox),
            "central",
        ),
        signer: LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer"),
        _blobs: dir,
    }
}

fn file(path: &str, value: &Value) -> (String, Vec<u8>) {
    (path.to_owned(), value.to_string().into_bytes())
}

fn manifest(name: &str, version: &str) -> (String, Vec<u8>) {
    file(
        "Library-manifest.json",
        &json!({
            "resourceType": "Library", "id": "manifest",
            "url": format!("http://health.go.ug/fhir/Library/{name}-manifest"), "version": version,
            "status": "active", "type": { "text": "asset-collection" }
        }),
    )
}

/// `ug.moh.anc@version`, its form at `form_version` with the item `bp` worded `bp_text`.
fn anc(version: &str, form_version: &str, bp_text: &str) -> Files {
    Files::from([
        file(
            "package.json",
            &json!({
                "name": "ug.moh.anc", "version": version, "fhirVersions": ["5.0.0"],
                "kind": "program", "platform": { "minSoftware": "0.3.0", "formatVersion": 1 }
            }),
        ),
        manifest("ug.moh.anc", version),
        file(
            "Questionnaire-anc-visit.json",
            &json!({
                "resourceType": "Questionnaire", "id": "anc-visit", "url": ANC,
                "version": form_version, "status": "active",
                "item": [
                    { "linkId": "bp", "text": bp_text, "type": "string" },
                    { "linkId": "pulse", "text": "Pulse", "type": "integer" }
                ]
            }),
        ),
    ])
}

fn item(link_id: &str) -> Key {
    Key::Item {
        questionnaire: ANC.into(),
        link_id: link_id.into(),
        part: Part::Text,
    }
}

/// `ug.moh.anc.lg@version`, translating `ug.moh.anc@2.x` into Luganda.
fn pack(version: &str) -> Files {
    Files::from([
        file(
            "package.json",
            &json!({
                "name": "ug.moh.anc.lg", "version": version, "fhirVersions": ["5.0.0"],
                "kind": "language", "platform": { "minSoftware": "0.3.0", "formatVersion": 1 },
                "translates": { "package": "ug.moh.anc", "version": "2.x", "language": "lg" }
            }),
        ),
        manifest("ug.moh.anc.lg", version),
        file(
            "configuration/translations.json",
            &json!({
                "$schema": health_packages::LANGUAGE_PACK,
                "language": "lg",
                "entries": [
                    { "key": item("bp"), "english": "Blood pressure", "text": "Puleesa" },
                    { "key": item("pulse"), "english": "Pulse", "text": "Okukuba kw'omutima" }
                ]
            }),
        ),
    ])
}

async fn publish_anc(central: &Central, version: &str, form_version: &str, bp_text: &str) {
    let publications = &central.publications;
    let id = format!("ug.moh.anc@{version}");
    publications
        .draft(&anc(version, form_version, bp_text), "ann")
        .await
        .expect("draft");
    let submitted = publications.submit(&id, "ann").await.expect("submit");
    assert_eq!(submitted.stage, Stage::Review, "{id}");
    publications.review(&id, "rita").await.expect("review");
    publications.approve(&id, "bob").await.expect("approve");
    publications.approve(&id, "cara").await.expect("approve");
    publications
        .publish(&id, "ops", &central.signer)
        .await
        .expect("publish");
}

#[tokio::test]
async fn a_pack_for_anc_2x_publishes_with_one_approval() {
    let central = central().await;
    let publications = &central.publications;
    let id = "ug.moh.anc.lg@1.0.0";
    publications
        .draft(&pack("1.0.0"), "tom")
        .await
        .expect("draft");

    // Nothing of ug.moh.anc@2.x is published yet: validation sends the pack back.
    let refused = publications.submit(id, "tom").await.expect("submit");
    assert_eq!(refused.stage, Stage::Draft);
    let (_, records) = publications.lifecycle(id).await.expect("records");
    let failed = serde_json::to_string(&records).expect("json");
    assert!(
        failed.contains("no version of ug.moh.anc@2.x is published"),
        "{failed}"
    );

    publish_anc(&central, "2.0.0", "2.0.0", "Blood pressure").await;
    let submitted = publications.submit(id, "tom").await.expect("submit");
    assert_eq!(submitted.stage, Stage::Review);
    assert_eq!(submitted.approvals_required(), 1);
    publications.review(id, "rita").await.expect("review");
    // Whoever held the draft may not approve it.
    assert!(matches!(
        publications.approve(id, "tom").await,
        Err(Error::Refused(Refused::Author))
    ));
    publications.approve(id, "nalule").await.expect("approve");
    let published = publications
        .publish(id, "ops", &central.signer)
        .await
        .expect("published with one approval");
    assert_eq!(published.approvers, ["nalule"]);

    let report = publications.translation_report(id).await.expect("report");
    assert_eq!(report.target, "ug.moh.anc@2.0.0");
    assert_eq!(report.report.stale().count(), 0);
    assert_eq!((report.report.translated, report.report.strings), (2, 2));
}

#[tokio::test]
async fn after_the_target_rewords_an_item_that_entry_is_reported_stale() {
    let central = central().await;
    let publications = &central.publications;
    publish_anc(&central, "2.0.0", "2.0.0", "Blood pressure").await;
    let id = "ug.moh.anc.lg@1.0.0";
    publications
        .draft(&pack("1.0.0"), "tom")
        .await
        .expect("draft");

    // A reworded display text is a patch to the form, and so to the package.
    publish_anc(&central, "2.0.1", "2.0.1", "Blood pressure (mmHg)").await;
    let report = publications.translation_report(id).await.expect("report");
    assert_eq!(report.target, "ug.moh.anc@2.0.1");
    let stale: Vec<_> = report.report.stale().collect();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].entry.key, item("bp"));
    assert_eq!(
        stale[0].status,
        Status::Stale {
            now: "Blood pressure (mmHg)".into()
        }
    );
    assert_eq!(report.report.served().len(), 1);

    // A 3.0.0 does not move a pack targeting 2.x.
    publish_anc(&central, "3.0.0", "3.0.0", "BP").await;
    let report = publications.translation_report(id).await.expect("report");
    assert_eq!(report.target, "ug.moh.anc@2.0.1");
}

#[tokio::test]
async fn a_pack_takes_semver_of_its_own() {
    let central = central().await;
    let publications = &central.publications;
    publish_anc(&central, "2.0.0", "2.0.0", "Blood pressure").await;
    let first = "ug.moh.anc.lg@1.0.0";
    publications
        .draft(&pack("1.0.0"), "tom")
        .await
        .expect("draft");
    publications.submit(first, "tom").await.expect("submit");
    publications.review(first, "rita").await.expect("review");
    publications
        .approve(first, "nalule")
        .await
        .expect("approve");
    publications
        .publish(first, "ops", &central.signer)
        .await
        .expect("publish");

    // A corrected wording goes out as a patch.
    let mut files = pack("1.0.1");
    let mut translations: Value =
        serde_json::from_slice(&files["configuration/translations.json"]).expect("json");
    translations["entries"][0]["text"] = json!("Puleesa y'omusaayi");
    files.insert(
        "configuration/translations.json".into(),
        translations.to_string().into_bytes(),
    );
    publications.draft(&files, "tom").await.expect("draft");
    let submitted = publications
        .submit("ug.moh.anc.lg@1.0.1", "tom")
        .await
        .expect("submit");
    assert_eq!(submitted.stage, Stage::Review);
}
