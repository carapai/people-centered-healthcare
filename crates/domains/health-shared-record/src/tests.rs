use super::*;

fn coded(text: &str) -> Coded {
    Coded {
        text: text.into(),
        system: Some("http://snomed.info/sct".into()),
        code: Some(text.into()),
    }
}

fn allergy(
    id: &str,
    criticality: Option<&str>,
    clinical: Option<&str>,
    verification: Option<&str>,
    recorded: &str,
) -> Allergy {
    Allergy {
        reference: format!("AllergyIntolerance/{id}"),
        substance: coded(id),
        criticality: criticality.map(Into::into),
        clinical_status: clinical.map(Into::into),
        verification: verification.map(Into::into),
        recorded: Some(recorded.into()),
    }
}

fn refs<T>(items: &Option<Vec<T>>, reference: impl Fn(&T) -> &str) -> Vec<String> {
    items
        .as_ref()
        .expect("section")
        .iter()
        .map(|i| reference(i).to_owned())
        .collect()
}

#[test]
fn current_allergies_with_the_high_criticality_ones_first() {
    let summary = summarize(Record {
        allergies: Some(vec![
            allergy(
                "a1",
                Some("low"),
                Some("active"),
                Some("confirmed"),
                "2026-01-01",
            ),
            allergy("a2", Some("high"), Some("active"), None, "2025-01-01"),
            allergy("a3", None, None, None, "2026-06-01"),
            allergy("gone", Some("high"), Some("resolved"), None, "2026-07-01"),
            allergy(
                "wrong",
                Some("high"),
                Some("active"),
                Some("entered-in-error"),
                "2026-07-01",
            ),
            allergy("no", None, Some("active"), Some("refuted"), "2026-07-01"),
        ]),
        ..Record::default()
    });
    assert_eq!(
        refs(&summary.allergies, |a| &a.reference),
        [
            "AllergyIntolerance/a2",
            "AllergyIntolerance/a3",
            "AllergyIntolerance/a1"
        ]
    );
}

#[test]
fn conditions_medications_and_programs_that_are_current_latest_first() {
    let condition =
        |id: &str, clinical: &str, verification: Option<&str>, onset: Option<&str>| Condition {
            reference: format!("Condition/{id}"),
            code: coded(id),
            clinical_status: Some(clinical.into()),
            verification: verification.map(Into::into),
            onset: onset.map(Into::into),
        };
    let medication = |id: &str, status: &str, authored: &str| Medication {
        reference: format!("MedicationRequest/{id}"),
        medication: coded(id),
        status: status.into(),
        dosage: None,
        authored: Some(authored.into()),
    };
    let program = |id: &str, status: &str| Program {
        reference: format!("EpisodeOfCare/{id}"),
        kind: coded(id),
        status: status.into(),
        start: None,
    };
    let summary = summarize(Record {
        conditions: Some(vec![
            condition("old", "active", None, Some("2020-01-01")),
            condition("new", "relapse", Some("confirmed"), Some("2026-01-01")),
            condition("undated", "recurrence", None, None),
            condition("over", "resolved", None, Some("2026-02-01")),
            condition(
                "mistake",
                "active",
                Some("entered-in-error"),
                Some("2026-02-01"),
            ),
        ]),
        medications: Some(vec![
            medication("m1", "active", "2026-01-01"),
            medication("m2", "on-hold", "2026-03-01"),
            medication("m3", "stopped", "2026-04-01"),
            medication("m4", "completed", "2026-04-01"),
        ]),
        programs: Some(vec![
            program("art", "active"),
            program("tb", "finished"),
            program("anc", "waitlist"),
        ]),
        ..Record::default()
    });
    assert_eq!(
        refs(&summary.conditions, |c| &c.reference),
        ["Condition/new", "Condition/old", "Condition/undated"]
    );
    assert_eq!(
        refs(&summary.medications, |m| &m.reference),
        ["MedicationRequest/m2", "MedicationRequest/m1"]
    );
    assert_eq!(
        refs(&summary.programs, |p| &p.reference),
        ["EpisodeOfCare/art", "EpisodeOfCare/anc"]
    );
}

#[test]
fn the_latest_reading_of_each_vital_and_the_latest_visits() {
    let vital = |id: &str, code: &str, status: &str, effective: &str| Vital {
        reference: format!("Observation/{id}"),
        code: coded(code),
        value: Some("61.50".into()),
        unit: Some("kg".into()),
        status: status.into(),
        effective: Some(effective.into()),
    };
    let visits: Vec<Visit> = (0..12)
        .map(|day| Visit {
            reference: format!("Encounter/e{day}"),
            status: if day == 11 {
                "entered-in-error".into()
            } else {
                "finished".into()
            },
            class: Some("AMB".into()),
            start: Some(format!("2026-11-{:02}", day + 1)),
            location: None,
        })
        .collect();
    let summary = summarize(Record {
        vitals: Some(vec![
            vital("w1", "weight", "final", "2026-10-01"),
            vital("w2", "weight", "final", "2026-11-01"),
            vital("w3", "weight", "entered-in-error", "2026-11-05"),
            vital("t1", "temperature", "final", "2026-09-01"),
            vital("t2", "temperature", "cancelled", "2026-11-05"),
        ]),
        visits: Some(visits),
        ..Record::default()
    });
    assert_eq!(
        refs(&summary.vitals, |v| &v.reference),
        ["Observation/w2", "Observation/t1"]
    );
    let shown = refs(&summary.visits, |v| &v.reference);
    assert_eq!(shown.len(), VISITS);
    assert_eq!(
        shown[0], "Encounter/e10",
        "the one entered in error is left out"
    );
}

#[test]
fn a_section_not_seen_stays_unseen() {
    let summary = summarize(Record {
        allergies: Some(Vec::new()),
        ..Record::default()
    });
    assert_eq!(
        summary.allergies,
        Some(Vec::new()),
        "seen, and nothing current"
    );
    assert_eq!(summary.conditions, None, "not seen at all");
}
