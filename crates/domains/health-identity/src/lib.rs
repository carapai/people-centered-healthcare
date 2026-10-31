//! Identity & Access rules (offline-auth.md § Credentials, § Tokens and sessions; ADR 0010).
//!
//! Pure policy: when an account is locked, which passwords are acceptable, how long sessions and
//! tokens live, and what a SMART App Launch v2 scope grants. The protocol, hashing and storage
//! are in `health-auth`.

mod account;
mod lockout;
mod password;
mod scope;
mod session;

pub use account::{AccountId, AccountStatus, LoginAccount};
pub use lockout::{Locked, Lockout, LockoutPolicy};
pub use password::{PasswordPolicy, PasswordRejected};
pub use scope::{Interaction, ResourceScope, Scope, ScopeContext, ScopeError, Scopes};
pub use session::{SessionPolicy, SessionState};
