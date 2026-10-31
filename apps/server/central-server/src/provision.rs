//! `central-server provision <username> <facility> <role>[,<role>…]`: creates or updates a Login
//! Account with a signed permission snapshot at `facility`, until the workforce registry provisions
//! staff (offline-auth.md § Permission snapshots). The password is read from
//! `HEALTH_PROVISION_PASSWORD_FILE`, and must pass the password policy and the breached-password
//! list (15 characters at central, where sign-in has no device factor).

use std::sync::Arc;

use health_auth::{AuthStore, BreachedPasswords, Credentials, SnapshotSigner, SurrealAuthStore};
use health_identity::snapshot::{PermissionSnapshot, RoleAssignment, SnapshotPolicy};
use health_identity::{
    AccountId, AccountStatus, Lockout, LoginAccount, PasswordPolicy, SessionPolicy,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};

use crate::config::Config;

/// Provisions one account; see the module documentation.
pub(crate) async fn run(
    config: &Config,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let [username, facility, roles] = args else {
        return Err(
            "usage: central-server provision <username> <facility> <role>[,<role>…]".into(),
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
    let credential = credentials
        .set_password(
            &PasswordPolicy::default(),
            BreachedPasswords::platform(),
            &password,
            false,
        )
        .map_err(|e| format!("the password is refused: {e}"))?;

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
    let store = SurrealAuthStore::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        SessionPolicy::default().hard_cap,
    );

    let account = match store.account_by_username(username).await? {
        Some(existing) => existing.id,
        None => AccountId(uuid::Uuid::now_v7()),
    };
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
