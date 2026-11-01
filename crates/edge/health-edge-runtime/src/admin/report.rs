//! The status report: the indicators the admin page shows and the alerts they raise, against
//! the thresholds in facility-edge.md § Health monitoring.

use std::time::Duration;

use health_time::{SignedDuration, Timestamp};
use serde::Serialize;

use crate::backup::Status;
use crate::ups::Power;

/// A volume above this share used raises an alert.
pub(crate) const DISK_ALERT_PERCENT: u64 = 80;
/// A task with this many failures within the supervisor's window raises an alert.
pub(crate) const TASK_ALERT_FAILURES: usize = 2;

/// Indicators this node does not report yet, and the work that adds each.
pub(crate) const PENDING: &[&str] = &[
    "sync",
    "clock-skew",
    "certificates",
    "permission-snapshot",
    "disk-encryption",
    "sessions",
];

/// What was measured, before it is judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Facts {
    pub(crate) node_id: String,
    pub(crate) version: &'static str,
    pub(crate) started: Timestamp,
    pub(crate) store_bytes: Option<u64>,
    pub(crate) volumes: Vec<Volume>,
    /// The scheduler's status and interval; `None` when backups are off.
    pub(crate) backup: Option<(Status, Duration)>,
    /// The last UPS poll; the outer `None` when no UPS is configured.
    pub(crate) ups: Option<Option<Power>>,
    pub(crate) tasks: Vec<(&'static str, usize)>,
    pub(crate) telemetry: Telemetry,
}

/// The node's status, as `GET /api/admin/status` returns it. Holds no personal health data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// When it was taken.
    pub at: Timestamp,
    /// Identity, version and uptime.
    pub node: Node,
    /// The embedded store's size on disk, when it could be measured.
    pub store_bytes: Option<u64>,
    /// The data volume, then the backup volume.
    pub volumes: Vec<Volume>,
    /// The backup schedule; `None` when backups are off.
    pub backup: Option<Backup>,
    /// The UPS; `None` when none is configured.
    pub ups: Option<Ups>,
    /// Supervised tasks that have failed, with their recent failures.
    pub tasks: Vec<Task>,
    /// The telemetry buffer and its shipping.
    pub telemetry: Telemetry,
    /// Indicators not reported yet.
    pub pending: Vec<&'static str>,
    /// The alerts raised now, most severe first.
    pub alerts: Vec<Alert>,
}

/// Identity, version and uptime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Node {
    /// `HEALTH_NODE_ID`.
    pub id: String,
    /// The running binary's version.
    pub version: &'static str,
    /// When this process started.
    pub started: Timestamp,
}

/// A volume the node writes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Volume {
    /// `data` or `backup`.
    pub name: &'static str,
    /// Its size.
    pub total_bytes: u64,
    /// What is left for the node.
    pub free_bytes: u64,
}

impl Volume {
    /// The share used, in whole percent.
    pub fn used_percent(&self) -> u64 {
        match self.total_bytes {
            0 => 0,
            total => (total - self.free_bytes.min(total)) * 100 / total,
        }
    }
}

/// The backup schedule's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Backup {
    /// How often a backup is taken.
    pub interval_seconds: u64,
    /// The last good backup.
    pub last_backup: Option<Timestamp>,
    /// The last failure, if it came after the last good backup.
    pub failed: Option<Timestamp>,
    /// The last test restore.
    pub last_test: Option<Timestamp>,
    /// Whether the last test restore passed.
    pub test_passed: Option<bool>,
}

/// The UPS's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ups {
    /// The last poll; `None` before the first.
    pub power: Option<Power>,
}

/// A supervised task that has failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Task {
    /// Its name.
    pub name: &'static str,
    /// Its failures within the supervisor's window.
    pub recent_failures: usize,
}

