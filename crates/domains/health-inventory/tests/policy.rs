//! The negative-stock policy, the stock card and rebuilding (ADR 0034).

use health_inventory::{
    Balance, Movement, MovementKind, Origin, Ownership, Policy, Refused, Status, StockUnit,
    rebuild, reversal, stock_card,
};
use health_time::Timestamp;

fn unit() -> StockUnit {
    StockUnit {
        store: "Location/pharmacy".into(),
        product: "amoxicillin-500".into(),
        batch: "B42".into(),
        expiry: "2027-06-30".into(),
        funding_source: "government".into(),
        ownership: Ownership::Owned,
        status: Status::Usable,
    }
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + seconds).expect("time")
}

fn movement(id: &str, kind: MovementKind, quantity: i64) -> Movement {
    Movement {
        id: id.into(),
        unit: unit(),
        kind,
        quantity,
        pack_size: 10,
        unit_cost: None,
        occurred_at: at(0),
        recorded_at: at(0),
        source_document: "doc".into(),
        actor: "u1".into(),
        node: "E1".into(),
        op_id: id.into(),
        reason: None,
        reverses: None,
        emergency: false,
    }
}

const AUTHORITY: Origin = Origin::Authority {
    may_emergency_dispense: false,
};

fn on_hand(n: i64) -> Balance {
    Balance {
        on_hand: n,
        ..Balance::default()
    }
}

#[test]
fn quantities_fit_their_kind_and_adjustments_need_a_reason() {
    let policy = Policy::default();
    assert_eq!(
        policy.decide(
            on_hand(5),
            &movement("m", MovementKind::Receipt, -1),
            AUTHORITY
        ),
        Err(Refused::BadQuantity(MovementKind::Receipt))
    );
    assert_eq!(
        policy.decide(
            on_hand(5),
            &movement("m", MovementKind::Issue, 1),
            AUTHORITY
        ),
        Err(Refused::BadQuantity(MovementKind::Issue))
    );
    assert_eq!(
        policy.decide(
            on_hand(5),
            &movement("m", MovementKind::Adjustment, 0),
            AUTHORITY
        ),
        Err(Refused::BadQuantity(MovementKind::Adjustment))
    );
    assert_eq!(
        policy.decide(
            on_hand(5),
            &movement("m", MovementKind::Adjustment, -1),
            AUTHORITY
        ),
        Err(Refused::ReasonRequired)
    );
    let mut adjusted = movement("m", MovementKind::Adjustment, -1);
    adjusted.reason = Some("damaged".into());
    assert_eq!(
        policy
            .decide(on_hand(5), &adjusted, AUTHORITY)
            .expect("ok")
            .balance
            .on_hand,
        4
    );
}

#[test]
fn issues_beyond_what_is_available_are_blocked_at_the_authority() {
    let policy = Policy::default();
    assert_eq!(
        policy
            .decide(
                on_hand(10),
                &movement("m", MovementKind::Issue, -10),
                AUTHORITY
            )
            .expect("ok")
            .balance
            .on_hand,
        0
    );
    assert_eq!(
        policy.decide(
            on_hand(10),
            &movement("m", MovementKind::Issue, -11),
            AUTHORITY
        ),
        Err(Refused::InsufficientStock { available: 10 })
    );
    // Reserved and held stock is not available.
    let reserved = Balance {
        on_hand: 10,
        reserved: 3,
        on_hold: 2,
        flagged: false,
    };
    assert_eq!(reserved.available(), 5);
    assert_eq!(
        policy.decide(
            reserved,
            &movement("m", MovementKind::TransferOut, -6),
            AUTHORITY
        ),
        Err(Refused::InsufficientStock { available: 5 })
    );
    // A relaxed store allows and flags it.
    let relaxed = Policy {
        allow_negative_issues: true,
        ..Policy::default()
    };
    let decision = relaxed
        .decide(
            on_hand(1),
            &movement("m", MovementKind::Issue, -3),
            AUTHORITY,
        )
        .expect("ok");
    assert_eq!(decision.balance.on_hand, -2);
    assert!(decision.balance.flagged);
}

#[test]
fn an_emergency_dispense_needs_the_permission_and_a_reason_and_is_flagged() {
    let policy = Policy::default();
    let mut dispense = movement("m", MovementKind::Dispense, -2);
    // Without the permission (or without asking for an emergency dispense): blocked.
    assert!(matches!(
        policy.decide(on_hand(1), &dispense, AUTHORITY),
        Err(Refused::InsufficientStock { .. })
    ));
    dispense.emergency = true;
    let permitted = Origin::Authority {
        may_emergency_dispense: true,
    };
    assert_eq!(
        policy.decide(on_hand(1), &dispense, permitted),
        Err(Refused::ReasonRequired)
    );
    dispense.reason = Some("patient in shock, count pending".into());
    let decision = policy
        .decide(on_hand(1), &dispense, permitted)
        .expect("emergency");
    assert!(decision.emergency);
    assert!(decision.balance.flagged);
    assert_eq!(decision.balance.on_hand, -1);
    // A store may forbid emergency dispensing altogether.
    let strict = Policy {
        block_emergency_dispense: true,
        ..Policy::default()
    };
    assert_eq!(
        strict.decide(on_hand(1), &dispense, permitted),
        Err(Refused::EmergencyBlocked)
    );
}

#[test]
fn late_operations_are_appended_and_flag_the_unit_for_reconciliation() {
    let decision = Policy::default()
        .decide(
            on_hand(1),
            &movement("m", MovementKind::Dispense, -4),
            Origin::Late,
        )
        .expect("appended");
    assert_eq!(decision.balance.on_hand, -3);
    assert!(decision.balance.flagged && decision.reconcile);
}

#[test]
fn the_stock_card_orders_by_occurrence_so_backdated_movements_land_in_place() {
    let receipt = movement("r", MovementKind::Receipt, 20);
    let mut issue = movement("i", MovementKind::Issue, -5);
    issue.occurred_at = at(100);
    issue.recorded_at = at(100);
    // Recorded later, but it happened before the issue.
    let mut backdated = movement("b", MovementKind::Issue, -3);
    backdated.occurred_at = at(50);
    backdated.recorded_at = at(200);
    let movements = vec![receipt, issue, backdated];
    let card: Vec<(String, i64)> = stock_card(&movements)
        .into_iter()
        .map(|(m, running)| (m.id.clone(), running))
        .collect();
    assert_eq!(
        card,
        [
            ("r".to_owned(), 20),
            ("b".to_owned(), 17),
            ("i".to_owned(), 12)
        ]
    );
    assert_eq!(rebuild(&movements), 12);
}

#[test]
fn a_reversal_negates_the_movement_and_names_it() {
    let issue = movement("i", MovementKind::Issue, -5);
    let undo = reversal(&issue, "i-rev", at(10), "issued to the wrong ward");
    assert_eq!(undo.kind, MovementKind::Reversal);
    assert_eq!(undo.quantity, 5);
    assert_eq!(undo.reverses.as_deref(), Some("i"));
    assert_eq!(rebuild(&[issue, undo]), 0);
    assert_eq!(
        unit().key(),
        "Location/pharmacy|amoxicillin-500|B42|2027-06-30|government|owned|usable"
    );
}
