//! Payments (billing-and-payments.md § Payments and § Other flows; ADR 0032; MASTER_PROMPT §68).
//!
//! - **Cash** is final at the till, recorded within an open cashier [`Shift`]. At close the
//!   counted cash is compared with what is expected; a variance needs a supervisor's approval.
//! - **Mobile money, card and bank transfer** are two-phase: [`Payment::pending`] against payments
//!   in transit, with the provider reference persisted first, then [`Payment::confirm`]ed only by
//!   provider evidence (a callback, a status poll or a statement line), or voided, or expired.
//! - **Customer-reported** payments (the reference from the customer's SMS, offline) are pending
//!   and unverified until a provider statement line confirms them.
//! - **Receipts** say "provisional" until a payment is posted: a pending payment is never shown
//!   as confirmed (§68).
//! - **Refunds** are new outgoing operations and need the refund permission; corrections are
//!   reversals, never edits.

mod shift;

use health_time::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub use shift::{Shift, ShiftRefused, ShiftStatus};

/// How a payment is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Method {
    /// Cash at the till: final.
    Cash,
    /// MTN Mobile Money.
    MtnMomo,
    /// Airtel Money.
    AirtelMoney,
    /// A card.
    Card,
    /// A bank transfer.
    BankTransfer,
    /// Reported by the customer (the reference from their SMS), recorded offline.
    CustomerReported,
}

impl Method {
    /// Whether it is two-phase: confirmed only by provider evidence.
    pub fn two_phase(self) -> bool {
        self != Self::Cash
    }
}

/// What a payment is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Purpose {
    /// Settles what the patient owes (allocated to their share, oldest first).
    Settlement,
    /// A deposit held for the patient, applied to invoices later.
    Deposit,
}

/// What proves a non-cash payment arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Evidence {
    /// The provider's callback.
    Callback,
    /// A status poll by the provider reference.
    Poll,
    /// A line of the provider's statement.
    Statement,
}

/// Where a payment is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Waiting for provider evidence, against payments in transit.
    Pending,
    /// Reported by the customer, waiting for a statement line.
    PendingUnverified,
    /// Final: cash at the till, or confirmed by the provider.
    Posted,
    /// Failed, or voided by the cashier.
    Voided,
    /// No evidence came before it expired.
    Expired,
    /// Corrected by a reversal.
    Reversed,
}

/// Why a payment step was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PaymentRefused {
    /// The amount is zero or less.
    #[error("a payment is more than zero")]
    NonPositive,
    /// A non-cash payment needs its provider reference before it is recorded.
    #[error("a non-cash payment needs its provider reference")]
    NoReference,
    /// Cash is taken only within an open shift.
    #[error("cash is taken within an open cashier shift")]
    NoShift,
    /// The step does not apply in the payment's status.
    #[error("cannot {action} a payment that is {status:?}")]
    WrongState {
        /// The payment's status.
        status: Status,
        /// The step.
        action: &'static str,
    },
    /// A customer-reported payment is confirmed only by a provider statement.
    #[error("a customer-reported payment is confirmed only by a statement line")]
    StatementRequired,
    /// A refund needs the refund permission.
    #[error("refunds need the refund permission")]
    NotPermitted,
    /// A refund is no more than what was paid and not yet refunded.
    #[error("refunds total at most what was paid")]
    TooMuch,
}

/// A payment received.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payment {
    /// The payment's id.
    pub id: String,
    /// How it was made.
    pub method: Method,
    /// What it is for.
    pub purpose: Purpose,
    /// The amount, in the facility's currency.
    pub amount: Decimal,
    /// Who paid (the Patient or Account it settles).
    pub payer: String,
    /// The provider reference (e.g. MTN's `X-Reference-Id`): the idempotency and reconciliation
    /// key, persisted before initiation. `None` for cash.
    pub reference: Option<String>,
    /// The cashier shift, for cash.
    pub shift: Option<String>,
    /// Where it is.
    pub status: Status,
    /// When it was recorded.
    pub recorded_at: Timestamp,
    /// When a pending payment expires.
    pub expires_at: Option<Timestamp>,
    /// What confirmed it.
    pub evidence: Option<Evidence>,
    /// Why it was voided or reversed.
    pub reason: Option<String>,
    /// What has been refunded of it.
    pub refunded: Decimal,
}

