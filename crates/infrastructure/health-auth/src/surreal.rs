//! The authorization server's state in SurrealDB (migration `0008_auth`): one transaction per
//! operation, so a code is redeemed at most once and a refresh token rotates at most once even
//! when requests race.

use health_access_policy::BreakGlassEvent;
use health_identity::snapshot::ClockFloor;
use health_identity::{
    AccountId, Lockout, LoginAccount, PhoneAccount, PhoneNumber, Scopes, SessionState,
};
use health_persistence::{Store, Transaction};
use health_surrealdb::{
    AuthSessionRow, BreakGlassReviewRow, BreakGlassRow, SurrealStore, SurrealTx,
};
use health_time::Timestamp;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::otp::PhoneOtp;
use crate::store::{
    AuthStore, AuthorizationCode, BreakGlassRecord, BreakGlassReview, LocalConfig, Session,
    StoreError,
};
use health_identity::snapshot::{LocalGrant, LocalRevocation};

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
        "last_active": session.last_active.to_string(),
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
        // Sessions stored before the idle lock count as active since their login.
        last_active: v["last_active"]
            .as_str()
            .and_then(|t| t.parse().ok())
            .or(match state {
                SessionState::Live { auth_time } => Some(auth_time),
                SessionState::Ended => None,
            })
            .unwrap_or(Timestamp::UNIX_EPOCH),
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
        review: row.review.map(|r| BreakGlassReview {
            reviewer: r.reviewer,
            at: r.at,
            appropriate: r.decision == "appropriate",
            note: r.note,
        }),
        escalated_at: row.escalated_at,
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

    /// Every snapshot at `facility`, by account: what a provisioning bundle carries.
    pub async fn snapshots_at(
        &self,
        facility: &str,
    ) -> Result<Vec<(AccountId, String)>, StoreError> {
        let mut tx = self.begin().await?;
        tx.auth_snapshots_at(facility)
            .await
            .map_err(failed)?
            .into_iter()
            .map(|(account, jws)| Ok((AccountId(account.parse().map_err(failed)?), jws)))
            .collect()
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
    async fn phone_account(&self, phone: &PhoneNumber) -> Result<Option<PhoneAccount>, StoreError> {
        let mut tx = self.begin().await?;
        tx.phone_account_by_phone(phone).await.map_err(failed)
    }

    async fn phone_account_by_id(&self, id: AccountId) -> Result<Option<PhoneAccount>, StoreError> {
        let mut tx = self.begin().await?;
        tx.phone_account(id).await.map_err(failed)
    }

    async fn create_phone_account(
        &self,
        account: PhoneAccount,
    ) -> Result<PhoneAccount, StoreError> {
        let created = async {
            let mut tx = self.begin().await?;
            tx.create_phone_account(&account).await.map_err(failed)?;
            tx.commit().await.map_err(failed)
        };
        // A concurrent sign-in that created it first wins on the unique index.
        let first = created.await;
        match self.phone_account(&account.phone).await? {
            Some(existing) => Ok(existing),
            None => first.map(|()| account),
        }
    }

    async fn phone_otp(&self, phone: &PhoneNumber) -> Result<Option<PhoneOtp>, StoreError> {
        let mut tx = self.begin().await?;
        tx.phone_otp(phone)
            .await
            .map_err(failed)?
            .map(|data| serde_json::from_str(&data).map_err(failed))
            .transpose()
    }

    async fn put_phone_otp(&self, otp: &PhoneOtp) -> Result<(), StoreError> {
        let data = serde_json::to_string(otp).map_err(failed)?;
        let mut tx = self.begin().await?;
        tx.put_phone_otp(&otp.challenge.phone, &data, otp.challenge.issued_at)
            .await
            .map_err(failed)?;
        tx.commit().await.map_err(failed)
    }

    async fn attempt_phone_otp(&self, before: u32, otp: &PhoneOtp) -> Result<bool, StoreError> {
        let data = serde_json::to_string(otp).map_err(failed)?;
        let mut tx = self.begin().await?;
        let updated = tx
            .attempt_phone_otp(&otp.challenge.phone, otp.challenge.issued_at, before, &data)
            .await
            .map_err(failed)?;
        // Two tries that both read `before` both match the WHERE; only one commit succeeds.
        match tx.commit().await {
            Ok(()) => Ok(updated),
            Err(health_persistence::WriteError::Conflict) => Ok(false),
            Err(other) => Err(failed(other)),
        }
    }

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
            account: session.account.to_string(),
            jkt: session.jkt.clone(),
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
        at: Timestamp,
    ) -> Result<bool, StoreError> {
        let Some(mut session) = self.session(id).await? else {
            return Ok(false);
        };
        session.refresh_hash.clone_from(&next);
        session.last_active = at;
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

    async fn device_session(
        &self,
        account: AccountId,
        jkt: &str,
    ) -> Result<Option<Session>, StoreError> {
        let mut tx = self.begin().await?;
        let found = tx
            .device_auth_sessions(&account.to_string(), jkt)
            .await
            .map_err(failed)?;
        for (id, _) in found {
            if let Some(session) = self.session(id.parse().map_err(failed)?).await?
                && matches!(session.state, SessionState::Live { .. })
            {
                return Ok(Some(session));
            }
        }
        Ok(None)
    }

    async fn pin(
        &self,
        account: AccountId,
        jkt: &str,
    ) -> Result<Option<(String, u32)>, StoreError> {
        let mut tx = self.begin().await?;
        tx.auth_pin(&account.to_string(), jkt).await.map_err(failed)
    }

    async fn put_pin(
        &self,
        account: AccountId,
        jkt: &str,
        credential: String,
        failures: u32,
    ) -> Result<(), StoreError> {
        let mut tx = self.begin().await?;
        tx.put_auth_pin(&account.to_string(), jkt, &credential, failures)
            .await
            .map_err(failed)?;
        tx.commit().await.map_err(failed)
    }

    async fn remove_pin(&self, account: AccountId, jkt: &str) -> Result<(), StoreError> {
        let mut tx = self.begin().await?;
        tx.remove_auth_pin(&account.to_string(), jkt)
            .await
            .map_err(failed)?;
        tx.commit().await.map_err(failed)
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
            review: None,
            escalated_at: None,
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

    async fn break_glass_reviews(&self, limit: usize) -> Result<Vec<BreakGlassRecord>, StoreError> {
        let mut tx = self.begin().await?;
        let rows = tx.break_glass_reviews(limit).await.map_err(failed)?;
        Ok(rows.into_iter().filter_map(break_glass_of).collect())
    }

    async fn review_break_glass(
        &self,
        id: &str,
        review: &BreakGlassReview,
        audit: Option<&Value>,
    ) -> Result<bool, StoreError> {
        let row = BreakGlassReviewRow {
            reviewer: review.reviewer.clone(),
            at: review.at,
            decision: if review.appropriate {
                "appropriate"
            } else {
                "inappropriate"
            }
            .into(),
            note: review.note.clone(),
        };
        let mut tx = self.begin().await?;
        if !tx.review_break_glass(id, &row).await.map_err(failed)? {
            return Ok(false);
        }
        // The decision and its AuditEvent commit together, or neither does.
        if let Some(audit) = audit {
            health_audit::append(&mut tx, audit).await.map_err(failed)?;
        }
        tx.commit().await.map_err(failed)?;
        Ok(true)
    }

    async fn escalate_break_glass(
        &self,
        now: Timestamp,
    ) -> Result<Vec<BreakGlassRecord>, StoreError> {
        let mut tx = self.begin().await?;
        let rows = tx.escalate_break_glass(now).await.map_err(failed)?;
        tx.commit().await.map_err(failed)?;
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

    async fn local_grants(
        &self,
        facility: &str,
        account: Option<AccountId>,
        at: Timestamp,
    ) -> Result<Vec<LocalGrant>, StoreError> {
        let mut tx = self.begin().await?;
        let account = account.map(|a| a.to_string());
        tx.local_grants(facility, account.as_deref(), at)
            .await
            .map_err(failed)?
            .iter()
            .map(|d| serde_json::from_str(d).map_err(failed))
            .collect()
    }

    async fn put_local_grant(
        &self,
        grant: &LocalGrant,
        jws: &str,
        audit: &Value,
    ) -> Result<(), StoreError> {
        let data = serde_json::to_string(grant).map_err(failed)?;
        let mut tx = self.begin().await?;
        tx.put_local_grant(
            &grant.id.to_string(),
            &grant.account.to_string(),
            &grant.facility,
            grant.not_after,
            &data,
            jws,
        )
        .await
        .map_err(failed)?;
        // The grant and its AuditEvent commit together, or neither does.
        health_audit::append(&mut tx, audit).await.map_err(failed)?;
        tx.commit().await.map_err(failed)
    }

    async fn local_revocation(
        &self,
        account: AccountId,
    ) -> Result<Option<LocalRevocation>, StoreError> {
        let mut tx = self.begin().await?;
        tx.local_revocation(&account.to_string())
            .await
            .map_err(failed)?
            .map(|d| serde_json::from_str(&d).map_err(failed))
            .transpose()
    }

    async fn local_revocations(&self) -> Result<Vec<LocalRevocation>, StoreError> {
        let mut tx = self.begin().await?;
        tx.local_revocations()
            .await
            .map_err(failed)?
            .iter()
            .map(|d| serde_json::from_str(d).map_err(failed))
            .collect()
    }

    async fn put_local_revocation(
        &self,
        revocation: &LocalRevocation,
        audit: &Value,
    ) -> Result<(), StoreError> {
        let data = serde_json::to_string(revocation).map_err(failed)?;
        let mut tx = self.begin().await?;
        tx.put_local_revocation(&revocation.account.to_string(), &data)
            .await
            .map_err(failed)?;
        health_audit::append(&mut tx, audit).await.map_err(failed)?;
        tx.commit().await.map_err(failed)
    }

    async fn local_config(&self) -> Result<Option<LocalConfig>, StoreError> {
        let mut tx = self.begin().await?;
        tx.local_config()
            .await
            .map_err(failed)?
            .map(|d| serde_json::from_str(&d).map_err(failed))
            .transpose()
    }

    async fn put_local_config(&self, config: &LocalConfig) -> Result<(), StoreError> {
        let data = serde_json::to_string(config).map_err(failed)?;
        let mut tx = self.begin().await?;
        tx.put_local_config(&data).await.map_err(failed)?;
        tx.commit().await.map_err(failed)
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
        let mut tx = self.begin().await?;
        tx.raise_auth_clock_floor(at).await.map_err(failed)?;
        match tx.commit().await {
            // A concurrent request raised the floor first. The floor only moves forward, and a
            // later observation raises it again, so losing the race refuses nothing.
            Ok(()) | Err(health_persistence::WriteError::Conflict) => Ok(()),
            Err(other) => Err(failed(other)),
        }
    }
}

fn device_row(device: &health_identity::Device) -> Result<health_surrealdb::DeviceRow, StoreError> {
    Ok(health_surrealdb::DeviceRow {
        id: device.id.to_string(),
        thumbprint: device.thumbprint.clone(),
        status: match device.status {
            health_identity::DeviceStatus::Active => "active",
            health_identity::DeviceStatus::Revoked => "revoked",
        },
        version: device.version,
        data: serde_json::to_string(device).map_err(failed)?,
    })
}

fn parsed<T: serde::de::DeserializeOwned>(data: Option<String>) -> Result<Option<T>, StoreError> {
    data.map(|d| serde_json::from_str(&d).map_err(failed))
        .transpose()
}

impl crate::devices::DeviceStore for SurrealAuthStore {
    async fn put_enrollment(
        &self,
        request: &health_identity::EnrollmentRequest,
        now: Timestamp,
    ) -> Result<bool, StoreError> {
        let data = serde_json::to_string(request).map_err(failed)?;
        let mut tx = self.begin().await?;
        tx.put_enrollment(
            &request.id.to_string(),
            &request.thumbprint,
            &request.code,
            request.expires,
            &data,
            now,
        )
        .await
        .map_err(failed)?;
        // A code another pending request holds fails the unique index at commit.
        Ok(tx.commit().await.is_ok())
    }

    async fn enrollment_by_code(
        &self,
        code: &str,
    ) -> Result<Option<health_identity::EnrollmentRequest>, StoreError> {
        let mut tx = self.begin().await?;
        parsed(tx.enrollment_by_code(code).await.map_err(failed)?)
    }

    async fn enrollment_by_thumbprint(
        &self,
        thumbprint: &str,
    ) -> Result<Option<health_identity::EnrollmentRequest>, StoreError> {
        let mut tx = self.begin().await?;
        parsed(
            tx.enrollment_by_thumbprint(thumbprint)
                .await
                .map_err(failed)?,
        )
    }

    async fn enrollments(
        &self,
        now: Timestamp,
    ) -> Result<Vec<health_identity::EnrollmentRequest>, StoreError> {
        let mut tx = self.begin().await?;
        tx.enrollments(now)
            .await
            .map_err(failed)?
            .iter()
            .map(|d| serde_json::from_str(d).map_err(failed))
            .collect()
    }

    async fn enroll(
        &self,
        request: Uuid,
        device: &health_identity::Device,
        audit: &Value,
    ) -> Result<bool, StoreError> {
        let row = device_row(device)?;
        let mut tx = self.begin().await?;
        if !tx
            .take_enrollment(&request.to_string())
            .await
            .map_err(failed)?
        {
            return Ok(false);
        }
        if tx.create_device(&row).await.is_err() {
            return Ok(false);
        }
        // The device and its AuditEvent commit together, or neither does; a concurrent
        // approval of the same request conflicts here.
        health_audit::append(&mut tx, audit).await.map_err(failed)?;
        Ok(tx.commit().await.is_ok())
    }

    async fn device_by_thumbprint(
        &self,
        thumbprint: &str,
    ) -> Result<Option<health_identity::Device>, StoreError> {
        let mut tx = self.begin().await?;
        parsed(tx.device_by_thumbprint(thumbprint).await.map_err(failed)?)
    }

    async fn device(
        &self,
        id: health_identity::DeviceId,
    ) -> Result<Option<health_identity::Device>, StoreError> {
        let mut tx = self.begin().await?;
        parsed(tx.device(&id.to_string()).await.map_err(failed)?)
    }

    async fn devices(&self) -> Result<Vec<health_identity::Device>, StoreError> {
        let mut tx = self.begin().await?;
        tx.devices()
            .await
            .map_err(failed)?
            .iter()
            .map(|d| serde_json::from_str(d).map_err(failed))
            .collect()
    }

    async fn update_device(
        &self,
        device: &health_identity::Device,
        before: u64,
        audit: &Value,
    ) -> Result<bool, StoreError> {
        let row = device_row(device)?;
        let mut tx = self.begin().await?;
        if !tx.update_device(&row, before).await.map_err(failed)? {
            return Ok(false);
        }
        health_audit::append(&mut tx, audit).await.map_err(failed)?;
        Ok(tx.commit().await.is_ok())
    }
}
