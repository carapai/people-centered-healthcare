//! The edge's configuration, from the service environment (deployment.md; secrets come from
//! files, never from the environment itself).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::ups::Ups;

/// What the edge needs to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeConfig {
    /// `HEALTH_BIND`, default `0.0.0.0:443` (lan-security.md: the edge listens on TCP 443 only).
    /// HTTPS, once the facility intermediate is installed; before that, nothing listens here.
    pub bind: SocketAddr,
    /// `HEALTH_CONSOLE_BIND`, default `127.0.0.1:8080`: plain HTTP for the edge's own screen,
    /// where the admin pages are served until staff sign-in (#75). A loopback address only, so
    /// nothing on the LAN reaches it, and never `HEALTH_BIND`'s own address and port.
    pub console_bind: SocketAddr,
    /// `HEALTH_DATA_DIR`: the embedded store's directory, owned by this process alone. Required.
    pub data_dir: PathBuf,
    /// `HEALTH_TENANT`, the SurrealDB namespace, default `ug`.
    pub tenant: String,
    /// `HEALTH_DATABASE`, default `main`.
    pub database: String,
    /// `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`. Optional: without it only logs are written.
    pub otlp_endpoint: Option<String>,
    /// `HEALTH_LOG`, an `EnvFilter` directive, default `info`.
    pub log: String,
    /// `HEALTH_UPS`, the NUT UPS to watch, as `name@host[:port]`. Optional: without it,
    /// durability rests on `sync=every`.
    pub ups: Option<Ups>,
    /// `HEALTH_UPS_POLL_SECS`, default 5.
    pub ups_poll: Duration,
    /// `HEALTH_DRAIN_SECS`: how long in-flight requests may take to finish on a stop, default 20.
    pub drain: Duration,
    /// `HEALTH_UPDATE_DIR`: staged packages, the previous binary and the pre-update store copy.
    /// Default: `updates` beside the data directory, so renames stay on one volume.
    pub update_dir: PathBuf,
    /// `HEALTH_UPDATE_KEY_FILE`: the pinned release key (hex SEC1), for staging packages.
    pub update_key_file: Option<PathBuf>,
    /// `HEALTH_NODE_ID`: the name backups are taken under, default `facility-edge`.
    pub node_id: String,
    /// `HEALTH_BACKUP_DIR`: where backups go, on the second volume. Optional: without it the
    /// node takes none, and says so at every start.
    pub backup_dir: Option<PathBuf>,
    /// `HEALTH_BACKUP_KEY_FILE`: the node's backup key, 64 hex characters. Required with
    /// `HEALTH_BACKUP_DIR`.
    pub backup_key_file: Option<PathBuf>,
    /// `HEALTH_BACKUP_INTERVAL_SECS`, default 3600 (hourly).
    pub backup_interval: Duration,
    /// `HEALTH_BACKUP_TEST_SECS`: how often the newest backup is test-restored, default 604800
    /// (weekly).
    pub backup_test_interval: Duration,
    /// `HEALTH_TELEMETRY_DIR`: the bounded buffer of log lines awaiting shipping. Default:
    /// `telemetry` beside the data directory.
    pub telemetry_dir: PathBuf,
    /// `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT`: the in-country collector's OTLP/HTTP logs endpoint,
    /// e.g. `http://otel-collector:4318/v1/logs`. Optional: without it, lines stay buffered.
    pub logs_endpoint: Option<String>,
    /// `HEALTH_TELEMETRY_SHIP_SECS`: how often the buffer is shipped, default 30.
    pub ship_interval: Duration,
    /// `HEALTH_INDICATORS_SECS`: how often the indicators are logged and alerts raised,
    /// default 60.
    pub indicators_interval: Duration,
    /// `HEALTH_ENCRYPTION_STATE_FILE`: where a privileged probe left the disk-encryption
    /// state (the Linux unit's `ExecStartPre`). Without it, the node probes itself, which needs
    /// administrator rights (the Windows service runs as LocalSystem).
    pub encryption_state_file: Option<PathBuf>,
    /// `HEALTH_ENCRYPTION_EXEMPTION`: the country-configuration grant exempting this facility
    /// from full-disk encryption (ADR 0048), shown as a standing alert.
    pub encryption_exemption: Option<String>,
    /// `HEALTH_ENCRYPTION_SECS`: how often disk encryption is checked, default 3600.
    pub encryption_interval: Duration,
    /// `HEALTH_PKI_DIR`: the national root and the facility intermediate's chain, installed at
    /// provisioning. Default: `pki` beside the data directory. The intermediate's key is in the
    /// keystore, never here.
    pub pki_dir: PathBuf,
    /// `HEALTH_ORIGINS`: the origins LAN devices reach this node by, comma-separated, default
    /// `https://<node-id>.local`. A device's DPoP proofs must name one of them.
    pub origins: Vec<String>,
    /// `HEALTH_TLS_DIR`: the server leaf and its key, which the node issues and renews itself.
    /// Default: `tls` beside the data directory.
    pub tls_dir: PathBuf,
    /// `HEALTH_DNS_NAME`: the facility's configured DNS name, named in the leaf beside
    /// `<node-id>.local`.
    pub dns_name: Option<String>,
    /// `HEALTH_CERTIFICATE_SECS`: how often the leaf is checked for renewal and address
    /// changes, default 3600.
    pub certificate_interval: Duration,
    /// `HEALTH_MDNS`: `off` stops the `_health-edge._tcp` announcement, which is on by default
    /// while the node serves HTTPS.
    pub mdns: bool,
}

