//! Patients' sign-in by one-time code (#111; ADR 0029): every sign-in sends a code to the
//! verified phone; there is no patient password. Codes leave through the [`OtpSender`] port.
//! Until the SMS provider is chosen (#235) the only adapter is [`DevOutbox`].

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use health_identity::{OtpChallenge, OtpPolicy, PhoneNumber};
use health_time::Timestamp;
use rand_core::RngCore;
use serde::{Deserialize, Serialize};

use crate::jose::{b64, sha256_b64};

/// Why a code could not be sent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the code could not be sent: {0}")]
pub struct OtpSendError(pub String);

/// The future an [`OtpSender`] returns.
pub type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), OtpSendError>> + Send + 'a>>;

/// Delivers a one-time code to a phone (the SMS channel, #235).
pub trait OtpSender: Send + Sync {
    /// Sends `code` to `phone`.
    fn send<'a>(&'a self, phone: &'a PhoneNumber, code: &'a str) -> SendFuture<'a>;
}

/// A development outbox: it keeps the codes it is given, for a developer or an e2e test to read.
/// Nothing reaches a phone.
#[derive(Debug, Clone, Default)]
pub struct DevOutbox(Arc<Mutex<Vec<(PhoneNumber, String)>>>);

impl DevOutbox {
    /// The last code sent to `phone`.
    pub fn latest(&self, phone: &PhoneNumber) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .rev()
            .find(|(to, _)| to == phone)
            .map(|(_, code)| code.clone())
    }
}

impl OtpSender for DevOutbox {
    fn send<'a>(&'a self, phone: &'a PhoneNumber, code: &'a str) -> SendFuture<'a> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((phone.clone(), code.to_owned()));
        Box::pin(async { Ok(()) })
    }
}

/// Sign-in by one-time code, for clients registered with [`Client::patient`](crate::Client).
#[derive(Clone)]
pub struct PhoneSignIn {
    /// The code rules.
    pub policy: OtpPolicy,
    /// The country code taken by national numbers (`0772…`), e.g. `256`.
    pub country_code: String,
    /// Where codes go.
    pub sender: Arc<dyn OtpSender>,
}

impl fmt::Debug for PhoneSignIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PhoneSignIn")
            .field("policy", &self.policy)
            .field("country_code", &self.country_code)
            .finish_non_exhaustive()
    }
}

impl PhoneSignIn {
    /// The default rules for `country_code`, sending through `sender`.
    pub fn new(country_code: impl Into<String>, sender: Arc<dyn OtpSender>) -> Self {
        Self {
            policy: OtpPolicy::default(),
            country_code: country_code.into(),
            sender,
        }
    }
}

/// A phone's current challenge and when codes were sent to it in the last hour, as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhoneOtp {
    /// The current challenge.
    pub challenge: OtpChallenge,
    /// When codes were sent, for the hourly limit.
    pub sent: Vec<Timestamp>,
}

/// A fresh code of `digits` digits, uniformly random.
pub(crate) fn new_code(digits: u32) -> String {
    let modulus = 10u64.pow(digits);
    // Rejection sampling keeps every code equally likely.
    let limit = u64::MAX - u64::MAX % modulus;
    loop {
        let n = rand_core::OsRng.next_u64();
        if n < limit {
            return format!("{:0width$}", n % modulus, width = digits as usize);
        }
    }
}

/// The stored form of `code`: a random salt and the SHA-256 of salt and code.
pub(crate) fn hash_code(code: &str) -> String {
    let mut salt = [0u8; 16];
    rand_core::OsRng.fill_bytes(&mut salt);
    let salt = b64(&salt);
    format!("{salt}${}", sha256_b64(&format!("{salt}{code}")))
}

/// Whether `code` is the one `stored` was made from, compared in constant time.
pub(crate) fn code_matches(code: &str, stored: &str) -> bool {
    let Some((salt, hash)) = stored.split_once('$') else {
        return false;
    };
    let presented = sha256_b64(&format!("{salt}{code}"));
    presented.len() == hash.len()
        && presented
            .bytes()
            .zip(hash.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}
