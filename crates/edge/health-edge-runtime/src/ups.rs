//! The UPS watch (facility-edge.md § Hardware and power baseline): the edge polls a NUT server
//! (`upsd`) for its UPS's status. On battery it warns; on low battery it stops gracefully.

use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Notify;

/// NUT's default port.
const NUT_PORT: u16 = 3493;
/// How long one status poll may take.
const POLL_TIMEOUT: Duration = Duration::from_secs(5);

/// A UPS served by NUT, written `name@host[:port]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ups {
    name: String,
    host: String,
    port: u16,
}

/// A UPS status poll that failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpsError {
    /// The NUT server did not answer in time, or the connection failed.
    #[error("the NUT server is unreachable")]
    Unreachable,
    /// The NUT server answered with an error or something unexpected.
    #[error("the NUT server refused the status request")]
    Refused,
}

/// What the last poll found, for the status page and its alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Power {
    /// On line power.
    Online,
    /// On battery.
    OnBattery,
    /// Low battery: the node is stopping.
    LowBattery,
    /// The NUT server did not answer.
    Unreachable,
}

impl Power {
    /// The name the status page and telemetry use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::OnBattery => "on-battery",
            Self::LowBattery => "low-battery",
            Self::Unreachable => "unreachable",
        }
    }
}

/// The last poll's [`Power`], shared with the status page; `None` before the first poll.
pub type PowerState = Arc<Mutex<Option<Power>>>;

impl FromStr for Ups {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        let (name, address) = s.trim().split_once('@').ok_or(())?;
        let (host, port) = match address.rsplit_once(':') {
            Some((host, port)) => (host, port.parse().map_err(drop)?),
            None => (address, NUT_PORT),
        };
        let word = |w: &str| !w.is_empty() && !w.contains(char::is_whitespace);
        if !word(name) || !word(host) {
            return Err(());
        }
        Ok(Self {
            name: name.to_owned(),
            host: host.to_owned(),
            port,
        })
    }
}

impl fmt::Display for Ups {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}:{}", self.name, self.host, self.port)
    }
}

impl Ups {
    /// The UPS's `ups.status` flags, e.g. `["OL"]` or `["OB", "LB"]`.
    pub async fn status(&self) -> Result<Vec<String>, UpsError> {
        tokio::time::timeout(POLL_TIMEOUT, self.ask())
            .await
            .map_err(|_| UpsError::Unreachable)?
    }

    async fn ask(&self) -> Result<Vec<String>, UpsError> {
        let stream = TcpStream::connect((self.host.as_str(), self.port))
            .await
            .map_err(|_| UpsError::Unreachable)?;
        let (read, mut write) = stream.into_split();
        write
            .write_all(format!("GET VAR {} ups.status\n", self.name).as_bytes())
            .await
            .map_err(|_| UpsError::Unreachable)?;
        let mut line = String::new();
        BufReader::new(read)
            .read_line(&mut line)
            .await
            .map_err(|_| UpsError::Unreachable)?;
        // `VAR <ups> ups.status "OB LB"`
        let prefix = format!("VAR {} ups.status ", self.name);
        let quoted = line
            .trim_end()
            .strip_prefix(&prefix)
            .ok_or(UpsError::Refused)?;
        let flags = quoted
            .strip_prefix('"')
            .and_then(|q| q.strip_suffix('"'))
            .ok_or(UpsError::Refused)?;
        Ok(flags.split_whitespace().map(str::to_owned).collect())
    }

    /// Polls every `interval` until the UPS reports low battery, then wakes `low_battery`.
    /// An unreachable NUT server is warned about and polled again: a UPS daemon that is down
    /// never stops the node. Each poll's result is left in `power`.
    pub async fn watch(
        self,
        interval: Duration,
        low_battery: Arc<Notify>,
        power: PowerState,
    ) -> Result<(), String> {
        let mut on_battery = false;
        let seen = |p: Power| *power.lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
        loop {
            match self.status().await {
                Ok(flags) if flags.iter().any(|f| f == "LB") => {
                    seen(Power::LowBattery);
                    tracing::warn!(outcome = "low-battery", "UPS battery low; stopping");
                    low_battery.notify_one();
                    return Ok(());
                }
                Ok(flags) => {
                    let now = flags.iter().any(|f| f == "OB");
                    seen(if now { Power::OnBattery } else { Power::Online });
                    if now != on_battery {
                        if now {
                            tracing::warn!(outcome = "on-battery", "UPS on battery");
                        } else {
                            tracing::info!(outcome = "on-line", "UPS back on line power");
                        }
                    }
                    on_battery = now;
                }
                Err(error) => {
                    seen(Power::Unreachable);
                    tracing::warn!(error.kind = %error, "UPS status unavailable");
                }
            }
            tokio::time::sleep(interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    /// A NUT server answering each connection with the next of `answers`.
    async fn nut(answers: Vec<&'static str>) -> Ups {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        tokio::spawn(async move {
            for answer in answers {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let (read, mut write) = stream.into_split();
                let mut line = String::new();
                let _ = BufReader::new(read).read_line(&mut line).await;
                assert_eq!(line, "GET VAR apc ups.status\n");
                let _ = write.write_all(answer.as_bytes()).await;
            }
        });
        format!("apc@127.0.0.1:{port}").parse().expect("ups")
    }

    #[test]
    fn a_ups_is_named_at_a_host() {
        let ups: Ups = "apc@nut.local".parse().expect("ups");
        assert_eq!(ups.to_string(), "apc@nut.local:3493");
        for bad in ["apc", "@host", "apc@", "apc@host:port", "a pc@host"] {
            assert!(bad.parse::<Ups>().is_err(), "{bad}");
        }
    }

    #[tokio::test]
    async fn the_status_flags_are_read_and_errors_are_refusals() {
        let ups = nut(vec!["VAR apc ups.status \"OB LB\"\n", "ERR UNKNOWN-UPS\n"]).await;
        assert_eq!(ups.status().await, Ok(vec!["OB".into(), "LB".into()]));
        assert_eq!(ups.status().await, Err(UpsError::Refused));
    }

    #[tokio::test]
    async fn low_battery_wakes_the_stop_and_an_outage_does_not() {
        let ups = nut(vec![
            "VAR apc ups.status \"OL\"\n",
            "ERR DRIVER-NOT-CONNECTED\n",
            "VAR apc ups.status \"OB\"\n",
            "VAR apc ups.status \"OB LB\"\n",
        ])
        .await;
        let low = Arc::new(Notify::new());
        let power = PowerState::default();
        let watched = tokio::time::timeout(
            Duration::from_secs(5),
            ups.watch(
                Duration::from_millis(10),
                Arc::clone(&low),
                Arc::clone(&power),
            ),
        )
        .await
        .expect("watch ends");
        assert_eq!(watched, Ok(()));
        assert_eq!(*power.lock().expect("power"), Some(Power::LowBattery));
        tokio::time::timeout(Duration::from_millis(100), low.notified())
            .await
            .expect("woken");
    }
}
