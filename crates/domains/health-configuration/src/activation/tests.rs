use std::collections::{BTreeMap, BTreeSet};

use health_time::Timestamp;

use super::*;
use crate::package::{PackageJson, PackageKind, Platform};

fn at(hour: u32) -> Timestamp {
    format!("2026-11-10T{hour:02}:00:00Z")
        .parse()
        .expect("time")
}

fn v(text: &str) -> Version {
    text.parse().expect("version")
}

fn activation(id: &str, package: &str, version: &str, scope: Scope, hour: u32) -> Activation {
    Activation {
        id: id.into(),
        package: package.into(),
        version: v(version),
        scope,
        effective_at: at(hour),
        issued_at: at(hour),
        by: "release-manager".into(),
        reason: None,
    }
}

fn kla() -> Chain {
    Chain {
        district: Some("kampala".into()),
        facility: Some("KLA01".into()),
        programs: BTreeSet::from(["art".to_owned()]),
    }
}

fn district(code: &str) -> Scope {
    Scope::District {
        district: code.into(),
    }
}

fn facility(code: &str) -> Scope {
    Scope::Facility {
        facility: code.into(),
    }
}

fn program(facility: &str, program: &str) -> Scope {
    Scope::Program {
        facility: facility.into(),
        program: program.into(),
    }
}

fn package(name: &str, version: &str, dependencies: &[(&str, &str)]) -> PackageJson {
    PackageJson {
        name: name.into(),
        version: version.into(),
        fhir_versions: vec!["5.0.0".into()],
        dependencies: dependencies
            .iter()
            .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
            .collect(),
        kind: PackageKind::Program,
        platform: Platform {
            min_software: "0.3.0".into(),
            format_version: 1,
        },
    }
}

fn release() -> Release {
    Release {
        software: v("0.4.1"),
        formats: BTreeSet::from([1]),
    }
}

fn set(entries: &[(&str, &str)]) -> ActiveSet {
    entries
        .iter()
        .map(|(n, version)| ((*n).to_owned(), v(version)))
        .collect()
}

#[test]
fn the_active_set_merges_the_scope_chain_most_specific_first() {
    let activations = [
        activation("a1", "ug.moh.core", "2.0.0", Scope::National, 8),
        activation("a2", "ug.moh.anc", "1.0.0", Scope::National, 8),
        activation("a3", "ug.moh.anc", "1.1.0", district("kampala"), 8),
        activation("a4", "ug.moh.anc", "1.2.0", facility("KLA01"), 8),
        // Another district's and another facility's do not apply.
        activation("a5", "ug.moh.core", "3.0.0", district("gulu"), 8),
        activation("a6", "ug.moh.tb", "1.0.0", facility("GUL02"), 8),
        // A program the facility runs applies; one it does not run does not.
        activation("a7", "ug.moh.art", "4.0.0", program("KLA01", "art"), 8),
        activation("a8", "ug.moh.hiv-pep", "1.0.0", program("KLA01", "pep"), 8),
    ];
    assert_eq!(
        active_set(&activations, &kla(), at(9)),
        set(&[
            ("ug.moh.anc", "1.2.0"),
            ("ug.moh.art", "4.0.0"),
            ("ug.moh.core", "2.0.0"),
        ])
    );
    // Central sits on no district or facility: it takes the national set only.
    assert_eq!(
        active_set(&activations, &Chain::default(), at(9)),
        set(&[("ug.moh.anc", "1.0.0"), ("ug.moh.core", "2.0.0")])
    );
}

#[test]
fn an_activation_waits_for_its_effective_time_and_rollback_is_a_later_one() {
    let mut upgrade = activation("a2", "ug.moh.anc", "1.1.0", Scope::National, 8);
    upgrade.effective_at = at(12);
    let mut activations = vec![
        activation("a1", "ug.moh.anc", "1.0.0", Scope::National, 6),
        upgrade,
    ];
    assert_eq!(
        active_set(&activations, &kla(), at(10)),
        set(&[("ug.moh.anc", "1.0.0")]),
        "not yet effective"
    );
    assert_eq!(
        active_set(&activations, &kla(), at(12)),
        set(&[("ug.moh.anc", "1.1.0")])
    );
    // Rollback: a new activation of the earlier version, issued later.
    let mut rollback = activation("a3", "ug.moh.anc", "1.0.0", Scope::National, 13);
    rollback.reason = Some("1.1.0 hides the danger-signs section".into());
    activations.push(rollback);
    assert_eq!(
        active_set(&activations, &kla(), at(14)),
        set(&[("ug.moh.anc", "1.0.0")])
    );
    // Every version ever activated is retained.
    assert_eq!(
        retained(&activations, &kla()),
        BTreeSet::from([
            ("ug.moh.anc".to_owned(), v("1.0.0")),
            ("ug.moh.anc".to_owned(), v("1.1.0")),
        ])
    );
}

