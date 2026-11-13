//! Collecting a payment by MTN MoMo, over the payment use cases.

use health_payments::{
    Evidence, Method, NewPayment, Payment, PaymentRefused, Reconciliation, StatementLine, Status,
};
use health_payments_app::{Payments, PaymentsError, Settings};
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
    /// The store, for the exceptions queue and the provider requests.
    store: SurrealStore,
    /// The facility whose merchant account and ledger these are.
    facility: String,
    mtn: MtnMomo<T>,
    clock: Arc<dyn Clock>,
    /// How long a payment waits for the payer before it expires.
    pending_for: SignedDuration,
    /// How long a request the provider has not heard of may still be on its way.
    grace: SignedDuration,
}

impl<T: MomoTransport> Collections<T> {
    /// Collections into `mtn`'s account, recorded in `store` under the facility's `settings`; a
    /// payment expires after `pending_for` without evidence.
    pub fn new(
        store: SurrealStore,
        settings: Settings,
        mtn: MtnMomo<T>,
        clock: Arc<dyn Clock>,
        pending_for: SignedDuration,
    ) -> Self {
        Self {
            facility: settings.facility.to_string(),
            payments: Payments::new(store.clone(), settings),
            store,
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
        // Which facility the reference is for, before anything goes out: a callback or a poll
        // finds its ledger and merchant account by it.
        self.store
            .put_provider_request(&reference, &self.facility, new.id, "mtn-momo")
            .await
            .map_err(|e| CollectionError::Payments(PaymentsError::Store(e.to_string())))?;
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

    /// MTN's callback for `reference`: a hint only, since it reaches a public route. The status is
    /// read back from MTN, and the payment settled on that, with the callback as its evidence.
    pub async fn callback(&self, reference: &str) -> Result<Settled, CollectionError> {
        let Some(payment) = self.pending_with(reference).await? else {
            return Ok(Settled::AlreadySettled);
        };
        let status = self.mtn.status(reference, self.clock.now()).await?;
        self.settle(&payment, status, Evidence::Callback).await
    }

    /// Reconciles MTN's statement `lines` with the pending payments: those it shows are
    /// confirmed by the statement, and a different amount or a line no payment has opens an
    /// exception in the queue.
    pub async fn reconcile_statement(
        &self,
        lines: &[StatementLine],
    ) -> Result<Reconciliation, CollectionError> {
        let mut result = self
            .payments
            .reconcile_statement(lines, self.clock.now())
            .await?;
        result.missing.clear();
        for (payment, recorded, provider) in &result.mismatched {
            let reference = self.payments.payment(payment).await?.reference;
            self.exception(
                "mismatched",
                reference.as_deref().unwrap_or(payment),
                Some(payment),
                Some(*recorded),
                Some(*provider),
            )
            .await?;
        }
        for reference in &result.unknown {
            let amount = lines
                .iter()
                .find(|line| line.reference == *reference)
                .map(|line| line.amount);
            self.exception("unknown", reference, None, None, amount)
                .await?;
        }
        Ok(result)
    }

    /// Reconciles MTN's statement `lines` for the period from `from` until `until`, as
    /// [`Self::reconcile_statement`] does, and also queues each payment requested in that period
    /// and posted here that the statement does not show (`missing`): money the ledger counts in
    /// the merchant account that MTN does not.
    pub async fn reconcile_period(
        &self,
        lines: &[StatementLine],
        from: health_time::Timestamp,
        until: health_time::Timestamp,
    ) -> Result<Reconciliation, CollectionError> {
        let mut result = self.reconcile_statement(lines).await?;
        let requested = self
            .store
            .provider_requests_between(&self.facility, from, until)
            .await
            .map_err(|e| CollectionError::Payments(PaymentsError::Store(e.to_string())))?;
        for (reference, payment) in requested {
            if lines.iter().any(|line| line.reference == reference) {
                continue;
            }
            let found = self.payments.payment(&payment).await?;
            if found.status == Status::Posted {
                self.exception(
                    "missing",
                    &reference,
                    Some(&payment),
                    Some(found.amount),
                    None,
                )
                .await?;
                result.missing.push(payment);
            }
        }
        Ok(result)
    }

    async fn exception(
        &self,
        kind: &str,
        reference: &str,
        payment: Option<&str>,
        recorded: Option<rust_decimal::Decimal>,
        provider: Option<rust_decimal::Decimal>,
    ) -> Result<(), CollectionError> {
        tracing::warn!(
            alert = "payment-exception",
            kind,
            reference,
            "a payment needs a person"
        );
        self.store
            .open_payment_exception(
                kind,
                reference,
                payment,
                recorded.map(|a| a.to_string()).as_deref(),
                provider.map(|a| a.to_string()).as_deref(),
            )
            .await
            .map_err(|e| CollectionError::Payments(PaymentsError::Store(e.to_string())))
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
            // Another facility's payment, with its own merchant account.
            if !self.is_ours(&reference).await? {
                continue;
            }
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

    /// The facility these collections are for.
    pub fn facility(&self) -> &str {
        &self.facility
    }

    /// Expires the pending payments whose time has passed: no evidence came.
    pub async fn expire_due(&self) -> Result<Vec<String>, CollectionError> {
        Ok(self.payments.expire_due(self.clock.now()).await?)
    }

    async fn is_ours(&self, reference: &str) -> Result<bool, CollectionError> {
        Ok(self
            .store
            .provider_request_facility(reference)
            .await
            .map_err(|e| CollectionError::Payments(PaymentsError::Store(e.to_string())))?
            .as_deref()
            == Some(self.facility.as_str()))
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
                if let Some(reference) = &payment.reference {
                    self.exception(
                        "amount",
                        reference,
                        Some(&payment.id),
                        Some(payment.amount),
                        Some(amount),
                    )
                    .await?;
                }
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