/// The telemetry buffer and its shipping.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Telemetry {
    /// Lines waiting to be shipped.
    pub buffered_bytes: u64,
    /// Lines dropped, oldest first, because the buffer was full.
    pub dropped_bytes: u64,
    /// Whether a collector is configured.
    pub shipping: bool,
    /// The last batch the collector accepted.
    pub last_shipped: Option<Timestamp>,
    /// Whether the last attempt failed.
    pub failing: bool,
}

/// An alert, by code; `subject` names the volume or task it is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct Alert {
    /// e.g. `disk-full`, `backup-stale`, `ups-on-battery`.
    pub code: &'static str,
    /// e.g. `data`, `backup`, or a task's name.
    pub subject: Option<&'static str>,
}

impl Alert {
    fn new(code: &'static str) -> Self {
        Self {
            code,
            subject: None,
        }
    }

    fn about(code: &'static str, subject: &'static str) -> Self {
        Self {
            code,
            subject: Some(subject),
        }
    }
}

/// Judges `facts` at `now`.
pub(crate) fn assess(facts: Facts, now: Timestamp) -> Report {
    let mut alerts = Vec::new();
    for volume in &facts.volumes {
        if volume.used_percent() > DISK_ALERT_PERCENT {
            alerts.push(Alert::about("disk-full", volume.name));
        }
    }
    let backup = match &facts.backup {
        None => {
            alerts.push(Alert::new("backups-off"));
            None
        }
        Some((status, interval)) => {
            let last_backup = status.last_backup.as_ref().map(|done| done.at);
            let failed = status
                .last_failure
                .as_ref()
                .map(|failure| failure.at)
                .filter(|at| last_backup.is_none_or(|good| *at > good));
            if failed.is_some() {
                alerts.push(Alert::new("backup-failed"));
            }
            // A new node's first backup is due at once, so it is late from its start.
            let since = last_backup.unwrap_or(facts.started);
            let stale = SignedDuration::try_from(*interval * 2).unwrap_or(SignedDuration::MAX);
            if now.duration_since(since) > stale {
                alerts.push(Alert::new("backup-stale"));
            }
            let test_passed = status.last_test.as_ref().map(|t| t.error.is_none());
            if test_passed == Some(false) {
                alerts.push(Alert::new("backup-test-failed"));
            }
            Some(Backup {
                interval_seconds: interval.as_secs(),
                last_backup,
                failed,
                last_test: status.last_test.as_ref().map(|t| t.at),
                test_passed,
            })
        }
    };
    let ups = facts.ups.map(|power| {
        match power {
            Some(Power::OnBattery) => alerts.push(Alert::new("ups-on-battery")),
            Some(Power::LowBattery) => alerts.push(Alert::new("ups-low-battery")),
            Some(Power::Unreachable) => alerts.push(Alert::new("ups-unreachable")),
            Some(Power::Online) | None => {}
        }
        Ups { power }
    });
    let tasks: Vec<Task> = facts
        .tasks
        .iter()
        .map(|(name, recent_failures)| Task {
            name,
            recent_failures: *recent_failures,
        })
        .collect();
    for task in &tasks {
        if task.recent_failures >= TASK_ALERT_FAILURES {
            alerts.push(Alert::about("task-restarts", task.name));
        }
    }
    alerts.sort_by_key(|alert| severity(alert.code));
    Report {
        at: now,
        node: Node {
            id: facts.node_id,
            version: facts.version,
            started: facts.started,
        },
        store_bytes: facts.store_bytes,
        volumes: facts.volumes,
        backup,
        ups,
        tasks,
        telemetry: facts.telemetry,
        pending: PENDING.to_vec(),
        alerts,
    }
}