/// A configuration value that is missing or malformed. Names the variable, never its value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// A required variable is unset.
    #[error("{0} is required")]
    Missing(&'static str),
    /// A variable does not parse.
    #[error("{0} is malformed")]
    Malformed(&'static str),
}

impl EdgeConfig {
    /// Reads the configuration through `lookup` (the process environment, in production).
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let get = |name: &str| lookup(name).filter(|v| !v.trim().is_empty());
        let seconds = |name: &'static str, default: u64| -> Result<Duration, ConfigError> {
            get(name)
                .map(|v| v.trim().parse().map_err(|_| ConfigError::Malformed(name)))
                .transpose()
                .map(|s| Duration::from_secs(s.unwrap_or(default)))
        };
        let positive = |name: &'static str, default: u64| match seconds(name, default)? {
            d if d.is_zero() => Err(ConfigError::Malformed(name)),
            d => Ok(d),
        };
        let node_id = get("HEALTH_NODE_ID").unwrap_or_else(|| "facility-edge".into());
        let origins = match get("HEALTH_ORIGINS") {
            Some(list) => list
                .split(',')
                .map(|o| o.trim().trim_end_matches('/').to_owned())
                .filter(|o| !o.is_empty())
                .map(|o| {
                    let host = o
                        .strip_prefix("https://")
                        .or_else(|| o.strip_prefix("http://"))
                        .ok_or(ConfigError::Malformed("HEALTH_ORIGINS"))?;
                    if host.is_empty() || host.contains(['/', '?', '#']) {
                        return Err(ConfigError::Malformed("HEALTH_ORIGINS"));
                    }
                    Ok(o)
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => vec![format!("https://{node_id}.local")],
        };
        let data_dir: PathBuf = get("HEALTH_DATA_DIR")
            .map(PathBuf::from)
            .ok_or(ConfigError::Missing("HEALTH_DATA_DIR"))?;
        let bind: SocketAddr = get("HEALTH_BIND")
            .unwrap_or_else(|| "0.0.0.0:443".into())
            .parse()
            .map_err(|_| ConfigError::Malformed("HEALTH_BIND"))?;
        let console_bind: SocketAddr = get("HEALTH_CONSOLE_BIND")
            .unwrap_or_else(|| "127.0.0.1:8080".into())
            .parse()
            .ok()
            .filter(|c: &SocketAddr| c.ip().is_loopback())
            .ok_or(ConfigError::Malformed("HEALTH_CONSOLE_BIND"))?;
        // A wildcard LAN address covers the loopback one too.
        if console_bind.port() == bind.port()
            && (bind.ip().is_unspecified() || bind.ip() == console_bind.ip())
        {
            return Err(ConfigError::Malformed("HEALTH_CONSOLE_BIND"));
        }
        Ok(Self {
            update_dir: get("HEALTH_UPDATE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.with_file_name("updates")),
            update_key_file: get("HEALTH_UPDATE_KEY_FILE").map(PathBuf::from),
            telemetry_dir: get("HEALTH_TELEMETRY_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.with_file_name("telemetry")),
            pki_dir: get("HEALTH_PKI_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.with_file_name("pki")),
            tls_dir: get("HEALTH_TLS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.with_file_name("tls")),
            data_dir,
            bind,
            console_bind,
            tenant: get("HEALTH_TENANT").unwrap_or_else(|| "ug".into()),
            database: get("HEALTH_DATABASE").unwrap_or_else(|| "main".into()),
            otlp_endpoint: get("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"),
            log: get("HEALTH_LOG").unwrap_or_else(|| "info".into()),
            ups: get("HEALTH_UPS")
                .map(|v| v.parse().map_err(|_| ConfigError::Malformed("HEALTH_UPS")))
                .transpose()?,
            ups_poll: seconds("HEALTH_UPS_POLL_SECS", 5)?,
            drain: seconds("HEALTH_DRAIN_SECS", 20)?,
            node_id,
            origins,
            dns_name: get("HEALTH_DNS_NAME").map(|n| n.trim().to_ascii_lowercase()),
            certificate_interval: positive("HEALTH_CERTIFICATE_SECS", 3600)?,
            mdns: match get("HEALTH_MDNS").as_deref().map(str::trim) {
                None | Some("on") => true,
                Some("off") => false,
                Some(_) => return Err(ConfigError::Malformed("HEALTH_MDNS")),
            },
            backup_key_file: match (get("HEALTH_BACKUP_DIR"), get("HEALTH_BACKUP_KEY_FILE")) {
                (Some(_), None) => return Err(ConfigError::Missing("HEALTH_BACKUP_KEY_FILE")),
                (_, key) => key.map(PathBuf::from),
            },
            backup_dir: get("HEALTH_BACKUP_DIR").map(PathBuf::from),
            backup_interval: positive("HEALTH_BACKUP_INTERVAL_SECS", 3600)?,
            backup_test_interval: positive("HEALTH_BACKUP_TEST_SECS", 604_800)?,
            logs_endpoint: get("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT"),
            ship_interval: positive("HEALTH_TELEMETRY_SHIP_SECS", 30)?,
            indicators_interval: positive("HEALTH_INDICATORS_SECS", 60)?,
            encryption_state_file: get("HEALTH_ENCRYPTION_STATE_FILE").map(PathBuf::from),
            encryption_exemption: get("HEALTH_ENCRYPTION_EXEMPTION").map(|g| g.trim().to_owned()),
            encryption_interval: positive("HEALTH_ENCRYPTION_SECS", 3600)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(vars: &[(&str, &str)]) -> Result<EdgeConfig, ConfigError> {
        EdgeConfig::from_lookup(|name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_owned())
        })
    }

    #[test]
    fn the_data_directory_is_required_and_the_rest_defaults() {
        assert_eq!(config(&[]), Err(ConfigError::Missing("HEALTH_DATA_DIR")));
        let c = config(&[("HEALTH_DATA_DIR", "/var/lib/health")]).expect("config");
        assert_eq!(c.bind.to_string(), "0.0.0.0:443");
        assert_eq!(c.tls_dir, PathBuf::from("/var/lib/tls"));
        assert_eq!(c.dns_name, None);
        assert_eq!(c.certificate_interval, Duration::from_secs(3600));
        assert!(c.mdns);
        assert_eq!((c.tenant.as_str(), c.database.as_str()), ("ug", "main"));
        assert_eq!(c.ups, None);
        assert_eq!(c.drain, Duration::from_secs(20));
        assert_eq!(c.update_dir, PathBuf::from("/var/lib/updates"));
        assert_eq!((c.backup_dir, c.backup_key_file), (None, None));
        assert_eq!(c.backup_interval, Duration::from_secs(3600));
        assert_eq!(c.backup_test_interval, Duration::from_secs(604_800));
        assert_eq!(c.telemetry_dir, PathBuf::from("/var/lib/telemetry"));
        assert_eq!(c.logs_endpoint, None);
        assert_eq!(
            (c.ship_interval, c.indicators_interval),
            (Duration::from_secs(30), Duration::from_secs(60))
        );
        assert_eq!(
            (c.encryption_state_file, c.encryption_exemption),
            (None, None)
        );
        assert_eq!(c.encryption_interval, Duration::from_secs(3600));
        assert_eq!(c.pki_dir, PathBuf::from("/var/lib/pki"));
        assert_eq!(c.origins, vec!["https://facility-edge.local".to_owned()]);
    }

    #[test]
    fn origins_are_bare_http_or_https() {
        let dir = ("HEALTH_DATA_DIR", "/d");
        let c = config(&[
            dir,
            (
                "HEALTH_ORIGINS",
                "https://ug-kla-0001.local/, http://10.0.0.5:8080",
            ),
        ])
        .expect("config");
        assert_eq!(
            c.origins,
            vec!["https://ug-kla-0001.local", "http://10.0.0.5:8080"]
        );
        for bad in ["ug-kla-0001.local", "https://x.local/api", "ftp://x"] {
            assert_eq!(
                config(&[dir, ("HEALTH_ORIGINS", bad)]),
                Err(ConfigError::Malformed("HEALTH_ORIGINS")),
                "{bad}"
            );
        }
    }

    #[test]
    fn the_console_listens_on_the_loopback_address_only() {
        let dir = ("HEALTH_DATA_DIR", "/d");
        let c = config(&[dir]).expect("config");
        assert_eq!(c.console_bind.to_string(), "127.0.0.1:8080");
        let c = config(&[dir, ("HEALTH_CONSOLE_BIND", "[::1]:9000")]).expect("config");
        assert_eq!(c.console_bind.to_string(), "[::1]:9000");
        for bad in ["0.0.0.0:8080", "10.0.0.5:8080", "nowhere"] {
            assert_eq!(
                config(&[dir, ("HEALTH_CONSOLE_BIND", bad)]),
                Err(ConfigError::Malformed("HEALTH_CONSOLE_BIND")),
                "{bad}"
            );
        }
        // Never the LAN listener's own address and port.
        for bind in ["0.0.0.0:8080", "127.0.0.1:8080", "[::]:8080"] {
            assert_eq!(
                config(&[dir, ("HEALTH_BIND", bind)]),
                Err(ConfigError::Malformed("HEALTH_CONSOLE_BIND")),
                "{bind}"
            );
        }
        assert!(config(&[dir, ("HEALTH_BIND", "10.0.0.5:8080")]).is_ok());
    }

    #[test]
    fn backups_need_their_key() {
        let dir = ("HEALTH_DATA_DIR", "/d");
        let backups = ("HEALTH_BACKUP_DIR", "/backup");
        assert_eq!(
            config(&[dir, backups]),
            Err(ConfigError::Missing("HEALTH_BACKUP_KEY_FILE"))
        );
        assert_eq!(
            config(&[dir, ("HEALTH_BACKUP_INTERVAL_SECS", "0")]),
            Err(ConfigError::Malformed("HEALTH_BACKUP_INTERVAL_SECS"))
        );
        let c = config(&[dir, backups, ("HEALTH_BACKUP_KEY_FILE", "/k")]).expect("config");
        assert_eq!(c.backup_dir, Some(PathBuf::from("/backup")));
        assert_eq!(c.backup_key_file, Some(PathBuf::from("/k")));
    }

    #[test]
    fn malformed_values_name_their_variable() {
        let dir = ("HEALTH_DATA_DIR", "/d");
        assert_eq!(
            config(&[dir, ("HEALTH_BIND", "nowhere")]),
            Err(ConfigError::Malformed("HEALTH_BIND"))
        );
        assert_eq!(
            config(&[dir, ("HEALTH_UPS", "@host")]),
            Err(ConfigError::Malformed("HEALTH_UPS"))
        );
        assert_eq!(
            config(&[dir, ("HEALTH_DRAIN_SECS", "soon")]),
            Err(ConfigError::Malformed("HEALTH_DRAIN_SECS"))
        );
        let c = config(&[dir, ("HEALTH_UPS", "apc@127.0.0.1")]).expect("config");
        assert_eq!(c.ups.expect("ups").to_string(), "apc@127.0.0.1:3493");
    }
}
