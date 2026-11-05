//! Publishing and verifying a package end to end (ADR 0024; configuration.md § Signing), with a
//! real national root and publication-signing certificate: a tampered package is refused, and an
//! unpinned dependency is rejected at publication.

use health_configuration::package::{INDEX, Problem};
use health_packages::{
    Digest, Error, Files, Limits, LocalSigner, Schemas, Verified, publish, verify,
};
use health_pki::{Authority, Reason, Revocations, Trust, new_key, request};
use health_time::{SignedDuration, Timestamp};

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

const SIGNED: &str = "2026-11-03T09:00:00Z";

/// A Form Layout schema, as a release would build in.
const LAYOUT: &str = "https://health.go.ug/schemas/form-layout/1";

fn schemas() -> Schemas {
    Schemas::platform()
        .with(
            LAYOUT,
            &serde_json::json!({
                "type": "object",
                "required": ["$schema", "sections"],
                "properties": {
                    "$schema": { "const": LAYOUT },
                    "sections": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["title"],
                            "properties": { "title": { "type": "string", "minLength": 1 } },
                            "additionalProperties": false
                        }
                    }
                },
                "additionalProperties": false
            }),
        )
        .expect("schema")
}

struct Pki {
    root: Authority,
    trust: Trust,
    signer: LocalSigner,
    serial: health_pki::Serial,
}

fn pki() -> Pki {
    let root = Authority::root("Uganda Health Root", at("2026-01-01T00:00:00Z")).expect("root");
    let key = new_key().expect("key");
    let issued = root
        .issue_publication(
            &request(&key, "MoH publication signing").expect("request"),
            "MoH publication signing",
            at("2026-06-01T00:00:00Z"),
        )
        .expect("publication certificate");
    Pki {
        trust: Trust::new(&root.certificate_pem()).expect("trust"),
        signer: LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer"),
        serial: issued.serial,
        root,
    }
}

