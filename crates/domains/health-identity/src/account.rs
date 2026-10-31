use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::lockout::Lockout;

/// A Login Account's id: a UUIDv7 minted by central.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(pub Uuid);

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Whether an account may start sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStatus {
    /// It may log in, subject to lockout.
    Active,
    /// Revoked or disabled by an administrator; never logs in.
    Disabled,
}

/// A Login Account: the credential a member of staff authenticates with (offline-auth.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginAccount {
    /// The account id.
    pub id: AccountId,
    /// The username, unique per identity provider.
    pub username: String,
    /// The Argon2id hash in PHC string format; opaque here.
    pub credential: String,
    /// Active or disabled.
    pub status: AccountStatus,
    /// Failed-login state, per user (per edge on an edge).
    pub lockout: Lockout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_id_prints_as_its_uuid() {
        let uuid = Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0001);
        assert_eq!(AccountId(uuid).to_string(), uuid.to_string());
    }
}
