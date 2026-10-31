//! The authorization server's state, behind a port: accounts, one-time authorization codes,
//! sessions with rotating refresh tokens, and the DPoP replay cache. `MemoryStore` serves tests
//! and development; the SurrealDB adapter persists it.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;

use health_identity::snapshot::ClockFloor;
use health_identity::{AccountId, Lockout, LoginAccount, Scopes, SessionState};
use health_time::Timestamp;
use uuid::Uuid;

/// A store failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("authorization store: {0}")]
pub struct StoreError(pub String);

/// An issued authorization code, redeemable once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationCode {
    /// The client it was issued to.
    pub client_id: String,
    /// The exact redirect URI of the request.
    pub redirect_uri: String,
    /// The PKCE S256 challenge.
    pub code_challenge: String,
    /// The scopes granted.
    pub scopes: Scopes,
    /// The OpenID Connect nonce, if sent.
    pub nonce: Option<String>,
    /// Who logged in.
    pub account: AccountId,
    /// The facility the session acts for, when the node keeps snapshots.
    pub facility: Option<String>,
    /// When the password was checked.
    pub auth_time: Timestamp,
    /// The code is refused after this.
    pub expires_at: Timestamp,
}

/// A login session, refreshable until the hard cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// The session id, carried in tokens as `sid`.
    pub id: Uuid,
    /// The account.
    pub account: AccountId,
    /// The client.
    pub client_id: String,
    /// The scopes granted.
    pub scopes: Scopes,
    /// Live or ended.
    pub state: SessionState,
    /// The facility the session acts for, when the node keeps snapshots (ADR 0056).
    pub facility: Option<String>,
    /// The DPoP key thumbprint the session's tokens are bound to, if any.
    pub jkt: Option<String>,
    /// The SHA-256 of the current refresh token; the previous one is refused.
    pub refresh_hash: String,
}

