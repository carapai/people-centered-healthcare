//! The registry review queue over SurrealDB: registration matching, reviewers' decisions, the
//! ageing alert, and the rule that a separated pair is never re-linked automatically.

use std::sync::{Arc, Mutex};

use health_client_registry::matching::MatchingRules;
use health_client_registry::{Decision, Field, ReviewKind, ReviewRefused};
use health_fhir_search::Registry;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, Store, Transaction, WriteError, WriteRequest};
use health_registries_app::fhir::{RegistryConfig, linked};
use health_registries_app::matching::RegistryBlocking;
use health_registries_app::review::{Registered, RegistryReview};
use health_registries_app::{AuditTrail, ClientRegistry, RegistryError};
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealStore, SurrealTx, Target, migrate,
};
use health_time::{FixedClock, SignedDuration, SystemClock, Timestamp};
use serde_json::{Value, json};

const NIN: &str = "urn:health:synthetic-data/nin";

#[derive(Default, Clone)]
struct Recorded(Arc<Mutex<Vec<(String, String)>>>);

impl AuditTrail<SurrealTx> for Recorded {
    async fn written(
        &self,
        _tx: &mut SurrealTx,
        actor: &str,
        reason: &str,
        _json: &RawJson,
    ) -> Result<(), WriteError> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((actor.into(), reason.into()));
        Ok(())
    }

    async fn recorded(
        &self,
        _tx: &mut SurrealTx,
        actor: &str,
        reason: &str,
        what: &str,
    ) -> Result<(), WriteError> {
        assert!(what.starts_with("urn:health:registry-review:"));
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((actor.into(), reason.into()));
        Ok(())
    }
}

struct Node {
    store: SurrealStore,
    review: RegistryReview<SurrealStore, Recorded>,
    registry: ClientRegistry<SurrealStore, Recorded>,
    audit: Recorded,
    clock: Arc<FixedClock>,
}

fn config() -> RegistryConfig {
    RegistryConfig {
        nin_system: NIN.into(),
    }
}

fn rules() -> MatchingRules {
    MatchingRules::platform_default(NIN, &[NIN, "urn:health:ecid"])
}

