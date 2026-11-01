//! Running the built `facility-edge` binary in tests.

#![allow(dead_code, unreachable_pub)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

pub const STARTUP: Duration = Duration::from_secs(60);

pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("port")
        .port()
}

/// The built binary.
pub const BINARY: &str = env!("CARGO_BIN_EXE_facility-edge");

pub fn edge(data: &Path, port: u16, args: &[&str], env: &[(&str, String)]) -> Child {
    edge_at(Path::new(BINARY), data, port, args, env)
}

/// Runs `program` (an installed copy of the binary) as the node.
pub fn edge_at(
    program: &Path,
    data: &Path,
    port: u16,
    args: &[&str],
    env: &[(&str, String)],
) -> Child {
    Command::new(program)
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
pub fn get(port: u16, path: &str) -> Option<u16> {
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

pub fn wait_ready(port: u16) {
    let started = Instant::now();
    while get(port, "/ready") != Some(200) {
        assert!(started.elapsed() < STARTUP, "not ready in time");
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn wait_exit(child: &mut Child) -> ExitStatus {
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

pub fn terminate(child: &Child) {
    let sent = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("kill");
    assert!(sent.success());
}
