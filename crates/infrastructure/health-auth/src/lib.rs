//! The OIDC / SMART App Launch v2 authorization server (offline-auth.md; ADR 0010). Central and
//! every edge run this same code: central with bearer or DPoP tokens, edges with DPoP required.
//!
//! - **Authorization code with PKCE** (S256 only; RFC 7636, RFC 8252). No password grant.
//! - **Tokens:** ES256 JWT access tokens, at most 15 minutes and never past the session's 12-hour
//!   cap, DPoP-bound (`cnf.jkt`) when the client sends a proof. Rotating refresh tokens: a
//!   replayed one ends the session. ID tokens for `openid`.
//! - **Logout:** `/revoke` ends the session; [`AccessVerifier`] checks the session on every
//!   request, so its access tokens stop working at once.
//! - **Credentials:** Argon2id with the fixed parameters and an optional pepper; lockout per
//!   `health_identity::LockoutPolicy`; one refusal message for every failure.
//! - **Permission snapshots** (ADR 0011): central signs them ([`SnapshotSigner`]); an edge's
//!   [`SnapshotGate`] requires a valid one for each login and refresh, behind the clock floor, and
//!   tokens carry `facility` and `snapshot_version` and never outlive the snapshot.

mod breached;
mod credentials;
pub mod jose;
mod otp;
mod patient;
mod server;
mod snapshot;
mod store;
mod subject;
mod surreal;

pub use breached::{BreachedPasswords, InvalidFilter};
pub use credentials::{Credentials, HashError, SetPasswordError};
pub use jose::{InvalidKey, TokenSigner};
pub use otp::{DevOutbox, OtpSendError, OtpSender, PhoneOtp, PhoneSignIn, SendFuture};
pub use patient::patient_layer;
pub use server::{AccessError, AccessVerifier, AuthConfig, AuthServer, Client, Principal};
pub use snapshot::{SnapshotError, SnapshotGate, SnapshotSigner, SnapshotVerifier};
pub use store::{
    AuthStore, AuthorizationCode, BreakGlassRecord, BreakGlassReview, MemoryStore, Session,
    StoreError,
};
pub use subject::{
    ALERTS_PERMISSION, ALERTS_WORKLIST, PURPOSE_HEADER, REVIEW_PERMISSION, REVIEW_WITHIN,
    REVIEWS_WORKLIST, Refused, Subjects, subject_layer,
};
pub use surreal::SurrealAuthStore;
