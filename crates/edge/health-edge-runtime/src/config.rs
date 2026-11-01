//! The edge's configuration, from the service environment (deployment.md; secrets come from
//! files, never from the environment itself).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::ups::Ups;

/// What the edge needs to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeConfig {
    /// `HEALTH_BIND`, default `0.0.0.0:8080`.
    pub bind: SocketAddr,
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
        let data_dir: PathBuf = get("HEALTH_DATA_DIR")
            .map(PathBuf::from)
            .ok_or(ConfigError::Missing("HEALTH_DATA_DIR"))?;
        Ok(Self {
            update_dir: get("HEALTH_UPDATE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.with_file_name("updates")),
            update_key_file: get("HEALTH_UPDATE_KEY_FILE").map(PathBuf::from),
            data_dir,
            bind: get("HEALTH_BIND")
                .unwrap_or_else(|| "0.0.0.0:8080".into())
                .parse()
                .map_err(|_| ConfigError::Malformed("HEALTH_BIND"))?,
            tenant: get("HEALTH_TENANT").unwrap_or_else(|| "ug".into()),
            database: get("HEALTH_DATABASE").unwrap_or_else(|| "main".into()),
            otlp_endpoint: get("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"),
            log: get("HEALTH_LOG").unwrap_or_else(|| "info".into()),
            ups: get("HEALTH_UPS")
                .map(|v| v.parse().map_err(|_| ConfigError::Malformed("HEALTH_UPS")))
                .transpose()?,
            ups_poll: seconds("HEALTH_UPS_POLL_SECS", 5)?,
            drain: seconds("HEALTH_DRAIN_SECS", 20)?,
            node_id: get("HEALTH_NODE_ID").unwrap_or_else(|| "facility-edge".into()),
            backup_key_file: match (get("HEALTH_BACKUP_DIR"), get("HEALTH_BACKUP_KEY_FILE")) {
                (Some(_), None) => return Err(ConfigError::Missing("HEALTH_BACKUP_KEY_FILE")),
                (_, key) => key.map(PathBuf::from),
            },
            backup_dir: get("HEALTH_BACKUP_DIR").map(PathBuf::from),
            backup_interval: positive("HEALTH_BACKUP_INTERVAL_SECS", 3600)?,
            backup_test_interval: positive("HEALTH_BACKUP_TEST_SECS", 604_800)?,
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
        assert_eq!(c.bind.to_string(), "0.0.0.0:8080");
        assert_eq!((c.tenant.as_str(), c.database.as_str()), ("ug", "main"));
        assert_eq!(c.ups, None);
        assert_eq!(c.drain, Duration::from_secs(20));
        assert_eq!(c.update_dir, PathBuf::from("/var/lib/updates"));
        assert_eq!((c.backup_dir, c.backup_key_file), (None, None));
        assert_eq!(c.backup_interval, Duration::from_secs(3600));
        assert_eq!(c.backup_test_interval, Duration::from_secs(604_800));
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
