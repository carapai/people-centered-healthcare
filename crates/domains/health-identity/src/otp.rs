use health_time::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

use crate::PhoneNumber;

/// What a one-time code is for. A patient signs in with a code to their verified phone every
/// time; there is no patient password.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OtpPurpose {
    /// Verifying a new phone number, which creates its Login Account (`level1`).
    SignUp,
    /// Signing in to an existing account.
    SignIn,
}

/// The one-time code rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OtpPolicy {
    /// Digits in a code.
    pub digits: u32,
    /// How long a code is valid.
    pub lifetime: SignedDuration,
    /// Wrong codes before the challenge is spent.
    pub max_attempts: u32,
    /// The least time between two codes to one phone.
    pub resend_after: SignedDuration,
    /// Codes to one phone per hour.
    pub per_hour: usize,
}

impl Default for OtpPolicy {
    /// Six digits, valid 5 minutes, 5 tries; one code a minute and 5 an hour per phone.
    fn default() -> Self {
        Self {
            digits: 6,
            lifetime: SignedDuration::from_mins(5),
            max_attempts: 5,
            resend_after: SignedDuration::from_mins(1),
            per_hour: 5,
        }
    }
}

/// Why a code was not sent, or not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OtpRefused {
    /// A code was sent less than a minute ago.
    #[error("wait before asking for another code")]
    TooSoon {
        /// When another may be sent.
        until: Timestamp,
    },
    /// Too many codes to this phone in the last hour.
    #[error("too many codes; try again later")]
    TooMany {
        /// When another may be sent.
        until: Timestamp,
    },
    /// The code has expired.
    #[error("the code has expired")]
    Expired,
    /// The code was already used.
    #[error("the code was already used")]
    Used,
    /// Too many wrong codes: ask for a new one.
    #[error("too many wrong codes")]
    Spent,
    /// The wrong code.
    #[error("the wrong code")]
    Wrong {
        /// Tries left.
        remaining: u32,
    },
}

/// A code sent to a phone. The code itself is never kept, only its hash, which this crate never
/// reads: the caller compares the presented code with it in constant time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtpChallenge {
    /// The phone it was sent to.
    pub phone: PhoneNumber,
    /// What it is for.
    pub purpose: OtpPurpose,
    /// The code's hash; opaque here.
    pub code_hash: String,
    /// When it was sent.
    pub issued_at: Timestamp,
    /// When it stops being valid.
    pub expires_at: Timestamp,
    /// Codes presented so far.
    pub attempts: u32,
    /// Whether it was accepted.
    pub used: bool,
}

impl OtpPolicy {
    /// Whether a code may be sent to a phone whose earlier codes were sent at `sent`.
    pub fn may_send(&self, sent: &[Timestamp], now: Timestamp) -> Result<(), OtpRefused> {
        if let Some(last) = sent.iter().max()
            && now < *last + self.resend_after
        {
            return Err(OtpRefused::TooSoon {
                until: *last + self.resend_after,
            });
        }
        let hour = SignedDuration::from_hours(1);
        let mut recent: Vec<Timestamp> =
            sent.iter().copied().filter(|at| now < *at + hour).collect();
        if recent.len() >= self.per_hour {
            recent.sort();
            return Err(OtpRefused::TooMany {
                until: recent[recent.len() - self.per_hour] + hour,
            });
        }
        Ok(())
    }

    /// The challenge for a code just sent.
    pub fn issue(
        &self,
        phone: PhoneNumber,
        purpose: OtpPurpose,
        code_hash: String,
        now: Timestamp,
    ) -> OtpChallenge {
        OtpChallenge {
            phone,
            purpose,
            code_hash,
            issued_at: now,
            expires_at: now + self.lifetime,
            attempts: 0,
            used: false,
        }
    }
}

impl OtpChallenge {
    /// Records one presented code; `matches` is whether it matched [`code_hash`](Self::code_hash).
    /// The attempt counts even when it is refused, so the caller stores the challenge either way.
    pub fn attempt(
        &mut self,
        policy: &OtpPolicy,
        matches: bool,
        now: Timestamp,
    ) -> Result<(), OtpRefused> {
        if self.used {
            return Err(OtpRefused::Used);
        }
        if now >= self.expires_at {
            return Err(OtpRefused::Expired);
        }
        if self.attempts >= policy.max_attempts {
            return Err(OtpRefused::Spent);
        }
        self.attempts += 1;
        if matches {
            self.used = true;
            Ok(())
        } else {
            Err(OtpRefused::Wrong {
                remaining: policy.max_attempts - self.attempts,
            })
        }
    }
}
