use super::*;

fn at() -> Timestamp {
    "2026-11-05T08:00:00Z".parse().expect("timestamp")
}

fn trigger(url: &str, kind: &str, item: ItemSource) -> ChargeTrigger {
    ChargeTrigger {
        url: url.into(),
        version: "1.0.0".into(),
        kind: kind.into(),
        service_points: vec![],
        codes: vec![],
        item,
        account: AccountBasis::Encounter,
        free_care: None,
    }
}

fn triggers() -> Vec<ChargeTrigger> {
    let mut consult = trigger(
        "https://health.go.ug/fhir/ChargeTrigger/opd-consult",
        "encounter.started",
        ItemSource::Fixed("consult".into()),
    );
    consult.service_points = vec!["opd".into()];
    let mut arv = trigger(
        "https://health.go.ug/fhir/ChargeTrigger/arv-free",
        "medication-dispense.completed",
        ItemSource::Event,
    );
    arv.codes = vec!["tdf-3tc-dtg".into()];
    arv.free_care = Some("funding/pepfar".into());
    arv.account = AccountBasis::Episode;
    vec![
        consult,
        trigger(
            "https://health.go.ug/fhir/ChargeTrigger/dispense",
            "medication-dispense.completed",
            ItemSource::Event,
        ),
        arv,
    ]
}

fn encounter_started(service_point: &str) -> ClinicalEvent {
    ClinicalEvent {
        id: "enc-1-start".into(),
        kind: "encounter.started".into(),
        at: at(),
        patient: "Patient/p-1".into(),
        encounter: Some("Encounter/e-1".into()),
        episode: None,
        service_point: Some(service_point.into()),
        items: vec![],
    }
}

fn dispense() -> ClinicalEvent {
    ClinicalEvent {
        id: "disp-1".into(),
        kind: "medication-dispense.completed".into(),
        at: at(),
        patient: "Patient/p-1".into(),
        encounter: Some("Encounter/e-1".into()),
        episode: Some("EpisodeOfCare/art-1".into()),
        service_point: Some("pharmacy".into()),
        items: vec![
            EventItem {
                key: "line-1".into(),
                code: "paracetamol".into(),
                quantity: Decimal::from(20),
            },
            EventItem {
                key: "line-2".into(),
                code: "tdf-3tc-dtg".into(),
                quantity: Decimal::from(30),
            },
        ],
    }
}

#[test]
fn an_event_is_captured_by_the_triggers_that_fire_on_it() {
    let charges = capture(&encounter_started("opd"), &triggers()).expect("captured");
    assert_eq!(charges.len(), 1);
    let consult = &charges[0];
    assert_eq!(consult.item, "consult");
    assert_eq!(consult.account, "Encounter/e-1");
    assert_eq!(consult.status, ChargeStatus::Planned);
    assert_eq!(
        consult.trigger,
        "https://health.go.ug/fhir/ChargeTrigger/opd-consult|1.0.0"
    );
    assert!(
        capture(&encounter_started("ipd"), &triggers())
            .expect("captured")
            .is_empty(),
        "the consultation trigger fires at OPD only"
    );
}

#[test]
fn a_dispense_charges_each_item_and_free_care_goes_to_its_funding_source() {
    let charges = capture(&dispense(), &triggers()).expect("captured");
    let summary: Vec<(&str, &str, Option<&str>)> = charges
        .iter()
        .map(|c| (c.item.as_str(), c.account.as_str(), c.free_care.as_deref()))
        .collect();
    assert_eq!(
        summary,
        [
            ("paracetamol", "Encounter/e-1", None),
            ("tdf-3tc-dtg", "Encounter/e-1", None),
            ("tdf-3tc-dtg", "EpisodeOfCare/art-1", Some("funding/pepfar")),
        ]
    );
    assert_eq!(charges[0].quantity, Decimal::from(20));
}

#[test]
fn replaying_an_event_gives_the_same_charge_ids() {
    let first = capture(&dispense(), &triggers()).expect("captured");
    let mut newer = triggers();
    for trigger in &mut newer {
        trigger.version = "2.0.0".into();
    }
    let replayed = capture(&dispense(), &newer).expect("captured");
    let ids = |charges: &[Charge]| charges.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
    assert_eq!(
        ids(&first),
        ids(&replayed),
        "the trigger's version is not in the id"
    );
    assert_eq!(
        first[0].id,
        charge_id(
            "disp-1",
            "line-1",
            "https://health.go.ug/fhir/ChargeTrigger/dispense"
        )
    );
    let unique: std::collections::BTreeSet<_> = ids(&first).into_iter().collect();
    assert_eq!(unique.len(), 3, "each item and trigger is its own charge");
}

#[test]
fn an_event_without_the_account_its_trigger_needs_is_refused() {
    let mut no_episode = dispense();
    no_episode.episode = None;
    assert!(matches!(
        capture(&no_episode, &triggers()),
        Err(CaptureError::NoAccount {
            basis: AccountBasis::Episode,
            ..
        })
    ));
}

fn price(gross: i64, patient: i64, payers: &[(&str, i64)]) -> ChargePrice {
    ChargePrice {
        gross: Decimal::from(gross),
        patient_share: Decimal::from(patient),
        payer_shares: payers
            .iter()
            .map(|(p, a)| ((*p).to_owned(), Decimal::from(*a)))
            .collect(),
        versions: vec!["kam|2".into()],
        estimate: false,
    }
}

#[test]
fn the_lifecycle_and_cancellation() {
    let charge = capture(&encounter_started("opd"), &triggers()).expect("captured")[0].clone();
    assert_eq!(
        charge.priced(price(10_000, 3_000, &[("Coverage/c", 6_000)])),
        Err(ChargeError::BadPrice),
        "the shares must add up"
    );
    let billable = charge
        .priced(price(10_000, 4_000, &[("Coverage/c", 6_000)]))
        .expect("priced");
    assert_eq!(billable.status, ChargeStatus::Billable);

    let (cancelled, reversal) = billable
        .cancel("encounter entered in error")
        .expect("cancel");
    assert_eq!(
        (cancelled.status, reversal),
        (ChargeStatus::EnteredInError, Reversal::Post)
    );
    let (again, reversal) = cancelled.cancel("again").expect("idempotent");
    assert_eq!((again.status, reversal), (cancelled.status, Reversal::None));

    let (not_billable, reversal) = charge.cancel("never happened").expect("cancel");
    assert_eq!(
        (not_billable.status, reversal),
        (ChargeStatus::NotBillable, Reversal::None)
    );

    let billed = billable.billed().expect("billed");
    assert_eq!(billed.cancel("late"), Err(ChargeError::Billed));
    assert_eq!(
        billed.priced(price(1, 1, &[])),
        Err(ChargeError::WrongStatus(ChargeStatus::Billed))
    );
}

#[test]
fn free_care_leaves_the_patient_nothing() {
    let arv = capture(&dispense(), &triggers()).expect("captured")[2].clone();
    assert_eq!(
        arv.priced(price(30_000, 1_000, &[("funding/pepfar", 29_000)])),
        Err(ChargeError::BadPrice)
    );
    let priced = arv
        .priced(price(30_000, 0, &[("funding/pepfar", 30_000)]))
        .expect("priced at the reference tariff");
    assert_eq!(priced.price.expect("price").patient_share, Decimal::ZERO);
}
