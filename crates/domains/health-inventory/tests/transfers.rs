//! Two-step transfers and Reconciliation Records (inventory-and-procurement.md § Transfers;
//! GSDP §13.2).

use health_inventory::reconciliation::{
    Discrepancy, ReconciliationRecord, ResolveRefused, RootCause,
};
use health_inventory::transfers::{
    Counted, Transfer, TransferLine, TransferRefused, TransferStatus, discrepancies,
};
use health_inventory::{Ownership, Status};
use health_time::Timestamp;

fn line(quantity: i64) -> TransferLine {
    TransferLine {
        product: "amoxicillin-500".into(),
        batch: "B42".into(),
        expiry: "2027-06-30".into(),
        funding_source: "government".into(),
        ownership: Ownership::Owned,
        quantity,
    }
}

fn transfer() -> Transfer {
    Transfer::new(
        "t1",
        "Location/pharmacy",
        "Location/ward",
        None,
        vec![line(10), line(4)],
    )
    .expect("transfer")
}

fn count(line: usize, good: i64, damaged: i64) -> Counted {
    Counted {
        line,
        good,
        damaged,
    }
}

#[test]
fn a_transfer_needs_lines_and_two_stores() {
    assert!(matches!(
        Transfer::new("t", "a", "b", None, vec![]),
        Err(TransferRefused::Invalid(_))
    ));
    assert!(matches!(
        Transfer::new("t", "a", "b", None, vec![line(0)]),
        Err(TransferRefused::Invalid(_))
    ));
    assert!(matches!(
        Transfer::new("t", "a", "a", None, vec![line(1)]),
        Err(TransferRefused::Invalid(_))
    ));
}

#[test]
fn dispatch_then_receive_as_sent() {
    let mut t = transfer();
    let outs = t.dispatch("u1").expect("dispatch");
    assert_eq!(
        outs.iter().map(|l| l.quantity).collect::<Vec<_>>(),
        [-10, -4]
    );
    assert!(outs.iter().all(|l| l.unit.store == "Location/pharmacy"));
    assert_eq!(t.status, TransferStatus::InTransit);
    assert!(matches!(
        t.dispatch("u1"),
        Err(TransferRefused::WrongState { .. })
    ));
    let (ins, found) = t
        .receive(vec![count(0, 10, 0), count(1, 4, 0)], "u2")
        .expect("receive");
    assert!(found.is_empty());
    assert_eq!(ins.iter().map(|l| l.quantity).collect::<Vec<_>>(), [10, 4]);
    assert!(
        ins.iter()
            .all(|l| l.unit.store == "Location/ward" && l.unit.status == Status::Usable)
    );
    assert_eq!(t.status, TransferStatus::Received);
    assert_eq!(t.received_by.as_deref(), Some("u2"));
}

#[test]
fn short_damaged_and_excess_stock_is_found_on_receipt() {
    let mut t = transfer();
    t.dispatch("u1").expect("dispatch");
    // Every line counted once, never negative.
    assert_eq!(
        t.clone().receive(vec![count(0, 10, 0)], "u2"),
        Err(TransferRefused::BadCount)
    );
    assert_eq!(
        t.clone()
            .receive(vec![count(0, 10, 0), count(0, 1, 0)], "u2"),
        Err(TransferRefused::BadCount)
    );
    assert_eq!(
        t.clone()
            .receive(vec![count(0, -1, 0), count(1, 4, 0)], "u2"),
        Err(TransferRefused::BadCount)
    );
    // Line 0: 7 good and 1 damaged of 10; line 1: 5 of 4.
    let (ins, found) = t
        .receive(vec![count(0, 7, 1), count(1, 5, 0)], "u2")
        .expect("receive");
    assert_eq!(ins.len(), 3);
    assert!(
        ins.iter()
            .any(|l| l.unit.status == Status::Quarantine && l.quantity == 1)
    );
    let summary: Vec<(Discrepancy, i64)> =
        found.iter().map(|f| (f.discrepancy, f.quantity)).collect();
    assert_eq!(
        summary,
        [
            (Discrepancy::Damaged, -1),
            (Discrepancy::Short, -2),
            (Discrepancy::Excess, 1)
        ]
    );
    assert_eq!(t.status, TransferStatus::Reconciling);
    assert!(matches!(t.close(false), Err(TransferRefused::Invalid(_))));
    t.close(true).expect("closed");
    assert_eq!(t.status, TransferStatus::Closed);
    // Central's matching of a dispatch with a receipt is the same comparison.
    assert_eq!(discrepancies(&t.lines, &t.to_store, &t.counted), found);
}

#[test]
fn a_direct_transfer_is_within_one_facility_by_one_user() {
    let mut t = transfer();
    let legs = t.direct("u1").expect("direct");
    assert_eq!(legs.iter().map(|l| l.quantity).sum::<i64>(), 0);
    assert!(t.direct);
    assert_eq!(
        (t.dispatched_by.as_deref(), t.received_by.as_deref()),
        (Some("u1"), Some("u1"))
    );
    assert_eq!(t.status, TransferStatus::Received);
    let mut between = Transfer::new(
        "t2",
        "Location/a",
        "Location/b",
        Some(("KAM013".into(), "KLA001".into())),
        vec![line(1)],
    )
    .expect("transfer");
    assert_eq!(between.direct("u1"), Err(TransferRefused::NotDirect));
}

#[test]
fn a_reconciliation_record_is_resolved_once_with_its_root_cause() {
    let at = Timestamp::from_second(1_800_000_000).expect("time");
    let mut record =
        ReconciliationRecord::open("r1", "unit", Discrepancy::Short, -2, "Transfer/t1", at);
    assert_eq!(
        record.resolve(RootCause::Other, " ", "u1", at),
        Err(ResolveRefused::NoteRequired)
    );
    record
        .resolve(RootCause::TransitLoss, "box crushed on the truck", "u1", at)
        .expect("resolved");
    assert_eq!(
        record.resolution.as_ref().map(|r| r.cause),
        Some(RootCause::TransitLoss)
    );
    assert_eq!(
        record.resolve(RootCause::Theft, "", "u2", at),
        Err(ResolveRefused::AlreadyResolved)
    );
}
