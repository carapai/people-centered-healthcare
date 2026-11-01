use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::Request;
use health_observability::LogSink as _;
use tower::ServiceExt as _;

use super::*;
use crate::backup::{Done, Status};
use crate::supervisor::{RestartPolicy, Supervisor};
use crate::ups::Power;

fn monitor(dir: &Path) -> Monitor {
    let data_dir = dir.join("store");
    std::fs::create_dir_all(&data_dir).expect("store");
    std::fs::write(data_dir.join("000001.sst"), [0_u8; 100]).expect("file");
    Monitor {
        node_id: "facility-edge".into(),
        started: Timestamp::now(),
        data_dir,
        backups: Some((
            Backups::new(dir.join("backups"), "facility-edge"),
            Duration::from_secs(3600),
        )),
        power: Some(Arc::new(Mutex::new(Some(Power::OnBattery)))),
        restarts: Supervisor::new(RestartPolicy::default()).restarts(),
        buffer: None,
        shipping: ShipState::default(),
    }
}

async fn get_status(monitor: Monitor, peer: &str) -> Response {
    let peer: SocketAddr = peer.parse().expect("peer");
    routes(monitor)
        .layer(MockConnectInfo(peer))
        .oneshot(
            Request::get("/api/admin/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response")
}

#[tokio::test]
async fn the_status_is_served_to_the_node_itself_only() {
    let dir = tempfile::tempdir().expect("dir");
    let monitor = monitor(dir.path());
    std::fs::create_dir_all(dir.path().join("backups")).expect("backups");
    let response = get_status(monitor.clone(), "127.0.0.1:50000").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("body");
    let report: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(report["node"]["id"], "facility-edge");
    assert_eq!(report["store_bytes"], 100);
    assert_eq!(report["ups"]["power"], "on-battery");
    assert_eq!(report["volumes"][0]["name"], "data");
    assert!(report["volumes"][0]["total_bytes"].as_u64() > Some(0));
    let codes: Vec<&str> = report["alerts"]
        .as_array()
        .expect("alerts")
        .iter()
        .filter_map(|a| a["code"].as_str())
        .collect();
    assert!(codes.contains(&"ups-on-battery"), "{codes:?}");

    assert_eq!(
        get_status(monitor.clone(), "[::1]:50000").await.status(),
        StatusCode::OK
    );
    for remote in ["192.168.1.20:50000", "[fe80::1]:50000"] {
        assert_eq!(
            get_status(monitor.clone(), remote).await.status(),
            StatusCode::FORBIDDEN,
            "{remote}"
        );
    }
}

#[tokio::test]
async fn the_report_reads_the_backup_schedule() {
    let dir = tempfile::tempdir().expect("dir");
    let monitor = monitor(dir.path());
    let (backups, _) = monitor.backups.clone().expect("backups");
    std::fs::create_dir_all(backups.root()).expect("root");
    let at = Timestamp::now();
    let status = Status {
        last_attempt: Some(at),
        last_backup: Some(Done {
            at,
            backup: "b".into(),
        }),
        ..Status::default()
    };
    std::fs::write(
        backups.root().join("status.json"),
        serde_json::to_vec(&status).expect("json"),
    )
    .expect("status");
    let report = monitor.report().await;
    let backup = report.backup.expect("backup");
    assert_eq!(backup.last_backup, Some(at));
    assert_eq!(backup.interval_seconds, 3600);
    assert_eq!(report.volumes.len(), 2, "the data and backup volumes");
}

/// A collector that refuses batches until `online`, keeping what it accepts.
async fn collector(online: Arc<AtomicBool>, got: Arc<Mutex<Vec<serde_json::Value>>>) -> String {
    let app = Router::new().route(
        "/v1/logs",
        axum::routing::post(move |body: axum::body::Bytes| {
            let (online, got) = (Arc::clone(&online), Arc::clone(&got));
            async move {
                if !online.load(Ordering::SeqCst) {
                    return StatusCode::SERVICE_UNAVAILABLE;
                }
                let batch = serde_json::from_slice(&body).expect("otlp json");
                got.lock().expect("got").push(batch);
                StatusCode::OK
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move { axum::serve(listener, app).await });
    format!("http://127.0.0.1:{port}/v1/logs")
}

async fn until(done: impl Fn() -> bool) {
    let started = std::time::Instant::now();
    while !done() {
        assert!(started.elapsed() < Duration::from_secs(10), "timed out");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn lines_buffered_offline_are_shipped_when_online() {
    let dir = tempfile::tempdir().expect("dir");
    let buffer = Arc::new(Buffer::open(dir.path(), DEFAULT_CAP).expect("buffer"));
    buffer.write(r#"{"ts":"2026-11-02T09:00:00Z","level":"WARN","fields":{"alert":"disk-full"}}"#);
    let online = Arc::new(AtomicBool::new(false));
    let got = Arc::new(Mutex::new(Vec::new()));
    let endpoint = collector(Arc::clone(&online), Arc::clone(&got)).await;
    let state = ShipState::default();
    let shipper = tokio::spawn(ship(
        Arc::clone(&buffer),
        endpoint,
        "facility-edge".into(),
        Duration::from_millis(50),
        Arc::clone(&state),
    ));

    until(|| state.lock().expect("state").failing).await;
    assert!(buffer.stats().0 > 0, "kept while the collector refuses");
    assert!(got.lock().expect("got").is_empty());

    buffer.write(r#"{"ts":"2026-11-02T09:00:01Z","level":"INFO","fields":{"alert_count":1}}"#);
    online.store(true, Ordering::SeqCst);
    until(|| buffer.stats().0 == 0 && !state.lock().expect("state").failing).await;
    shipper.abort();
    let batches = got.lock().expect("got").clone();
    let records: usize = batches
        .iter()
        .map(|b| {
            b["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
                .as_array()
                .map_or(0, Vec::len)
        })
        .sum();
    assert!(records >= 2, "both lines shipped: {batches:?}");
    assert!(state.lock().expect("state").last_shipped.is_some());
}
