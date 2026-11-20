use super::*;
use crate::package::PackageKind;

fn tracing() -> Point {
    Point {
        key: "tracing.missed-visit-days".into(),
        kind: Type::Integer,
        default: Value::Integer(14),
        allowed: Some(Allowed::Range {
            minimum: 14,
            maximum: 28,
        }),
        class: Class::ClinicallyVisible,
        description: None,
    }
}

fn columns() -> Point {
    Point {
        key: "worklist.columns".into(),
        kind: Type::Order,
        default: Value::Codes(vec!["name".into(), "age".into(), "waiting".into()]),
        allowed: Some(Allowed::Values {
            values: vec![
                Value::Code("name".into()),
                Value::Code("age".into()),
                Value::Code("waiting".into()),
            ],
        }),
        class: Class::Presentation,
        description: None,
    }
}

fn gates() -> Point {
    Point {
        key: "financial-gates".into(),
        kind: Type::Boolean,
        default: Value::Boolean(true),
        allowed: None,
        class: Class::Operational,
        description: None,
    }
}

fn setting(key: &str, value: Value) -> Setting {
    Setting {
        package: "ug.moh.anc".into(),
        key: key.into(),
        value,
    }
}

#[test]
fn declarations_read_as_written() {
    let json = r#"{ "key": "tracing.missed-visit-days", "type": "integer", "default": 14,
        "allowed": { "minimum": 14, "maximum": 28 }, "class": "clinically-visible" }"#;
    let read: Point = serde_json::from_str(json).expect("point");
    assert_eq!(read, tracing());
    let json = r#"{ "key": "worklist.columns", "type": "order",
        "default": ["name", "age", "waiting"],
        "allowed": { "values": ["name", "age", "waiting"] }, "class": "presentation" }"#;
    assert_eq!(
        serde_json::from_str::<Point>(json).expect("point"),
        columns()
    );
    assert_eq!(
        serde_json::to_value(Class::ClinicallyVisible).ok(),
        Some(serde_json::json!("clinically-visible"))
    );
}

#[test]
fn sound_declarations_have_no_problems() {
    assert_eq!(point_problems(&[tracing(), columns(), gates()]), []);
}

#[test]
fn a_declaration_must_allow_its_default_and_suit_its_type() {
    let mut narrow = tracing();
    narrow.default = Value::Integer(7);
    let mut empty = tracing();
    empty.key = "empty".into();
    empty.allowed = Some(Allowed::Range {
        minimum: 28,
        maximum: 14,
    });
    let mut listless = columns();
    listless.key = "listless".into();
    listless.allowed = None;
    let mut ranged = columns();
    ranged.key = "ranged".into();
    ranged.allowed = Some(Allowed::Range {
        minimum: 1,
        maximum: 2,
    });
    assert_eq!(
        point_problems(&[narrow, empty, listless, ranged, gates(), gates()]),
        [
            PointProblem::Default {
                key: "tracing.missed-visit-days".into(),
                problem: ValueProblem::NotAllowed {
                    value: Value::Integer(7),
                    allowed: Allowed::Range {
                        minimum: 14,
                        maximum: 28
                    },
                },
            },
            PointProblem::Allowed {
                key: "empty".into(),
                detail: "the range 28 to 14 is empty".into(),
            },
            PointProblem::Allowed {
                key: "listless".into(),
                detail: "a point of type order must say which values it allows".into(),
            },
            PointProblem::Allowed {
                key: "ranged".into(),
                detail: "a point of type order allows a list of codes, not a range".into(),
            },
            PointProblem::Duplicate("financial-gates".into()),
        ]
    );
}

#[test]
fn no_point_may_hide_a_question() {
    let mut hiding = gates();
    hiding.key = "anc-visit.section.danger-signs.hidden".into();
    let mut showing = gates();
    showing.key = "danger-signs.visible".into();
    let mut bad = gates();
    bad.key = "Danger Signs".into();
    assert_eq!(
        point_problems(&[hiding, showing, bad]),
        [
            PointProblem::Hides("anc-visit.section.danger-signs.hidden".into()),
            PointProblem::Hides("danger-signs.visible".into()),
            PointProblem::Key("Danger Signs".into()),
        ]
    );
    // An order reorders: leaving a section out would hide it.
    assert_eq!(
        columns().check(&Value::Codes(vec!["waiting".into(), "name".into()])),
        Err(ValueProblem::LeavesOut(vec!["age".into()]))
    );
    assert_eq!(
        columns().check(&Value::Codes(vec![
            "waiting".into(),
            "name".into(),
            "age".into()
        ])),
        Ok(())
    );
}