/// What every payment has: who paid how much, how, and for what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPayment<'a> {
    /// The payment's id.
    pub id: &'a str,
    /// How it is made.
    pub method: Method,
    /// What it is for.
    pub purpose: Purpose,
    /// The amount.
    pub amount: Decimal,
    /// Who pays.
    pub payer: &'a str,
}

/// What a receipt says about the payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptStatus {
    /// Waiting for the provider: "provisional".
    Provisional,
    /// Final.
    Confirmed,
    /// Not paid: voided, expired or reversed.
    Void,
}

impl Payment {
    fn new(new: &NewPayment<'_>, at: Timestamp) -> Result<Self, PaymentRefused> {
        if new.amount <= Decimal::ZERO {
            return Err(PaymentRefused::NonPositive);
        }
        Ok(Self {
            id: new.id.to_owned(),
            method: new.method,
            purpose: new.purpose,
            amount: new.amount,
            payer: new.payer.to_owned(),
            reference: None,
            shift: None,
            status: Status::Posted,
            recorded_at: at,
            expires_at: None,
            evidence: None,
            reason: None,
            refunded: Decimal::ZERO,
        })
    }

    /// Cash taken within `shift` (which must be open): final at once.
    pub fn cash(
        new: &NewPayment<'_>,
        shift: &mut Shift,
        at: Timestamp,
    ) -> Result<Self, PaymentRefused> {
        if new.method != Method::Cash {
            return Err(PaymentRefused::WrongState {
                status: Status::Pending,
                action: "take as cash",
            });
        }
        let mut payment = Self::new(new, at)?;
        shift
            .take(new.amount)
            .map_err(|_| PaymentRefused::NoShift)?;
        payment.shift = Some(shift.id.clone());
        Ok(payment)
    }

    /// A two-phase payment, pending until provider evidence or `expires_at`; `reference` is
    /// generated and persisted with it before the provider is asked. A customer-reported one is
    /// pending and unverified.
    pub fn pending(
        new: &NewPayment<'_>,
        reference: &str,
        at: Timestamp,
        expires_at: Timestamp,
    ) -> Result<Self, PaymentRefused> {
        let method = new.method;
        if !method.two_phase() {
            return Err(PaymentRefused::WrongState {
                status: Status::Posted,
                action: "hold pending",
            });
        }
        if reference.trim().is_empty() {
            return Err(PaymentRefused::NoReference);
        }
        let mut payment = Self::new(new, at)?;
        payment.reference = Some(reference.trim().to_owned());
        payment.expires_at = Some(expires_at);
        payment.status = if method == Method::CustomerReported {
            Status::PendingUnverified
        } else {
            Status::Pending
        };
        Ok(payment)
    }

    fn expect_pending(&self, action: &'static str) -> Result<(), PaymentRefused> {
        if matches!(self.status, Status::Pending | Status::PendingUnverified) {
            Ok(())
        } else {
            Err(PaymentRefused::WrongState {
                status: self.status,
                action,
            })
        }
    }

    /// Posts it on provider evidence. A customer-reported payment needs a statement line.
    pub fn confirm(&mut self, evidence: Evidence) -> Result<(), PaymentRefused> {
        self.expect_pending("confirm")?;
        if self.status == Status::PendingUnverified && evidence != Evidence::Statement {
            return Err(PaymentRefused::StatementRequired);
        }
        self.evidence = Some(evidence);
        self.status = Status::Posted;
        Ok(())
    }

