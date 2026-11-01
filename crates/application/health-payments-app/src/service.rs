//! Payments and cashier shifts at the facility authority (#254; billing-and-payments.md; ADR
//! 0032). Each step is one transaction: the payment, its ledger posting (checked against the
//! chart first) and any change to its shift. A provider reference is claimed once, with the
//! pending payment, before the provider is asked. Receipts are numbered from the node-prefixed
//! sequence (ADR 0014).

use health_financial_ledger::{
    ChartOfAccounts, ChartTemplate, Currency, Dimensions, Ledger, PostingRefused, PostingStore,
};
use health_identifiers::{NumberFormat, ShortCode};
use health_payments::{
    Evidence, Method, NewPayment, Payment, PaymentRefused, PaymentStore, Reconciliation, Shift,
    ShiftRefused, StatementLine, reconcile,
};
use health_persistence::{CounterKey, GuardKey, Store, StoreError, Transaction, WriteError};
use health_time::Timestamp;
use rust_decimal::Decimal;

use crate::{Accounts, Ledgering, PostingError, Step};

/// Why a payment use case failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PaymentsError {
    /// The payment rules refused it.
    #[error(transparent)]
    Payment(#[from] PaymentRefused),
    /// The shift rules refused it.
    #[error(transparent)]
    Shift(#[from] ShiftRefused),
    /// No posting fits the step.
    #[error(transparent)]
    Posting(#[from] PostingError),
    /// The ledger refused the posting.
    #[error(transparent)]
    Ledger(#[from] PostingRefused),
    /// No such payment or shift.
    #[error("{0} is not known")]
    NotFound(String),
    /// The provider reference is already used.
    #[error("the provider reference {0} is already used")]
    DuplicateReference(String),
    /// The till already has an open shift.
    #[error("till {0} already has an open shift")]
    ShiftOpen(String),
    /// The store failed, or a concurrent change came first; try again.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for PaymentsError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for PaymentsError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

/// A facility's payment settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The facility.
    pub facility: ShortCode,
    /// This node, in receipt numbers.
    pub node: ShortCode,
    /// The ledger's currency.
    pub currency: Currency,
    /// The chart of accounts (with its tills and merchant accounts).
    pub chart: ChartTemplate,
    /// Where payments land.
    pub accounts: Accounts,
    /// The receipt number format.
    pub receipts: NumberFormat,
}

/// The payment use cases over `store`.
pub struct Payments<S> {
    store: S,
    settings: Settings,
}

fn open_guard(till: &str) -> GuardKey {
    GuardKey(format!("open-shift|{till}"))
}

fn year(at: Timestamp) -> i32 {
    i32::from(at.to_zoned(jiff::tz::TimeZone::UTC).year())
}

impl<S> Payments<S>
where
    S: Store,
    S::Tx: PaymentStore + PostingStore,
{
    /// The use cases over `store` with `settings`.
    pub fn new(store: S, settings: Settings) -> Self {
        Self { store, settings }
    }

    fn ledgering(&self) -> Ledgering<'_> {
        Ledgering {
            accounts: &self.settings.accounts,
            facility: self.settings.facility.as_str(),
            currency: self.settings.currency,
        }
    }

    fn chart(&self) -> Result<Ledger, PaymentsError> {
        let chart = ChartOfAccounts::from_template(&self.settings.chart)
            .map_err(|e| PaymentsError::Store(e.to_string()))?;
        Ok(Ledger::new(
            self.settings.facility.as_str(),
            self.settings.currency,
            chart,
        ))
    }

    /// Checks and appends the posting of `step`.
    async fn post(
        &self,
        tx: &mut S::Tx,
        payment: &Payment,
        step: Step,
        till: Option<&str>,
        at: Timestamp,
    ) -> Result<(), PaymentsError> {
        let posting = self.ledgering().post(payment, step, till, at)?;
        self.chart()?.check(&posting)?;
        tx.append_posting(&posting).await?;
        Ok(())
    }

    async fn receipt_number(&self, tx: &mut S::Tx, at: Timestamp) -> Result<String, PaymentsError> {
        let s = &self.settings;
        let counter = CounterKey(format!("receipt|{}|{}", s.facility, s.node));
        let sequence = tx.next_sequence(&counter, &s.receipts, year(at)).await?;
        Ok(s.receipts.format(&s.facility, &s.node, year(at), sequence))
    }

    async fn load_payment(tx: &mut S::Tx, id: &str) -> Result<Payment, PaymentsError> {
        tx.payment(id)
            .await?
            .ok_or_else(|| PaymentsError::NotFound(format!("Payment/{id}")))
    }

    async fn load_shift(tx: &mut S::Tx, id: &str) -> Result<Shift, PaymentsError> {
        tx.shift(id)
            .await?
            .ok_or_else(|| PaymentsError::NotFound(format!("Shift/{id}")))
    }

    async fn commit(tx: S::Tx) -> Result<(), PaymentsError> {
        tx.commit().await.map_err(PaymentsError::from)
    }

    /// Opens a shift at `till` for `cashier`; a till has one open shift at a time.
    pub async fn open_shift(
        &self,
        id: &str,
        till: &str,
        cashier: &str,
        float: Decimal,
        at: Timestamp,
    ) -> Result<Shift, PaymentsError> {
        let mut tx = self.store.begin().await?;
        if tx.open_shift(till).await?.is_some() {
            return Err(PaymentsError::ShiftOpen(till.to_owned()));
        }
        // One open shift per till, even when two cashiers open it at once.
        tx.claim_guard(open_guard(till))
            .await
            .map_err(|e| match e {
                WriteError::GuardHeld(_) | WriteError::Conflict => {
                    PaymentsError::ShiftOpen(till.to_owned())
                }
                other => PaymentsError::from(other),
            })?;
        let shift = Shift::open(id, till, cashier, float, at)?;
        tx.put_shift(&shift).await?;
        Self::commit(tx).await?;
        Ok(shift)
    }

    /// Takes cash in shift `shift`: final, with its receipt.
    pub async fn take_cash(
        &self,
        new: &NewPayment<'_>,
        shift: &str,
        at: Timestamp,
    ) -> Result<Payment, PaymentsError> {
        let mut tx = self.store.begin().await?;
        let mut open = Self::load_shift(&mut tx, shift).await?;
        let mut payment = Payment::cash(new, &mut open, at)?;
        payment.receipt = Some(self.receipt_number(&mut tx, at).await?);
        self.post(&mut tx, &payment, Step::Recorded, Some(&open.till), at)
            .await?;
        tx.put_payment(&payment).await?;
        tx.put_shift(&open).await?;
        Self::commit(tx).await?;
        Ok(payment)
    }

    /// Records a two-phase payment, pending until `expires_at`, with its provider `reference`
    /// claimed once (the idempotency and reconciliation key) and a provisional receipt.
    pub async fn record_pending(
        &self,
        new: &NewPayment<'_>,
        reference: &str,
        at: Timestamp,
        expires_at: Timestamp,
    ) -> Result<Payment, PaymentsError> {
        let mut payment = Payment::pending(new, reference, at, expires_at)?;
        let mut tx = self.store.begin().await?;
        let guard = GuardKey(format!("payment-reference|{}", reference.trim()));
        tx.claim_guard(guard).await.map_err(|e| match e {
            WriteError::GuardHeld(_) | WriteError::Conflict => {
                PaymentsError::DuplicateReference(reference.trim().to_owned())
            }
            other => PaymentsError::from(other),
        })?;
        payment.receipt = Some(self.receipt_number(&mut tx, at).await?);
        self.post(&mut tx, &payment, Step::Recorded, None, at)
            .await?;
        tx.put_payment(&payment).await?;
        Self::commit(tx).await?;
        Ok(payment)
    }

    async fn change(
        &self,
        id: &str,
        step: Step,
        at: Timestamp,
        apply: impl FnOnce(&mut Payment) -> Result<(), PaymentRefused>,
    ) -> Result<Payment, PaymentsError> {
        let mut tx = self.store.begin().await?;
        let mut payment = Self::load_payment(&mut tx, id).await?;
        apply(&mut payment)?;
        let till = match &payment.shift {
            Some(shift) => Some(Self::load_shift(&mut tx, shift).await?.till),
            None => None,
        };
        self.post(&mut tx, &payment, step, till.as_deref(), at)
            .await?;
        tx.put_payment(&payment).await?;
        Self::commit(tx).await?;
        Ok(payment)
    }

    /// Confirms a pending payment on provider evidence: it moves from payments in transit to the
    /// merchant account.
    pub async fn confirm(
        &self,
        id: &str,
        evidence: Evidence,
        at: Timestamp,
    ) -> Result<Payment, PaymentsError> {
        self.change(id, Step::Confirmed, at, |p| p.confirm(evidence))
            .await
    }

    /// Voids a pending payment.
    pub async fn void(
        &self,
        id: &str,
        reason: &str,
        at: Timestamp,
    ) -> Result<Payment, PaymentsError> {
        self.change(id, Step::Ended, at, |p| p.void(reason)).await
    }

    /// Reverses a posted payment (error correction).
    pub async fn reverse(
        &self,
        id: &str,
        reason: &str,
        at: Timestamp,
    ) -> Result<Payment, PaymentsError> {
        self.change(id, Step::Reversed, at, |p| p.reverse(reason))
            .await
    }

    /// Refunds `amount` of a posted payment, by someone with the refund permission. Cash is paid
    /// out of its till's open shift.
    pub async fn refund(
        &self,
        id: &str,
        amount: Decimal,
        permitted: bool,
        at: Timestamp,
    ) -> Result<Payment, PaymentsError> {
        let mut tx = self.store.begin().await?;
        let mut payment = Self::load_payment(&mut tx, id).await?;
        payment.refund(amount, permitted)?;
        let mut till = None;
        if payment.method == Method::Cash {
            let original = match &payment.shift {
                Some(shift) => Self::load_shift(&mut tx, shift).await?,
                None => return Err(PaymentsError::NotFound("the payment's shift".into())),
            };
            let mut open = tx.open_shift(&original.till).await?.ok_or_else(|| {
                PaymentsError::NotFound(format!("an open shift at {}", original.till))
            })?;
            open.pay_out(amount)?;
            tx.put_shift(&open).await?;
            till = Some(original.till);
        }
        self.post(
            &mut tx,
            &payment,
            Step::Refunded(amount),
            till.as_deref(),
            at,
        )
        .await
        .map_err(|e| match e {
            PaymentsError::Ledger(PostingRefused::DuplicateId(_)) => {
                PaymentsError::Store("a refund was recorded at the same time; try again".into())
            }
            other => other,
        })?;
        tx.put_payment(&payment).await?;
        Self::commit(tx).await?;
        Ok(payment)
    }

    /// Expires the pending payments whose expiry has passed at `now`; their ids.
    pub async fn expire_due(&self, now: Timestamp) -> Result<Vec<String>, PaymentsError> {
        let pending = {
            let mut tx = self.store.begin().await?;
            tx.pending_payments().await?
        };
        let mut expired = Vec::new();
        for payment in pending {
            if payment.expires_at.is_some_and(|at| now >= at) {
                self.change(&payment.id, Step::Ended, now, |p| p.expire(now).map(|_| ()))
                    .await?;
                expired.push(payment.id);
            }
        }
        Ok(expired)
    }

    /// Closes shift `id` with the cash counted; a variance waits for a supervisor.
    pub async fn close_shift(&self, id: &str, counted: Decimal) -> Result<Shift, PaymentsError> {
        let mut tx = self.store.begin().await?;
        let mut shift = Self::load_shift(&mut tx, id).await?;
        shift.close(counted)?;
        tx.release_guard(open_guard(&shift.till)).await?;
        tx.put_shift(&shift).await?;
        Self::commit(tx).await?;
        Ok(shift)
    }

    /// A supervisor approves shift `id`'s variance: it is posted to till variance.
    pub async fn approve_shift(
        &self,
        id: &str,
        supervisor: &str,
        at: Timestamp,
    ) -> Result<Shift, PaymentsError> {
        let mut tx = self.store.begin().await?;
        let mut shift = Self::load_shift(&mut tx, id).await?;
        shift.approve(supervisor)?;
        if let Some(posting) = self.ledgering().variance(&shift, at) {
            self.chart()?.check(&posting)?;
            tx.append_posting(&posting).await?;
        }
        tx.put_shift(&shift).await?;
        Self::commit(tx).await?;
        Ok(shift)
    }

    /// Reconciles a provider statement with the pending payments: those it confirms are
    /// confirmed by the statement; mismatched amounts and unknown lines come back as exceptions
    /// for a person. (Posted payments missing from a statement are central's daily check, #124.)
    pub async fn reconcile_statement(
        &self,
        lines: &[StatementLine],
        at: Timestamp,
    ) -> Result<Reconciliation, PaymentsError> {
        let pending = {
            let mut tx = self.store.begin().await?;
            tx.pending_payments().await?
        };
        let result = reconcile(&pending, lines);
        for id in &result.confirm {
            self.confirm(id, Evidence::Statement, at).await?;
        }
        Ok(result)
    }

    /// Payment `id`.
    pub async fn payment(&self, id: &str) -> Result<Payment, PaymentsError> {
        let mut tx = self.store.begin().await?;
        Self::load_payment(&mut tx, id).await
    }

    /// Shift `id` and its payments.
    pub async fn shift(&self, id: &str) -> Result<(Shift, Vec<Payment>), PaymentsError> {
        let mut tx = self.store.begin().await?;
        let shift = Self::load_shift(&mut tx, id).await?;
        let payments = tx.payments_of_shift(id).await?;
        Ok((shift, payments))
    }

    /// The balance of each account in `codes`, from the stored postings.
    pub async fn balances(&self, codes: &[&str]) -> Result<Vec<(String, Decimal)>, PaymentsError> {
        let mut tx = self.store.begin().await?;
        let postings = tx.postings().await?;
        let chart = ChartOfAccounts::from_template(&self.settings.chart)
            .map_err(|e| PaymentsError::Store(e.to_string()))?;
        let ledger = Ledger::from_postings(
            self.settings.facility.as_str(),
            self.settings.currency,
            chart,
            postings,
        )?;
        Ok(codes
            .iter()
            .map(|code| {
                (
                    (*code).to_owned(),
                    ledger.balance(code, |_: &Dimensions| true).amount(),
                )
            })
            .collect())
    }
}
