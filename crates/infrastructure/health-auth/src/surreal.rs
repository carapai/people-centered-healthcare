//! The authorization server's state in SurrealDB (migration `0008_auth`): one transaction per
//! operation, so a code is redeemed at most once and a refresh token rotates at most once even
//! when requests race.

use health_access_policy::BreakGlassEvent;
use health_identity::snapshot::ClockFloor;
use health_identity::{AccountId, Lockout, LoginAccount, Scopes, SessionState};
use health_persistence::{Store, Transaction};
use health_surrealdb::{AuthSessionRow, BreakGlassRow, SurrealStore, SurrealTx};
use health_time::Timestamp;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::store::{AuthStore, AuthorizationCode, BreakGlassRecord, Session, StoreError};

/// The SurrealDB [`AuthStore`].
#[derive(Clone)]
pub struct SurrealAuthStore {
    store: SurrealStore,
    session_cap: health_time::SignedDuration,
}

fn failed(error: impl std::fmt::Display) -> StoreError {
    StoreError(error.to_string())
}

fn code_json(code: &AuthorizationCode) -> Value {
    json!({
        "client_id": code.client_id,
        "redirect_uri": code.redirect_uri,
        "code_challenge": code.code_challenge,
        "scopes": code.scopes.to_string(),
        "nonce": code.nonce,
        "account": code.account.to_string(),
        "facility": code.facility,
        "auth_time": code.auth_time.to_string(),
        "expires_at": code.expires_at.to_string(),
    })
}

fn code_of(v: &Value) -> Option<AuthorizationCode> {
    let ts = |k: &str| v[k].as_str()?.parse::<Timestamp>().ok();
    Some(AuthorizationCode {
        client_id: v["client_id"].as_str()?.to_owned(),
        redirect_uri: v["redirect_uri"].as_str()?.to_owned(),
        code_challenge: v["code_challenge"].as_str()?.to_owned(),
        scopes: Scopes::parse(v["scopes"].as_str()?).ok()?,
        nonce: v["nonce"].as_str().map(str::to_owned),
        account: AccountId(v["account"].as_str()?.parse().ok()?),
        facility: v["facility"].as_str().map(str::to_owned),
        auth_time: ts("auth_time")?,
        expires_at: ts("expires_at")?,
    })
}

fn session_json(session: &Session) -> Value {
    let auth_time = match session.state {
        SessionState::Live { auth_time } => Some(auth_time.to_string()),
        SessionState::Ended => None,
    };
    json!({
        "account": session.account.to_string(),
        "client_id": session.client_id,
        "scopes": session.scopes.to_string(),
        "auth_time": auth_time,
        "facility": session.facility,
        "jkt": session.jkt,
        "refresh_hash": session.refresh_hash,
    })
}

fn session_of(id: &str, row: &AuthSessionRow) -> Option<Session> {
    let v: Value = serde_json::from_str(&row.data).ok()?;
    let state = match (
        row.live,
        v["auth_time"].as_str().and_then(|t| t.parse().ok()),
    ) {
        (true, Some(auth_time)) => SessionState::Live { auth_time },
        _ => SessionState::Ended,
    };
    Some(Session {
        id: id.parse().ok()?,
        account: AccountId(v["account"].as_str()?.parse().ok()?),
        client_id: v["client_id"].as_str()?.to_owned(),
        scopes: Scopes::parse(v["scopes"].as_str()?).ok()?,
        state,
        facility: v["facility"].as_str().map(str::to_owned),
        jkt: v["jkt"].as_str().map(str::to_owned),
        refresh_hash: row.refresh_hash.clone(),
    })
}

fn break_glass_of(row: BreakGlassRow) -> Option<BreakGlassRecord> {
    Some(BreakGlassRecord {
        id: row.id,
        session: row.session.parse().ok()?,
        event: BreakGlassEvent {
            account: row.account,
            facility: row.facility,
            patient: row.patient,
            reason: row.reason,
            justification: row.justification,
            at: row.at,
            expires_at: row.expires_at,
        },
        review_due: row.review_due,
        acknowledged: row.acknowledged,
    })
}

impl SurrealAuthStore {
    /// The store over `store`, purging sessions `session_cap` after their login.
    pub fn new(store: SurrealStore, session_cap: health_time::SignedDuration) -> Self {
        Self { store, session_cap }
    }

    /// Adds or replaces a Login Account (provisioning from central).
    pub async fn put_account(&self, account: &LoginAccount) -> Result<(), StoreError> {
        {
            let mut tx = self.begin().await?;
            tx.put_login_account(account).await.map_err(failed)?;
            tx.commit().await.map_err(failed)?;
            Ok::<_, StoreError>(())
        }
    }

