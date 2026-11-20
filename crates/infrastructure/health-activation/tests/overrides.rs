//! A node refuses to switch to a base version that narrows a range below its facility's value
//! (ADR 0026, ADR 0067; #507). Central cannot tell which district a facility is in, so a base
//! activated for a district is checked here, where the node knows its chain.

use std::collections::BTreeMap;
use std::sync::Arc;

use health_activation::{Activator, Outcome, activate};
use health_configuration::activation::{Chain, Scope};
use health_packages::overrides::{OVERRIDE_POINTS_SCHEMA, OVERRIDES_SCHEMA};
use health_packages::{Files, LocalSigner, Schemas, publish};
use health_persistence::{Store as _, Transaction as _};
use health_pki::{Authority, Trust, new_key, request};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use serde_json::{Value, json};

fn file(path: &str, value: &Value) -> (String, Vec<u8>) {
    (path.to_owned(), value.to_string().into_bytes())
}

fn package(name: &str, version: &str, kind: &str, configuration: (String, Vec<u8>)) -> Files {
    Files::from([
        file(
            "package.json",
            &json!({
                "name": name, "version": version, "fhirVersions": ["5.0.0"],
                "kind": kind, "platform": { "minSoftware": "0.1.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json",
            &json!({
                "resourceType": "Library", "id": "manifest",
                "url": format!("http://health.go.ug/fhir/Library/{name}-manifest"),
                "version": version, "status": "active", "type": { "text": "asset-collection" }
            }),
        ),
        configuration,
    ])
}

/// `ug.moh.anc`, allowing tracing after a missed visit in `minimum`–28 days.
fn anc(version: &str, minimum: i64) -> Files {
    package(
        "ug.moh.anc",
        version,
        "program",
        file(
            "configuration/override-points.json",
            &json!({
                "$schema": OVERRIDE_POINTS_SCHEMA,
                "points": [{
                    "key": "tracing.missed-visit-days", "type": "integer", "default": minimum,
                    "allowed": { "minimum": minimum, "maximum": 28 }, "class": "clinically-visible"
                }]
            }),
        ),
    )
}

fn kam013() -> Files {
    package(
        "ug.facility.kam013",
        "1.0.0",
        "override",
        file(
            "configuration/overrides.json",
            &json!({
                "$schema": OVERRIDES_SCHEMA,
                "settings": [{ "package": "ug.moh.anc", "key": "tracing.missed-visit-days", "value": 14 }]
            }),
        ),
    )
}

#[tokio::test]
async fn a_base_narrowing_a_range_below_the_facilitys_value_is_not_switched_to() {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "edge".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    let dir = tempfile::tempdir().expect("blobs");
    let blobs = health_files::FsBlobStore::open(dir.path()).expect("blob store");

    let now = Timestamp::now();
    let root = Authority::root("Uganda Health Root", now).expect("root");
    let key = new_key().expect("key");
    let issued = root
        .issue_publication(
            &request(&key, "MoH publication signing").expect("csr"),
            "MoH publication signing",
            now,
        )
        .expect("publication certificate");
    let signer = LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer");
    let schemas = Schemas::platform();

    // Central's published tarballs, by sha-256, and an activation record for each.
    let mut tarballs = BTreeMap::new();
    let mut record = |id: &str, files: &Files, scope: Scope, at: &str| {
        let published = publish(files, &schemas, &signer, now).expect("publish");
        tarballs.insert(published.digest.to_string(), published.tgz.clone());
        let (name, version) = published.id.split_once('@').expect("id");
        json!({
            "activation": {
                "id": id, "package": name, "version": version, "scope": scope,
                "effectiveAt": at, "issuedAt": at, "by": "ops"
            },
            "digest": published.digest.to_string(),
            "signature": published.signature
        })
    };
    let records = [
        record(
            "act-1",
            &anc("2.0.0", 14),
            Scope::National,
            "2026-01-01T00:00:00Z",
        ),
        record(
            "act-2",
            &kam013(),
            Scope::Facility {
                facility: "KAM013".into(),
            },
            "2026-01-01T00:00:00Z",
        ),
        record(
            "act-3",
            &anc("3.0.0", 21),
            Scope::District {
                district: "kampala".into(),
            },
            "2026-01-02T00:00:00Z",
        ),
    ];
    let issue = |body: &Value| {
        let store = store.clone();
        let body = body.clone();
        async move {
            let mut tx = store.begin().await.expect("begin");
            tx.put_package_activation(
                body["activation"]["id"].as_str().expect("id"),
                body["activation"]["package"].as_str().expect("package"),
                body["activation"]["issuedAt"].as_str().expect("issued"),
                &body.to_string(),
            )
            .await
            .expect("activation");
            tx.commit().await.expect("commit");
        }
    };
    let tarballs = Arc::new(tarballs);
    let fetch = |digest: String| {
        let tarballs = tarballs.clone();
        async move { tarballs.get(&digest).cloned().ok_or(digest) }
    };
    let activator = Activator::new(
        Trust::new(&root.certificate_pem()).expect("trust"),
        Chain {
            district: Some("kampala".into()),
            facility: Some("KAM013".into()),
            ..Chain::default()
        },
    )
    .expect("activator");

    issue(&records[0]).await;
    issue(&records[1]).await;
    let report = activate(&store, Some(&blobs), &activator, now, fetch)
        .await
        .expect("activate")
        .expect("a switch");
    assert_eq!(report.outcome, Outcome::Activated, "{report:?}");

    // Kampala's base 3.0.0 allows 21–28 days; KAM013 chose 14.
    issue(&records[2]).await;
    let report = activate(&store, Some(&blobs), &activator, now, fetch)
        .await
        .expect("activate")
        .expect("an attempt");
    assert_eq!(report.outcome, Outcome::Failed, "{report:?}");
    assert_eq!(
        report.problems,
        [
            "ug.facility.kam013@1.0.0 overrides ug.moh.anc@3.0.0: ug.moh.anc \
             tracing.missed-visit-days: 14 is not allowed: 21 to 28"
        ]
    );
    assert_eq!(
        report.active.get("ug.moh.anc").map(ToString::to_string),
        Some("2.0.0".to_owned()),
        "the previous set stays"
    );
}
