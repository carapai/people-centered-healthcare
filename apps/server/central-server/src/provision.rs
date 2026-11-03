//! `central-server provision <username> <facility> <role>[,<role>…]`: creates or updates a Login
//! Account with a signed permission snapshot at `facility`, until the workforce registry provisions
//! staff (offline-auth.md § Permission snapshots). The password is read from
//! `HEALTH_PROVISION_PASSWORD_FILE`, and must pass the password policy and the breached-password
//! list (15 characters at central, where sign-in has no device factor).
//!
//! `central-server provision bundle …` exports a facility's provisioning bundle for one edge
//! (ADR 0059).
//!
//! `central-server provision code <facility> --by <operator> …` issues a one-time code an edge
//! provisions itself with (#268).

use std::sync::Arc;

use health_auth::{
    AuthStore, BreachedPasswords, Bundle, BundleAccount, Credentials, SnapshotSigner,
    SurrealAuthStore,
};
use health_identity::snapshot::{PermissionSnapshot, RoleAssignment, SnapshotPolicy};
use health_identity::{
    AccountId, AccountStatus, Lockout, LoginAccount, PasswordPolicy, SessionPolicy,
};
use health_provisioning::NodeRegistry;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};

use crate::config::Config;

/// Provisions one account; see the module documentation.
pub(crate) async fn run(
    config: &Config,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("bundle") => return bundle(config, &args[1..]).await,
        Some("snapshot-keys") => return snapshot_keys(config, &args[1..]),
        Some("code") => return code(config, &args[1..]).await,
        _ => {}
    }
    let [username, facility, roles] = args else {
        return Err(
            "usage: central-server provision <username> <facility> <role>[,<role>…] | bundle <facility> <node-id> <intermediate.pem> <out> | snapshot-keys <out> | code <facility> --by <operator> [--dns <name>]… [--exempt <reason>]".into(),
        );
    };
    let files = config
        .auth
        .as_ref()
        .ok_or("provisioning needs HEALTH_AUTH_ISSUER and the auth settings")?;
    let settings = crate::auth_settings(files)?;
    let policy = settings
        .policy
        .as_ref()
        .ok_or("provisioning needs HEALTH_ACCESS_POLICY_FILE")?;
    let snapshot_key = settings
        .snapshot_keys
        .first()
        .ok_or("provisioning needs HEALTH_AUTH_SNAPSHOT_KEYS_FILE")?;
    let password_file = std::env::var("HEALTH_PROVISION_PASSWORD_FILE")
        .map_err(|_| "HEALTH_PROVISION_PASSWORD_FILE is required")?;
    let password = std::fs::read_to_string(&password_file)
        .map_err(|_| "cannot read HEALTH_PROVISION_PASSWORD_FILE")?
        .trim_end_matches(['\n', '\r'])
        .to_owned();
    let credentials =
        Credentials::new(settings.pepper.clone()).map_err(|_| "the pepper cannot be used")?;

    let store = open_store(config).await?;

    let account = match store.account_by_username(username).await? {
        Some(existing) => existing.id,
        None => AccountId(uuid::Uuid::now_v7()),
    };
    // Sealed for this account under central's pepper key (ADR 0059).
    let credential = credentials
        .set_password(
            account,
            &PasswordPolicy::default(),
            BreachedPasswords::platform(),
            &password,
            false,
        )
        .map_err(|e| format!("the password is refused: {e}"))?;
    store
        .put_account(&LoginAccount {
            id: account,
            username: username.clone(),
            credential,
            status: AccountStatus::Active,
            lockout: Lockout::default(),
        })
        .await?;
    let issued = Timestamp::now();
    let snapshot = PermissionSnapshot {
        account,
        facility: facility.clone(),
        roles: roles
            .split(',')
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .map(|role| RoleAssignment {
                role: role.to_owned(),
                locations: Vec::new(),
            })
            .collect(),
        programs: Vec::new(),
        issued_at: issued,
        not_after: SnapshotPolicy::default().not_after(issued),
        snapshot_version: u64::try_from(issued.as_second()).unwrap_or(0),
    };
    let jws = SnapshotSigner::from_secret(snapshot_key)
        .map_err(|_| "HEALTH_AUTH_SNAPSHOT_KEYS_FILE holds an invalid key")?
        .sign(&snapshot, &policy.roles)
        .map_err(|e| format!("the snapshot is refused: {e}"))?;
    store.put_snapshot(account, facility, &jws).await?;
    std::io::Write::write_all(
        &mut std::io::stdout(),
        format!("provisioned {username} at {facility}\n").as_bytes(),
    )?;
    Ok(())
}

/// The authorization server's store at central, migrated.
async fn open_store(config: &Config) -> Result<SurrealAuthStore, Box<dyn std::error::Error>> {
    Ok(SurrealAuthStore::new(
        open_surreal(config).await?,
        SessionPolicy::default().hard_cap,
    ))
}

/// The database, migrated.
async fn open_surreal(config: &Config) -> Result<SurrealStore, Box<dyn std::error::Error>> {
    let db_password = std::fs::read_to_string(&config.surrealdb_password_file)
        .map_err(|_| "cannot read HEALTH_SURREALDB_PASSWORD_FILE")?
        .trim()
        .to_owned();
    let connection = Connection::open(&Settings {
        target: Target::Remote {
            endpoint: config.surrealdb_endpoint.clone(),
            username: config.surrealdb_user.clone(),
            password: db_password,
        },
        namespace: config.tenant.clone(),
        database: config.database.clone(),
    })
    .await
    .map_err(|_| "the database is unavailable")?;
    migrate(&connection, MIGRATIONS)
        .await
        .map_err(|_| "the database could not be migrated")?;
    Ok(SurrealStore::new(connection, Arc::new(SystemClock)))
}