fn files() -> Files {
    let file =
        |path: &str, json: serde_json::Value| (path.to_owned(), json.to_string().into_bytes());
    Files::from([
        file(
            "package.json",
            serde_json::json!({
                "name": "ug.moh.anc", "version": "1.4.0", "fhirVersions": ["5.0.0"],
                "dependencies": { "ug.moh.core": "2.1.0" },
                "kind": "program", "platform": { "minSoftware": "0.3.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json",
            serde_json::json!({
                "resourceType": "Library", "id": "manifest",
                "url": "http://health.go.ug/fhir/Library/ug.moh.anc-manifest", "version": "1.4.0",
                "status": "active", "type": { "text": "asset-collection" },
                "relatedArtifact": [{
                    "type": "depends-on",
                    "resource": "http://health.go.ug/fhir/ValueSet/anc-visit-type|2.1.0"
                }]
            }),
        ),
        file(
            "Questionnaire-anc-visit.json",
            serde_json::json!({
                "resourceType": "Questionnaire", "id": "anc-visit",
                "url": "http://health.go.ug/fhir/Questionnaire/anc-visit", "version": "1.4.0",
                "status": "active"
            }),
        ),
        file(
            "configuration/layouts/anc-visit.json",
            serde_json::json!({ "$schema": LAYOUT, "sections": [{ "title": "Visit" }] }),
        ),
        file(
            "tests/anc-visit/extraction.json",
            serde_json::json!({ "case": "first visit" }),
        ),
    ])
}

fn check(tgz: &[u8], sig: &str, pki: &Pki) -> Result<Verified, Error> {
    verify(
        tgz,
        sig,
        None,
        &pki.trust,
        &schemas(),
        Limits::default(),
        at("2026-11-04T09:00:00Z"),
    )
}

#[test]
fn a_published_package_verifies_with_its_identity() {
    let pki = pki();
    let published = publish(&files(), &schemas(), &pki.signer, at(SIGNED)).expect("publishes");
    assert_eq!(published.id, "ug.moh.anc@1.4.0");
    assert_eq!(published.digest, Digest::of(&published.tgz));
    // Deterministic: the same files publish to the same bytes.
    assert_eq!(
        publish(&files(), &schemas(), &pki.signer, at(SIGNED))
            .expect("again")
            .tgz,
        published.tgz
    );

    let verified = verify(
        &published.tgz,
        &published.signature,
        Some(&published.digest),
        &pki.trust,
        &schemas(),
        Limits::default(),
        at("2026-11-04T09:00:00Z"),
    )
    .expect("verifies");
    assert_eq!(verified.signed_at, at(SIGNED));
    assert_eq!(verified.contents.id(), "ug.moh.anc@1.4.0");
    assert!(verified.files.contains_key(INDEX));
    let index: serde_json::Value = serde_json::from_slice(&verified.files[INDEX]).expect("index");
    assert_eq!(index["index-version"], 2);
    assert_eq!(
        index["files"]
            .as_array()
            .map(|f| f.iter().map(|e| e["filename"].clone()).collect::<Vec<_>>()),
        Some(vec![
            "Library-manifest.json".into(),
            "Questionnaire-anc-visit.json".into()
        ])
    );
}

#[test]
fn unpinned_dependencies_are_rejected_at_publication() {
    let pki = pki();
    let mut ranged = files();
    let mut package: serde_json::Value =
        serde_json::from_slice(&ranged["package.json"]).expect("package.json");
    package["dependencies"]["ug.moh.core"] = "^2.1.0".into();
    ranged.insert("package.json".into(), package.to_string().into_bytes());
    assert_eq!(
        publish(&ranged, &schemas(), &pki.signer, at(SIGNED)),
        Err(Error::NotPublishable(vec![Problem::Unpinned {
            name: "ug.moh.core".into(),
            range: "^2.1.0".into()
        }]))
    );

    let mut unpinned_manifest = files();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&unpinned_manifest["Library-manifest.json"]).expect("manifest");
    manifest["relatedArtifact"][0]["resource"] =
        "http://health.go.ug/fhir/ValueSet/anc-visit-type".into();
    unpinned_manifest.insert(
        "Library-manifest.json".into(),
        manifest.to_string().into_bytes(),
    );
    assert_eq!(
        publish(&unpinned_manifest, &schemas(), &pki.signer, at(SIGNED)),
        Err(Error::NotPublishable(vec![Problem::UnpinnedCanonical(
            "http://health.go.ug/fhir/ValueSet/anc-visit-type".into()
        )]))
    );
}

#[test]
fn a_tampered_package_is_refused() {
    let pki = pki();
    let published = publish(&files(), &schemas(), &pki.signer, at(SIGNED)).expect("publishes");

    // Any byte changed.
    for offset in [0, published.tgz.len() / 2, published.tgz.len() - 1] {
        let mut tampered = published.tgz.clone();
        tampered[offset] ^= 0x01;
        assert!(
            matches!(
                check(&tampered, &published.signature, &pki),
                Err(Error::Signature(_))
            ),
            "byte {offset}"
        );
    }

    // A different package, under the first one's signature.
    let mut other = files();
    other.insert("tests/extra.json".into(), b"{}".to_vec());
    let swapped = publish(&other, &schemas(), &pki.signer, at(SIGNED)).expect("publishes");
    assert!(matches!(
        check(&swapped.tgz, &published.signature, &pki),
        Err(Error::Signature(_))
    ));

    // Not the package the activation named.
    assert!(matches!(
        verify(
            &swapped.tgz,
            &swapped.signature,
            Some(&published.digest),
            &pki.trust,
            &schemas(),
            Limits::default(),
            at("2026-11-04T09:00:00Z"),
        ),
        Err(Error::Digest { .. })
    ));

    // A forged signature: the payload carried, or the signature bytes altered.
    let (header, signature) = published.signature.split_once("..").expect("detached JWS");
    for forged in [
        format!("{header}.e30.{signature}"),
        format!(
            "{header}..{}{}",
            if signature.starts_with('A') { 'B' } else { 'A' },
            &signature[1..]
        ),
        "not a jws".to_owned(),
    ] {
        assert!(
            matches!(
                check(&published.tgz, &forged, &pki),
                Err(Error::Signature(_))
            ),
            "{forged}"
        );
    }
}

#[test]
fn only_the_roots_publication_signer_is_trusted() {
    let pki = pki();
    // A key with a self-made certificate.
    let rogue_root = Authority::root("Rogue", at("2026-01-01T00:00:00Z")).expect("root");
    let key = new_key().expect("key");
    let rogue = rogue_root
        .issue_publication(
            &request(&key, "rogue").expect("request"),
            "rogue",
            at("2026-06-01T00:00:00Z"),
        )
        .expect("certificate");
    let rogue_signer = LocalSigner::from_pem(&key.serialize_pem(), rogue.pem()).expect("signer");
    let forged = publish(&files(), &schemas(), &rogue_signer, at(SIGNED)).expect("publishes");
    assert!(matches!(
        check(&forged.tgz, &forged.signature, &pki),
        Err(Error::Untrusted(_))
    ));

    // The real certificate, with another key: the signature fails.
    let impostor =
        LocalSigner::from_pem(&new_key().expect("key").serialize_pem(), pki.signer_chain())
            .expect("signer");
    let forged = publish(&files(), &schemas(), &impostor, at(SIGNED)).expect("publishes");
    assert!(matches!(
        check(&forged.tgz, &forged.signature, &pki),
        Err(Error::Signature(_))
    ));
}

#[test]
fn the_certificate_is_checked_when_signing_and_revocation_counts_at_any_time() {
    let mut pki = pki();
    let published = publish(&files(), &schemas(), &pki.signer, at(SIGNED)).expect("publishes");
    // Years later, past the certificate's yearly rotation, the package still verifies.
    let later = at(SIGNED)
        .checked_add(SignedDuration::from_hours(24 * 365 * 3))
        .expect("later");
    assert!(
        verify(
            &published.tgz,
            &published.signature,
            None,
            &pki.trust,
            &schemas(),
            Limits::default(),
            later
        )
        .is_ok()
    );
    // But not if signed after the certificate expired, nor in the future.
    let expired = publish(&files(), &schemas(), &pki.signer, later).expect("publishes");
    assert!(matches!(
        verify(
            &expired.tgz,
            &expired.signature,
            None,
            &pki.trust,
            &schemas(),
            Limits::default(),
            later
        ),
        Err(Error::Untrusted(_))
    ));
    let early = publish(
        &files(),
        &schemas(),
        &pki.signer,
        at("2026-11-05T09:00:00Z"),
    )
    .expect("publishes");
    assert!(matches!(
        check(&early.tgz, &early.signature, &pki),
        Err(Error::Signature(_))
    ));

    // Revoked through the root's CRL: refused, whenever it was signed.
    let mut revocations = Revocations::default();
    revocations.revoke(
        pki.serial.clone(),
        Reason::KeyCompromise,
        at("2026-11-10T00:00:00Z"),
    );
    let crl = revocations
        .crl(&pki.root, at("2026-11-10T00:00:00Z"))
        .expect("crl");
    pki.trust.add_crl(&crl).expect("crl");
    assert!(matches!(
        check(&published.tgz, &published.signature, &pki),
        Err(Error::Untrusted(_))
    ));
}

impl Pki {
    fn signer_chain(&self) -> String {
        health_packages::PublicationSigner::chain_pem(&self.signer).to_owned()
    }
}

#[test]
fn configuration_must_validate_against_a_schema_the_release_knows() {
    let pki = pki();
    let with_layout = |layout: serde_json::Value| {
        let mut files = files();
        files.insert(
            "configuration/layouts/anc-visit.json".into(),
            layout.to_string().into_bytes(),
        );
        files
    };
    let refused = |files: &Files| match publish(files, &schemas(), &pki.signer, at(SIGNED)) {
        Err(Error::Configuration { path, detail }) => Some((path, detail)),
        _ => None,
    };
    let path = "configuration/layouts/anc-visit.json".to_owned();

    // No $schema, one the release does not know, and one the file does not meet.
    let (at_path, detail) =
        refused(&with_layout(serde_json::json!({ "sections": [] }))).expect("refused");
    assert_eq!(
        (at_path.as_str(), detail.as_str()),
        (path.as_str(), "names no $schema")
    );
    let (_, detail) = refused(&with_layout(
        serde_json::json!({ "$schema": "https://example.org/mine", "sections": [] }),
    ))
    .expect("refused");
    assert!(
        detail.contains("not a schema this release reads"),
        "{detail}"
    );
    let (_, detail) = refused(&with_layout(
        serde_json::json!({ "$schema": LAYOUT, "sections": [{ "title": "" }], "extra": true }),
    ))
    .expect("refused");
    assert!(detail.contains("/sections/0/title"), "{detail}");
    assert!(detail.contains("extra"), "{detail}");

    // Configuration is JSON.
    let mut yaml = files();
    yaml.insert("configuration/settings.yaml".into(), b"a: 1".to_vec());
    let (at_path, _) = refused(&yaml).expect("refused");
    assert_eq!(at_path, "configuration/settings.yaml");

    // A release that does not know the schema refuses a package that another one published.
    let published = publish(&files(), &schemas(), &pki.signer, at(SIGNED)).expect("publishes");
    assert!(matches!(
        verify(
            &published.tgz,
            &published.signature,
            None,
            &pki.trust,
            &Schemas::platform(),
            Limits::default(),
            at("2026-11-04T09:00:00Z"),
        ),
        Err(Error::Configuration { .. })
    ));

    // An unusable schema is refused when built in.
    assert!(matches!(
        Schemas::platform().with("https://x", &serde_json::json!({ "type": 12 })),
        Err(Error::Configuration { .. })
    ));
}

#[test]
fn the_release_reads_form_layouts() {
    let schemas = Schemas::platform();
    assert!(
        schemas
            .uris()
            .any(|uri| uri == health_packages::FORM_LAYOUT)
    );
    let pki = pki();
    let layout = |value: serde_json::Value| {
        let mut files = files();
        files.insert(
            "configuration/layouts/anc-visit.json".into(),
            value.to_string().into_bytes(),
        );
        publish(&files, &schemas, &pki.signer, at(SIGNED))
    };
    assert!(
        layout(serde_json::json!({
            "$schema": health_packages::FORM_LAYOUT,
            "url": "http://health.go.ug/fhir/layout/anc-visit", "version": "1.0.0",
            "questionnaire": "http://health.go.ug/fhir/Questionnaire/anc-visit|1.4.0",
            "sections": [{ "title": "History", "items": ["age", "pregnant"] }],
            "items": { "pregnant": { "widget": "radio", "hint": "As the client reports it" } }
        }))
        .is_ok()
    );
    assert!(matches!(
        layout(serde_json::json!({
            "$schema": health_packages::FORM_LAYOUT,
            "url": "u", "version": "1", "questionnaire": "no-version",
            "sections": [], "items": { "age": { "hidden": true } }
        })),
        Err(Error::Configuration { .. })
    ));
}