    /// Stores a signed permission snapshot (provisioning from central).
    pub async fn put_snapshot(
        &self,
        account: AccountId,
        facility: &str,
        jws: &str,
    ) -> Result<(), StoreError> {
        {
            let mut tx = self.begin().await?;
            tx.put_auth_snapshot(account, facility, jws)
                .await
                .map_err(failed)?;
            tx.commit().await.map_err(failed)?;
            Ok::<_, StoreError>(())
        }
    }

    /// Deletes expired codes, jtis and sessions (a background job); returns how many rows went.
    pub async fn purge(&self, now: Timestamp) -> Result<usize, StoreError> {
        {
            let mut tx = self.begin().await?;
            let value = tx.purge_auth(now).await.map_err(failed)?;
            tx.commit().await.map_err(failed)?;
            Ok::<_, StoreError>(value)
        }
    }

    async fn begin(&self) -> Result<SurrealTx, StoreError> {
        self.store.begin().await.map_err(failed)
    }
}

impl AuthStore for SurrealAuthStore {
    async fn account_by_username(
        &self,
        username: &str,
    ) -> Result<Option<LoginAccount>, StoreError> {
        let mut tx = self.begin().await?;
        tx.login_account_by_username(username).await.map_err(failed)
    }

    async fn account(&self, id: AccountId) -> Result<Option<LoginAccount>, StoreError> {
        let mut tx = self.begin().await?;
        tx.login_account(id).await.map_err(failed)
    }

    async fn set_lockout(&self, id: AccountId, lockout: Lockout) -> Result<(), StoreError> {
        {
            let mut tx = self.begin().await?;
            tx.set_login_lockout(id, lockout).await.map_err(failed)?;
            tx.commit().await.map_err(failed)?;
            Ok::<_, StoreError>(())
        }
    }

    async fn put_code(&self, code_hash: String, code: AuthorizationCode) -> Result<(), StoreError> {
        let data = code_json(&code).to_string();
        {
            let mut tx = self.begin().await?;
            tx.put_auth_code(&code_hash, &data, code.expires_at)
                .await
                .map_err(failed)?;
            tx.commit().await.map_err(failed)?;
            Ok::<_, StoreError>(())
        }
    }

    async fn take_code(&self, code_hash: &str) -> Result<Option<AuthorizationCode>, StoreError> {
        let mut tx = self.begin().await?;
        let data: Option<String> = tx.take_auth_code(code_hash).await.map_err(failed)?;
        tx.commit().await.map_err(failed)?;
        Ok(data
            .and_then(|d| serde_json::from_str::<Value>(&d).ok())
            .and_then(|v| code_of(&v)))
    }

    async fn put_session(&self, session: Session) -> Result<(), StoreError> {
        let auth_time = match session.state {
            SessionState::Live { auth_time } => auth_time,
            SessionState::Ended => Timestamp::now(),
        };
        let row = AuthSessionRow {
            data: session_json(&session).to_string(),
            refresh_hash: session.refresh_hash.clone(),
            live: matches!(session.state, SessionState::Live { .. }),
            ends_at: auth_time.checked_add(self.session_cap).unwrap_or(auth_time),
        };
        let id = session.id.to_string();
        {
            let mut tx = self.begin().await?;
            tx.put_auth_session(&id, &row).await.map_err(failed)?;
            tx.commit().await.map_err(failed)?;
            Ok::<_, StoreError>(())
        }
    }

    async fn session(&self, id: Uuid) -> Result<Option<Session>, StoreError> {
        let mut tx = self.begin().await?;
        let id = id.to_string();
        Ok(tx
            .auth_session(&id)
            .await
            .map_err(failed)?
            .and_then(|row| session_of(&id, &row)))
    }

    async fn session_by_refresh(
        &self,
        refresh_hash: &str,
    ) -> Result<Option<(Session, bool)>, StoreError> {
        let mut tx = self.begin().await?;
        let found = tx
            .auth_session_by_refresh(refresh_hash)
            .await
            .map_err(failed)?;
        Ok(found.and_then(|(id, row, current)| session_of(&id, &row).map(|s| (s, current))))
    }

