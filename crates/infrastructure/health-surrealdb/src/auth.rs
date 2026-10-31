//! The authorization server's state (migration `0008_auth`), used by `health-auth`'s
//! SurrealDB store inside one transaction per operation. Codes and refresh tokens are keyed by
//! their SHA-256 hashes; the tokens themselves are never stored.

use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount};
use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;

use crate::store::{SurrealTx, unavailable, write_error};

fn status_code(status: AccountStatus) -> &'static str {
    match status {
        AccountStatus::Active => "active",
        AccountStatus::Disabled => "disabled",
    }
}

const ACCOUNT: &str = "[record::id(id), username, credential, status, <string>failures, \
    IF locked_until THEN <string>locked_until ELSE '' END]";

fn account(row: Vec<String>) -> Result<LoginAccount, StoreError> {
    let invalid = || StoreError::Unavailable("auth_account row has an invalid shape".into());
    let [id, username, credential, status, failures, locked]: [String; 6] =
        row.try_into().map_err(|_| invalid())?;
    Ok(LoginAccount {
        id: AccountId(id.parse().map_err(|_| invalid())?),
        username,
        credential,
        status: if status == "active" {
            AccountStatus::Active
        } else {
            AccountStatus::Disabled
        },
        lockout: Lockout {
            failures: failures.parse().map_err(|_| invalid())?,
            locked_until: if locked.is_empty() {
                None
            } else {
                Some(locked.parse().map_err(|_| invalid())?)
            },
        },
    })
}

/// A stored session: its JSON, as `health-auth` wrote it, and whether it is still live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSessionRow {
    /// The session's JSON.
    pub data: String,
    /// The current refresh token's hash.
    pub refresh_hash: String,
    /// Live, or ended by logout, revocation or replay.
    pub live: bool,
    /// When it stops being refreshable (its purge time).
    pub ends_at: Timestamp,
}

/// A stored break-glass grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakGlassRow {
    /// The grant's id.
    pub id: String,
    /// The session it was granted to.
    pub session: String,
    /// Who broke glass.
    pub account: String,
    /// At which facility.
    pub facility: String,
    /// For whom (`Patient/{id}`).
    pub patient: String,
    /// The reason code (`BTG`).
    pub reason: String,
    /// The user's justification.
    pub justification: String,
    /// When it was granted.
    pub at: Timestamp,
    /// When it ends.
    pub expires_at: Timestamp,
    /// When the privacy officer's review is due.
    pub review_due: Timestamp,
    /// Whether the in-charge acknowledged the alert.
    pub acknowledged: bool,
}

impl SurrealTx {
    async fn rows(
        &mut self,
        sql: &str,
        binds: Vec<(&'static str, String)>,
    ) -> Result<Vec<Vec<String>>, StoreError> {
        let mut query = self.tx()?.query(sql.to_owned());
        for bind in binds {
            query = query.bind(bind);
        }
        query
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)
    }

