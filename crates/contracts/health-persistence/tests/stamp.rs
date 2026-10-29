//! `stamp_meta` sets only the server-owned fields and keeps everything else as received.

use health_identifiers::{OpId, ResourceId};
use health_persistence::{RawJson, RawJsonError, ResourceType, stamp_meta};
use health_time::Timestamp;

fn at() -> Timestamp {
    "2026-10-29T08:00:00.25Z".parse().expect("timestamp")
}

fn patient() -> ResourceType {
    "Patient".parse().expect("type")
}

#[test]
fn sets_id_version_and_last_updated_and_keeps_the_rest() {
    let input = RawJson::from_validated(
        r#"{"resourceType":"Patient","id":"client-sent","meta":{"profile":["p"],"versionId":"old"},"_birthDate":{"extension":[{"url":"u","valueDecimal":0.010}]},"birthDate":"1974-12-25"}"#,
    );
    let (id, op) = (ResourceId::new(), OpId::new());
    let stamped = stamp_meta(&input, &patient(), id, op, at()).expect("stamp");
    assert_eq!(
        stamped.as_str(),
        format!(
            r#"{{"resourceType":"Patient","id":"{id}","meta":{{"profile":["p"],"versionId":"{op}","lastUpdated":"2026-10-29T08:00:00.250Z"}},"_birthDate":{{"extension":[{{"url":"u","valueDecimal":0.010}}]}},"birthDate":"1974-12-25"}}"#
        )
    );
}

#[test]
fn adds_meta_when_absent() {
    let input = RawJson::from_validated(r#"{"resourceType":"Patient"}"#);
    let stamped =
        stamp_meta(&input, &patient(), ResourceId::new(), OpId::new(), at()).expect("stamp");
    let value: serde_json::Value = serde_json::from_str(stamped.as_str()).expect("json");
    assert_eq!(value["meta"]["lastUpdated"], "2026-10-29T08:00:00.250Z");
}

#[test]
fn rejects_a_different_resource_type_or_non_object() {
    for text in [
        r#"{"resourceType":"Observation"}"#,
        "[]",
        r#"{"resourceType":"Patient","meta":3}"#,
    ] {
        let result = stamp_meta(
            &RawJson::from_validated(text),
            &patient(),
            ResourceId::new(),
            OpId::new(),
            at(),
        );
        assert_eq!(
            result,
            Err(RawJsonError::WrongShape {
                expected: "Patient".into()
            }),
            "{text}"
        );
    }
    assert!(matches!(
        stamp_meta(
            &RawJson::from_validated("{"),
            &patient(),
            ResourceId::new(),
            OpId::new(),
            at()
        ),
        Err(RawJsonError::Syntax(_))
    ));
}

#[test]
fn resource_type_names_are_validated() {
    assert!("MedicationRequest".parse::<ResourceType>().is_ok());
    for bad in ["", "patient", "Patient1", "Pa tient"] {
        assert!(bad.parse::<ResourceType>().is_err(), "{bad}");
    }
}

#[test]
fn raw_json_debug_never_prints_the_body() {
    let json =
        RawJson::from_validated(r#"{"resourceType":"Patient","name":[{"family":"Okello"}]}"#);
    assert_eq!(format!("{json:?}"), "RawJson(55 bytes)");
}
