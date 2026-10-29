//! Central's configuration, from the environment (deployment.md; secrets come from files, never
//! from the environment itself).

use std::net::SocketAddr;
use std::path::PathBuf;

/// What central needs to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Config {
    /// `HEALTH_BIND`, default `0.0.0.0:8080`.
    pub(crate) bind: SocketAddr,
    /// `HEALTH_SURREALDB_ENDPOINT`, e.g. `ws://surrealdb:8000`. Required.
    pub(crate) surrealdb_endpoint: String,
    /// `HEALTH_SURREALDB_USER`, default `root`.
    pub(crate) surrealdb_user: String,
    /// `HEALTH_SURREALDB_PASSWORD_FILE`: a file holding the password. Required.
    pub(crate) surrealdb_password_file: PathBuf,
    /// `HEALTH_TENANT`, the SurrealDB namespace, default `ug`.
    pub(crate) tenant: String,
    /// `HEALTH_DATABASE`, default `main`.
    pub(crate) database: String,
    /// `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, the in-country collector. Optional: without it
    /// only logs are written.
    pub(crate) otlp_endpoint: Option<String>,
    /// `HEALTH_LOG`, an `EnvFilter` directive, default `info`.
    pub(crate) log: String,
}

/// A missing or invalid setting. It names the variable, never its value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ConfigError {
    /// A required variable is unset or empty.
    #[error("{0} is required")]
    Missing(&'static str),
    /// A variable cannot be read.
    #[error("{0} is invalid")]
    Invalid(&'static str),
}

impl Config {
    /// Reads the configuration from `lookup` (the process environment in `main`).
    pub(crate) fn from_lookup(
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, ConfigError> {
        let get = |name: &str| lookup(name).filter(|v| !v.trim().is_empty());
        let required = |name: &'static str| get(name).ok_or(ConfigError::Missing(name));
        Ok(Self {
            bind: get("HEALTH_BIND")
                .unwrap_or_else(|| "0.0.0.0:8080".to_owned())
                .parse()
                .map_err(|_| ConfigError::Invalid("HEALTH_BIND"))?,
            surrealdb_endpoint: required("HEALTH_SURREALDB_ENDPOINT")?,
            surrealdb_user: get("HEALTH_SURREALDB_USER").unwrap_or_else(|| "root".to_owned()),
            surrealdb_password_file: required("HEALTH_SURREALDB_PASSWORD_FILE")?.into(),
            tenant: get("HEALTH_TENANT").unwrap_or_else(|| "ug".to_owned()),
            database: get("HEALTH_DATABASE").unwrap_or_else(|| "main".to_owned()),
            otlp_endpoint: get("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"),
            log: get("HEALTH_LOG").unwrap_or_else(|| "info".to_owned()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        }
    }

    #[test]
    fn defaults_apply_and_required_settings_are_named() {
        let config = Config::from_lookup(lookup(&[
            ("HEALTH_SURREALDB_ENDPOINT", "ws://surrealdb:8000"),
            ("HEALTH_SURREALDB_PASSWORD_FILE", "/run/secrets/surrealdb"),
        ]))
        .expect("config");
        assert_eq!(config.bind.to_string(), "0.0.0.0:8080");
        assert_eq!(
            (config.tenant.as_str(), config.database.as_str()),
            ("ug", "main")
        );
        assert_eq!(config.otlp_endpoint, None);
        assert_eq!(
            Config::from_lookup(lookup(&[("HEALTH_SURREALDB_ENDPOINT", "ws://x")])),
            Err(ConfigError::Missing("HEALTH_SURREALDB_PASSWORD_FILE"))
        );
        assert_eq!(
            Config::from_lookup(lookup(&[
                ("HEALTH_SURREALDB_ENDPOINT", "ws://x"),
                ("HEALTH_SURREALDB_PASSWORD_FILE", "/p"),
                ("HEALTH_BIND", "not an address"),
            ])),
            Err(ConfigError::Invalid("HEALTH_BIND"))
        );
    }
}