    async fn run(
        &mut self,
        sql: &str,
        binds: Vec<(&'static str, String)>,
    ) -> Result<Vec<String>, WriteError> {
        let mut query = self.tx()?.query(sql.to_owned());
        for bind in binds {
            query = query.bind(bind);
        }
        query
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(0)
            .map_err(|e| WriteError::Store(unavailable(e)))
    }

    /// Adds or replaces a Login Account (provisioning).
    pub async fn put_login_account(&mut self, account: &LoginAccount) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('auth_account', $id) CONTENT { username: $username, credential: $credential, \
             status: $status, failures: <int>$failures, \
             locked_until: IF $locked = '' THEN NONE ELSE <datetime>$locked END } RETURN VALUE record::id(id)",
            vec![
                ("id", account.id.to_string()),
                ("username", account.username.clone()),
                ("credential", account.credential.clone()),
                ("status", status_code(account.status).to_owned()),
                ("failures", account.lockout.failures.to_string()),
                ("locked", account.lockout.locked_until.map(|t| t.to_string()).unwrap_or_default()),
            ],
        )
        .await?;
        Ok(())
    }

    /// The account with `username`.
    pub async fn login_account_by_username(
        &mut self,
        username: &str,
    ) -> Result<Option<LoginAccount>, StoreError> {
        let rows = self
            .rows(
                &format!(
                    "SELECT VALUE {ACCOUNT} FROM auth_account WHERE username = $username LIMIT 1"
                ),
                vec![("username", username.to_owned())],
            )
            .await?;
        rows.into_iter().next().map(account).transpose()
    }

    /// The account with `id`.
    pub async fn login_account(
        &mut self,
        id: AccountId,
    ) -> Result<Option<LoginAccount>, StoreError> {
        let rows = self
            .rows(
                &format!("SELECT VALUE {ACCOUNT} FROM type::record('auth_account', $id)"),
                vec![("id", id.to_string())],
            )
            .await?;
        rows.into_iter().next().map(account).transpose()
    }

    /// Records an account's lockout state.
    pub async fn set_login_lockout(
        &mut self,
        id: AccountId,
        lockout: Lockout,
    ) -> Result<(), WriteError> {
        self.run(
            "UPDATE type::record('auth_account', $id) SET failures = <int>$failures, \
             locked_until = IF $locked = '' THEN NONE ELSE <datetime>$locked END RETURN VALUE record::id(id)",
            vec![
                ("id", id.to_string()),
                ("failures", lockout.failures.to_string()),
                ("locked", lockout.locked_until.map(|t| t.to_string()).unwrap_or_default()),
            ],
        )
        .await?;
        Ok(())
    }

    /// Stores an authorization code's data under its hash.
    pub async fn put_auth_code(
        &mut self,
        hash: &str,
        data: &str,
        expires_at: Timestamp,
    ) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('auth_code', $hash) CONTENT { data: $data, expires_at: <datetime>$expires } RETURN VALUE record::id(id)",
            vec![("hash", hash.to_owned()), ("data", data.to_owned()), ("expires", expires_at.to_string())],
        )
        .await?;
        Ok(())
    }

    /// Removes and returns a code's data: inside one transaction, so two concurrent
    /// redemptions cannot both see it (one commit conflicts).
    pub async fn take_auth_code(&mut self, hash: &str) -> Result<Option<String>, WriteError> {
        let rows = self
            .run(
                "DELETE type::record('auth_code', $hash) RETURN VALUE $before.data",
                vec![("hash", hash.to_owned())],
            )
            .await?;
        Ok(rows.into_iter().next())
    }

    /// Stores a new session.
    pub async fn put_auth_session(
        &mut self,
        id: &str,
        row: &AuthSessionRow,
    ) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('auth_session', $id) CONTENT { data: $data, refresh_hash: $hash, \
             live: <bool>$live, ends_at: <datetime>$ends } RETURN VALUE record::id(id)",
            vec![
                ("id", id.to_owned()),
                ("data", row.data.clone()),
                ("hash", row.refresh_hash.clone()),
                ("live", row.live.to_string()),
                ("ends", row.ends_at.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    /// A session's stored data.
    pub async fn auth_session(&mut self, id: &str) -> Result<Option<AuthSessionRow>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data, refresh_hash, <string>live, <string>ends_at] FROM type::record('auth_session', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        rows.into_iter().next().map(session_row).transpose()
    }

    /// The session whose current or retired refresh token hashes to `hash`, and whether it was
    /// the current one.
    pub async fn auth_session_by_refresh(
        &mut self,
        hash: &str,
    ) -> Result<Option<(String, AuthSessionRow, bool)>, StoreError> {
        let current = self
            .rows(
                "SELECT VALUE [record::id(id), data, refresh_hash, <string>live, <string>ends_at] FROM auth_session \
                 WHERE refresh_hash = $hash LIMIT 1",
                vec![("hash", hash.to_owned())],
            )
            .await?;
        if let Some(mut row) = current.into_iter().next() {
            let id = row.remove(0);
            return Ok(Some((id, session_row(row)?, true)));
        }
        let retired = self
            .rows(
                "SELECT VALUE [session] FROM type::record('auth_retired_refresh', $hash)",
                vec![("hash", hash.to_owned())],
            )
            .await?;
        let Some(id) = retired
            .into_iter()
            .next()
            .and_then(|r| r.into_iter().next())
        else {
            return Ok(None);
        };
        Ok(self.auth_session(&id).await?.map(|row| (id, row, false)))
    }

    /// Rotates a session's refresh token if `previous` is still current (compare-and-set),
    /// retiring it; `data` is the session's JSON with the new hash.
    pub async fn rotate_auth_refresh(
        &mut self,
        id: &str,
        previous: &str,
        next: &str,
        data: &str,
    ) -> Result<bool, WriteError> {
        let updated = self
            .run(
                "UPDATE type::record('auth_session', $id) SET refresh_hash = $next, data = $data \
                 WHERE refresh_hash = $previous AND live = true RETURN VALUE record::id(id)",
                vec![
                    ("id", id.to_owned()),
                    ("previous", previous.to_owned()),
                    ("next", next.to_owned()),
                    ("data", data.to_owned()),
                ],
            )
            .await?;
        if updated.is_empty() {
            return Ok(false);
        }
        self.run(
            "CREATE type::record('auth_retired_refresh', $previous) CONTENT { session: $id, \
             ends_at: (SELECT VALUE ends_at FROM ONLY type::record('auth_session', $id)) } RETURN VALUE record::id(id)",
            vec![("id", id.to_owned()), ("previous", previous.to_owned())],
        )
        .await?;
        Ok(true)
    }

    /// Ends a session; `data` is its JSON in the ended state.
    pub async fn end_auth_session(&mut self, id: &str, data: &str) -> Result<(), WriteError> {
        self.run(
            "UPDATE type::record('auth_session', $id) SET live = false, data = $data RETURN VALUE record::id(id)",
            vec![("id", id.to_owned()), ("data", data.to_owned())],
        )
        .await?;
        Ok(())
    }

    /// Records a DPoP proof's `jti` until `until`; false if it is already recorded (a replay).
    pub async fn remember_auth_jti(
        &mut self,
        jti: &str,
        until: Timestamp,
    ) -> Result<bool, WriteError> {
        let seen = self
            .rows(
                "SELECT VALUE [record::id(id)] FROM type::record('auth_jti', $jti)",
                vec![("jti", jti.to_owned())],
            )
            .await?;
        if !seen.is_empty() {
            return Ok(false);
        }
        self.run(
            "CREATE type::record('auth_jti', $jti) CONTENT { until: <datetime>$until } RETURN VALUE record::id(id)",
            vec![("jti", jti.to_owned()), ("until", until.to_string())],
        )
        .await?;
        Ok(true)
    }

    /// A permission snapshot (the signed JWS) for `account` at `facility`.
    pub async fn auth_snapshot(
        &mut self,
        account: AccountId,
        facility: &str,
    ) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [jws] FROM type::record('auth_snapshot', [$account, $facility])",
                vec![
                    ("account", account.to_string()),
                    ("facility", facility.to_owned()),
                ],
            )
            .await?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
    }

    /// The facilities `account` holds snapshots for, in id order: a range scan over the
    /// `[account, facility]` record ids.
    pub async fn auth_snapshot_facilities(
        &mut self,
        account: AccountId,
    ) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [record::id(id)[1]] FROM auth_snapshot:[$account, NONE]..=[$account, {}]",
                vec![("account", account.to_string())],
            )
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }

    /// Stores a permission snapshot, replacing the previous one.
    pub async fn put_auth_snapshot(
        &mut self,
        account: AccountId,
        facility: &str,
        jws: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('auth_snapshot', [$account, $facility]) CONTENT { jws: $jws } RETURN VALUE record::id(id)[1]",
            vec![("account", account.to_string()), ("facility", facility.to_owned()), ("jws", jws.to_owned())],
        )
        .await?;
        Ok(())
    }

    /// Records a break-glass grant (migration `0009_break_glass`).
    pub async fn put_break_glass(&mut self, row: &BreakGlassRow) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('break_glass', $id) CONTENT { session: $grant_session, account: $account, \
             facility: $facility, patient: $patient, reason: $reason, justification: $justification, \
             at: <datetime>$at, expires_at: <datetime>$expires_at, review_due: <datetime>$review_due, \
             acknowledged: false } RETURN VALUE record::id(id)",
            vec![
                ("id", row.id.clone()),
                // `$session` is reserved by SurrealDB.
                ("grant_session", row.session.clone()),
                ("account", row.account.clone()),
                ("facility", row.facility.clone()),
                ("patient", row.patient.clone()),
                ("reason", row.reason.clone()),
                ("justification", row.justification.clone()),
                ("at", row.at.to_string()),
                ("expires_at", row.expires_at.to_string()),
                ("review_due", row.review_due.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    /// The latest break-glass grant of `session` still active at `now`.
    pub async fn active_break_glass(
        &mut self,
        session: &str,
        now: Timestamp,
    ) -> Result<Option<BreakGlassRow>, StoreError> {
        let rows = self
            .break_glass_rows(
                "WHERE session = $key AND expires_at > <datetime>$now ORDER BY at DESC LIMIT 1",
                session,
                Some(now),
            )
            .await?;
        Ok(rows.into_iter().next())
    }

    /// The unacknowledged break-glass alerts of `facility`, oldest first.
    pub async fn break_glass_alerts(
        &mut self,
        facility: &str,
    ) -> Result<Vec<BreakGlassRow>, StoreError> {
        self.break_glass_rows(
            "WHERE facility = $key AND acknowledged = false ORDER BY at ASC LIMIT 1000",
            facility,
            None,
        )
        .await
    }

    /// Marks alert `id` of `facility` acknowledged; false when there is none.
    pub async fn acknowledge_break_glass(
        &mut self,
        id: &str,
        facility: &str,
    ) -> Result<bool, WriteError> {
        let updated = self
            .run(
                "UPDATE type::record('break_glass', $id) SET acknowledged = true \
                 WHERE facility = $facility RETURN VALUE record::id(id)",
                vec![("id", id.to_owned()), ("facility", facility.to_owned())],
            )
            .await?;
        Ok(!updated.is_empty())
    }

    async fn break_glass_rows(
        &mut self,
        filter: &str,
        key: &str,
        now: Option<Timestamp>,
    ) -> Result<Vec<BreakGlassRow>, StoreError> {
        let mut binds = vec![("key", key.to_owned())];
        if let Some(now) = now {
            binds.push(("now", now.to_string()));
        }
        let rows = self
            .rows(
                &format!(
                    "SELECT VALUE [<string>record::id(id), session, account, facility, patient, reason, \
                     justification, <string>at, <string>expires_at, <string>review_due, <string>acknowledged] \
                     FROM break_glass {filter}"
                ),
                binds,
            )
            .await?;
        rows.into_iter()
            .map(|r| {
                let ts = |i: usize| {
                    r.get(i)
                        .and_then(|t| t.parse::<Timestamp>().ok())
                        .ok_or_else(|| StoreError::Unavailable("break_glass row is invalid".into()))
                };
                let text = |i: usize| r.get(i).cloned().unwrap_or_default();
                Ok(BreakGlassRow {
                    id: text(0),
                    session: text(1),
                    account: text(2),
                    facility: text(3),
                    patient: text(4),
                    reason: text(5),
                    justification: text(6),
                    at: ts(7)?,
                    expires_at: ts(8)?,
                    review_due: ts(9)?,
                    acknowledged: text(10) == "true",
                })
            })
            .collect()
    }

    /// The persisted clock floor.
    pub async fn auth_clock_floor(&mut self) -> Result<Option<Timestamp>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [<string>at] FROM auth_clock_floor:floor",
                vec![],
            )
            .await?;
        rows.into_iter()
            .next()
            .and_then(|r| r.into_iter().next())
            .map(|t| {
                t.parse()
                    .map_err(|_| StoreError::Unavailable("auth_clock_floor is invalid".into()))
            })
            .transpose()
    }

    /// Raises the clock floor to `at`; it never moves back.
    pub async fn raise_auth_clock_floor(&mut self, at: Timestamp) -> Result<(), WriteError> {
        self.run(
            "UPSERT auth_clock_floor:floor SET at = IF at AND at > <datetime>$at THEN at ELSE <datetime>$at END RETURN VALUE <string>at",
            vec![("at", at.to_string())],
        )
        .await?;
        Ok(())
    }

    /// Deletes expired codes and jtis, and sessions (with their retired refresh hashes) past their
    /// end; returns how many rows went.
    pub async fn purge_auth(&mut self, now: Timestamp) -> Result<usize, WriteError> {
        let mut gone = 0;
        for (table, field) in [
            ("auth_code", "expires_at"),
            ("auth_jti", "until"),
            ("auth_retired_refresh", "ends_at"),
            ("auth_session", "ends_at"),
        ] {
            let ids = self
                .run(&format!("DELETE {table} WHERE {field} < <datetime>$now RETURN VALUE <string>record::id($before.id)"), vec![("now", now.to_string())])
                .await?;
            gone += ids.len();
        }
        Ok(gone)
    }
}

fn session_row(row: Vec<String>) -> Result<AuthSessionRow, StoreError> {
    let invalid = || StoreError::Unavailable("auth_session row has an invalid shape".into());
    let [data, refresh_hash, live, ends_at]: [String; 4] = row.try_into().map_err(|_| invalid())?;
    Ok(AuthSessionRow {
        data,
        refresh_hash,
        live: live == "true",
        ends_at: ends_at.parse().map_err(|_| invalid())?,
    })
}
