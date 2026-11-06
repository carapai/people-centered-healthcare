use super::*;

fn program(reference: &str, version: &str, start: &str) -> Program {
    Program {
        reference: reference.into(),
        version: version.into(),
        title: format!("{reference} program"),
        status: Some("active".into()),
        start: Some(start.into()),
        facility: Some("KAM013".into()),
    }
}

fn step(reference: &str, version: &str, stage: Stage, at: &str, episode: Option<&str>) -> Step {
    Step {
        reference: reference.into(),
        version: version.into(),
        stage,
        at: at.into(),
        title: format!("{reference} title"),
        status: Some("active".into()),
        episode: episode.map(str::to_owned),
        facility: Some("KAM013".into()),
        waiting: false,
    }
}

fn work(task: &str, version: &str, focus: &str, open: bool) -> Work {
    Work {
        task: task.into(),
        version: version.into(),
        focus: focus.into(),
        open,
    }
}

fn inputs() -> (Vec<Program>, Vec<Step>, Vec<Work>) {
    (
        vec![
            program("EpisodeOfCare/hiv", "a1", "2026-03-01T08:00:00Z"),
            program("EpisodeOfCare/dm", "a2", "2026-01-01T08:00:00Z"),
        ],
        vec![
            step(
                "Encounter/1",
                "b1",
                Stage::Visit,
                "2026-01-05T09:00:00Z",
                Some("EpisodeOfCare/dm"),
            ),
            step(
                "ServiceRequest/2",
                "b2",
                Stage::Order,
                "2026-01-05T09:30:00Z",
                Some("EpisodeOfCare/dm"),
            ),
            step(
                "DiagnosticReport/3",
                "b3",
                Stage::Result,
                "2026-01-06T10:00:00Z",
                Some("EpisodeOfCare/dm"),
            ),
            step(
                "Encounter/4",
                "b4",
                Stage::Visit,
                "2026-03-02T09:00:00Z",
                Some("EpisodeOfCare/hiv"),
            ),
            step(
                "ServiceRequest/5",
                "b5",
                Stage::Referral,
                "2026-03-02T09:30:00Z",
                Some("EpisodeOfCare/hiv"),
            ),
            step(
                "Encounter/6",
                "b6",
                Stage::Visit,
                "2026-02-01T09:00:00Z",
                None,
            ),
            // In a program the reader does not see.
            step(
                "Appointment/7",
                "b7",
                Stage::Appointment,
                "2026-04-01T09:00:00Z",
                Some("EpisodeOfCare/unseen"),
            ),
        ],
        vec![
            work("Task/a", "c1", "ServiceRequest/2", false),
            work("Task/b", "c2", "ServiceRequest/5", true),
            work("Task/c", "c3", "Appointment/7", true),
        ],
    )
}

fn shape(journey: &Journey) -> Vec<(Option<&str>, Vec<(&str, bool)>)> {
    journey
        .stretches
        .iter()
        .map(|s| {
            (
                s.program.as_ref().map(|p| p.reference.as_str()),
                s.steps
                    .iter()
                    .map(|st| (st.reference.as_str(), st.waiting))
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn the_journey_is_arranged_by_program_with_what_waits_on_workflow() {
    let (programs, steps, work) = inputs();
    let built = journey(programs, steps, work);
    assert_eq!(
        shape(&built),
        [
            (
                Some("EpisodeOfCare/dm"),
                vec![
                    ("Encounter/1", false),
                    ("ServiceRequest/2", false),
                    ("DiagnosticReport/3", false)
                ]
            ),
            (
                Some("EpisodeOfCare/hiv"),
                vec![("Encounter/4", false), ("ServiceRequest/5", true)]
            ),
            (None, vec![("Encounter/6", false), ("Appointment/7", true)]),
        ],
        "the earliest program first; care outside a seen program last"
    );
    let waiting: Vec<&str> = built
        .waiting()
        .iter()
        .map(|s| s.reference.as_str())
        .collect();
    assert_eq!(waiting, ["ServiceRequest/5", "Appointment/7"]);
}

#[test]
fn waiting_comes_from_workflow_not_clinical_status() {
    let (programs, mut steps, _) = inputs();
    // A clinically completed order still waits while its Task is open...
    steps[1].status = Some("completed".into());
    let built = journey(
        programs.clone(),
        steps.clone(),
        vec![work("Task/a", "c1", "ServiceRequest/2", true)],
    );
    assert!(built.stretches[0].steps[1].waiting);
    // ...and an active one with no open work does not.
    steps[1].status = Some("active".into());
    let built = journey(
        programs,
        steps,
        vec![
            work("Task/a", "c1", "ServiceRequest/2", true),
            work("Task/a", "c9", "ServiceRequest/2", false),
        ],
    );
    assert!(
        !built.stretches[0].steps[1].waiting,
        "the Task's newer version is done"
    );
}

#[test]
fn the_same_resources_in_any_order_make_the_same_journey() {
    let (programs, steps, work) = inputs();
    let built = journey(programs.clone(), steps.clone(), work.clone());
    let mut reversed = (programs.clone(), steps.clone(), work.clone());
    reversed.0.reverse();
    reversed.1.reverse();
    reversed.2.reverse();
    assert_eq!(journey(reversed.0, reversed.1, reversed.2), built);
    // Given twice, or with an older version of a step alongside, it is the same.
    let mut twice = (programs.clone(), steps.clone(), work.clone());
    twice.0.extend(programs);
    twice.1.extend(steps.clone());
    twice.2.extend(work);
    let mut older = steps[0].clone();
    older.version = "a0".into();
    older.title = "an older title".into();
    twice.1.push(older);
    let (p, s, w) = twice;
    assert_eq!(journey(p, s, w), built);
}

#[test]
fn nothing_makes_an_empty_journey() {
    assert_eq!(
        journey(vec![], vec![], vec![]),
        Journey { stretches: vec![] }
    );
    let lone = journey(
        vec![program("EpisodeOfCare/dm", "a", "2026-01-01T00:00:00Z")],
        vec![],
        vec![],
    );
    assert_eq!(
        shape(&lone),
        [(Some("EpisodeOfCare/dm"), vec![])],
        "a program with no steps yet still shows"
    );
}
