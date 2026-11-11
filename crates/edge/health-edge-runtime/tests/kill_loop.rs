//! Recovery under kill -9 (#141; testing.md § Recovery): a child process writes to the edge's
//! store (SurrealKV) and reports each commit as it is acknowledged; it is killed at a random
//! moment, the store is reopened, and every acknowledged write must be there, the change log
//! must have no gap and match the commit counter, and every transaction on it must be whole.
//! Then again, `HEALTH_KILL_ROUNDS` times (5 by default; nightly runs more).

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, ResourceType, Store, Transaction, WriteRequest};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;

const DIR: &str = "HEALTH_KILL_LOOP_DIR";

async fn open(dir: &Path) -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::SurrealKv {
            path: dir.join("store"),
        },
        namespace: "ug".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

fn observation() -> ResourceType {
    "Observation".parse().expect("type")
}

/// The child: writes transactions of one to three Observations until it is killed, printing
/// `ack <id>…` once each has committed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "run by the kill loop as its child"]
async fn writer() {
    let Some(dir) = std::env::var_os(DIR) else {
        return;
    };
    let store = open(Path::new(&dir)).await;
    // Until it is killed.
    for n in (0u64..3).cycle() {
        let ids: Vec<ResourceId> = (0..=n).map(|_| ResourceId::new()).collect();
        let mut tx = store.begin().await.expect("begin");
        for id in &ids {
            tx.write(WriteRequest {
                resource_type: observation(),
                id: *id,
                json: RawJson::from_validated(
                    r#"{"resourceType":"Observation","status":"final","code":{"text":"kill loop"}}"#,
                ),
                op_id: OpId::new(),
                precondition: Precondition::MustNotExist,
            })
            .await
            .expect("write");
        }
        tx.commit().await.expect("commit");
        let line: Vec<String> = ids.iter().map(ToString::to_string).collect();
        // Locked per line: libtest's own thread prints too.
        let mut out = std::io::stdout().lock();
        writeln!(out, "ack {}", line.join(" ")).expect("stdout");
        out.flush().expect("flush");
    }
}

/// A small deterministic generator, so a failing round can be replayed from its seed.
struct XorShift(u64);

impl XorShift {
    fn below(&mut self, bound: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % bound
    }
}

/// One round: the writer runs for a while, is killed, and its acknowledged ids are returned.
fn round(dir: &Path, millis: u64) -> Vec<ResourceId> {
    let mut child = Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "writer",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(DIR, dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn writer");
    let stdout = child.stdout.take().expect("stdout");
    let acked = Arc::new(Mutex::new(Vec::new()));
    let reader = {
        let acked = Arc::clone(&acked);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Some(ids) = line.strip_prefix("ack ") {
                    let mut acked = acked.lock().expect("lock");
                    acked.extend(
                        ids.split(' ')
                            .map(|id| id.parse::<ResourceId>().expect("id")),
                    );
                }
            }
        })
    };
    std::thread::sleep(Duration::from_millis(millis));
    // SIGKILL on Unix, TerminateProcess on Windows: no shutdown runs.
    child.kill().expect("kill");
    child.wait().expect("wait");
    reader.join().expect("reader");
    Arc::try_unwrap(acked)
        .expect("reader done")
        .into_inner()
        .expect("lock")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acknowledged_writes_survive_kill_9_and_the_log_stays_whole() {
    let rounds: u64 = std::env::var("HEALTH_KILL_ROUNDS")
        .ok()
        .and_then(|r| r.parse().ok())
        .unwrap_or(5);
    let seed = std::env::var("HEALTH_KILL_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
                | 1
        });
    eprintln!("kill loop: {rounds} rounds, HEALTH_KILL_SEED={seed}");
    let mut random = XorShift(seed);
    let dir = tempfile::tempdir().expect("dir");
    // Migrated once here: a fresh store's migrations take a minute in a debug build, a reopen a
    // few seconds.
    drop(open(dir.path()).await);
    let mut acknowledged = BTreeSet::new();
    for n in 1..=rounds {
        // From before the reopen ends to several seconds of writes: some kills land while the
        // store opens, most mid-transaction.
        let millis = 1_000 + random.below(6_000);
        let acked = round(dir.path(), millis);
        let in_round = acked.len();
        acknowledged.extend(acked);

        let store = open(dir.path()).await;
        let cursor = store.cursor().await.expect("cursor");
        let changes = store
            .changes_after(0, usize::try_from(cursor.seq).expect("seq") + 10)
            .await
            .expect("changes");
        eprintln!(
            "round {n}: killed after {millis} ms, {in_round} writes acknowledged, log at {}",
            cursor.seq
        );
        // The log has no gap and ends at the counter.
        let seqs: Vec<u64> = changes.iter().map(|c| c.seq).collect();
        assert_eq!(
            seqs,
            (1..=cursor.seq).collect::<Vec<_>>(),
            "round {n} (seed {seed}): the log has a gap or runs past the counter"
        );
        // Every acknowledged write is on the log and readable.
        let logged: BTreeSet<ResourceId> = changes
            .iter()
            .map(|c| c.resource_id.parse().expect("id"))
            .collect();
        let missing: Vec<_> = acknowledged.difference(&logged).collect();
        assert!(
            missing.is_empty(),
            "round {n} (seed {seed}): acknowledged but lost: {missing:?}"
        );
        // Every transaction on the log is whole, and what it wrote is there.
        let mut transactions: BTreeMap<String, usize> = BTreeMap::new();
        let mut tx = store.begin().await.expect("begin");
        for change in &changes {
            *transactions.entry(change.tx_id.to_string()).or_default() += 1;
            let id: ResourceId = change.resource_id.parse().expect("id");
            let stored = tx.read(&observation(), id).await.expect("read");
            assert!(
                stored.is_some(),
                "round {n} (seed {seed}): {id} is on the log but not stored"
            );
        }
        drop(tx);
        for (tx_id, entries) in &transactions {
            assert!(
                (1..=3).contains(entries),
                "round {n} (seed {seed}): transaction {tx_id} has {entries} entries"
            );
        }
        // And the store takes writes again.
        let mut tx = store.begin().await.expect("begin");
        tx.write(WriteRequest {
            resource_type: observation(),
            id: ResourceId::new(),
            json: RawJson::from_validated(r#"{"resourceType":"Observation","status":"final"}"#),
            op_id: OpId::new(),
            precondition: Precondition::MustNotExist,
        })
        .await
        .expect("write after recovery");
        tx.commit().await.expect("commit after recovery");
        drop(store);
    }
    assert!(
        !acknowledged.is_empty(),
        "the writer acknowledged nothing in {rounds} rounds"
    );
}
