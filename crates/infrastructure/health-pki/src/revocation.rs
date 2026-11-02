//! An issuer's revocation list and the CRLs it publishes (ADR 0058 § Revocation).

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::{Authority, Result, Serial};

/// Everything one issuer has revoked, and the number of its last CRL. The root keeps one at
/// the ceremony; central keeps the issuing CA's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revocations {
    /// The last CRL's number; each CRL takes the next.
    pub number: u64,
    /// The revoked certificates, in the order they were revoked.
    pub revoked: Vec<Revoked>,
}

/// One revoked certificate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revoked {
    /// The certificate's serial number.
    pub serial: Serial,
    /// When it was revoked.
    pub at: Timestamp,
    /// Why.
    pub reason: Reason,
}

/// Why a certificate was revoked (RFC 5280 § 5.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// No reason given.
    Unspecified,
    /// Its key is, or may be, compromised.
    KeyCompromise,
    /// An issuing CA's key is, or may be, compromised.
    CaCompromise,
    /// It was replaced by a new certificate.
    Superseded,
    /// Its holder is decommissioned.
    CessationOfOperation,
}

impl Reason {
    pub(crate) fn code(self) -> rcgen::RevocationReason {
        match self {
            Self::Unspecified => rcgen::RevocationReason::Unspecified,
            Self::KeyCompromise => rcgen::RevocationReason::KeyCompromise,
            Self::CaCompromise => rcgen::RevocationReason::CaCompromise,
            Self::Superseded => rcgen::RevocationReason::Superseded,
            Self::CessationOfOperation => rcgen::RevocationReason::CessationOfOperation,
        }
    }
}

impl Revocations {
    /// Revokes `serial`; revoking it again keeps the first revocation.
    pub fn revoke(&mut self, serial: Serial, reason: Reason, at: Timestamp) {
        if !self.revoked.iter().any(|r| r.serial == serial) {
            self.revoked.push(Revoked { serial, at, reason });
        }
    }

    /// Whether `serial` is revoked.
    pub fn contains(&self, serial: &Serial) -> bool {
        self.revoked.iter().any(|r| &r.serial == serial)
    }

    /// The next CRL, signed by `issuer` at `at`, as PEM. It takes the next number, so keep the
    /// list once the CRL is published.
    pub fn crl(&mut self, issuer: &Authority, at: Timestamp) -> Result<String> {
        let number = self.number + 1;
        let crl = issuer.crl(&self.revoked, number, at)?;
        self.number = number;
        Ok(crl)
    }
}