#[test]
fn a_set_is_checked_for_staging_dependencies_and_software() {
    let staged = [
        package("ug.moh.core", "2.0.0", &[]),
        package("ug.moh.anc", "1.2.0", &[("ug.moh.core", "2.0.0")]),
        package("ug.moh.tb", "1.0.0", &[("ug.moh.core", "1.9.0")]),
        package("ug.moh.art", "4.0.0", &[("ug.moh.lab", "1.0.0")]),
    ];
    assert_eq!(
        check(
            &set(&[("ug.moh.core", "2.0.0"), ("ug.moh.anc", "1.2.0")]),
            &staged,
            &release()
        ),
        vec![]
    );
    let problems = check(
        &set(&[
            ("ug.moh.core", "2.0.0"),
            ("ug.moh.tb", "1.0.0"),
            ("ug.moh.art", "4.0.0"),
            ("ug.moh.malaria", "1.0.0"),
        ]),
        &staged,
        &release(),
    );
    assert_eq!(
        problems,
        vec![
            Problem::MissingDependency {
                package: "ug.moh.art".into(),
                dependency: "ug.moh.lab".into(),
                wanted: "1.0.0".into(),
            },
            Problem::NotStaged {
                package: "ug.moh.malaria".into(),
                version: v("1.0.0"),
            },
            Problem::WrongDependency {
                package: "ug.moh.tb".into(),
                dependency: "ug.moh.core".into(),
                wanted: "1.9.0".into(),
                active: v("2.0.0"),
            },
        ]
    );

    let mut newer = package("ug.moh.core", "2.0.0", &[]);
    newer.platform = Platform {
        min_software: "0.5.0".into(),
        format_version: 2,
    };
    let problems = check(&set(&[("ug.moh.core", "2.0.0")]), &[newer], &release());
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert_eq!(
        problems[0].to_string(),
        "ug.moh.core needs software 0.5.0 or later; this node runs 0.4.1"
    );
    assert_eq!(
        problems[1].to_string(),
        "ug.moh.core is in package format 2, which this release does not read"
    );
}

#[test]
fn a_failed_activation_keeps_the_previous_set_and_reports() {
    let staged = [
        package("ug.moh.core", "2.0.0", &[]),
        package("ug.moh.anc", "1.0.0", &[("ug.moh.core", "2.0.0")]),
        package("ug.moh.anc", "1.1.0", &[("ug.moh.core", "2.0.0")]),
    ];
    let mut activations = vec![
        activation("a1", "ug.moh.core", "2.0.0", Scope::National, 8),
        activation("a2", "ug.moh.anc", "1.0.0", Scope::National, 8),
    ];
    let first = switch(
        &ActiveSet::new(),
        &activations,
        &kla(),
        at(9),
        &staged,
        &release(),
    );
    let Switch::Activated { active, changes } = first else {
        panic!("{first:?}")
    };
    assert_eq!(changes.len(), 2);
    assert_eq!(
        switch(&active, &activations, &kla(), at(9), &staged, &release()),
        Switch::Unchanged
    );

    // An activation whose package has not been staged: nothing moves.
    activations.push(activation(
        "a3",
        "ug.moh.anc",
        "1.3.0",
        facility("KLA01"),
        10,
    ));
    let failed = switch(&active, &activations, &kla(), at(11), &staged, &release());
    assert_eq!(
        failed,
        Switch::Failed {
            wanted: set(&[("ug.moh.anc", "1.3.0"), ("ug.moh.core", "2.0.0")]),
            problems: vec![Problem::NotStaged {
                package: "ug.moh.anc".into(),
                version: v("1.3.0"),
            }],
        }
    );

    // A good one moves the one package that changed.
    activations.push(activation(
        "a4",
        "ug.moh.anc",
        "1.1.0",
        facility("KLA01"),
        12,
    ));
    let Switch::Activated {
        active: next,
        changes,
    } = switch(&active, &activations, &kla(), at(12), &staged, &release())
    else {
        panic!("activated")
    };
    assert_eq!(
        changes,
        vec![Change {
            package: "ug.moh.anc".into(),
            from: Some(v("1.0.0")),
            to: Some(v("1.1.0")),
        }]
    );
    assert_eq!(next.get("ug.moh.anc"), Some(&v("1.1.0")));
}

#[test]
fn activations_and_scopes_read_as_written() {
    let record = activation("a1", "ug.moh.art", "4.0.0", program("KLA01", "art"), 8);
    let json = serde_json::to_value(&record).expect("json");
    assert_eq!(
        json["scope"],
        serde_json::json!({"level": "program", "facility": "KLA01", "program": "art"})
    );
    assert_eq!(json["effectiveAt"], "2026-11-10T08:00:00Z");
    let back: Activation = serde_json::from_value(json).expect("back");
    assert_eq!(back, record);
    assert_eq!(
        program("KLA01", "art").to_string(),
        "program art at facility KLA01"
    );
    assert_eq!(district("kampala").to_string(), "district kampala");
    assert_eq!(Scope::National.to_string(), "national");
    assert_eq!(facility("KLA01").to_string(), "facility KLA01");
    assert_eq!(
        changes(&set(&[("a", "1.0.0")]), &BTreeMap::new()),
        vec![Change {
            package: "a".into(),
            from: Some(v("1.0.0")),
            to: None,
        }]
    );
}
