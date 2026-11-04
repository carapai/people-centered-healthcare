use std::collections::BTreeMap;

use super::*;
use crate::package::{Artefact, PackageJson, Platform};

fn version(text: &str) -> Version {
    text.parse().expect("version")
}

fn pinned(version: &str, fingerprint: &str) -> Pinned {
    Pinned {
        version: version.into(),
        fingerprint: fingerprint.into(),
    }
}

fn release(at: &str) -> Release {
    Release {
        version: version(at),
        kind: PackageKind::Program,
        dependencies: BTreeMap::from([("ug.moh.core".into(), "2.1.0".into())]),
        artefacts: BTreeMap::from([
            ("https://x/Questionnaire/anc".into(), pinned("1.0.0", "q1")),
            ("https://x/ValueSet/signs".into(), pinned("1.4.2", "v1")),
        ]),
        files: BTreeMap::from([
            ("configuration/layout.json".into(), "c1".into()),
            ("tests/visit.json".into(), "t1".into()),
        ]),
    }
}

fn required(latest: &Release, next: &Release) -> Bump {
    match check(latest, next, &BTreeMap::new()) {
        Ok(diff) => diff.required,
        Err(problems) => match problems.as_slice() {
            [BumpProblem::Understated { required, .. }] => *required,
            other => panic!("unexpected problems: {other:?}"),
        },
    }
}

#[test]
fn the_package_bump_follows_its_artefacts_bumps() {
    let latest = release("1.3.0");
    for (artefact, bump) in [
        ("1.0.1", Bump::Patch),
        ("1.1.0", Bump::Minor),
        ("2.0.0", Bump::Major),
    ] {
        let mut next = release("9.0.0");
        next.artefacts
            .insert("https://x/Questionnaire/anc".into(), pinned(artefact, "q2"));
        assert_eq!(required(&latest, &next), bump);
    }
}

#[test]
fn an_understated_version_is_refused_and_an_overstated_one_passes() {
    let latest = release("1.3.0");
    let mut next = release("1.3.1");
    next.artefacts
        .insert("https://x/ValueSet/signs".into(), pinned("2.0.0", "v2"));
    assert_eq!(
        check(&latest, &next, &BTreeMap::new()),
        Err(vec![BumpProblem::Understated {
            latest: version("1.3.0"),
            next: version("1.3.1"),
            actual: Bump::Patch,
            required: Bump::Major,
        }])
    );
    next.version = version("2.0.0");
    let diff = check(&latest, &next, &BTreeMap::new()).expect("major is enough");
    assert_eq!((diff.required, diff.actual), (Bump::Major, Bump::Major));

    let mut patch_only = release("2.0.0");
    patch_only
        .artefacts
        .insert("https://x/ValueSet/signs".into(), pinned("1.4.3", "v2"));
    assert_eq!(
        check(&latest, &patch_only, &BTreeMap::new())
            .expect("over-stated")
            .required,
        Bump::Patch
    );
}

#[test]
fn added_and_removed_artefacts_dependencies_and_files() {
    let latest = release("1.3.0");

    let mut next = release("9.0.0");
    next.artefacts
        .insert("https://x/Library/rules".into(), pinned("1.0.0", "l1"));
    assert_eq!(required(&latest, &next), Bump::Minor);

    let mut next = release("9.0.0");
    next.artefacts.remove("https://x/ValueSet/signs");
    assert_eq!(required(&latest, &next), Bump::Major);

    let mut next = release("9.0.0");
    next.dependencies
        .insert("ug.moh.terms".into(), "1.0.0".into());
    assert_eq!(required(&latest, &next), Bump::Minor);

    let mut next = release("9.0.0");
    next.dependencies.clear();
    assert_eq!(required(&latest, &next), Bump::Major);

    let mut next = release("9.0.0");
    next.dependencies
        .insert("ug.moh.core".into(), "2.1.5".into());
    assert_eq!(required(&latest, &next), Bump::Patch);

    let mut next = release("9.0.0");
    next.files
        .insert("configuration/layout.json".into(), "c2".into());
    assert_eq!(required(&latest, &next), Bump::Minor);

    let mut next = release("9.0.0");
    next.files.remove("configuration/layout.json");
    assert_eq!(required(&latest, &next), Bump::Major);

    let mut next = release("9.0.0");
    next.files.remove("tests/visit.json");
    next.files.insert("tests/other.json".into(), "t2".into());
    assert_eq!(required(&latest, &next), Bump::Patch);

    let mut next = release("9.0.0");
    next.kind = PackageKind::NationalCore;
    assert_eq!(required(&latest, &next), Bump::Major);

    assert_eq!(required(&latest, &release("9.0.0")), Bump::None);
}

#[test]
fn a_published_url_version_never_changes_content() {
    let latest = release("1.3.0");
    let mut next = release("1.4.0");
    next.artefacts.insert(
        "https://x/Questionnaire/anc".into(),
        pinned("1.0.0", "q-edited"),
    );
    assert_eq!(
        check(&latest, &next, &BTreeMap::new()),
        Err(vec![BumpProblem::Rewritten(
            "https://x/Questionnaire/anc|1.0.0".into()
        )])
    );
}

