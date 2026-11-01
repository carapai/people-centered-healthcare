//! Concurrent requests raising the clock floor over SurrealDB (memory engine): each request
//! with a snapshot observes it, so a lost write conflict must not refuse the request.

use std::sync::Arc;

use health_auth::{AuthStore, SurrealAuthStore};
use health_identity::SessionPolicy;
use health_identity::snapshot::ClockFloor;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_raises_of_the_clock_floor_all_succeed() {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "auth".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealAuthStore::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        SessionPolicy::default().hard_cap,
    );
    let mut raises = tokio::task::JoinSet::new();
    for i in 0..8 {
        let store = store.clone();
        raises.spawn(async move {
            let at = Timestamp::from_second(1_800_000_000 + i).expect("time");
            store.set_clock_floor(ClockFloor::at(at)).await
        });
    }
    for result in raises.join_all().await {
        assert_eq!(result, Ok(()));
    }
    let floor = store.clock_floor().await.expect("floor");
    assert!(floor.get().is_some());
}
