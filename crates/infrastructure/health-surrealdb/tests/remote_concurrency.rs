//! Many interactive transactions at once over the remote engine: none fails for want of a
//! session (the intermittent "Method not allowed" behind spurious 401s).
//! `HEALTH_TEST_SURREALDB_WS=ws://127.0.0.1:<port> cargo test -p health-surrealdb --features remote
//! --test remote_concurrency`, against `surreal start --user root --pass root memory`.

#![cfg(feature = "remote")]

use std::sync::Arc;

use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, ResourceType, Store, Transaction, WriteRequest};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};

const OBSERVATION: &str = r#"{"resourceType":"Observation","status":"final","code":{"text":"x"}}"#;
const TASKS: usize = 320;
const ROUNDS: usize = 6;

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_transactions_over_one_remote_store_all_succeed() {
    let Some(endpoint) = std::env::var("HEALTH_TEST_SURREALDB_WS").ok() else {
        eprintln!("skipped: set HEALTH_TEST_SURREALDB_WS");
        return;
    };
    let settings = Settings {
        target: Target::Remote {
            endpoint,
            username: "root".into(),
            password: "root".into(),
        },
        namespace: format!("c{}", std::process::id()),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open remote");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let at: Timestamp = "2026-11-06T08:00:00Z".parse().expect("timestamp");
    let store = SurrealStore::new(connection, Arc::new(FixedClock::new(at)));
    let observation: ResourceType = "Observation".parse().expect("type");

    let mut tasks = Vec::new();
    for task in 0..TASKS {
        let store = store.clone();
        let observation = observation.clone();
        tasks.push(tokio::spawn(async move {
            let mut failures = Vec::new();
            for round in 0..ROUNDS {
                let outcome: Result<(), String> = async {
                    let mut tx = store.begin().await.map_err(|e| format!("begin: {e}"))?;
                    tx.read(&observation, ResourceId::new())
                        .await
                        .map_err(|e| format!("read: {e}"))?;
                    match (task + round) % 3 {
                        // A write, committed.
                        0 => {
                            tx.write(WriteRequest {
                                resource_type: observation.clone(),
                                id: ResourceId::new(),
                                json: RawJson::from_validated(OBSERVATION),
                                op_id: OpId::new(),
                                precondition: Precondition::MustNotExist,
                            })
                            .await
                            .map_err(|e| format!("write: {e}"))?;
                            tx.commit().await.map_err(|e| format!("commit: {e}"))
                        }
                        // A read, committed.
                        1 => tx.commit().await.map_err(|e| format!("commit: {e}")),
                        // A read, dropped (cancelled in the background).
                        _ => {
                            drop(tx);
                            Ok(())
                        }
                    }
                }
                .await;
                if let Err(e) = outcome {
                    failures.push(e);
                }
            }
            failures
        }));
    }
    let mut failures = Vec::new();
    for task in tasks {
        failures.extend(task.await.expect("task"));
    }
    assert!(
        failures.is_empty(),
        "{} of {} transactions failed, e.g. {:?}",
        failures.len(),
        TASKS * ROUNDS,
        &failures[..failures.len().min(5)]
    );
}