async fn node() -> Node {
    let settings = Settings {
        target: Target::Memory,
        namespace: "review".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(Registry::new(jiff::tz::Offset::UTC))
        .with_blocking(Arc::new(RegistryBlocking::new(rules(), config())));
    let audit = Recorded::default();
    let clock = Arc::new(FixedClock::new(
        Timestamp::from_second(1_800_000_000).expect("time"),
    ));
    let review = RegistryReview::new(
        ClientRegistry::new(store.clone(), audit.clone(), config()),
        rules(),
        clock.clone(),
    );
    let registry = ClientRegistry::new(store.clone(), audit.clone(), config());
    Node {
        store,
        review,
        registry,
        audit,
        clock,
    }
}

/// Grace Akello of Gulu, with variations.
fn grace(family: &str, phone: &str, nin: Option<(&str, bool)>, trust: &str) -> Value {
    let mut p = json!({
        "resourceType": "Patient",
        "meta": { "tag": [{ "system": "urn:health:registry:trust", "code": trust }] },
        "name": [{ "family": family, "given": ["Grace"] }],
        "gender": "female",
        "birthDate": "1990-04-02",
        "address": [{ "district": "Gulu" }],
        "telecom": [{ "system": "phone", "value": phone }],
    });
    if let Some((value, verified)) = nin {
        let mut identifier = json!({ "system": NIN, "value": value });
        if verified {
            identifier["extension"] = json!([{ "url": "urn:health:registry:verification", "valueCode": "nira-verified" }]);
        }
        p["identifier"] = json!([identifier]);
    }
    p
}

async fn create(store: &SurrealStore, json: Value) -> ResourceId {
    let id = ResourceId::new();
    let mut tx = store.begin().await.expect("tx");
    tx.write(WriteRequest {
        resource_type: "Patient".parse().expect("type"),
        id,
        json: RawJson::from_validated(json.to_string()),
        op_id: OpId::new(),
        precondition: Precondition::MustNotExist,
    })
    .await
    .expect("write");
    tx.commit().await.expect("commit");
    id
}

async fn golden_of(store: &SurrealStore, source: ResourceId) -> Option<String> {
    let mut tx = store.begin().await.expect("tx");
    let stored = tx
        .read(&"Patient".parse().expect("type"), source)
        .await
        .expect("read")
        .expect("exists");
    linked(
        &serde_json::from_str(stored.json.as_str()).expect("json"),
        "refer",
    )
    .into_iter()
    .next()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unlinked_pairs_are_never_relinked_automatically() {
    let node = node().await;
    let a = create(
        &node.store,
        grace(
            "Akello",
            "+256772000001",
            Some(("CF90AAAA1234XY", true)),
            "staff-entered",
        ),
    )
    .await;
    let Registered::NewIdentity { golden } = node.review.register(a, "matching").await.expect("a")
    else {
        panic!("the first registration is a new identity");
    };
    // The same NIRA-verified NIN: a certain match, linked without review.
    let b = create(
        &node.store,
        grace(
            "Akello",
            "+256772000002",
            Some(("CF90AAAA1234XY", true)),
            "staff-entered",
        ),
    )
    .await;
    assert_eq!(
        node.review.register(b, "matching").await.expect("b"),
        Registered::AutoLinked { golden }
    );

    // A reviewer separates them. Matching B again must never put it back.
    node.registry.unlink(b, "reviewer-1").await.expect("unlink");
    let again = node
        .review
        .register(b, "matching")
        .await
        .expect("register again");
    let Registered::NewIdentity { golden: other } = again else {
        panic!("a separated pair must not be re-linked or even queued: {again:?}");
    };
    assert_ne!(other, golden);
    assert_eq!(golden_of(&node.store, b).await, Some(other.to_string()));
    assert_eq!(golden_of(&node.store, a).await, Some(golden.to_string()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn possible_matches_queue_for_review_and_decisions_are_carried_out() {
    let node = node().await;
    let a = create(
        &node.store,
        grace(
            "Akello",
            "+256772000001",
            Some(("CF90AAAA1234XY", false)),
            "staff-entered",
        ),
    )
    .await;
    let Registered::NewIdentity { golden } = node.review.register(a, "matching").await.expect("a")
    else {
        panic!("new")
    };

    // A spelling variant, another phone, no NIN: possibly the same person, so it is queued.
    let c = create(
        &node.store,
        grace("Akelo", "+256772000003", None, "staff-entered"),
    )
    .await;
    let Registered::Queued { reviews } = node.review.register(c, "matching").await.expect("c")
    else {
        panic!("queued")
    };
    assert_eq!(reviews.len(), 1);
    assert_eq!(
        golden_of(&node.store, c).await,
        None,
        "unlinked while under review, and usable for care"
    );
    let pending = node.review.pending(10).await.expect("pending");
    assert_eq!(pending.len(), 1);
    let ReviewKind::PotentialMatch { golden: g, .. } = &pending[0].kind else {
        panic!("potential match")
    };
    assert_eq!(*g, golden.to_string());

    // Different people: C becomes its own identity, and is never matched to A's record again.
    node.review
        .decide(&reviews[0], Decision::DifferentPeople, "reviewer-2")
        .await
        .expect("decide");
    let c_golden = golden_of(&node.store, c)
        .await
        .expect("c has its own golden record");
    assert_ne!(c_golden, golden.to_string());
    assert_eq!(
        node.review
            .decide(&reviews[0], Decision::SamePerson, "reviewer-2")
            .await,
        Err(RegistryError::Review(ReviewRefused::AlreadyDecided))
    );
    assert!(node.review.pending(10).await.expect("pending").is_empty());

    // Another variant, judged the same person: linked to A's record.
    let e = create(
        &node.store,
        grace("Akelo", "+256772000004", None, "self-reported"),
    )
    .await;
    let Registered::Queued { reviews } = node.review.register(e, "matching").await.expect("e")
    else {
        panic!("queued")
    };
    let to_a = node.review.pending(10).await.expect("pending").into_iter().find(|r| matches!(&r.kind, ReviewKind::PotentialMatch { golden: g, .. } if *g == golden.to_string())).expect("a match with A");
    assert!(reviews.contains(&to_a.id));
    node.review
        .decide(&to_a.id, Decision::SamePerson, "reviewer-2")
        .await
        .expect("same person");
    assert_eq!(golden_of(&node.store, e).await, Some(golden.to_string()));

    // E's self-reported family name disagrees with A's staff-entered one: a survivorship item.
    let survivorship = node
        .review
        .pending(10)
        .await
        .expect("pending")
        .into_iter()
        .find(|r| {
            matches!(
                r.kind,
                ReviewKind::Survivorship {
                    field: Field::Family,
                    ..
                }
            )
        })
        .expect("survivorship item");
    assert_eq!(
        node.review
            .decide(&survivorship.id, Decision::SamePerson, "reviewer-2")
            .await,
        Err(RegistryError::Review(ReviewRefused::WrongDecision))
    );
    node.review
        .decide(&survivorship.id, Decision::TakeIncoming, "reviewer-2")
        .await
        .expect("take incoming");
    let mut tx = node.store.begin().await.expect("tx");
    let stored = tx
        .read(&"Patient".parse().expect("type"), golden)
        .await
        .expect("read")
        .expect("golden");
    let golden_json: Value = serde_json::from_str(stored.json.as_str()).expect("json");
    assert_eq!(golden_json["name"][0]["family"], "Akelo");

    // Every decision was audited, with the reviewer.
    let audited = node.audit.0.lock().expect("audit").clone();
    for decision in ["different-people", "same-person", "take-incoming"] {
        assert!(
            audited
                .iter()
                .any(|(who, why)| who == "reviewer-2" && why == decision),
            "{decision} audited"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_items_alert_after_seven_days() {
    let node = node().await;
    let a = create(
        &node.store,
        grace(
            "Akello",
            "+256772000001",
            Some(("CF90AAAA1234XY", false)),
            "staff-entered",
        ),
    )
    .await;
    node.review.register(a, "matching").await.expect("a");
    let c = create(
        &node.store,
        grace("Akelo", "+256772000003", None, "staff-entered"),
    )
    .await;
    node.review.register(c, "matching").await.expect("c");
    node.clock.advance(SignedDuration::from_hours(6 * 24));
    assert!(node.review.overdue(10).await.expect("overdue").is_empty());
    node.clock.advance(SignedDuration::from_hours(24));
    assert_eq!(node.review.overdue(10).await.expect("overdue").len(), 1);
}
