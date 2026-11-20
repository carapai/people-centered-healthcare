use super::*;

const ANC: &str = "http://health.go.ug/fhir/Questionnaire/anc-visit";

fn translates() -> Translates {
    Translates {
        package: "ug.moh.anc".into(),
        version: "2.x".into(),
        language: "lg".into(),
    }
}

fn item(link_id: &str) -> Key {
    Key::Item {
        questionnaire: ANC.into(),
        link_id: link_id.into(),
        part: Part::Text,
    }
}

fn entry(key: Key, english: &str, text: &str) -> Entry {
    Entry {
        key,
        english: english.into(),
        text: text.into(),
    }
}

#[test]
fn a_pack_targets_one_major_version_of_one_package_in_one_language() {
    let target = translates();
    assert_eq!(target.major(), Some(2));
    assert_eq!(target.pack_name(), "ug.moh.anc.lg");
    assert_eq!(
        target_problems("ug.moh.anc.lg", PackageKind::Language, false, Some(&target)),
        []
    );
    for version in ["2", "2.1.0", "^2.0.0", "02.x", "x", ".x", "2.X"] {
        let target = Translates {
            version: version.into(),
            ..translates()
        };
        assert_eq!(target.major(), None, "{version}");
    }
}

#[test]
fn every_target_problem_is_reported() {
    assert_eq!(
        target_problems("ug.moh.anc.lg", PackageKind::Language, false, None),
        [PackProblem::NoTarget]
    );
    assert_eq!(
        target_problems(
            "ug.moh.anc",
            PackageKind::Program,
            false,
            Some(&translates())
        ),
        [PackProblem::NotAPack]
    );
    let target = Translates {
        package: "UG.MoH".into(),
        version: "2.1.0".into(),
        language: "Luganda".into(),
    };
    assert_eq!(
        target_problems("ug.moh.anc.lg", PackageKind::Language, true, Some(&target)),
        [
            PackProblem::Target("UG.MoH".into()),
            PackProblem::TargetVersion("2.1.0".into()),
            PackProblem::Language("Luganda".into()),
            PackProblem::Name {
                expected: "UG.MoH.luganda".into(),
                actual: "ug.moh.anc.lg".into()
            },
            PackProblem::Dependencies("UG.MoH@2.1.0".into()),
        ]
    );
}

#[test]
fn language_tags_are_bcp_47_shaped() {
    for good in ["lg", "nyn", "sw", "sw-KE", "en-GB"] {
        assert!(valid_language(good), "{good}");
    }
    for bad in ["", "l", "LG", "luganda", "sw_KE", "sw-", "sw-K"] {
        assert!(!valid_language(bad), "{bad}");
    }
}

#[test]
fn duplicate_and_blank_entries_are_problems() {
    let entries = [
        entry(item("bp"), "Blood pressure", "Puleesa"),
        entry(item("bp"), "Blood pressure", "Puleesa"),
        entry(item("pulse"), "Pulse", " "),
    ];
    assert_eq!(
        entry_problems(&entries),
        [
            PackProblem::Duplicate(item("bp")),
            PackProblem::Blank(item("pulse")),
        ]
    );
}

#[test]
fn a_reworded_target_makes_its_entry_stale_and_a_missing_one_gone() {
    let entries = [
        entry(item("bp"), "Blood pressure", "Puleesa"),
        entry(item("pulse"), "Pulse", "Okukuba kw'omutima"),
        entry(item("removed"), "Old question", "Ekibuuzo ekikadde"),
    ];
    let english = BTreeMap::from([
        (item("bp"), "Blood pressure".to_owned()),
        (item("pulse"), "Pulse rate".to_owned()),
        (item("weight"), "Weight".to_owned()),
    ]);
    let report = report(&entries, &english);
    let stale: Vec<_> = report.stale().collect();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].entry.key, item("pulse"));
    assert_eq!(
        stale[0].status,
        Status::Stale {
            now: "Pulse rate".into()
        }
    );
    let gone: Vec<_> = report.gone().map(|checked| &checked.entry.key).collect();
    assert_eq!(gone, [&item("removed")]);
    // Only the current entry is served; the rest fall back to English.
    assert_eq!(report.served(), BTreeMap::from([(&item("bp"), "Puleesa")]));
    assert_eq!((report.translated, report.strings), (1, 3));
}

#[test]
fn keys_read_and_print_as_their_address() {
    let key: Key = serde_json::from_value(serde_json::json!({
        "kind": "answer", "questionnaire": ANC, "linkId": "visit-type", "code": "first"
    }))
    .expect("key");
    assert_eq!(
        key.to_string(),
        format!("{ANC} item visit-type answer first")
    );
    let section = Key::Section {
        layout: "http://health.go.ug/fhir/layout/anc".into(),
        section: "vitals".into(),
    };
    assert_eq!(
        serde_json::to_value(&section).ok(),
        Some(serde_json::json!({
            "kind": "section", "layout": "http://health.go.ug/fhir/layout/anc", "section": "vitals"
        }))
    );
}