/// `central-server provision code <facility> --by <operator> [--dns <name>]… [--exempt <reason>]`:
/// a one-time code for an edge to provision itself with (#268), printed once. `--exempt` grants
/// the node an exemption from full-disk encryption (ADR 0048), audited, with its reason.
async fn code(config: &Config, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    const USAGE: &str = "usage: central-server provision code <facility> --by <operator> [--dns <name>]… [--exempt <reason>]";
    let (facility, rest) = args.split_first().ok_or(USAGE)?;
    let (mut by, mut dns, mut exempt) = (None, Vec::new(), None);
    let mut flags = rest.iter();
    while let Some(flag) = flags.next() {
        let value = flags.next().ok_or(USAGE)?.clone();
        match flag.as_str() {
            "--by" => by = Some(value),
            "--dns" => dns.push(value),
            "--exempt" => exempt = Some(value),
            _ => return Err(USAGE.into()),
        }
    }
    let by = by.ok_or(USAGE)?;
    let registry = NodeRegistry::new(
        open_surreal(config).await?,
        Arc::new(SystemClock),
        "central",
    );
    let (text, issued) = registry.issue_code(facility, dns, exempt, &by).await?;
    let mut out = std::io::stdout();
    std::io::Write::write_all(
        &mut out,
        format!(
            "{text}\nfor {facility}, redeemable once until {}\n",
            issued.expires
        )
        .as_bytes(),
    )?;
    Ok(())
}

/// `central-server provision bundle <facility> <node-id> <intermediate.pem> <out>`: the
/// facility's staff and their snapshots for one edge (ADR 0059), signed with the snapshot key, each
/// hash encrypted to the edge's facility intermediate (the certificate central issued it).
async fn bundle(config: &Config, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let [facility, node_id, intermediate, out] = args else {
        return Err(
            "usage: central-server provision bundle <facility> <node-id> <intermediate.pem> <out>"
                .into(),
        );
    };
    let files = config
        .auth
        .as_ref()
        .ok_or("a bundle needs HEALTH_AUTH_ISSUER and the auth settings")?;
    let settings = crate::auth_settings(files)?;
    let signer = settings
        .snapshot_keys
        .first()
        .ok_or("a bundle needs HEALTH_AUTH_SNAPSHOT_KEYS_FILE")
        .and_then(|key| {
            SnapshotSigner::from_secret(key).map_err(|_| "the snapshot key is invalid")
        })?;
    let credentials =
        Credentials::new(settings.pepper.clone()).map_err(|_| "the pepper cannot be used")?;
    let pem =
        std::fs::read_to_string(intermediate).map_err(|_| format!("cannot read {intermediate}"))?;
    let inspected = health_pki::inspect(&pem)
        .map_err(|e| format!("{intermediate} is not a certificate: {e}"))?;
    let recipient = p256::PublicKey::from_sec1_bytes(&inspected.public_key)
        .map_err(|_| format!("{intermediate} does not hold a P-256 key"))?;

    let store = open_store(config).await?;
    let mut accounts = Vec::new();
    let mut snapshots = Vec::new();
    for (id, jws) in store.snapshots_at(facility).await? {
        let Some(account) = store.account(id).await? else {
            continue;
        };
        // Central's own sealing comes off; the bundle encrypts the hash to the edge instead.
        let phc = credentials
            .open(account.id, &account.credential)
            .ok_or_else(|| {
                format!(
                    "{}'s credential does not open under this pepper key",
                    account.username
                )
            })?;
        accounts.push(BundleAccount {
            id: account.id,
            username: account.username,
            status: account.status,
            phc,
        });
        snapshots.push(jws);
    }
    // The country's role mappings and locally grantable roles, for offline administration.
    let local = settings
        .policy
        .as_ref()
        .map(|policy| health_auth::LocalConfig {
            roles: policy.roles.clone(),
            grantable: policy.locally_grantable.clone(),
        })
        .unwrap_or_default();
    let sealed = Bundle {
        facility: facility.clone(),
        node_id: node_id.clone(),
        issued_at: Timestamp::now(),
        accounts,
        snapshots,
        local,
    }
    .seal(&signer, &recipient)?;
    std::fs::write(out, sealed).map_err(|_| format!("cannot write {out}"))?;
    std::io::Write::write_all(
        &mut std::io::stdout(),
        format!(
            "bundle for {node_id} at {facility}, encrypted to {}: {out}\n",
            inspected.spki_sha256
        )
        .as_bytes(),
    )?;
    Ok(())
}

/// `central-server provision snapshot-keys <out>`: the snapshot public keys edges trust
/// (`HEALTH_SNAPSHOT_KEYS_FILE` on an edge), as a JSON array of JWKs, the active key first.
fn snapshot_keys(config: &Config, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let [out] = args else {
        return Err("usage: central-server provision snapshot-keys <out>".into());
    };
    let files = config
        .auth
        .as_ref()
        .ok_or("snapshot keys need HEALTH_AUTH_ISSUER and the auth settings")?;
    let keys = crate::auth_settings(files)?
        .snapshot_keys
        .iter()
        .map(|key| SnapshotSigner::from_secret(key).map(|s| s.public()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "HEALTH_AUTH_SNAPSHOT_KEYS_FILE holds an invalid key")?;
    if keys.is_empty() {
        return Err("HEALTH_AUTH_SNAPSHOT_KEYS_FILE holds no key".into());
    }
    std::fs::write(out, serde_json::to_string_pretty(&keys)?)
        .map_err(|_| format!("cannot write {out}"))?;
    Ok(())
}
