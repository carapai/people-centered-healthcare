//! Stock counts without freezing the store (inventory-and-procurement.md § Stock counts).

use health_inventory::counts::{CountMode, CountRefused, CountStatus, StockCount};
use health_inventory::{Movement, MovementKind, Ownership, Status, StockUnit};
use health_time::Timestamp;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + seconds).expect("time")
}

fn unit(batch: &str) -> StockUnit {
    StockUnit {
        store: "Location/pharmacy".into(),
        product: "amoxicillin-500".into(),
        batch: batch.into(),
        expiry: "2027-06-30".into(),
        funding_source: "government".into(),
        ownership: Ownership::Owned,
        status: Status::Usable,
    }
}

fn issue(batch: &str, quantity: i64, recorded: i64) -> Movement {
    Movement {
        id: format!("m-{batch}-{recorded}"),
        unit: unit(batch),
        kind: MovementKind::Issue,
        quantity,
        pack_size: 1,
        unit_cost: None,
        occurred_at: at(recorded),
        recorded_at: at(recorded),
        source_document: "doc".into(),
        actor: "u1".into(),
        node: "E1".into(),
        op_id: format!("m-{batch}-{recorded}"),
        reason: None,
        reverses: None,
        emergency: false,
    }
}

fn count(mode: CountMode) -> StockCount {
    StockCount::start(
        "c1",
        "Location/pharmacy",
        mode,
        vec![(unit("A"), 10), (unit("B"), 4)],
        at(0),
    )
    .expect("count")
}

#[test]
fn a_blind_count_hides_what_the_ledger_expects() {
    assert_eq!(
        StockCount::start("c", "s", CountMode::Full, vec![], at(0)),
        Err(CountRefused::Empty)
    );
    assert!(
        count(CountMode::Blind)
            .sheet()
            .iter()
            .all(|l| l.expected.is_none())
    );
    assert_eq!(
        count(CountMode::Full)
            .sheet()
            .iter()
            .map(|l| l.expected)
            .collect::<Vec<_>>(),
        [Some(10), Some(4)]
    );
}

#[test]
fn counting_then_submitting() {
    let mut c = count(CountMode::Cycle);
    assert_eq!(c.record(0, -1, None), Err(CountRefused::Negative));
    assert_eq!(c.record(5, 1, None), Err(CountRefused::NoLine(5)));
    c.record(0, 9, Some(" breakage ")).expect("counted");
    assert_eq!(c.submit("u1"), Err(CountRefused::Uncounted(1)));
    c.record(1, 4, None).expect("counted");
    c.submit("u1").expect("submitted");
    assert_eq!(c.status, CountStatus::Submitted);
    assert!(matches!(
        c.record(1, 3, None),
        Err(CountRefused::WrongState { .. })
    ));
    assert_eq!(c.lines[0].reason.as_deref(), Some("breakage"));
}

#[test]
fn movements_during_the_count_are_not_variances() {
    let mut c = count(CountMode::Full);
    // 3 issued from A while counting (after the start), 1 before it (already in the balance).
    let since = vec![issue("A", -3, 10), issue("A", -1, -5)];
    c.record(0, 7, None).expect("counted");
    c.record(1, 3, Some("breakage")).expect("counted");
    let variances = c.variances(&since);
    assert_eq!(
        variances
            .iter()
            .map(|v| (v.expected, v.counted, v.variance))
            .collect::<Vec<_>>(),
        [(7, 7, 0), (4, 3, -1)]
    );
}

#[test]
fn approval_needs_reasons_and_a_supervisor_above_the_threshold() {
    let mut c = count(CountMode::Full);
    c.record(0, 4, None).expect("counted");
    c.record(1, 4, None).expect("counted");
    assert!(matches!(
        c.approve("s1", true, 5, &[]),
        Err(CountRefused::WrongState { .. })
    ));
    c.submit("u1").expect("submitted");
    assert_eq!(
        c.approve("s1", true, 5, &[]),
        Err(CountRefused::ReasonRequired(0))
    );
    c.lines[0].reason = Some("expired, not written off".into());
    assert_eq!(
        c.approve("u2", false, 5, &[]),
        Err(CountRefused::SupervisorRequired(-6))
    );
    let posted = c.approve("s1", true, 5, &[]).expect("approved");
    assert_eq!(posted.iter().map(|v| v.variance).collect::<Vec<_>>(), [-6]);
    assert_eq!(c.status, CountStatus::Approved);
    assert_eq!(c.approved_by.as_deref(), Some("s1"));
}