#[test]
fn versions_may_not_go_down_or_stand_still() {
    let latest = release("1.3.0");
    for next in ["1.3.0", "1.2.9", "1.3.0-rc.1"] {
        let problems = check(&latest, &release(next), &BTreeMap::new()).expect_err(next);
        assert!(
            problems.contains(&BumpProblem::NotNewer {
                latest: version("1.3.0"),
                next: version(next),
            }),
            "{next}: {problems:?}"
        );
    }

    let mut next = release("2.0.0");
    next.artefacts
        .insert("https://x/ValueSet/signs".into(), pinned("1.4.1", "v2"));
    next.dependencies
        .insert("ug.moh.core".into(), "2.0.0".into());
    let problems = check(&latest, &next, &BTreeMap::new()).expect_err("downgrades");
    assert!(problems.contains(&BumpProblem::Downgraded {
        name: "https://x/ValueSet/signs".into(),
        from: "1.4.2".into(),
        to: "1.4.1".into(),
    }));
    assert!(problems.contains(&BumpProblem::Downgraded {
        name: "ug.moh.core".into(),
        from: "2.1.0".into(),
        to: "2.0.0".into(),
    }));
}

#[test]
fn artefact_versions_must_be_exact() {
    let latest = release("1.3.0");
    let mut next = release("2.0.0");
    next.artefacts
        .insert("https://x/ValueSet/signs".into(), pinned("v2", "v2"));
    next.artefacts
        .insert("https://x/Library/new".into(), pinned("2024", "n"));
    let problems = check(&latest, &next, &BTreeMap::new()).expect_err("inexact");
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(
        problems
            .iter()
            .all(|problem| matches!(problem, BumpProblem::Version { .. }))
    );
}

#[test]
fn major_zero_and_pre_releases_promise_nothing() {
    let mut latest = release("0.3.0");
    let mut next = release("0.4.0");
    next.artefacts.remove("https://x/ValueSet/signs");
    let diff = check(&latest, &next, &BTreeMap::new()).expect("a minor bump is enough under 0.y.z");
    assert_eq!(diff.required, Bump::Minor);

    next.version = version("0.3.1");
    assert!(check(&latest, &next, &BTreeMap::new()).is_err());

    latest.version = version("2.0.0-rc.1");
    next.version = version("2.0.0");
    let diff = check(&latest, &next, &BTreeMap::new()).expect("rc to release");
    assert_eq!(diff.actual, Bump::None);
}

#[test]
fn a_release_is_read_from_package_contents() {
    let contents = PackageContents {
        package: PackageJson {
            name: "ug.moh.anc".into(),
            version: "1.3.0".into(),
            fhir_versions: vec!["5.0.0".into()],
            dependencies: BTreeMap::from([("ug.moh.core".into(), "2.1.0".into())]),
            kind: PackageKind::Program,
            platform: Platform {
                min_software: "1.0.0".into(),
                format_version: 1,
            },
        },
        manifest: Some(vec![]),
        artefacts: vec![
            Artefact {
                path: MANIFEST.into(),
                resource_type: "Library".into(),
                id: Some("manifest".into()),
                url: Some("https://x/Library/manifest".into()),
                version: Some("1.3.0".into()),
            },
            Artefact {
                path: "Questionnaire-anc.json".into(),
                resource_type: "Questionnaire".into(),
                id: Some("anc".into()),
                url: Some("https://x/Questionnaire/anc".into()),
                version: Some("1.0.0".into()),
            },
            Artefact {
                path: "Bundle-example.json".into(),
                resource_type: "Bundle".into(),
                id: Some("example".into()),
                url: None,
                version: None,
            },
        ],
        paths: vec![
            PACKAGE_JSON.into(),
            INDEX.into(),
            MANIFEST.into(),
            "Questionnaire-anc.json".into(),
            "Bundle-example.json".into(),
            "configuration/layout.json".into(),
        ],
    };
    let fingerprints = BTreeMap::from([
        ("Questionnaire-anc.json".into(), "q1".into()),
        ("Bundle-example.json".into(), "b1".into()),
        ("configuration/layout.json".into(), "c1".into()),
    ]);
    let release = Release::of(&contents, &fingerprints).expect("release");
    assert_eq!(release.version, version("1.3.0"));
    assert_eq!(
        release.artefacts,
        BTreeMap::from([("https://x/Questionnaire/anc".into(), pinned("1.0.0", "q1"))])
    );
    assert_eq!(
        release.files,
        BTreeMap::from([
            ("Bundle-example.json".into(), "b1".into()),
            ("configuration/layout.json".into(), "c1".into()),
        ])
    );
}

#[test]
fn an_artefacts_own_bump_is_checked_against_its_content() {
    let url = "https://x/Questionnaire/anc";
    let latest = release("1.3.0");
    let mut next = release("1.4.0");
    next.artefacts.insert(url.into(), pinned("1.1.0", "q2"));
    let removed = ContentChange {
        at: "item[bp]".into(),
        nature: Nature::Removed,
        bump: Bump::Major,
    };

    // The item is gone, but the form claims a minor bump.
    let content = BTreeMap::from([(url.to_owned(), vec![removed.clone()])]);
    assert_eq!(
        check(&latest, &next, &content),
        Err(vec![BumpProblem::ArtefactUnderstated {
            url: url.into(),
            from: "1.0.0".into(),
            to: "1.1.0".into(),
            actual: Bump::Minor,
            required: Bump::Major,
            changes: vec![removed],
        }])
    );

    // A new optional item is a minor bump, and the package follows.
    let added = ContentChange {
        at: "item[pulse]".into(),
        nature: Nature::Added,
        bump: Bump::Minor,
    };
    let content = BTreeMap::from([(url.to_owned(), vec![added])]);
    assert_eq!(
        check(&latest, &next, &content).expect("minor").required,
        Bump::Minor
    );
}