    /// Voids it: the provider reported a failure, or the cashier voided it.
    pub fn void(&mut self, reason: &str) -> Result<(), PaymentRefused> {
        self.expect_pending("void")?;
        self.reason = Some(reason.to_owned());
        self.status = Status::Voided;
        Ok(())
    }

    /// Expires it, if no evidence came before its expiry; `false` while it still may.
    pub fn expire(&mut self, now: Timestamp) -> Result<bool, PaymentRefused> {
        self.expect_pending("expire")?;
        if self.expires_at.is_some_and(|at| now >= at) {
            self.status = Status::Expired;
            return Ok(true);
        }
        Ok(false)
    }

    /// Reverses a posted payment (error correction).
    pub fn reverse(&mut self, reason: &str) -> Result<(), PaymentRefused> {
        if self.status != Status::Posted {
            return Err(PaymentRefused::WrongState {
                status: self.status,
                action: "reverse",
            });
        }
        self.reason = Some(reason.to_owned());
        self.status = Status::Reversed;
        Ok(())
    }

    /// Records a refund of `amount` of this posted payment, by someone with the refund
    /// permission. The refund itself is a new outgoing operation.
    pub fn refund(&mut self, amount: Decimal, permitted: bool) -> Result<(), PaymentRefused> {
        if !permitted {
            return Err(PaymentRefused::NotPermitted);
        }
        if self.status != Status::Posted {
            return Err(PaymentRefused::WrongState {
                status: self.status,
                action: "refund",
            });
        }
        if amount <= Decimal::ZERO {
            return Err(PaymentRefused::NonPositive);
        }
        if self.refunded + amount > self.amount {
            return Err(PaymentRefused::TooMuch);
        }
        self.refunded += amount;
        Ok(())
    }

    /// Whether it is confirmed: only a posted payment is. A pending one never is (§68).
    pub fn is_confirmed(&self) -> bool {
        self.status == Status::Posted
    }

    /// What its receipt says.
    pub fn receipt(&self) -> ReceiptStatus {
        match self.status {
            Status::Posted => ReceiptStatus::Confirmed,
            Status::Pending | Status::PendingUnverified => ReceiptStatus::Provisional,
            Status::Voided | Status::Expired | Status::Reversed => ReceiptStatus::Void,
        }
    }
}

/// One line of a provider's statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementLine {
    /// The provider reference.
    pub reference: String,
    /// The amount received.
    pub amount: Decimal,
}

/// What a provider statement says about the payments (the daily reconciliation at central).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reconciliation {
    /// Pending payments the statement confirms: confirm each with [`Evidence::Statement`].
    pub confirm: Vec<String>,
    /// Exceptions: a payment whose statement amount differs (payment id, recorded, statement).
    pub mismatched: Vec<(String, Decimal, Decimal)>,
    /// Exceptions: statement lines no payment has.
    pub unknown: Vec<String>,
    /// Exceptions: posted payments the statement does not have.
    pub missing: Vec<String>,
}

/// Matches `payments` (of one provider) with its statement `lines` by provider reference.
/// Exceptions go to a queue for a person; nothing is confirmed on a mismatched amount.
pub fn reconcile(payments: &[Payment], lines: &[StatementLine]) -> Reconciliation {
    let mut result = Reconciliation::default();
    for line in lines {
        let Some(payment) = payments
            .iter()
            .find(|p| p.reference.as_deref() == Some(line.reference.as_str()))
        else {
            result.unknown.push(line.reference.clone());
            continue;
        };
        if payment.amount != line.amount {
            result
                .mismatched
                .push((payment.id.clone(), payment.amount, line.amount));
        } else if matches!(payment.status, Status::Pending | Status::PendingUnverified) {
            result.confirm.push(payment.id.clone());
        }
    }
    for payment in payments
        .iter()
        .filter(|p| p.status == Status::Posted && p.method.two_phase())
    {
        let listed = lines
            .iter()
            .any(|l| Some(l.reference.as_str()) == payment.reference.as_deref());
        if !listed {
            result.missing.push(payment.id.clone());
        }
    }
    result
}