#[test]
fn an_override_setting_an_undeclared_key_is_refused() {
    let settings = [
        setting("tracing.missed-visit-days", Value::Integer(21)),
        setting("triage.skip-vitals", Value::Boolean(true)),
        Setting {
            package: "ug.moh.tb".into(),
            key: "anything".into(),
            value: Value::Boolean(true),
        },
    ];
    assert_eq!(
        check("ug.moh.anc", &[tracing(), columns()], &settings),
        [SettingProblem::Undeclared {
            package: "ug.moh.anc".into(),
            key: "triage.skip-vitals".into(),
        }]
    );
    assert_eq!(
        bases(&settings),
        BTreeSet::from(["ug.moh.anc", "ug.moh.tb"])
    );
}

#[test]
fn values_out_of_range_of_the_wrong_type_or_twice_are_refused() {
    let settings = [
        setting("tracing.missed-visit-days", Value::Integer(30)),
        setting("worklist.columns", Value::Code("name".into())),
        setting("financial-gates", Value::Boolean(false)),
    ];
    assert_eq!(
        check("ug.moh.anc", &[tracing(), columns(), gates()], &settings),
        [
            SettingProblem::Value {
                package: "ug.moh.anc".into(),
                key: "tracing.missed-visit-days".into(),
                problem: ValueProblem::NotAllowed {
                    value: Value::Integer(30),
                    allowed: Allowed::Range {
                        minimum: 14,
                        maximum: 28
                    },
                },
            },
            SettingProblem::Value {
                package: "ug.moh.anc".into(),
                key: "worklist.columns".into(),
                problem: ValueProblem::WrongType {
                    expected: Type::Order,
                    value: Value::Code("name".into()),
                },
            },
        ]
    );
    assert_eq!(
        columns().check(&Value::Codes(vec![
            "name".into(),
            "name".into(),
            "age".into()
        ])),
        Err(ValueProblem::Twice("name".into()))
    );
    let twice = [
        setting("financial-gates", Value::Boolean(false)),
        setting("financial-gates", Value::Boolean(true)),
    ];
    assert_eq!(
        setting_problems(&twice),
        [SettingProblem::Duplicate {
            package: "ug.moh.anc".into(),
            key: "financial-gates".into(),
        }]
    );
}

#[test]
fn a_narrowed_base_orphans_or_outranges_a_scopes_value() {
    let chosen = [
        setting("tracing.missed-visit-days", Value::Integer(14)),
        setting(
            "worklist.columns",
            Value::Codes(vec!["age".into(), "name".into(), "waiting".into()]),
        ),
    ];
    assert_eq!(check("ug.moh.anc", &[tracing(), columns()], &chosen), []);
    // Version 3 narrows the range to 21–28 and drops the worklist point.
    let mut narrowed = tracing();
    narrowed.allowed = Some(Allowed::Range {
        minimum: 21,
        maximum: 28,
    });
    narrowed.default = Value::Integer(21);
    let problems = check("ug.moh.anc", &[narrowed], &chosen);
    assert_eq!(problems.len(), 2);
    assert!(matches!(problems[0], SettingProblem::Value { .. }));
    assert_eq!(
        problems[1],
        SettingProblem::Undeclared {
            package: "ug.moh.anc".into(),
            key: "worklist.columns".into(),
        }
    );
}

#[test]
fn an_override_package_holds_only_its_settings() {
    let paths = |extra: &[&str]| -> Vec<String> {
        [
            "package.json",
            "Library-manifest.json",
            ".index.json",
            OVERRIDES,
        ]
        .iter()
        .chain(extra)
        .map(|p| (*p).to_owned())
        .collect()
    };
    assert_eq!(
        package_problems(PackageKind::Override, false, &paths(&[])),
        []
    );
    assert_eq!(
        package_problems(
            PackageKind::Override,
            true,
            &paths(&[
                "StructureDefinition-patient.json",
                "configuration/layouts/anc.json",
                OVERRIDE_POINTS,
            ])
        ),
        [
            OverrideProblem::Dependencies,
            OverrideProblem::Carries("StructureDefinition-patient.json".into()),
            OverrideProblem::Carries("configuration/layouts/anc.json".into()),
            OverrideProblem::Carries(OVERRIDE_POINTS.into()),
        ]
    );
    assert_eq!(
        package_problems(PackageKind::Program, true, &paths(&[OVERRIDE_POINTS])),
        [OverrideProblem::NotAnOverride]
    );
    assert_eq!(
        package_problems(PackageKind::Language, false, &[OVERRIDE_POINTS.into()]),
        [OverrideProblem::Declares("language".into())]
    );
}
