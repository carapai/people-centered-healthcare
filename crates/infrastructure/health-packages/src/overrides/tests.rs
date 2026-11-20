use health_configuration::overrides::{PointProblem, SettingProblem, ValueProblem};
use health_configuration::package::{MANIFEST, PACKAGE_JSON, Problem};
use serde_json::{Value, json};

use super::*;
use crate::{Packed, Schemas};

fn file(path: &str, value: &Value) -> (String, Vec<u8>) {
    (path.to_owned(), value.to_string().into_bytes())
}

fn package(name: &str, kind: &str, extra: (String, Vec<u8>)) -> Packed {
    let files = Files::from([
        file(
            PACKAGE_JSON,
            &json!({
                "name": name, "version": "1.0.0", "fhirVersions": ["5.0.0"],
                "kind": kind, "platform": { "minSoftware": "0.3.0", "formatVersion": 1 }
            }),
        ),
        file(
            MANIFEST,
            &json!({
                "resourceType": "Library", "id": "manifest",
                "url": format!("http://health.go.ug/fhir/Library/{name}-manifest"),
                "version": "1.0.0", "status": "active", "type": { "text": "asset-collection" }
            }),
        ),
        extra,
    ]);
    crate::pack(&files).expect("packed")
}

fn base(points: Value) -> Packed {
    package(
        "ug.moh.anc",
        "program",
        file(
            OVERRIDE_POINTS,
            &json!({ "$schema": OVERRIDE_POINTS_SCHEMA, "points": points }),
        ),
    )
}

fn facility(settings: Value) -> Packed {
    package(
        "ug.facility.kam013",
        "override",
        file(
            OVERRIDES,
            &json!({ "$schema": OVERRIDES_SCHEMA, "settings": settings }),
        ),
    )
}

fn tracing(minimum: i64) -> Value {
    json!({
        "key": "tracing.missed-visit-days", "type": "integer", "default": minimum,
        "allowed": { "minimum": minimum, "maximum": 28 }, "class": "clinically-visible",
        "description": "Days after a missed visit before tracing starts"
    })
}

#[test]
fn declarations_and_settings_publish_under_the_release_schemas() {
    let schemas = Schemas::platform();
    let base = base(json!([
        tracing(14),
        { "key": "worklist.columns", "type": "order", "default": ["name", "age"],
          "allowed": { "values": ["name", "age"] }, "class": "presentation" }
    ]));
    assert_eq!(crate::check(&base, &schemas), Ok(()));
    assert_eq!(points(&base.files).expect("points").len(), 2);
    let facility = facility(json!([
        { "package": "ug.moh.anc", "key": "tracing.missed-visit-days", "value": 21 }
    ]));
    assert_eq!(crate::check(&facility, &schemas), Ok(()));
    assert_eq!(
        check("ug.moh.anc", &base.files, &facility.files).expect("readable"),
        []
    );
}

#[test]
fn a_schema_refuses_an_unknown_class_or_shape() {
    let schemas = Schemas::platform();
    let mut point = tracing(14);
    point["class"] = json!("cosmetic");
    // It does not read as a declaration, before the schema is even asked.
    assert!(matches!(
        crate::check(&base(json!([point])), &schemas),
        Err(crate::Error::Json { .. } | crate::Error::Configuration { .. })
    ));
    let loose = facility(json!([{ "package": "ug.moh.anc", "key": "x", "value": 1, "why": "?" }]));
    assert!(matches!(
        crate::check(&loose, &schemas),
        Err(crate::Error::Configuration { .. })
    ));
}

#[test]
fn a_default_outside_its_values_or_a_hiding_point_is_not_publishable() {
    let mut point = tracing(14);
    point["default"] = json!(7);
    let hiding = json!({
        "key": "danger-signs.hidden", "type": "boolean", "default": false, "class": "presentation"
    });
    let refused = crate::check(&base(json!([point, hiding])), &Schemas::platform());
    let Err(crate::Error::NotPublishable(problems)) = refused else {
        panic!("published: {refused:?}");
    };
    assert!(matches!(
        problems[0],
        Problem::Point(PointProblem::Default { .. })
    ));
    assert_eq!(
        problems[1],
        Problem::Point(PointProblem::Hides("danger-signs.hidden".into()))
    );
}

#[test]
fn an_override_setting_an_undeclared_key_or_twice_is_refused() {
    let facility = facility(json!([
        { "package": "ug.moh.anc", "key": "triage.skip-vitals", "value": true },
        { "package": "ug.moh.anc", "key": "tracing.missed-visit-days", "value": 21 },
        { "package": "ug.moh.anc", "key": "tracing.missed-visit-days", "value": 28 }
    ]));
    assert_eq!(
        crate::check(&facility, &Schemas::platform()),
        Err(crate::Error::NotPublishable(vec![Problem::Setting(
            SettingProblem::Duplicate {
                package: "ug.moh.anc".into(),
                key: "tracing.missed-visit-days".into(),
            }
        )]))
    );
    let problems = check(
        "ug.moh.anc",
        &base(json!([tracing(21)])).files,
        &facility.files,
    )
    .expect("readable");
    assert_eq!(
        problems[0],
        SettingProblem::Undeclared {
            package: "ug.moh.anc".into(),
            key: "triage.skip-vitals".into(),
        }
    );
    assert!(problems.iter().all(|problem| !matches!(
        problem,
        SettingProblem::Value {
            problem: ValueProblem::NotAllowed { .. },
            ..
        }
    )));
}

#[test]
fn a_base_with_no_declarations_orphans_every_setting() {
    let plain = package(
        "ug.moh.anc",
        "program",
        file("tests/readme.json", &json!({})),
    );
    let facility = facility(json!([
        { "package": "ug.moh.anc", "key": "tracing.missed-visit-days", "value": 21 }
    ]));
    assert_eq!(
        check("ug.moh.anc", &plain.files, &facility.files).expect("readable"),
        [SettingProblem::Undeclared {
            package: "ug.moh.anc".into(),
            key: "tracing.missed-visit-days".into(),
        }]
    );
}