/// Lower is more severe: what stops the node or loses data comes first.
fn severity(code: &str) -> u8 {
    match code {
        "ups-low-battery" => 0,
        "disk-full" => 1,
        "backup-failed" | "backup-stale" | "backup-test-failed" | "backups-off" => 2,
        "ups-on-battery" => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use crate::backup::{Done, Failed, Test};

    use super::*;

    const HOUR: Duration = Duration::from_secs(3600);

    fn at(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    fn healthy() -> Facts {
        Facts {
            node_id: "facility-edge".into(),
            version: "0.1.0",
            started: at("2026-11-02T08:00:00Z"),
            store_bytes: Some(1 << 20),
            volumes: vec![
                Volume {
                    name: "data",
                    total_bytes: 100,
                    free_bytes: 50,
                },
                Volume {
                    name: "backup",
                    total_bytes: 100,
                    free_bytes: 90,
                },
            ],
            backup: Some((
                Status {
                    last_attempt: Some(at("2026-11-02T09:00:00Z")),
                    last_backup: Some(Done {
                        at: at("2026-11-02T09:00:00Z"),
                        backup: "20261102T090000.000Z".into(),
                    }),
                    last_failure: None,
                    last_test: Some(Test {
                        at: at("2026-11-02T09:00:05Z"),
                        backup: Some("20261102T090000.000Z".into()),
                        error: None,
                    }),
                },
                HOUR,
            )),
            ups: Some(Some(Power::Online)),
            tasks: vec![],
            telemetry: Telemetry::default(),
        }
    }

    fn codes(report: &Report) -> Vec<(&'static str, Option<&'static str>)> {
        report.alerts.iter().map(|a| (a.code, a.subject)).collect()
    }

    #[test]
    fn a_healthy_node_raises_nothing() {
        let report = assess(healthy(), at("2026-11-02T10:30:00Z"));
        assert_eq!(codes(&report), vec![]);
        assert_eq!(
            report.backup.as_ref().and_then(|b| b.test_passed),
            Some(true)
        );
        assert_eq!(report.volumes[0].used_percent(), 50);
        assert!(report.pending.contains(&"sync"));
    }

    #[test]
    fn each_threshold_raises_its_alert_most_severe_first() {
        let mut facts = healthy();
        facts.volumes[1].free_bytes = 19; // 81% used
        facts.ups = Some(Some(Power::OnBattery));
        facts.tasks = vec![("ups", 1), ("backup", 2)];
        if let Some((status, _)) = &mut facts.backup {
            status.last_failure = Some(Failed {
                at: at("2026-11-02T10:00:00Z"),
                error: "disk full".into(),
            });
            if let Some(test) = &mut status.last_test {
                test.error = Some("no migrations".into());
            }
        }
        // Two intervals and a minute after the last good backup.
        let report = assess(facts, at("2026-11-02T11:01:00Z"));
        assert_eq!(
            codes(&report),
            vec![
                ("disk-full", Some("backup")),
                ("backup-failed", None),
                ("backup-stale", None),
                ("backup-test-failed", None),
                ("ups-on-battery", None),
                ("task-restarts", Some("backup")),
            ]
        );
    }

    #[test]
    fn a_failure_before_the_last_good_backup_is_over() {
        let mut facts = healthy();
        if let Some((status, _)) = &mut facts.backup {
            status.last_failure = Some(Failed {
                at: at("2026-11-02T08:00:00Z"),
                error: "disk full".into(),
            });
        }
        let report = assess(facts, at("2026-11-02T09:30:00Z"));
        assert_eq!(codes(&report), vec![]);
        assert_eq!(report.backup.and_then(|b| b.failed), None);
    }

    #[test]
    fn a_node_without_backups_or_a_first_backup_says_so() {
        let mut facts = healthy();
        facts.backup = None;
        facts.ups = None;
        let report = assess(facts, at("2026-11-02T09:00:00Z"));
        assert_eq!(codes(&report), vec![("backups-off", None)]);
        assert_eq!(report.ups, None);

        let mut facts = healthy();
        facts.backup = Some((Status::default(), HOUR));
        facts.ups = Some(None);
        assert_eq!(
            codes(&assess(facts.clone(), at("2026-11-02T09:59:00Z"))),
            vec![]
        );
        assert_eq!(
            codes(&assess(facts, at("2026-11-02T10:01:00Z"))),
            vec![("backup-stale", None)]
        );
    }
}
