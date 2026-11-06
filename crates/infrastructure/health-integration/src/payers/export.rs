//! Government and donor funding: claims as reports, not adjudication.

use std::future::Future;

use health_coverage::claims::{Adjudication, Claim, ClaimPayer};
use health_coverage::{Coverage, Payer, PayerError};
use jiff::civil::Date;

/// Where exported claims go (a report file, a portal upload).
pub trait Export: Send + Sync {
    /// Writes `lines` (CSV, with a header) as the export named `name`.
    fn write(
        &self,
        name: &str,
        lines: &[String],
    ) -> impl Future<Output = Result<(), PayerError>> + Send;
}

/// A funding source that is reported to, not claimed from.
pub struct ExportPayer<E> {
    export: E,
}

impl<E: Export> ExportPayer<E> {
    /// The adapter over `export`.
    pub fn new(export: E) -> Self {
        Self { export }
    }

    /// Where the claims went.
    pub fn export(&self) -> &E {
        &self.export
    }

    /// `claim` as CSV: a header, then one line per item.
    #[must_use]
    pub fn lines(claim: &Claim) -> Vec<String> {
        let mut lines =
            vec!["claim,appeals,patient,coverage,sequence,item,quantity,claimed,date".to_owned()];
        for item in &claim.items {
            lines.push(
                [
                    claim.id.as_str(),
                    claim.appeals.as_deref().unwrap_or_default(),
                    claim.patient.as_str(),
                    claim.coverage.as_str(),
                    &item.sequence.to_string(),
                    &item.item.replace(',', " "),
                    &item.quantity.to_string(),
                    &item.claimed.to_string(),
                    item.at.to_string().get(..10).unwrap_or_default(),
                ]
                .join(","),
            );
        }
        lines
    }
}

impl<E: Export> Payer for ExportPayer<E> {
    /// A funding source's eligibility is the patient's program enrolment, decided by pricing's
    /// eligibility rules, not by asking anyone: always eligible here.
    async fn eligible(&self, _coverage: &Coverage, _day: Date) -> Result<bool, PayerError> {
        Ok(true)
    }
}

impl<E: Export> ClaimPayer for ExportPayer<E> {
    async fn submit(&self, claim: &Claim) -> Result<String, PayerError> {
        let name = format!("claim-{}.csv", claim.id);
        self.export.write(&name, &Self::lines(claim)).await?;
        Ok(name)
    }

    /// Reports are not adjudicated: what was accepted is recorded by hand.
    async fn adjudication(
        &self,
        _payer_reference: &str,
    ) -> Result<Option<Adjudication>, PayerError> {
        Ok(None)
    }
}
