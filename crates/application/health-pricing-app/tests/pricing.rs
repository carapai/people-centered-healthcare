//! Pricing at central over SurrealDB 3.3.5 (in-memory): drafting, one approver who is not the
//! author, retirement that never rewrites the past, and charges priced from the kept versions.

use std::sync::Arc;

use health_pricing::{
    Adjustment, Applicability, Approval, Effective, Entry, Kind, LifecycleError, Line, PriceBook,
    PriceRule, Request, Scope, Stage,
};
use health_pricing_app::{Pricing, PricingAppError};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{Clock, FixedClock, Timestamp};
use rust_decimal::Decimal;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

async fn pricing() -> Pricing<SurrealStore> {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "pricing".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(at("2026-11-04T09:00:00Z")));
    Pricing::new(SurrealStore::new(connection, clock.clone()), clock, "UGX")
}

fn book(price: i64, from: &str) -> PriceBook {
    PriceBook {
        id: "kam".into(),
        version: 0,
        scope: Scope::Facility("KAM013".into()),
        currency: "UGX".into(),
        effective: Effective {
            from: at(from),
            to: None,
        },
        approval: Approval::Draft,
        author: String::new(),
        entries: vec![Entry {
            item: "consult".into(),
            unit: Decimal::from(price),
            tiers: vec![],
        }],
    }
}

fn surcharge() -> PriceRule {
    PriceRule {
        id: "after-hours".into(),
        version: 0,
        stage: Stage::Surcharge,
        priority: 1,
        items: vec![],
        applicability: Some("%afterHours".into()),
        adjustment: Adjustment::Percent {
            percent: Decimal::from(20),
        },
        effective: Effective {
            from: at("2026-01-01T00:00:00Z"),
            to: None,
        },
        approval: Approval::Draft,
        author: String::new(),
    }
}

struct AfterHours;

impl Applicability for AfterHours {
    fn applies(&self, expression: &str, _: &Line) -> Result<bool, String> {
        Ok(expression == "%afterHours")
    }
}

fn consult(when: &str) -> Request {
    Request {
        service_time: at(when),
        currency: "UGX".into(),
        minor_units: 0,
        scopes: vec![Scope::Facility("KAM013".into())],
        lines: vec![Line {
            id: "L1".into(),
            item: "consult".into(),
            quantity: Decimal::ONE,
        }],
        coverages: vec![],
        overrides: vec![],
    }
}

#[tokio::test]
async fn a_book_is_drafted_approved_by_someone_else_and_superseded_without_rewriting_history() {
    let pricing = pricing().await;
    let first = pricing
        .draft_book(book(10_000, "2026-01-01T00:00:00Z"), "author-1")
        .await
        .expect("draft");
    assert_eq!(first.version, 1);
    let redrafted = pricing
        .draft_book(book(11_000, "2026-01-01T00:00:00Z"), "author-1")
        .await
        .expect("redraft");
    assert_eq!(
        redrafted.version, 1,
        "a draft is replaced while it is a draft"
    );

    assert_eq!(
        pricing.approve_book("kam", 1, "author-1").await,
        Err(PricingAppError::Lifecycle(LifecycleError::OwnApproval))
    );
    pricing
        .approve_book("kam", 1, "finance-1")
        .await
        .expect("approved");
    assert_eq!(
        pricing.approve_book("kam", 1, "finance-2").await,
        Err(PricingAppError::Lifecycle(LifecycleError::NotDraft))
    );

    let second = pricing
        .draft_book(book(12_000, "2026-12-01T00:00:00Z"), "author-1")
        .await
        .expect("draft v2");
    assert_eq!(second.version, 2);
    assert!(matches!(
        pricing.approve_book("kam", 2, "finance-1").await,
        Err(PricingAppError::Lifecycle(LifecycleError::Overlaps(_)))
    ));
    assert!(matches!(
        pricing
            .retire_book("kam", 1, at("2026-06-01T00:00:00Z"))
            .await,
        Err(PricingAppError::Lifecycle(LifecycleError::BadRetirement))
    ));
    pricing
        .retire_book("kam", 1, at("2026-12-01T00:00:00Z"))
        .await
        .expect("retired from December");
    pricing
        .approve_book("kam", 2, "finance-1")
        .await
        .expect("v2 approved from December");

    let price = |when: &'static str| {
        let pricing = &pricing;
        async move {
            pricing
                .price(&consult(when), &AfterHours)
                .await
                .expect("priced")
                .lines[0]
                .gross
        }
    };
    assert_eq!(price("2026-11-04T10:00:00Z").await, Decimal::from(11_000));
    assert_eq!(price("2026-12-02T10:00:00Z").await, Decimal::from(12_000));

    let (books, _) = pricing.catalogue().await.expect("catalogue");
    assert_eq!(books.len(), 2, "every version is kept");
}

