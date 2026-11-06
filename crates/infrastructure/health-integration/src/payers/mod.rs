//! Payer adapters (pricing-and-insurance.md § Eligibility, preauthorization and claims; #126):
//! `health-coverage`'s [`Payer`](health_coverage::Payer) and
//! [`ClaimPayer`](health_coverage::claims::ClaimPayer) ports, one adapter per kind of payer. **No
//! national payer or claim format is assumed** (Uganda's NHIS is not in force).
//!
//! - **[`FhirPayer`]**: a payer that speaks FHIR R5. Eligibility is a CoverageEligibilityRequest,
//!   answered by a CoverageEligibilityResponse's `insurance.inforce`. A claim is POSTed as a
//!   `Claim` (an appeal names the original in `related`), and its `ClaimResponse` is read back
//!   by `request`: item adjudication `benefit` amounts, and `outcome: error` as a denial.
//! - **[`ExportPayer`]**: government and donor funding, whose "claims" are reports, not
//!   adjudication. Each claim is written to an [`Export`] as a CSV line, and its adjudication is
//!   recorded by hand.
//!
//! Both talk through ports, so the contract tests run them against mocks: [`PayerTransport`] (a
//! payer's HTTP API, bound to an HTTP client when a payer is onboarded) and [`Export`].

mod export;
mod fhir;

use std::future::Future;

pub use export::{Export, ExportPayer};
pub use fhir::FhirPayer;
use serde_json::Value;

/// Why a payer's API could not be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    /// It could not be reached, or did not answer in time.
    #[error("the payer could not be reached")]
    Unreachable,
}

/// A payer's HTTP API: JSON in, a status and JSON out.
pub trait PayerTransport: Send + Sync {
    /// POSTs `body` to `path`.
    fn post(
        &self,
        path: &str,
        body: &Value,
    ) -> impl Future<Output = Result<(u16, Value), TransportError>> + Send;
    /// GETs `path`.
    fn get(&self, path: &str) -> impl Future<Output = Result<(u16, Value), TransportError>> + Send;
}
