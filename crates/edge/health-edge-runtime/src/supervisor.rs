//! Supervised background tasks (facility-edge.md § Process model): a failed task restarts with
//! exponential backoff; one that keeps failing stops the node, so the service manager restarts
//! the whole process cleanly.

use std::future::Future;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::Instant;

/// When a failed task restarts, and when its failures stop the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartPolicy {
    /// The first restart's delay; each further one doubles it.
    pub initial: Duration,
    /// The longest delay.
    pub max: Duration,
    /// This many failures within [`RestartPolicy::window`] stop the node.
    pub limit: usize,
    /// The window [`RestartPolicy::limit`] counts failures over.
    pub window: Duration,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(60),
            limit: 5,
            window: Duration::from_secs(600),
        }
    }
}

/// The node's background tasks. Dropping it aborts them.
#[derive(Debug)]
pub struct Supervisor {
    policy: RestartPolicy,
    tasks: JoinSet<()>,
    failed: mpsc::Sender<&'static str>,
    failures: mpsc::Receiver<&'static str>,
}

impl Supervisor {
    /// No tasks yet.
    pub fn new(policy: RestartPolicy) -> Self {
        let (failed, failures) = mpsc::channel(1);
        Self {
            policy,
            tasks: JoinSet::new(),
            failed,
            failures,
        }
    }

    /// Runs the task `make` builds, rebuilding and restarting it whenever it fails or panics.
    /// A task that returns `Ok` is finished and not restarted.
    pub fn spawn<F, Fut>(&mut self, name: &'static str, make: F)
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let policy = self.policy;
        let failed = self.failed.clone();
        self.tasks.spawn(async move {
            let mut delay = policy.initial;
            let mut recent: Vec<Instant> = Vec::new();
            loop {
                let outcome = match tokio::spawn(make()).await {
                    Ok(Ok(())) => return,
                    Ok(Err(reason)) => reason,
                    Err(_) => "panicked".to_owned(),
                };
                let now = Instant::now();
                recent.retain(|at| now.duration_since(*at) < policy.window);
                recent.push(now);
                tracing::warn!(
                    task = name,
                    failures = recent.len(),
                    error.kind = outcome.as_str(),
                    "supervised task failed"
                );
                if recent.len() >= policy.limit {
                    let _ = failed.send(name).await;
                    return;
                }
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(policy.max);
            }
        });
    }

    /// Waits until some task has failed [`RestartPolicy::limit`] times within the window, and
    /// names it. Pending forever while every task is healthy.
    pub async fn failed(&mut self) -> &'static str {
        match self.failures.recv().await {
            Some(name) => name,
            None => std::future::pending().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn quick(limit: usize) -> RestartPolicy {
        RestartPolicy {
            initial: Duration::from_millis(5),
            max: Duration::from_millis(20),
            limit,
            window: Duration::from_secs(60),
        }
    }

    #[tokio::test]
    async fn a_failing_task_restarts_until_it_succeeds() {
        let runs = Arc::new(AtomicUsize::new(0));
        let mut supervisor = Supervisor::new(quick(10));
        let counter = Arc::clone(&runs);
        supervisor.spawn("flaky", move || {
            let counter = Arc::clone(&counter);
            async move {
                match counter.fetch_add(1, Ordering::SeqCst) {
                    0 => Err("first".to_owned()),
                    1 => panic!("second"),
                    _ => Ok(()),
                }
            }
        });
        let stopped = tokio::time::timeout(Duration::from_millis(300), supervisor.failed()).await;
        assert!(stopped.is_err(), "a recovering task never stops the node");
        assert_eq!(runs.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_task_that_keeps_failing_stops_the_node() {
        let mut supervisor = Supervisor::new(quick(3));
        supervisor.spawn("healthy", std::future::pending);
        supervisor.spawn("broken", || async { Err("always".to_owned()) });
        let name = tokio::time::timeout(Duration::from_secs(5), supervisor.failed())
            .await
            .expect("the node stops");
        assert_eq!(name, "broken");
    }
}
