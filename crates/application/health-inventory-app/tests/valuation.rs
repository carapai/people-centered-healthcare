//! Valuation over SurrealDB (memory engine), rebuilt from the ledger, and the inventory-
//! accounting postings that follow from it.

use std::sync::Arc;

use health_financial_ledger::{
    COST_OF_GOODS, ChartOfAccounts, ChartTemplate, Currency, INVENTORY, Ledger, PAYABLE,
};
use health_inventory::receiving::{GoodsReceived, ReceivedLine};
use health_inventory::valuation::Method;
use health_inventory::{Movement, MovementKind, Origin, Ownership, Policy, Status, StockUnit};
use health_inventory_app::{Inventory, Sender, inventory_postings};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SignedDuration, SystemClock, Timestamp};
use rust_decimal::Decimal;

async fn inventory() -> Inventory<SurrealStore> {
    let settings = Settings {
        target: Target::Memory,
        namespace: "valuation".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    Inventory::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        Policy::default(),
    )
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000).expect("time") + SignedDuration::from_secs(seconds)
}

fn sender() -> Sender {
    Sender {
        actor: "pharm-1".into(),
        node: "E1".into(),
    }
}

fn received(id: &str, ownership: Ownership, cost: i64) -> GoodsReceived {
    GoodsReceived {
        id: id.into(),
        store: "Location/pharmacy".into(),
        supplier: "NMS".into(),
        delivery: id.into(),
        lines: vec![ReceivedLine {
            product: "amoxicillin-500".into(),
            batch: "B42".into(),
            expiry: "2027-06-30".into(),
            funding_source: "global-fund".into(),
            ownership,
            quantity: 100,
            pack_size: 1,
            unit_cost: Decimal::from(cost),
        }],
    }
}

fn unit() -> StockUnit {
    StockUnit {
        store: "Location/pharmacy".into(),
        product: "amoxicillin-500".into(),
        batch: "B42".into(),
        expiry: "2027-06-30".into(),
        funding_source: "global-fund".into(),
        ownership: Ownership::Owned,
        status: Status::Usable,
    }
}

async fn stocked() -> Inventory<SurrealStore> {
    let inventory = inventory().await;
    inventory
        .receive_goods(&received("grn-1", Ownership::Owned, 10), &sender(), at(0))
        .await
        .expect("grn-1");
    inventory
        .receive_goods(&received("grn-2", Ownership::Owned, 20), &sender(), at(10))
        .await
        .expect("grn-2");
    let issue = Movement {
        id: "issue-1".into(),
        unit: unit(),
        kind: MovementKind::Issue,
        quantity: -150,
        pack_size: 1,
        unit_cost: None,
        occurred_at: at(20),
        recorded_at: at(20),
        source_document: "doc".into(),
        actor: "u1".into(),
        node: "E1".into(),
        op_id: "issue-1".into(),
        reason: None,
        reverses: None,
        emergency: false,
    };
    inventory
        .record(
            &issue,
            Origin::Authority {
                may_emergency_dispense: false,
            },
        )
        .await
        .expect("issued");
    inventory
}

#[tokio::test]
async fn valuation_rebuilds_from_the_ledger_by_either_method() {
    let inventory = stocked().await;
    let weighted = inventory
        .valuation("Location/pharmacy", Method::WeightedAverage)
        .await
        .expect("valued");
    assert_eq!(weighted.total, Decimal::from(750));
    let fifo = inventory
        .valuation("Location/pharmacy", Method::Fifo)
        .await
        .expect("valued");
    assert_eq!(fifo.total, Decimal::from(1000));
    // A projection: asking again gives the same.
    assert_eq!(
        inventory
            .valuation("Location/pharmacy", Method::WeightedAverage)
            .await
            .expect("valued"),
        weighted
    );
}

#[tokio::test]
async fn inventory_accounting_posts_each_costed_movement_and_balances() {
    let inventory = stocked().await;
    // Consignment stock received beside it is not owned: it posts nothing.
    let mut consigned = received("grn-3", Ownership::Consignment, 99);
    consigned.lines[0].batch = "C7".into();
    inventory
        .receive_goods(&consigned, &sender(), at(30))
        .await
        .expect("grn-3");
    let valued = inventory
        .valuation("Location/pharmacy", Method::WeightedAverage)
        .await
        .expect("valued");
    assert_eq!(
        valued.total,
        Decimal::from(750),
        "consignment carries no value"
    );

    let ugx = Currency::from_code("UGX").expect("currency");
    let chart = ChartOfAccounts::from_template(&ChartTemplate {
        inventory: true,
        ..ChartTemplate::default()
    })
    .expect("chart");
    let mut ledger = Ledger::new("KAM013", ugx, chart);
    let mut posted = 0;
    for (unit, valuation) in &valued.units {
        for posting in inventory_postings(unit, &valuation.costed, "KAM013", ugx, at(40)) {
            ledger.post(posting).expect("posted");
            posted += 1;
        }
    }
    assert_eq!(
        posted, 3,
        "two receipts and an issue; the consignment receipt posts nothing"
    );
    assert_eq!(ledger.trial_balance(), Decimal::ZERO);
    let all = |_: &health_financial_ledger::Dimensions| true;
    assert_eq!(ledger.balance(INVENTORY, all).amount(), Decimal::from(750));
    assert_eq!(
        ledger.balance(COST_OF_GOODS, all).amount(),
        Decimal::from(2250)
    );
    assert_eq!(ledger.balance(PAYABLE, all).amount(), Decimal::from(3000));
    assert!(ledger.postings().iter().all(|p| {
        p.lines
            .iter()
            .all(|l| l.dimensions.funding_source.as_deref() == Some("global-fund"))
    }));
}
