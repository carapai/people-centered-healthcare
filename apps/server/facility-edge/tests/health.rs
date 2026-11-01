//! Health monitoring through the built binary (facility-edge.md § Health monitoring): the
//! status page's report from the node itself, and indicators buffered while the collector is
//! unreachable, then shipped to it.

#![cfg(unix)]

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{STARTUP, edge, terminate, wait_exit, wait_ready};

/// `GET path`'s status and body.
fn fetch(port: u16, path: &str) -> Option<(u16, String)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: edge\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let status = response.split_whitespace().nth(1)?.parse().ok()?;
    let (_, body) = response.split_once("\r\n\r\n")?;
    Some((status, body.to_owned()))
}

/// An OTLP/HTTP collector that answers 503 until `online`, keeping the batches it accepts.
fn collector(online: Arc<AtomicBool>, got: Arc<Mutex<Vec<String>>>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let mut reader = BufReader::new(stream);
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let up = online.load(Ordering::SeqCst);
            if up {
                got.lock()
                    .expect("got")
                    .push(String::from_utf8_lossy(&body).into_owned());
            }
            let status = if up {
                "200 OK"
            } else {
                "503 Service Unavailable"
            };
            let _ = write!(
                reader.get_mut(),
                "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
        }
    });
    port
}

fn until(what: &str, done: impl Fn() -> bool) {
    let started = Instant::now();
    while !done() {
        assert!(started.elapsed() < STARTUP, "{what}: timed out");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn indicators_are_shown_offline_and_shipped_when_online() {
    let dir = tempfile::tempdir().expect("dir");
    let telemetry = dir.path().join("telemetry");
    let online = Arc::new(AtomicBool::new(false));
    let got = Arc::new(Mutex::new(Vec::new()));
    let collector = collector(Arc::clone(&online), Arc::clone(&got));
    let port = common::free_port();
    let mut node = edge(
        &dir.path().join("store"),
        port,
        &[],
        &[
            ("HEALTH_LOG", "info".into()),
            ("HEALTH_INDICATORS_SECS", "1".into()),
            ("HEALTH_TELEMETRY_SHIP_SECS", "1".into()),
            ("HEALTH_TELEMETRY_DIR", telemetry.display().to_string()),
            (
                "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                format!("http://127.0.0.1:{collector}/v1/logs"),
            ),
        ],
    );
    wait_ready(port);

    // Offline: the page has the report, and the lines wait in the buffer.
    let (status, body) = fetch(port, "/api/admin/status").expect("status");
    assert_eq!(status, 200, "{body}");
    let report: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(report["node"]["id"], "facility-edge");
    assert!(report["store_bytes"].as_u64() > Some(0));
    assert_eq!(report["telemetry"]["shipping"], true);
    assert!(
        report["alerts"]
            .as_array()
            .expect("alerts")
            .iter()
            .any(|a| a["code"] == "backups-off"),
        "{report}"
    );
    until("the shipper reports failing", || {
        fetch(port, "/api/admin/status")
            .and_then(|(_, body)| serde_json::from_str::<serde_json::Value>(&body).ok())
            .is_some_and(|r| r["telemetry"]["failing"] == true)
    });
    let buffered = || {
        std::fs::read_dir(&telemetry)
            .map(|d| {
                d.filter_map(Result::ok)
                    .filter_map(|e| e.metadata().ok())
                    .map(|m| m.len())
                    .sum::<u64>()
            })
            .unwrap_or(0)
    };
    assert!(buffered() > 0, "lines are buffered while offline");
    assert!(got.lock().expect("got").is_empty());

    // Online: the buffered indicators and alerts reach the collector.
    online.store(true, Ordering::SeqCst);
    until("the alert reaches the collector", || {
        got.lock()
            .expect("got")
            .iter()
            .any(|b| b.contains(r#""stringValue":"backups-off""#))
    });
    until("an indicators line reaches the collector", || {
        got.lock()
            .expect("got")
            .iter()
            .any(|b| b.contains(r#""key":"store_bytes""#))
    });
    terminate(&node);
    assert_eq!(wait_exit(&mut node).code(), Some(0));
}
