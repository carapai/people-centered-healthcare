//! Collecting a payment by MTN MoMo, over the payment use cases.

use health_payments::{Evidence, Method, NewPayment, Payment, PaymentRefused, Status};
use health_payments_app::{Payments, PaymentsError};
use health_surrealdb::SurrealStore;
use health_time::{Clock, SignedDuration};
use std::sync::Arc;

use super::mtn::{Initiated, MomoError, MomoTransport, MtnMomo, ProviderStatus, reference};

/// Why a collection step failed.
#[derive(Debug, thiserror::Error)]
pub enum CollectionError {
    /// The payment use cases refused it, or the store failed.
    #[error(transparent)]
    Payments(#[from] PaymentsError),
    /// The provider could not be asked.
    #[error(transparent)]
    Provider(#[from] MomoError),
    /// The callback names no payment pending here.
    #[error("no payment has the reference {0}")]
    UnknownReference(String),
}

/// What a callback or a poll did to a payment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    /// Confirmed: posted on the provider's evidence.
    Confirmed,
    /// Voided: the provider reported a failure, or never had the request.
    Voided(String),
    /// Still waiting for the payer.
    Waiting,
    /// Already confirmed or ended: a duplicate callback, or a callback racing a poll.
    AlreadySettled,
    /// The provider's amount is not the payment's: left pending, for the exceptions queue.
    AmountMismatch {
        /// The payment's amount.
        recorded: rust_decimal::Decimal,
        /// What the provider reports.
        provider: rust_decimal::Decimal,
    },
}

/// MTN MoMo collections for one facility's merchant account.
pub struct Collections<T> {
    payments: Payments<SurrealStore>,
    mtn: MtnMomo<T>,
    clock: Arc<dyn Clock>,
    /// How long a payment waits for the payer before it expires.
    pending_for: SignedDuration,
    /// How long a request the provider has not heard of may still be on its way.
    grace: SignedDuration,
}

impl<T: MomoTransport> Collections<T> {
    /// Collections into `mtn`'s account, recorded through `payments`; a payment expires after
    /// `pending_for` without evidence.
    pub fn new(
        payments: Payments<SurrealStore>,
        mtn: MtnMomo<T>,
        clock: Arc<dyn Clock>,
        pending_for: SignedDuration,
    ) -> Self {
        Self {
            payments,
            mtn,
            clock,
            pending_for,
            grace: SignedDuration::from_mins(5),
        }
    }

    /// The MTN API.
    pub fn mtn(&self) -> &MtnMomo<T> {
        &self.mtn
    }

    /// Asks `msisdn` to pay `new` (an MTN MoMo payment) by MoMo: the payment is stored pending,
    /// with a fresh reference, before the request goes out. A refused request voids it; one whose
    /// answer was lost stays pending, for the poll to settle.
    pub async fn initiate(
        &self,
        new: &NewPayment<'_>,
        msisdn: &str,
        note: &str,
        shift: Option<&str>,
    ) -> Result<Payment, CollectionError> {
        let now = self.clock.now();
        let expires = now
            .checked_add(self.pending_for)
            .map_err(|e| CollectionError::Provider(MomoError::Unexpected(e.to_string())))?;
        let reference = reference();
        let payment = self
            .payments
            .record_pending(new, &reference, shift, now, expires)
            .await?;
        match self
            .mtn
            .request_to_pay(&reference, new.amount, msisdn, new.id, note, now)
            .await
        {
            Initiated::Accepted | Initiated::Unknown => Ok(payment),
            Initiated::Refused(why) => Ok(self
                .payments
                .void(new.id, &format!("MTN refused the request: {why}"), now)
                .await?),
        }
    }

    /// MTN's callback for `reference` with its `body`.
    pub async fn callback(
        &self,
        reference: &str,
        body: &serde_json::Value,
    ) -> Result<Settled, CollectionError> {
        let status = ProviderStatus::from_body(body)?;
        match self.pending_with(reference).await? {
            Some(payment) => self.settle(&payment, status, Evidence::Callback).await,
            None => Ok(Settled::AlreadySettled),
        }
    }

    /// Asks MTN about every pending MoMo payment, settling those it has an answer for: what a
    /// dropped callback leaves behind. Returns each payment's id and what became of it.
    pub async fn poll(&self) -> Result<Vec<(String, Settled)>, CollectionError> {
        let now = self.clock.now();
        let mut settled = Vec::new();
        for payment in self.payments.pending().await? {
            let Some(reference) = payment
                .reference
                .clone()
                .filter(|_| payment.method == Method::MtnMomo && payment.status == Status::Pending)
            else {
                continue;
            };
            let status = match self.mtn.status(&reference, now).await {
                Ok(status) => status,
                // Unreachable now: the next poll asks again.
                Err(MomoError::Unreachable) => continue,
                Err(other) => return Err(other.into()),
            };
            let outcome = self.settle(&payment, status, Evidence::Poll).await?;
            settled.push((payment.id.clone(), outcome));
        }
        Ok(settled)
    }

    async fn pending_with(&self, reference: &str) -> Result<Option<Payment>, CollectionError> {
        Ok(self
            .payments
            .pending()
            .await?
            .into_iter()
            .find(|p| p.reference.as_deref() == Some(reference)))
    }

    /// Applies `status` to `payment`, a pending one, on `evidence`.
    async fn settle(
        &self,
        payment: &Payment,
        status: ProviderStatus,
        evidence: Evidence,
    ) -> Result<Settled, CollectionError> {
        let now = self.clock.now();
        let result = match status {
            ProviderStatus::Pending => return Ok(Settled::Waiting),
            ProviderStatus::Successful { amount, .. } if amount != payment.amount => {
                tracing::warn!(
                    alert = "momo-amount-mismatch",
                    payment = %payment.id,
                    recorded = %payment.amount,
                    provider = %amount,
                    "MTN reports another amount: the payment stays pending for review"
                );
                return Ok(Settled::AmountMismatch {
                    recorded: payment.amount,
                    provider: amount,
                });
            }
            ProviderStatus::Successful { .. } => self
                .payments
                .confirm(&payment.id, evidence, now)
                .await
                .map(|_| Settled::Confirmed),
            ProviderStatus::Failed { reason } => {
                let why = format!("MTN: {reason}");
                self.payments
                    .void(&payment.id, &why, now)
                    .await
                    .map(|_| Settled::Voided(why))
            }
            ProviderStatus::NotFound => {
                let waited = payment
                    .recorded_at
                    .checked_add(self.grace)
                    .is_ok_and(|until| now >= until);
                if !waited {
                    return Ok(Settled::Waiting);
                }
                let why = "the request never reached MTN, and it is not sent twice".to_owned();
                self.payments
                    .void(&payment.id, &why, now)
                    .await
                    .map(|_| Settled::Voided(why))
            }
        };
        match result {
            Ok(settled) => Ok(settled),
            // Settled meanwhile by the other path (a callback and a poll, or a repeat): once only.
            Err(PaymentsError::Payment(PaymentRefused::WrongState { .. })) => {
                Ok(Settled::AlreadySettled)
            }
            Err(other) => Err(other.into()),
        }
    }
}
