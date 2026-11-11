//! The OIDC / SMART App Launch v2 authorization server (offline-auth.md; ADR 0010). Central and
//! every edge run this same code: central with bearer or DPoP tokens, edges with DPoP required.
//!
//! - **Authorization code with PKCE** (S256 only; RFC 7636, RFC 8252). No password grant.
//! - **Tokens:** ES256 JWT access tokens, at most 15 minutes and never past the session's 12-hour
//!   cap, DPoP-bound (`cnf.jkt`) when the client sends a proof. Rotating refresh tokens: a
//!   replayed one ends the session. ID tokens for `openid`.
//! - **Logout:** `/revoke` ends the session; [`AccessVerifier`] checks the session on every
//!   request, so its access tokens stop working at once.
//! - **Credentials:** Argon2id with the fixed parameters, stored encrypted under the node's pepper
//!   key (ADR 0059); lockout per
//!   `health_identity::LockoutPolicy`; one refusal message for every failure.
//! - **Permission snapshots** (ADR 0011): central signs them ([`SnapshotSigner`]); an edge's
//!   [`SnapshotGate`] requires a valid one for each login and refresh, behind the clock floor, and
//!   tokens carry `facility` and `snapshot_version` and never outlive the snapshot.
//! - **Devices** (lan-security.md; ADR 0009): [`DeviceGate`] enrolls LAN devices by pairing
//!   code and admin approval, keeps the device registry, and refuses any request whose DPoP proof
//!   is not by an enrolled, active device.

mod breached;
mod bundle;
mod caregivers;
mod credentials;
mod devices;
pub mod jose;
mod local_admin;
mod otp;
mod patient;
mod provisioning;
mod server;
mod snapshot;
mod store;
mod subject;
mod surreal;

pub use breached::{BreachedPasswords, InvalidFilter};
pub use bundle::{BUNDLE_TYP, Bundle, BundleAccount, BundleError};
pub use caregivers::{CAREGIVER_PERMISSION, CaregiverRoutes};
pub use credentials::{Credentials, HashError, PEPPER_LEN, SetPasswordError};
pub use devices::{CONSOLE, DeviceAdmin, DeviceConfig, DeviceGate, DeviceStore, EnrolledDevice};
pub use jose::{InvalidKey, TokenSigner};
pub use local_admin::{GrantSigner, LocalAdmin};
pub use otp::{DevOutbox, OtpSendError, OtpSender, PhoneOtp, PhoneSignIn, SendFuture};
pub use patient::patient_layer;
pub use provisioning::{ImportError, Imported, Provisioning, RecipientKey};
pub use server::{
    AccessError, AccessVerifier, AuthConfig, AuthServer, Client, DeviceAnswer, DeviceCheck,
    Principal,
};
pub use snapshot::{SnapshotError, SnapshotGate, SnapshotSigner, SnapshotVerifier};
pub use store::{
    AuthStore, AuthorizationCode, BreakGlassRecord, BreakGlassReview, LocalConfig, MemoryStore,
    SensitiveLabel, Session, StoreError,
};
pub use subject::{
    ALERTS_PERMISSION, ALERTS_WORKLIST, PURPOSE_HEADER, REVIEW_PERMISSION, REVIEW_WITHIN,
    REVIEWS_WORKLIST, Refused, Subjects, signed_in, subject_layer,
};
pub use surreal::SurrealAuthStore;
