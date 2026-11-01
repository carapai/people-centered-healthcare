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
    /// The authorization server, when `HEALTH_AUTH_ISSUER` is set.
    pub(crate) auth: Option<AuthFiles>,
}

/// Where the authorization server's settings and secrets are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthFiles {
    /// `HEALTH_AUTH_ISSUER`, e.g. `https://central.example/auth`.
    pub(crate) issuer: String,
    /// `HEALTH_AUTH_KEYS_FILE`: the signing keys, one hex line each, the active key first.
    /// Required with an issuer.
    pub(crate) keys_file: PathBuf,
    /// `HEALTH_AUTH_PEPPER_FILE`: the Argon2id pepper. Optional.
    pub(crate) pepper_file: Option<PathBuf>,
    /// `HEALTH_AUTH_CLIENT_ID`, default `health-client` (the web client).
    pub(crate) client_id: String,
    /// `HEALTH_AUTH_REDIRECT_URIS`: the web client's redirect URIs, comma-separated. Required
    /// with an issuer.
    pub(crate) redirect_uris: Vec<String>,
    /// `HEALTH_AUTH_LOGIN_PAGE`: the web client's sign-in page (e.g. `/sign-in`). Optional:
    /// without it the authorization server shows its own minimal form.
    pub(crate) login_page: Option<String>,
    /// `HEALTH_AUTH_SNAPSHOT_KEYS_FILE`: central's snapshot-signing keys, one hex line each,
    /// the active key first. Optional: with it, sessions act for one facility (ADR 0056).
    pub(crate) snapshot_keys_file: Option<PathBuf>,
    /// `HEALTH_ACCESS_POLICY_FILE`: role → permission mappings and label rules (JSON).
    /// Optional: with it and the snapshot keys, `/fhir` is mounted.
    pub(crate) access_policy_file: Option<PathBuf>,
    /// `HEALTH_REGISTRY_NIN_SYSTEM`: the national ID identifier system. Optional: with it, the
    /// client registry's reviewer operations are served under `/fhir` (#197).
    pub(crate) registry_nin_system: Option<String>,
    /// `HEALTH_NODE_CODE`, default `C1`: this node's short code in the numbers it mints, such as
    /// MRNs (ADR 0014).
    pub(crate) node_code: String,
    /// The patient portal's sign-in (#111), when `HEALTH_PORTAL_REDIRECT_URIS` is set.
    pub(crate) portal: Option<PortalFiles>,
}

/// The patient portal's client and its one-time codes (#111; ADR 0029).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PortalFiles {
    /// `HEALTH_PORTAL_CLIENT_ID`, default `health-portal`.
    pub(crate) client_id: String,
    /// `HEALTH_PORTAL_REDIRECT_URIS`: the portal's redirect URIs, comma-separated.
    pub(crate) redirect_uris: Vec<String>,
    /// `HEALTH_PORTAL_COUNTRY_CODE`, default `256`: the country code of national phone numbers.
    pub(crate) country_code: String,
    /// `HEALTH_DEV_OTP_OUTBOX`: a file that one-time codes are appended to, for development and
    /// e2e only. Required with the portal until an SMS provider is chosen (#235).
    pub(crate) otp_outbox: PathBuf,
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
            auth: match get("HEALTH_AUTH_ISSUER") {
                None => None,
                Some(issuer) => Some(AuthFiles {
                    issuer,
                    keys_file: required("HEALTH_AUTH_KEYS_FILE")?.into(),
                    pepper_file: get("HEALTH_AUTH_PEPPER_FILE").map(PathBuf::from),
                    client_id: get("HEALTH_AUTH_CLIENT_ID")
                        .unwrap_or_else(|| "health-client".to_owned()),
                    redirect_uris: required("HEALTH_AUTH_REDIRECT_URIS")?
                        .split(',')
                        .map(str::trim)
                        .filter(|u| !u.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    login_page: get("HEALTH_AUTH_LOGIN_PAGE"),
                    snapshot_keys_file: get("HEALTH_AUTH_SNAPSHOT_KEYS_FILE").map(PathBuf::from),
                    access_policy_file: get("HEALTH_ACCESS_POLICY_FILE").map(PathBuf::from),
                    registry_nin_system: get("HEALTH_REGISTRY_NIN_SYSTEM"),
                    node_code: get("HEALTH_NODE_CODE").unwrap_or_else(|| "C1".to_owned()),
                    portal: match get("HEALTH_PORTAL_REDIRECT_URIS") {
                        None => None,
                        Some(uris) => Some(PortalFiles {
                            client_id: get("HEALTH_PORTAL_CLIENT_ID")
                                .unwrap_or_else(|| "health-portal".to_owned()),
                            redirect_uris: uris
                                .split(',')
                                .map(str::trim)
                                .filter(|u| !u.is_empty())
                                .map(str::to_owned)
                                .collect(),
                            country_code: get("HEALTH_PORTAL_COUNTRY_CODE")
                                .unwrap_or_else(|| "256".to_owned()),
                            otp_outbox: required("HEALTH_DEV_OTP_OUTBOX")?.into(),
                        }),
                    },
                }),
            },
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
        assert_eq!(config.auth, None);
        let with_auth = Config::from_lookup(lookup(&[
            ("HEALTH_SURREALDB_ENDPOINT", "ws://x"),
            ("HEALTH_SURREALDB_PASSWORD_FILE", "/p"),
            ("HEALTH_AUTH_ISSUER", "https://central.ug/auth"),
            ("HEALTH_AUTH_KEYS_FILE", "/run/secrets/auth-keys"),
            (
                "HEALTH_AUTH_REDIRECT_URIS",
                "https://central.ug/callback, https://central.ug/silent",
            ),
        ]))
        .expect("config")
        .auth
        .expect("auth");
        assert_eq!(with_auth.client_id, "health-client");
        assert_eq!(with_auth.redirect_uris.len(), 2);
        assert_eq!(with_auth.portal, None);
        let auth = [
            ("HEALTH_SURREALDB_ENDPOINT", "ws://x"),
            ("HEALTH_SURREALDB_PASSWORD_FILE", "/p"),
            ("HEALTH_AUTH_ISSUER", "https://central.ug/auth"),
            ("HEALTH_AUTH_KEYS_FILE", "/k"),
            ("HEALTH_AUTH_REDIRECT_URIS", "https://central.ug/callback"),
            (
                "HEALTH_PORTAL_REDIRECT_URIS",
                "https://central.ug/portal/callback",
            ),
        ];
        assert_eq!(
            Config::from_lookup(lookup(&auth)),
            Err(ConfigError::Missing("HEALTH_DEV_OTP_OUTBOX"))
        );
        let mut with_outbox = auth.to_vec();
        with_outbox.push(("HEALTH_DEV_OTP_OUTBOX", "/tmp/otp"));
        let portal = Config::from_lookup(lookup(&with_outbox))
            .expect("config")
            .auth
            .and_then(|a| a.portal)
            .expect("portal");
        assert_eq!(
            (portal.client_id.as_str(), portal.country_code.as_str()),
            ("health-portal", "256")
        );
        assert_eq!(
            Config::from_lookup(lookup(&[
                ("HEALTH_SURREALDB_ENDPOINT", "ws://x"),
                ("HEALTH_SURREALDB_PASSWORD_FILE", "/p"),
                ("HEALTH_AUTH_ISSUER", "https://central.ug/auth"),
            ])),
            Err(ConfigError::Missing("HEALTH_AUTH_KEYS_FILE"))
        );
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
