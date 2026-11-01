//! Valuation rebuilt from the movement ledger: weighted average and FIFO, consignment, and
//! increases without a cost.

use health_inventory::valuation::{Layer, Method, value};
use health_inventory::{Movement, MovementKind, Ownership, Status, StockUnit};
use health_time::Timestamp;
use rust_decimal::Decimal;

fn unit(ownership: Ownership) -> StockUnit {
    StockUnit {
        store: "Location/pharmacy".into(),
        product: "amoxicillin-500".into(),
        batch: "B42".into(),
        expiry: "2027-06-30".into(),
        funding_source: "government".into(),
        ownership,
        status: Status::Usable,
    }
}

fn movement(id: &str, kind: MovementKind, quantity: i64, cost: Option<i64>, at: i64) -> Movement {
    Movement {
        id: id.into(),
        unit: unit(Ownership::Owned),
        kind,
        quantity,
        pack_size: 1,
        unit_cost: cost.map(Decimal::from),
        occurred_at: Timestamp::from_second(1_800_000_000 + at).expect("time"),
        recorded_at: Timestamp::from_second(1_800_000_000 + at).expect("time"),
        source_document: "doc".into(),
        actor: "u1".into(),
        node: "E1".into(),
        op_id: id.into(),
        reason: None,
        reverses: None,
        emergency: false,
    }
}

/// 100 at 10, then 100 at 20, then 150 issued.
fn ledger() -> Vec<Movement> {
    vec![
        movement("r1", MovementKind::Receipt, 100, Some(10), 0),
        movement("r2", MovementKind::Receipt, 100, Some(20), 10),
        movement("i1", MovementKind::Issue, -150, None, 20),
    ]
}

#[test]
fn weighted_average_values_issues_at_the_average() {
    let v = value(&unit(Ownership::Owned), &ledger(), Method::WeightedAverage);
    assert_eq!(v.quantity, 50);
    assert_eq!(v.unit_cost, Decimal::from(15));
    assert_eq!(v.value, Decimal::from(750));
    assert_eq!(v.costed[2].value, Decimal::from(-2250));
    assert!(v.layers.is_empty());
}

#[test]
fn fifo_values_issues_at_the_oldest_receipts() {
    let v = value(&unit(Ownership::Owned), &ledger(), Method::Fifo);
    assert_eq!(v.quantity, 50);
    // 100 at 10 and 50 at 20 issued: 2000; 50 at 20 left: 1000.
    assert_eq!(v.costed[2].value, Decimal::from(-2000));
    assert_eq!(v.value, Decimal::from(1000));
    assert_eq!(
        v.layers,
        [Layer {
            quantity: 50,
            unit_cost: Decimal::from(20)
        }]
    );
}

#[test]
fn valuation_rebuilds_from_the_ledger_in_any_order() {
    let mut shuffled = ledger();
    shuffled.reverse();
    for method in [Method::WeightedAverage, Method::Fifo] {
        assert_eq!(
            value(&unit(Ownership::Owned), &shuffled, method),
            value(&unit(Ownership::Owned), &ledger(), method)
        );
    }
}

#[test]
fn consignment_stock_carries_no_value() {
    let v = value(
        &unit(Ownership::Consignment),
        &ledger(),
        Method::WeightedAverage,
    );
    assert_eq!(v.quantity, 50);
    assert_eq!(v.value, Decimal::ZERO);
    assert!(v.costed.iter().all(|c| c.value.is_zero()));
}

#[test]
fn increases_without_a_cost_come_in_at_the_current_cost() {
    let mut movements = ledger();
    // A transfer-in of 50 with no cost: at the average of 15 (weighted), the last layer's 20
    // (FIFO).
    movements.push(movement("t1", MovementKind::TransferIn, 50, None, 30));
    let weighted = value(&unit(Ownership::Owned), &movements, Method::WeightedAverage);
    assert_eq!(weighted.value, Decimal::from(1500));
    let fifo = value(&unit(Ownership::Owned), &movements, Method::Fifo);
    assert_eq!(fifo.value, Decimal::from(2000));
    // A first receipt without a cost has nothing to be costed by.
    let first = vec![movement("r0", MovementKind::Receipt, 10, None, 0)];
    let v = value(&unit(Ownership::Owned), &first, Method::WeightedAverage);
    assert_eq!((v.value, v.uncosted), (Decimal::ZERO, 10));
}

#[test]
fn stock_issued_to_nothing_has_no_value_left() {
    let movements = vec![
        movement("r1", MovementKind::Receipt, 10, Some(5), 0),
        movement("d1", MovementKind::Dispense, -12, None, 10),
    ];
    for method in [Method::WeightedAverage, Method::Fifo] {
        let v = value(&unit(Ownership::Owned), &movements, method);
        assert_eq!(v.quantity, -2);
        assert_eq!(v.value, Decimal::ZERO);
    }
}
