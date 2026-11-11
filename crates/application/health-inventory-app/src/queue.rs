//! Writers take turns per store within a node (#141, the stock soak): two use cases on the same
//! store run one after the other instead of racing to commit and rerunning, which under load
//! left callers `Busy`. The balance record's write-write conflict still guards against writers
//! in other processes, and the reruns stay for them.
//!
//! A use case that knows its stores before it starts waits for each of them, in key order so
//! two waiting for the same pair cannot deadlock, and lets other stores' work run alongside. One
//! that learns its stores only inside its transaction (completing a dispense by its id, say)
//! waits for the whole node instead. No use case takes a turn while holding one.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, LazyLock, Mutex as Registry};

use tokio::sync::{Mutex, OwnedMutexGuard, OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

static NODE: LazyLock<Arc<RwLock<()>>> = LazyLock::new(|| Arc::new(RwLock::new(())));
static STORES: LazyLock<Registry<HashMap<String, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Registry::new(HashMap::new()));

/// A turn: held until dropped.
#[must_use = "the turn ends when it is dropped"]
pub(crate) enum Turn {
    /// The named stores, other stores' work alongside.
    Stores {
        _node: OwnedRwLockReadGuard<()>,
        _stores: Vec<OwnedMutexGuard<()>>,
    },
    /// The whole node.
    Node { _node: OwnedRwLockWriteGuard<()> },
}

/// Waits for a turn at `stores`.
pub(crate) async fn stores<'a>(stores: impl IntoIterator<Item = &'a str>) -> Turn {
    let keys: BTreeSet<&str> = stores.into_iter().collect();
    let node = Arc::clone(&NODE).read_owned().await;
    let locks: Vec<Arc<Mutex<()>>> = {
        let mut registry = STORES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        keys.iter()
            .map(|key| Arc::clone(registry.entry((*key).to_owned()).or_default()))
            .collect()
    };
    let mut held = Vec::with_capacity(locks.len());
    for lock in locks {
        held.push(lock.lock_owned().await);
    }
    Turn::Stores {
        _node: node,
        _stores: held,
    }
}

/// Waits for a turn at the whole node.
pub(crate) async fn node() -> Turn {
    Turn::Node {
        _node: Arc::clone(&NODE).write_owned().await,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn one_store_at_a_time_other_stores_alongside_and_the_node_alone() {
        static INSIDE: AtomicUsize = AtomicUsize::new(0);
        static MOST: AtomicUsize = AtomicUsize::new(0);
        let enter = || {
            let now = INSIDE.fetch_add(1, Ordering::SeqCst) + 1;
            MOST.fetch_max(now, Ordering::SeqCst);
        };
        // Eight writers on one store never overlap.
        let tasks: Vec<_> = (0..8)
            .map(|_| {
                tokio::spawn(async move {
                    let _turn = stores(["test/one-store"]).await;
                    enter();
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    INSIDE.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for task in tasks {
            task.await.expect("task");
        }
        assert_eq!(MOST.load(Ordering::SeqCst), 1);

        // Two stores run alongside; the node waits for both, and they for it.
        let a = stores(["test/a"]).await;
        let b = tokio::time::timeout(Duration::from_secs(1), stores(["test/b", "test/c"]))
            .await
            .expect("another store is not held up");
        assert!(
            tokio::time::timeout(Duration::from_millis(50), node())
                .await
                .is_err(),
            "the node waits for stores in use"
        );
        drop((a, b));
        let whole = node().await;
        assert!(
            tokio::time::timeout(Duration::from_millis(50), stores(["test/a"]))
                .await
                .is_err(),
            "a store waits for the node"
        );
        drop(whole);
    }
}
