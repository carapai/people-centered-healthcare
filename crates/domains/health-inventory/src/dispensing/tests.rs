use std::collections::BTreeMap;

use health_time::{SignedDuration, Timestamp};
use jiff::civil::date;

use super::*;
use crate::Ownership;

fn unit(batch: &str, expiry: &str, funding: &str) -> StockUnit {
    StockUnit {
        store: "KAM013-pharmacy".into(),
        product: "amoxicillin-500".into(),
        batch: batch.into(),
        expiry: expiry.into(),
        funding_source: funding.into(),
        ownership: Ownership::Owned,
        status: Status::Usable,
    }
}

fn candidate(batch: &str, expiry: &str, funding: &str, available: i64) -> Candidate {
    Candidate {
        unit: unit(batch, expiry, funding),
        available,
    }
}

fn need(quantity: i64, programs: &[&str]) -> Need {
    Need {
        product: "amoxicillin-500".into(),
        quantity,
        course_ends: date(2026, 11, 13),
        programs: programs.iter().map(|p| (*p).to_owned()).collect(),
    }
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

const BUFFER: BatchRule = BatchRule { buffer_days: 30 };

fn batches(picks: &[Pick]) -> Vec<(&str, i64)> {
    picks
        .iter()
        .map(|p| (p.unit.batch.as_str(), p.quantity))
        .collect()
}

#[test]
fn the_earliest_expiry_that_lasts_the_course_is_taken_first() {
    let stock = [
        candidate("LATE", "2027-12-31", "gou", 100),
        candidate("SOON", "2027-01-31", "gou", 10),
        // Expires within the buffer after the course: never chosen.
        candidate("EXPIRING", "2026-12-01", "gou", 500),
        candidate("TIE-BIG", "2027-01-31", "gou", 50),
    ];
    let picks = fefo(&stock, &need(15, &[]), &Eligibility::default(), BUFFER).expect("picked");
    assert_eq!(
        batches(&picks),
        [("SOON", 10), ("TIE-BIG", 5)],
        "earliest expiry, the smaller of a tie first, across batches when one is short"
    );
    assert_eq!(
        fefo(&stock, &need(200, &[]), &Eligibility::default(), BUFFER),
        Err(SelectRefused::NotEnough { usable: 160 })
    );
    assert_eq!(
        fefo(&stock, &need(0, &[]), &Eligibility::default(), BUFFER),
        Err(SelectRefused::BadQuantity)
    );
}

#[test]
fn donor_stock_goes_only_to_eligible_patients() {
    let stock = [
        candidate("DONOR", "2027-01-31", "pepfar", 100),
        candidate("GOU", "2027-06-30", "gou", 100),
    ];
    let rules = Eligibility::new([("pepfar", "hiv")]);
    let anyone = fefo(&stock, &need(10, &[]), &rules, BUFFER).expect("picked");
    assert_eq!(
        batches(&anyone),
        [("GOU", 10)],
        "the donor batch, though earlier, is not theirs"
    );
    let enrolled = fefo(&stock, &need(10, &["hiv"]), &rules, BUFFER).expect("picked");
    assert_eq!(batches(&enrolled), [("DONOR", 10)]);
    // Not even a pharmacist's own choice reaches it.
    let donor = [Pick {
        unit: unit("DONOR", "2027-01-31", "pepfar"),
        quantity: 10,
    }];
    assert!(matches!(
        chosen(
            &stock,
            &donor,
            &need(10, &[]),
            &rules,
            BUFFER,
            Some("asked for")
        ),
        Err(SelectRefused::NotEligible(_))
    ));
}

#[test]
fn quarantined_and_other_products_are_never_candidates() {
    let mut held = candidate("HELD", "2027-01-31", "gou", 100);
    held.unit.status = Status::Quarantine;
    let mut other = candidate("OTHER", "2027-01-31", "gou", 100);
    other.unit.product = "paracetamol-500".into();
    assert_eq!(
        fefo(
            &[held, other],
            &need(1, &[]),
            &Eligibility::default(),
            BUFFER
        ),
        Err(SelectRefused::NotEnough { usable: 0 })
    );
}

#[test]
fn overriding_first_expiry_needs_a_reason() {
    let stock = [
        candidate("SOON", "2027-01-31", "gou", 100),
        candidate("LATE", "2027-12-31", "gou", 100),
    ];
    let late = [Pick {
        unit: unit("LATE", "2027-12-31", "gou"),
        quantity: 10,
    }];
    let rules = Eligibility::default();
    assert_eq!(
        chosen(&stock, &late, &need(10, &[]), &rules, BUFFER, None),
        Err(SelectRefused::ReasonRequired)
    );
    assert_eq!(
        chosen(
            &stock,
            &late,
            &need(10, &[]),
            &rules,
            BUFFER,
            Some("patient's own brand")
        ),
        Ok(late.to_vec())
    );
    let soon = fefo(&stock, &need(10, &[]), &rules, BUFFER).expect("picked");
    assert_eq!(
        chosen(&stock, &soon, &need(10, &[]), &rules, BUFFER, None),
        Ok(soon.clone()),
        "the first-expiry choice needs no reason"
    );
    assert_eq!(
        chosen(&stock, &late, &need(11, &[]), &rules, BUFFER, Some("x")),
        Err(SelectRefused::BadQuantity),
        "the choice must meet the need"
    );
    let too_much = [Pick {
        unit: unit("LATE", "2027-12-31", "gou"),
        quantity: 101,
    }];
    assert_eq!(
        chosen(
            &stock,
            &too_much,
            &need(101, &[]),
            &rules,
            BUFFER,
            Some("x")
        ),
        Err(SelectRefused::NotEnough { usable: 100 })
    );
}

#[test]
fn reservations_hold_stock_only_while_live() {
    let start = at("2026-11-06T08:00:00Z");
    let dispense = Reservation::hold(
        "r1",
        &unit("SOON", "2027-01-31", "gou"),
        10,
        Purpose::Dispense,
        "MedicationDispense/d1",
        start,
    );
    let pick = Reservation::hold(
        "r2",
        &unit("SOON", "2027-01-31", "gou"),
        5,
        Purpose::TransferPick,
        "Transfer/t1",
        start,
    );
    assert_eq!(dispense.expires, at("2026-11-06T12:00:00Z"));
    assert_eq!(pick.expires, at("2026-11-09T08:00:00Z"));
    let both = [dispense.clone(), pick.clone()];
    assert_eq!(reserved(&both, start), 15);
    let later = start
        .checked_add(SignedDuration::from_hours(5))
        .expect("later");
    assert_eq!(
        reserved(&both, later),
        5,
        "the dispense's lapsed; the pick's holds"
    );
    let released = Reservation {
        state: ReservationState::Released,
        ..pick
    };
    assert_eq!(reserved(&[dispense, released], start), 10);
}

#[test]
fn completing_a_dispense_consumes_its_reservations_in_full_or_in_part() {
    let start = at("2026-11-06T08:00:00Z");
    let held = [
        Reservation::hold(
            "r1",
            &unit("SOON", "2027-01-31", "gou"),
            10,
            Purpose::Dispense,
            "MedicationDispense/d1",
            start,
        ),
        Reservation::hold(
            "r2",
            &unit("TIE", "2027-01-31", "gou"),
            5,
            Purpose::Dispense,
            "MedicationDispense/d1",
            start,
        ),
    ];
    let given = BTreeMap::from([("r1".to_owned(), 10), ("r2".to_owned(), 2)]);
    let settled = settle(&held, &given).expect("settled");
    assert_eq!(
        settled
            .iter()
            .map(|s| (s.reservation.id.as_str(), s.given, s.reservation.state))
            .collect::<Vec<_>>(),
        [
            ("r1", 10, ReservationState::Consumed),
            ("r2", 2, ReservationState::Consumed)
        ],
        "a partial dispense gives 2 of 5; the other 3 go back to available"
    );
    assert_eq!(
        settle(&held, &BTreeMap::from([("r1".to_owned(), 11)])),
        Err(DispenseRefused::Exceeds("r1".into()))
    );
    assert_eq!(
        settle(&held, &BTreeMap::from([("r9".to_owned(), 1)])),
        Err(DispenseRefused::NotHeld("r9".into()))
    );
    assert_eq!(
        settle(&held, &BTreeMap::new()),
        Err(DispenseRefused::Nothing)
    );
}

#[test]
fn substitutions_need_the_right_approval() {
    use Approval::{Dispenser, Pharmacist, Prescriber};
    use Substitution::{Brand, Generic, Pack, Therapeutic};
    assert_eq!(may_substitute(Generic, Dispenser), Ok(()));
    assert_eq!(
        may_substitute(Brand, Dispenser),
        Err(SubstitutionRefused::NeedsPharmacist)
    );
    assert_eq!(may_substitute(Pack, Pharmacist), Ok(()));
    assert_eq!(
        may_substitute(Therapeutic, Pharmacist),
        Err(SubstitutionRefused::NeedsPrescriber)
    );
    assert_eq!(may_substitute(Therapeutic, Prescriber), Ok(()));
}

#[test]
fn a_return_goes_to_quarantine() {
    let back = returned(&unit("SOON", "2027-01-31", "gou"));
    assert_eq!(back.status, Status::Quarantine);
    assert_eq!(back.batch, "SOON");
}