#[tokio::test]
async fn an_approved_rule_prices_charges_and_the_authority_checks_the_versions() {
    let pricing = pricing().await;
    pricing
        .draft_book(book(10_000, "2026-01-01T00:00:00Z"), "author-1")
        .await
        .expect("draft");
    pricing
        .approve_book("kam", 1, "finance-1")
        .await
        .expect("approve");
    pricing
        .draft_rule(surcharge(), "author-1")
        .await
        .expect("draft rule");

    let before = pricing
        .price(&consult("2026-11-04T20:00:00Z"), &AfterHours)
        .await
        .expect("priced");
    assert_eq!(
        before.lines[0].gross,
        Decimal::from(10_000),
        "a draft rule does not apply"
    );

    pricing
        .approve_rule("after-hours", 1, "finance-1")
        .await
        .expect("approve rule");
    let priced = pricing
        .price(&consult("2026-11-04T20:00:00Z"), &AfterHours)
        .await
        .expect("priced");
    let line = &priced.lines[0];
    assert_eq!(line.gross, Decimal::from(12_000));
    assert_eq!(line.components[1].kind, Kind::Surcharge);
    assert_eq!(
        pricing
            .check(&priced, at("2026-11-04T20:00:00Z"))
            .await
            .expect("check"),
        vec![]
    );
    assert_eq!(
        pricing.approve_rule("night", 1, "finance-1").await,
        Err(PricingAppError::NotFound("night|1".into()))
    );
}

async fn definition(pricing_store: &SurrealStore, id: &str) -> serde_json::Value {
    use health_persistence::{ResourceType, Store, Transaction};
    let kind: ResourceType = "ChargeItemDefinition".parse().expect("type");
    let id = health_identifiers::ResourceId::from_uuid(id.parse().expect("uuid")).expect("id");
    let mut tx = pricing_store.begin().await.expect("begin");
    let head = tx.read(&kind, id).await.expect("read").expect("projected");
    let issues = health_fhir_validation::validate_structure(head.json.as_str());
    assert!(issues.is_empty(), "valid FHIR R5: {issues:?}");
    serde_json::from_str(head.json.as_str()).expect("json")
}

#[tokio::test]
async fn approved_versions_are_projected_as_charge_item_definitions() {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "pricing-fhir".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(at("2026-11-04T09:00:00Z")));
    let store = SurrealStore::new(connection, clock.clone());
    let pricing = Pricing::new(store.clone(), clock, "UGX");

    let mut tiered = book(500, "2026-01-01T00:00:00Z");
    tiered.entries[0].tiers = vec![health_pricing::Tier {
        min: Decimal::from(10),
        unit: Decimal::new(4005, 1),
    }];
    let draft = pricing.draft_book(tiered, "author-1").await.expect("draft");
    let id = health_pricing_app::fhir::entry_id(&draft, "consult");
    {
        use health_persistence::{ResourceType, Store, Transaction};
        let kind: ResourceType = "ChargeItemDefinition".parse().expect("type");
        let rid = health_identifiers::ResourceId::from_uuid(id.parse().expect("uuid")).expect("id");
        let mut tx = store.begin().await.expect("begin");
        assert!(
            tx.read(&kind, rid).await.expect("read").is_none(),
            "drafts are not projected"
        );
    }
    let approved = pricing
        .approve_book("kam", 1, "finance-1")
        .await
        .expect("approve");
    let projected = definition(&store, &id).await;
    assert_eq!(projected["status"], "active");
    assert_eq!(projected["version"], "1");
    assert_eq!(projected["code"]["text"], "consult");
    assert_eq!(
        projected["propertyGroup"][0]["priceComponent"][0],
        serde_json::json!({ "type": "base", "amount": { "value": 500, "currency": "UGX" } })
    );
    assert_eq!(
        projected["propertyGroup"][1]["applicability"][0]["condition"]["expression"],
        "%quantity >= 10"
    );
    assert_eq!(
        projected["propertyGroup"][1]["priceComponent"][0]["amount"]["value"],
        serde_json::json!(400.5)
    );
    assert_eq!(projected["extension"][0]["valueString"], "facility|KAM013");

    pricing
        .retire_book("kam", 1, at("2026-12-01T00:00:00Z"))
        .await
        .expect("retire");
    assert_eq!(
        definition(&store, &id).await["applicability"][0]["effectivePeriod"]["end"],
        "2026-12-01T00:00:00Z"
    );
    assert_eq!(approved.version, 1);

    pricing
        .draft_rule(surcharge(), "author-1")
        .await
        .expect("draft rule");
    let rule = pricing
        .approve_rule("after-hours", 1, "finance-1")
        .await
        .expect("approve rule");
    let projected = definition(&store, &health_pricing_app::fhir::rule_id(&rule)).await;
    let group = &projected["propertyGroup"][0];
    assert_eq!(
        group["applicability"][0]["condition"]["expression"],
        "%afterHours"
    );
    assert_eq!(
        group["priceComponent"][0],
        serde_json::json!({ "type": "surcharge", "factor": 0.2 })
    );
    assert_eq!(projected["extension"][0]["valueCode"], "surcharge");
    assert_eq!(projected["extension"][1]["valueInteger"], 1);
}
