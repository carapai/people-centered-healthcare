//! The `facility-edge` binary as a service manager runs it: readiness after migrations, a
//! graceful stop, recovery after being killed, the systemd notifications, the UPS stop, and an
//! old binary refusing a newer store (facility-edge.md; testing.md § recovery).

#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::UnixDatagram;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use health_surrealdb::{Connection, MIGRATIONS, Migration, Settings, Target, migrate};

const STARTUP: Duration = Duration::from_secs(60);

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("port")
        .port()
}

fn edge(data: &Path, port: u16, args: &[&str], env: &[(&str, String)]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_facility-edge"))
        .args(args)
        .env_clear()
        .env("HEALTH_DATA_DIR", data)
        .env("HEALTH_BIND", format!("127.0.0.1:{port}"))
        .env("HEALTH_DRAIN_SECS", "5")
        .env("HEALTH_LOG", "warn")
        .envs(env.iter().map(|(k, v)| (*k, v.as_str())))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn facility-edge")
}

/// The status code of `GET path`, if the node answers.
fn get(port: u16, path: &str) -> Option<u16> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: edge\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    response.split_whitespace().nth(1)?.parse().ok()
}

fn wait_ready(port: u16) {
    let started = Instant::now();
    while get(port, "/ready") != Some(200) {
        assert!(started.elapsed() < STARTUP, "not ready in time");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn wait_exit(child: &mut Child) -> ExitStatus {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            return status;
        }
        if started.elapsed() > STARTUP {
            let _ = child.kill();
            panic!("did not exit in time");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn terminate(child: &Child) {
    let sent = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("kill");
    assert!(sent.success());
}

#[test]
fn it_reports_ready_after_migrating_and_stops_gracefully_on_sigterm() {
    let dir = tempfile::tempdir().expect("dir");
    let port = free_port();
    let mut node = edge(&dir.path().join("store"), port, &[], &[]);
    wait_ready(port);
    assert_eq!(get(port, "/health"), Some(200));
    terminate(&node);
    assert_eq!(wait_exit(&mut node).code(), Some(0));
}

#[test]
fn it_recovers_after_being_killed_and_no_second_process_opens_its_store() {
    let dir = tempfile::tempdir().expect("dir");
    let store = dir.path().join("store");
    let port = free_port();
    let mut node = edge(&store, port, &[], &[]);
    wait_ready(port);
    // The process owns the store alone.
    let mut second = edge(&store, free_port(), &[], &[]);
    assert_eq!(wait_exit(&mut second).code(), Some(3));
    // A hard kill (a crash, or a power cut with the OS still up) leaves a store that reopens.
    node.kill().expect("kill");
    let _ = node.wait();
    let port = free_port();
    let mut restarted = edge(&store, port, &[], &[]);
    wait_ready(port);
    terminate(&restarted);
    assert_eq!(wait_exit(&mut restarted).code(), Some(0));
}

/// Collects notifications into `seen` until one starts with `until`.
fn receive(socket: &UnixDatagram, seen: &mut Vec<String>, until: &str) {
    let started = Instant::now();
    let mut buf = [0_u8; 512];
    while !seen.iter().any(|m| m.starts_with(until)) {
        assert!(started.elapsed() < STARTUP, "no {until}: {seen:?}");
        if let Ok(n) = socket.recv(&mut buf) {
            seen.push(String::from_utf8_lossy(&buf[..n]).into_owned());
        }
    }
}

#[test]
fn under_systemd_it_notifies_ready_watchdog_and_stopping() {
    let dir = tempfile::tempdir().expect("dir");
    let socket_path = dir.path().join("notify");
    let socket = UnixDatagram::bind(&socket_path).expect("bind");
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let port = free_port();
    let mut node = edge(
        &dir.path().join("store"),
        port,
        &["--service"],
        &[
            ("NOTIFY_SOCKET", socket_path.to_string_lossy().into_owned()),
            ("WATCHDOG_USEC", "1000000".into()),
        ],
    );
    let mut seen: Vec<String> = Vec::new();
    receive(&socket, &mut seen, "READY=1");
    receive(&socket, &mut seen, "WATCHDOG=1");
    assert!(seen[0].starts_with("EXTEND_TIMEOUT_USEC="), "{seen:?}");
    assert_eq!(get(port, "/ready"), Some(200));
    terminate(&node);
    receive(&socket, &mut seen, "STOPPING=1");
    assert_eq!(wait_exit(&mut node).code(), Some(0));

    // `--service` without a service manager is a configuration error.
    let mut orphan = edge(&dir.path().join("other"), free_port(), &["--service"], &[]);
    assert_eq!(wait_exit(&mut orphan).code(), Some(2));
}

#[test]
fn a_ups_low_battery_signal_stops_it_gracefully() {
    let nut = TcpListener::bind("127.0.0.1:0").expect("nut");
    let nut_port = nut.local_addr().expect("addr").port();
    // On line power for the first polls, then on battery and low.
    std::thread::spawn(move || {
        for (i, stream) in nut.incoming().enumerate() {
            let Ok(mut stream) = stream else { return };
            let mut line = String::new();
            let _ = BufReader::new(&stream).read_line(&mut line);
            let status = if i < 2 { "OL" } else { "OB LB" };
            let _ = writeln!(stream, "VAR apc ups.status \"{status}\"");
        }
    });
    let dir = tempfile::tempdir().expect("dir");
    let port = free_port();
    let mut node = edge(
        &dir.path().join("store"),
        port,
        &[],
        &[
            ("HEALTH_UPS", format!("apc@127.0.0.1:{nut_port}")),
            ("HEALTH_UPS_POLL_SECS", "1".into()),
        ],
    );
    wait_ready(port);
    assert_eq!(wait_exit(&mut node).code(), Some(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_old_binary_refuses_a_store_a_newer_one_migrated() {
    let dir = tempfile::tempdir().expect("dir");
    let store = dir.path().join("store");
    // A newer release migrated this store one step past this binary.
    {
        let connection = Connection::open(&Settings {
            target: Target::RocksDb {
                path: store.clone(),
            },
            namespace: "ug".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        let mut newer = MIGRATIONS.to_vec();
        newer.push(Migration {
            version: 9999,
            name: "9999_from_the_future",
            sql: "DEFINE TABLE from_the_future SCHEMALESS;",
        });
        migrate(&connection, &newer).await.expect("migrate");
    }
    // The embedded store closes in the background after its last handle drops; until then the
    // binary finds it locked (exit 3).
    let started = Instant::now();
    loop {
        let mut node = edge(&store, free_port(), &[], &[]);
        match wait_exit(&mut node).code() {
            Some(3) if started.elapsed() < STARTUP => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            code => {
                assert_eq!(code, Some(4));
                break;
            }
        }
    }
}
