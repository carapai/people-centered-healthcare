//! Golden and behavioural tests. The UUIDv5 and Luhn expectations were computed independently
//! with Python's `uuid.uuid5` and a reference Luhn mod N implementation, so they pin the
//! derivations across platforms and languages.

use std::collections::BTreeSet;

use health_identifiers::{
    CheckDigit, DerivationName, IdError, NodeId, NumberError, NumberFormat, OpId,
    PLATFORM_NAMESPACE, ResourceId, SequenceState, ShortCode, VersionRule, define_id, derive_id,
    luhn_mod_36,
};

fn rid(text: &str) -> ResourceId {
    text.parse().expect("valid resource id")
}

fn code(text: &str) -> ShortCode {
    text.parse().expect("valid code")
}

#[test]
fn platform_namespace_is_fixed() {
    assert_eq!(
        PLATFORM_NAMESPACE.to_string(),
        "83664680-8814-416c-a4bb-2cc772956887"
    );
}

#[test]
fn extraction_ids_match_golden_values() {
    let response = rid("0192f0a1-7b3c-7d2e-9f10-3a4b5c6d7e8f");
    let mapping = "https://health.example.org/fhir/Library/anc-vitals-mapping";
    let repeated = DerivationName::extraction(
        response,
        mapping,
        "vitals/weight",
        Some("3f9c1d2e"),
        "Observation",
    );
    assert_eq!(
        repeated.as_str(),
        "0192f0a1-7b3c-7d2e-9f10-3a4b5c6d7e8f|https://health.example.org/fhir/Library/anc-vitals-mapping|vitals/weight#3f9c1d2e|Observation"
    );
    assert_eq!(
        derive_id(&repeated).to_string(),
        "61db913f-8e3e-55ae-a818-fa99a2d2dc55"
    );
    let single = DerivationName::extraction(response, mapping, "vitals/bp", None, "Observation");
    assert_eq!(
        derive_id(&single).to_string(),
        "59cbd9eb-1001-5758-bf6e-b6f5547955d4"
    );
}

#[test]
fn mapping_version_does_not_change_the_derived_id() {
    let response = rid("0192f0a1-7b3c-7d2e-9f10-3a4b5c6d7e8f");
    let url = "https://health.example.org/fhir/Library/anc-vitals-mapping";
    let v1 = DerivationName::extraction(
        response,
        &format!("{url}|1.0.0"),
        "vitals/bp",
        None,
        "Observation",
    );
    let v2 = DerivationName::extraction(
        response,
        &format!("{url}|2.3.1"),
        "vitals/bp",
        None,
        "Observation",
    );
    assert_eq!(derive_id(&v1), derive_id(&v2));
    assert_eq!(
        derive_id(&v1).to_string(),
        "59cbd9eb-1001-5758-bf6e-b6f5547955d4"
    );
}

#[test]
fn rule_ids_match_golden_values() {
    let source: OpId = "0192f0a2-0000-7000-8000-000000000001"
        .parse()
        .expect("op id");
    let name = DerivationName::rule(
        source,
        "https://health.example.org/fhir/PlanDefinition/charge-from-dispense|4",
    );
    assert_eq!(
        name.as_str(),
        "0192f0a2-0000-7000-8000-000000000001|https://health.example.org/fhir/PlanDefinition/charge-from-dispense"
    );
    assert_eq!(
        derive_id(&name).to_string(),
        "e3100b06-48be-5177-a45c-9b39c35bf6b0"
    );
}

#[test]
fn derived_names_hash_as_utf8() {
    assert_eq!(
        derive_id(&DerivationName::rule("ünïcödé", "x")).to_string(),
        "c28110f2-aa5c-57c3-85ca-db10839f9206"
    );
}

#[test]
fn derived_ids_are_v5_and_valid_resource_ids() {
    let id = derive_id(&DerivationName::rule("a", "b"));
    assert_eq!(id.as_uuid().get_version_num(), 5);
    assert_eq!(id.to_string().parse::<ResourceId>(), Ok(id));
}

#[test]
fn minted_ids_are_v7_lowercase_hyphenated_and_round_trip() {
    let id = NodeId::new();
    let text = id.to_string();
    assert_eq!(id.as_uuid().get_version_num(), 7);
    assert_eq!(text.len(), 36);
    assert_eq!(text, text.to_lowercase());
    assert_eq!(text.parse::<NodeId>(), Ok(id));
}

#[test]
fn minted_ids_are_distinct() {
    let ids: BTreeSet<OpId> = (0..10_000).map(|_| OpId::new()).collect();
    assert_eq!(ids.len(), 10_000);
}

#[test]
fn only_the_canonical_form_parses() {
    let canonical = "0192f0a1-7b3c-7d2e-9f10-3a4b5c6d7e8f";
    for text in [
        canonical.to_uppercase(),
        canonical.replace('-', ""),
        format!("{{{canonical}}}"),
        format!("urn:uuid:{canonical}"),
        String::from("not-a-uuid"),
    ] {
        assert_eq!(
            text.parse::<ResourceId>(),
            Err(IdError::NotCanonical(text.clone())),
            "{text}"
        );
    }
}

#[test]
fn version_rules_are_enforced() {
    let v4 = "9d3e7c1a-5b2f-4e8d-a1c3-0f2b4d6e8a9c";
    assert!(matches!(
        v4.parse::<ResourceId>(),
        Err(IdError::WrongVersion { version: 4, .. })
    ));
    let v5 = derive_id(&DerivationName::rule("a", "b")).to_string();
    assert!(v5.parse::<ResourceId>().is_ok());
    assert!(matches!(
        v5.parse::<NodeId>(),
        Err(IdError::WrongVersion {
            version: 5,
            rule: VersionRule::V7,
            ..
        })
    ));
}