/// The authorization server's persistence port.
pub trait AuthStore: Send + Sync {
    /// The account with `username`.
    fn account_by_username(
        &self,
        username: &str,
    ) -> impl Future<Output = Result<Option<LoginAccount>, StoreError>> + Send;
    /// The account with `id`.
    fn account(
        &self,
        id: AccountId,
    ) -> impl Future<Output = Result<Option<LoginAccount>, StoreError>> + Send;
    /// Records an account's lockout state.
    fn set_lockout(
        &self,
        id: AccountId,
        lockout: Lockout,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;
    /// Stores a new code under `code_hash`.
    fn put_code(
        &self,
        code_hash: String,
        code: AuthorizationCode,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;
    /// Removes and returns the code: a code is redeemed at most once.
    fn take_code(
        &self,
        code_hash: &str,
    ) -> impl Future<Output = Result<Option<AuthorizationCode>, StoreError>> + Send;
    /// Stores a new session.
    fn put_session(&self, session: Session) -> impl Future<Output = Result<(), StoreError>> + Send;
    /// The session with `id`.
    fn session(&self, id: Uuid)
    -> impl Future<Output = Result<Option<Session>, StoreError>> + Send;
    /// The session whose current or previous refresh token hashes to `refresh_hash`, and whether
    /// it was the current one. A previous one means the token was replayed.
    fn session_by_refresh(
        &self,
        refresh_hash: &str,
    ) -> impl Future<Output = Result<Option<(Session, bool)>, StoreError>> + Send;
    /// Rotates the refresh token, if `previous` is still the current one (compare-and-set).
    fn rotate_refresh(
        &self,
        id: Uuid,
        previous: &str,
        next: String,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;
    /// Ends a session.
    fn end_session(&self, id: Uuid) -> impl Future<Output = Result<(), StoreError>> + Send;
    /// Records a DPoP proof's `jti` until `until`; false if it was already seen (a replay).
    fn remember_jti(
        &self,
        jti: &str,
        until: Timestamp,
        now: Timestamp,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;
    /// The signed permission snapshot (a compact JWS) for `account` at `facility`, as central
    /// provisioned it. Only edges keep snapshots; the default has none.
    fn snapshot(
        &self,
        account: AccountId,
        facility: &str,
    ) -> impl Future<Output = Result<Option<String>, StoreError>> + Send {
        let _ = (account, facility);
        async { Ok(None) }
    }
    /// The facilities `account` holds snapshots for, sorted. The default has none.
    fn facilities(
        &self,
        account: AccountId,
    ) -> impl Future<Output = Result<Vec<String>, StoreError>> + Send {
        let _ = account;
        async { Ok(Vec::new()) }
    }
    /// The persisted clock floor.
    fn clock_floor(&self) -> impl Future<Output = Result<ClockFloor, StoreError>> + Send {
        async { Ok(ClockFloor::default()) }
    }
    /// Persists the clock floor; it never moves back.
    fn set_clock_floor(
        &self,
        floor: ClockFloor,
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        let _ = floor;
        async { Ok(()) }
    }
}

#[derive(Debug, Default)]
struct State {
    accounts: HashMap<AccountId, LoginAccount>,
    codes: HashMap<String, AuthorizationCode>,
    sessions: HashMap<Uuid, Session>,
    /// Retired refresh-token hashes, to detect replay.
    retired: HashMap<String, Uuid>,
    jtis: HashMap<String, Timestamp>,
    snapshots: HashMap<(AccountId, String), String>,
    floor: ClockFloor,
}

/// An in-memory store.
#[derive(Debug, Default)]
pub struct MemoryStore(Mutex<State>);

impl MemoryStore {
    /// Adds or replaces an account (provisioning from central does this in production).
    pub fn put_account(&self, account: LoginAccount) {
        self.lock().accounts.insert(account.id, account);
    }

    /// Stores a signed snapshot, as provisioning from central does.
    pub fn put_snapshot(&self, account: AccountId, facility: &str, jws: String) {
        self.lock()
            .snapshots
            .insert((account, facility.to_owned()), jws);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl AuthStore for MemoryStore {
    async fn account_by_username(
        &self,
        username: &str,
    ) -> Result<Option<LoginAccount>, StoreError> {
        Ok(self
            .lock()
            .accounts
            .values()
            .find(|a| a.username == username)
            .cloned())
    }

    async fn account(&self, id: AccountId) -> Result<Option<LoginAccount>, StoreError> {
        Ok(self.lock().accounts.get(&id).cloned())
    }

    async fn set_lockout(&self, id: AccountId, lockout: Lockout) -> Result<(), StoreError> {
        if let Some(account) = self.lock().accounts.get_mut(&id) {
            account.lockout = lockout;
        }
        Ok(())
    }

    async fn put_code(&self, code_hash: String, code: AuthorizationCode) -> Result<(), StoreError> {
        self.lock().codes.insert(code_hash, code);
        Ok(())
    }

    async fn take_code(&self, code_hash: &str) -> Result<Option<AuthorizationCode>, StoreError> {
        Ok(self.lock().codes.remove(code_hash))
    }

    async fn put_session(&self, session: Session) -> Result<(), StoreError> {
        self.lock().sessions.insert(session.id, session);
        Ok(())
    }

    async fn session(&self, id: Uuid) -> Result<Option<Session>, StoreError> {
        Ok(self.lock().sessions.get(&id).cloned())
    }

    async fn session_by_refresh(
        &self,
        refresh_hash: &str,
    ) -> Result<Option<(Session, bool)>, StoreError> {
        let state = self.lock();
        if let Some(session) = state
            .sessions
            .values()
            .find(|s| s.refresh_hash == refresh_hash)
        {
            return Ok(Some((session.clone(), true)));
        }
        Ok(state
            .retired
            .get(refresh_hash)
            .and_then(|id| state.sessions.get(id))
            .map(|s| (s.clone(), false)))
    }

    async fn rotate_refresh(
        &self,
        id: Uuid,
        previous: &str,
        next: String,
    ) -> Result<bool, StoreError> {
        let mut state = self.lock();
        let Some(session) = state.sessions.get_mut(&id) else {
            return Ok(false);
        };
        if session.refresh_hash != previous {
            return Ok(false);
        }
        session.refresh_hash = next;
        state.retired.insert(previous.to_owned(), id);
        Ok(true)
    }

    async fn end_session(&self, id: Uuid) -> Result<(), StoreError> {
        if let Some(session) = self.lock().sessions.get_mut(&id) {
            session.state = SessionState::Ended;
        }
        Ok(())
    }

    async fn remember_jti(
        &self,
        jti: &str,
        until: Timestamp,
        now: Timestamp,
    ) -> Result<bool, StoreError> {
        let mut state = self.lock();
        state.jtis.retain(|_, expiry| *expiry > now);
        Ok(state.jtis.insert(jti.to_owned(), until).is_none())
    }

    async fn snapshot(
        &self,
        account: AccountId,
        facility: &str,
    ) -> Result<Option<String>, StoreError> {
        Ok(self
            .lock()
            .snapshots
            .get(&(account, facility.to_owned()))
            .cloned())
    }

    async fn facilities(&self, account: AccountId) -> Result<Vec<String>, StoreError> {
        let mut facilities: Vec<String> = self
            .lock()
            .snapshots
            .keys()
            .filter(|(a, _)| *a == account)
            .map(|(_, f)| f.clone())
            .collect();
        facilities.sort();
        Ok(facilities)
    }

    async fn clock_floor(&self) -> Result<ClockFloor, StoreError> {
        Ok(self.lock().floor)
    }

    async fn set_clock_floor(&self, floor: ClockFloor) -> Result<(), StoreError> {
        let mut state = self.lock();
        if let Some(at) = floor.get() {
            state.floor = state.floor.observe(at);
        }
        Ok(())
    }
}
