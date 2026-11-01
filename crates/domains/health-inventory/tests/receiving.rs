//! Goods received notes, GS1 DataMatrix, requisitions and min/max reorder suggestions.

use health_inventory::receiving::{
    GoodsReceived, ReceivedLine, ReceivingRefused, ReorderPolicy, RequestedLine, Requisition,
    RequisitionStatus, parse_gs1, suggest,
};
use health_inventory::{Ownership, Status};
use rust_decimal::Decimal;

fn line() -> ReceivedLine {
    ReceivedLine {
        product: "amoxicillin-500".into(),
        batch: "B42".into(),
        expiry: "2027-06-30".into(),
        funding_source: "government".into(),
        ownership: Ownership::Owned,
        quantity: 1000,
        pack_size: 100,
        unit_cost: Decimal::new(150, 0),
    }
}

fn note(lines: Vec<ReceivedLine>) -> GoodsReceived {
    GoodsReceived {
        id: "grn-1".into(),
        store: "Location/pharmacy".into(),
        supplier: "NMS".into(),
        delivery: "INV-2026-0042".into(),
        lines,
    }
}

#[test]
fn a_goods_received_note_is_complete_before_it_is_received() {
    assert_eq!(note(vec![line()]).validate(), Ok(()));
    assert_eq!(line().unit("Location/pharmacy").status, Status::Usable);
    let mut blank = note(vec![line()]);
    blank.supplier = " ".into();
    assert!(matches!(
        blank.validate(),
        Err(ReceivingRefused::Invalid(_))
    ));
    assert!(matches!(
        note(vec![]).validate(),
        Err(ReceivingRefused::Invalid(_))
    ));
    for broken in [
        ReceivedLine {
            batch: String::new(),
            ..line()
        },
        ReceivedLine {
            quantity: 0,
            ..line()
        },
        ReceivedLine {
            pack_size: 0,
            ..line()
        },
        ReceivedLine {
            unit_cost: Decimal::new(-1, 0),
            ..line()
        },
    ] {
        assert!(matches!(
            note(vec![broken]).validate(),
            Err(ReceivingRefused::Invalid(_))
        ));
    }
}

#[test]
fn gs1_datamatrix_as_scanned_and_as_printed() {
    let scanned = parse_gs1("]d20109501101530003172706301012AB34\u{1d}21SN789").expect("scanned");
    assert_eq!(scanned.gtin.as_deref(), Some("09501101530003"));
    assert_eq!(scanned.expiry.as_deref(), Some("2027-06-30"));
    assert_eq!(scanned.batch.as_deref(), Some("12AB34"));
    assert_eq!(scanned.serial.as_deref(), Some("SN789"));
    let printed = parse_gs1("(01)09501101530003(17)280200(10)LOT7").expect("printed");
    assert_eq!(
        printed.expiry.as_deref(),
        Some("2028-02-29"),
        "day 00 is the month's last"
    );
    assert_eq!(printed.batch.as_deref(), Some("LOT7"));
    assert_eq!(printed.serial, None);
    for bad in [
        "0195011015",
        "(17)271330",
        "(99)123",
        "(10)",
        "01095011015300031727",
    ] {
        assert!(
            matches!(parse_gs1(bad), Err(ReceivingRefused::Barcode(_))),
            "{bad}"
        );
    }
}

#[test]
fn a_requisition_is_approved_then_fulfilled_by_a_transfer() {
    let lines = vec![RequestedLine {
        product: "amoxicillin-500".into(),
        quantity: 200,
    }];
    assert!(Requisition::new("r", "Location/ward", "Location/ward", lines.clone(), "n1").is_err());
    assert!(Requisition::new("r", "a", "b", vec![], "n1").is_err());
    let mut r = Requisition::new(
        "r1",
        "Location/ward",
        "Location/pharmacy",
        lines.clone(),
        "nurse-1",
    )
    .expect("requisition");
    assert!(matches!(
        r.fulfil("t1"),
        Err(ReceivingRefused::WrongState { .. })
    ));
    r.approve("pharm-1").expect("approved");
    r.fulfil("t1").expect("fulfilled");
    assert_eq!(r.status, RequisitionStatus::Fulfilled);
    assert_eq!(r.transfer.as_deref(), Some("t1"));
    let mut refused =
        Requisition::new("r2", "Location/ward", "Location/pharmacy", lines, "nurse-1")
            .expect("requisition");
    refused.reject("pharm-1").expect("rejected");
    assert!(matches!(
        refused.approve("pharm-1"),
        Err(ReceivingRefused::WrongState { .. })
    ));
}

#[test]
fn reorder_suggestions_order_up_to_the_maximum_at_the_minimum() {
    let policy = ReorderPolicy::default();
    // 300 used in 30 days: 300 a month; min 300, max 900.
    let low = suggest("amox", 250, 300, 30, policy);
    assert_eq!((low.min, low.max, low.order), (300, 900, 650));
    assert_eq!(low.monthly, Decimal::new(300, 0));
    let enough = suggest("amox", 301, 300, 30, policy);
    assert_eq!(enough.order, 0);
    // 100 used in 90 days: 33.33 a month, levels rounded up.
    let slow = suggest("ors", 10, 100, 90, policy);
    assert_eq!((slow.min, slow.max, slow.order), (34, 100, 90));
    // No consumption: nothing to suggest from.
    assert_eq!(suggest("new", 0, 0, 30, policy).order, 0);
    assert_eq!(suggest("x", 0, 10, 0, policy).order, 0);
}