    async fn rotate_refresh(
        &self,
        id: Uuid,
        previous: &str,
        next: String,
    ) -> Result<bool, StoreError> {
        let Some(mut session) = self.session(id).await? else {
            return Ok(false);
        };
        session.refresh_hash.clone_from(&next);
        let data = session_json(&session).to_string();
        let id = id.to_string();
        {
            let mut tx = self.begin().await?;
            let value = tx
                .rotate_auth_refresh(&id, previous, &next, &data)
                .await
                .map_err(failed)?;
            tx.commit().await.map_err(failed)?;
            Ok::<_, StoreError>(value)
        }
    }

    async fn end_session(&self, id: Uuid) -> Result<(), StoreError> {
        let Some(mut session) = self.session(id).await? else {
            return Ok(());
        };
        session.state = SessionState::Ended;
        let data = session_json(&session).to_string();
        let id = id.to_string();
        {
            let mut tx = self.begin().await?;
            tx.end_auth_session(&id, &data).await.map_err(failed)?;
            tx.commit().await.map_err(failed)?;
            Ok::<_, StoreError>(())
        }
    }

    async fn remember_jti(
        &self,
        jti: &str,
        until: Timestamp,
        _now: Timestamp,
    ) -> Result<bool, StoreError> {
        // Two concurrent uses of one jti both find it absent; only one commit succeeds, and the
        // other fails as a store error, so the proof is refused either way.
        {
            let mut tx = self.begin().await?;
            let value = tx.remember_auth_jti(jti, until).await.map_err(failed)?;
            tx.commit().await.map_err(failed)?;
            Ok::<_, StoreError>(value)
        }
    }

    async fn snapshot(
        &self,
        account: AccountId,
        facility: &str,
    ) -> Result<Option<String>, StoreError> {
        let mut tx = self.begin().await?;
        tx.auth_snapshot(account, facility).await.map_err(failed)
    }

    async fn grant_break_glass(
        &self,
        record: &BreakGlassRecord,
        audit: Option<&Value>,
    ) -> Result<(), StoreError> {
        let event = &record.event;
        let row = BreakGlassRow {
            id: record.id.clone(),
            session: record.session.to_string(),
            account: event.account.clone(),
            facility: event.facility.clone(),
            patient: event.patient.clone(),
            reason: event.reason.clone(),
            justification: event.justification.clone(),
            at: event.at,
            expires_at: event.expires_at,
            review_due: record.review_due,
            acknowledged: record.acknowledged,
        };
        let mut tx = self.begin().await?;
        tx.put_break_glass(&row).await.map_err(failed)?;
        // The grant and its AuditEvent commit together, or neither does.
        if let Some(audit) = audit {
            health_audit::append(&mut tx, audit).await.map_err(failed)?;
        }
        tx.commit().await.map_err(failed)
    }

    async fn active_break_glass(
        &self,
        session: Uuid,
        now: Timestamp,
    ) -> Result<Option<BreakGlassRecord>, StoreError> {
        let mut tx = self.begin().await?;
        let row = tx
            .active_break_glass(&session.to_string(), now)
            .await
            .map_err(failed)?;
        Ok(row.and_then(break_glass_of))
    }

    async fn break_glass_alerts(
        &self,
        facility: &str,
    ) -> Result<Vec<BreakGlassRecord>, StoreError> {
        let mut tx = self.begin().await?;
        let rows = tx.break_glass_alerts(facility).await.map_err(failed)?;
        Ok(rows.into_iter().filter_map(break_glass_of).collect())
    }

    async fn acknowledge_break_glass(&self, id: &str, facility: &str) -> Result<bool, StoreError> {
        let mut tx = self.begin().await?;
        let done = tx
            .acknowledge_break_glass(id, facility)
            .await
            .map_err(failed)?;
        tx.commit().await.map_err(failed)?;
        Ok(done)
    }

    async fn facilities(&self, account: AccountId) -> Result<Vec<String>, StoreError> {
        let mut tx = self.begin().await?;
        tx.auth_snapshot_facilities(account).await.map_err(failed)
    }

    async fn clock_floor(&self) -> Result<ClockFloor, StoreError> {
        let mut tx = self.begin().await?;
        Ok(tx
            .auth_clock_floor()
            .await
            .map_err(failed)?
            .map_or_else(ClockFloor::default, ClockFloor::at))
    }

    async fn set_clock_floor(&self, floor: ClockFloor) -> Result<(), StoreError> {
        let Some(at) = floor.get() else { return Ok(()) };
        {
            let mut tx = self.begin().await?;
            tx.raise_auth_clock_floor(at).await.map_err(failed)?;
            tx.commit().await.map_err(failed)?;
            Ok::<_, StoreError>(())
        }
    }
}