#[test]
fn ids_serialize_as_strings() {
    let id = rid("0192f0a1-7b3c-7d2e-9f10-3a4b5c6d7e8f");
    let json = serde_json::to_string(&id).expect("serialize");
    assert_eq!(json, "\"0192f0a1-7b3c-7d2e-9f10-3a4b5c6d7e8f\"");
    assert_eq!(
        serde_json::from_str::<ResourceId>(&json).expect("deserialize"),
        id
    );
    assert!(
        serde_json::from_str::<ResourceId>("\"0192F0A1-7B3C-7D2E-9F10-3A4B5C6D7E8F\"").is_err()
    );
}

define_id! {
    /// A Module-declared id type.
    PatientId, VersionRule::V7OrV5
}

#[test]
fn modules_declare_their_own_id_types() {
    let id = PatientId::new();
    assert_eq!(id.to_string().parse::<PatientId>(), Ok(id));
    assert_eq!(PatientId::RULE, VersionRule::V7OrV5);
}

#[test]
fn short_codes_are_validated() {
    assert!("KLA01".parse::<ShortCode>().is_ok());
    for bad in ["", "kla01", "KLA-01", "ABCDEFGHI", "É1"] {
        assert_eq!(
            bad.parse::<ShortCode>(),
            Err(NumberError::InvalidCode(bad.to_owned())),
            "{bad}"
        );
    }
}

#[test]
fn default_format_is_facility_node_sequence() {
    let format = NumberFormat::default();
    assert_eq!(
        format.format(&code("KLA01"), &code("E1"), 2026, 42),
        "KLA01-E1-42"
    );
}

#[test]
fn full_format_with_prefix_year_padding_and_check() {
    let format = NumberFormat {
        prefix: Some(code("MRN")),
        padding: 7,
        check_digit: Some(CheckDigit::LuhnMod36),
        yearly_reset: true,
    };
    let number = format.format(&code("KLA01"), &code("D07"), 2026, 123);
    assert_eq!(&number[..number.len() - 1], "MRN-KLA01-D07-2026-0000123");
}

#[test]
fn luhn_mod_36_matches_golden_values() {
    assert_eq!(luhn_mod_36("UGKLA01E1000042"), 'V');
    assert_eq!(luhn_mod_36("KCCAE10000001"), 'Q');
    assert_eq!(luhn_mod_36("MRN2026D070000123"), 'F');
}

#[test]
fn luhn_mod_36_detects_single_substitutions() {
    let body = "KCCAE10000001";
    let check = luhn_mod_36(body);
    for (i, original) in body.char_indices() {
        for replacement in "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"
            .chars()
            .filter(|c| *c != original)
        {
            let mut altered = body.to_owned();
            altered.replace_range(i..=i, &replacement.to_string());
            assert_ne!(luhn_mod_36(&altered), check, "{altered}");
        }
    }
}

#[test]
fn check_digit_is_appended_over_the_unhyphenated_number() {
    let format = NumberFormat {
        padding: 7,
        check_digit: Some(CheckDigit::LuhnMod36),
        ..NumberFormat::default()
    };
    assert_eq!(
        format.format(&code("KCCA"), &code("E1"), 2026, 1),
        "KCCA-E1-0000001Q"
    );
}

#[test]
fn padding_never_truncates() {
    let format = NumberFormat {
        padding: 3,
        ..NumberFormat::default()
    };
    assert_eq!(
        format.format(&code("F"), &code("N"), 2026, 123_456),
        "F-N-123456"
    );
}

#[test]
fn sequences_start_at_one_and_never_repeat() {
    let format = NumberFormat::default();
    let mut state = SequenceState::default();
    let mut seen = BTreeSet::new();
    for expected in 1..=1_000 {
        let (next, value) = state.next(&format, 2026).expect("next");
        assert_eq!(value, expected);
        assert!(seen.insert(value));
        state = next;
    }
    assert_eq!(
        state,
        SequenceState {
            year: None,
            last: 1_000
        }
    );
}

#[test]
fn yearly_reset_restarts_only_in_a_later_year() {
    let format = NumberFormat {
        yearly_reset: true,
        ..NumberFormat::default()
    };
    let (s1, v1) = SequenceState::default().next(&format, 2026).expect("first");
    let (s2, v2) = s1.next(&format, 2026).expect("second");
    assert_eq!((v1, v2), (1, 2));
    let (s3, v3) = s2.next(&format, 2027).expect("new year");
    assert_eq!(
        (s3, v3),
        (
            SequenceState {
                year: Some(2027),
                last: 1
            },
            1
        )
    );
    assert_eq!(
        s3.next(&format, 2026),
        Err(NumberError::YearWentBack {
            last: 2027,
            asked: 2026
        })
    );
}

#[test]
fn exhausted_counter_is_an_error() {
    let state = SequenceState {
        year: None,
        last: u64::MAX,
    };
    assert_eq!(
        state.next(&NumberFormat::default(), 2026),
        Err(NumberError::Exhausted)
    );
}

#[test]
fn number_format_reads_from_package_json() {
    let format: NumberFormat = serde_json::from_str(
        r#"{"prefix":"MRN","padding":6,"check-digit":"luhn-mod-36","yearly-reset":false}"#,
    )
    .expect("format");
    assert_eq!(
        format.format(&code("KLA01"), &code("E1"), 2026, 9).len(),
        "MRN-KLA01-E1-000009".len() + 1
    );
    assert!(serde_json::from_str::<NumberFormat>(r#"{"prefix":"mrn"}"#).is_err());
    assert!(serde_json::from_str::<NumberFormat>(r#"{"unknown":1}"#).is_err());
}
