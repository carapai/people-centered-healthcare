use health_time::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Where a cashier shift is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShiftStatus {
    /// Taking cash.
    Open,
    /// Closed with a variance, waiting for a supervisor.
    AwaitingApproval,
    /// Settled: counted equals expected, or a supervisor approved the variance.
    Settled,
}

/// Why a shift step was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShiftRefused {
    /// The step does not apply in the shift's status.
    #[error("cannot {action} a shift that is {status:?}")]
    WrongState {
        /// The shift's status.
        status: ShiftStatus,
        /// The step.
        action: &'static str,
    },
    /// An amount is zero or more (a float, a count) or more than zero (cash taken or paid out).
    #[error("the amount is not valid")]
    BadAmount,
    /// A variance is approved by a supervisor other than the cashier.
    #[error("a supervisor other than the cashier approves a variance")]
    NotSupervisor,
}

/// A cashier's shift at a till: the opening float, the cash taken and paid out, and, at close,
/// the counted cash against the expected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shift {
    /// The shift's id.
    pub id: String,
    /// The till (its cash account is `cash/{till}`).
    pub till: String,
    /// The cashier.
    pub cashier: String,
    /// When it opened.
    pub opened_at: Timestamp,
    /// The float it opened with.
    pub float: Decimal,
    /// Cash taken.
    pub taken: Decimal,
    /// Cash paid out (refunds).
    pub paid_out: Decimal,
    /// Where it is.
    pub status: ShiftStatus,
    /// The cash counted at close.
    pub counted: Option<Decimal>,
    /// Who approved the variance.
    pub approved_by: Option<String>,
}

impl Shift {
    /// Opens a shift with `float` in the drawer.
    pub fn open(
        id: &str,
        till: &str,
        cashier: &str,
        float: Decimal,
        at: Timestamp,
    ) -> Result<Self, ShiftRefused> {
        if float < Decimal::ZERO {
            return Err(ShiftRefused::BadAmount);
        }
        Ok(Self {
            id: id.to_owned(),
            till: till.to_owned(),
            cashier: cashier.to_owned(),
            opened_at: at,
            float,
            taken: Decimal::ZERO,
            paid_out: Decimal::ZERO,
            status: ShiftStatus::Open,
            counted: None,
            approved_by: None,
        })
    }

    fn expect(&self, status: ShiftStatus, action: &'static str) -> Result<(), ShiftRefused> {
        if self.status == status {
            Ok(())
        } else {
            Err(ShiftRefused::WrongState {
                status: self.status,
                action,
            })
        }
    }

    pub(crate) fn take(&mut self, amount: Decimal) -> Result<(), ShiftRefused> {
        self.expect(ShiftStatus::Open, "take cash in")?;
        if amount <= Decimal::ZERO {
            return Err(ShiftRefused::BadAmount);
        }
        self.taken += amount;
        Ok(())
    }

    /// Pays cash out of the drawer (a cash refund).
    pub fn pay_out(&mut self, amount: Decimal) -> Result<(), ShiftRefused> {
        self.expect(ShiftStatus::Open, "pay cash out of")?;
        if amount <= Decimal::ZERO {
            return Err(ShiftRefused::BadAmount);
        }
        self.paid_out += amount;
        Ok(())
    }

    /// What should be in the drawer: the float, plus cash taken, less cash paid out.
    pub fn expected(&self) -> Decimal {
        self.float + self.taken - self.paid_out
    }

    /// Counted minus expected, once closed.
    pub fn variance(&self) -> Option<Decimal> {
        self.counted.map(|c| c - self.expected())
    }

    /// Closes it with the cash counted. Without a variance it is settled; with one it waits for
    /// a supervisor.
    pub fn close(&mut self, counted: Decimal) -> Result<Decimal, ShiftRefused> {
        self.expect(ShiftStatus::Open, "close")?;
        if counted < Decimal::ZERO {
            return Err(ShiftRefused::BadAmount);
        }
        self.counted = Some(counted);
        let variance = counted - self.expected();
        self.status = if variance.is_zero() {
            ShiftStatus::Settled
        } else {
            ShiftStatus::AwaitingApproval
        };
        Ok(variance)
    }

    /// A supervisor, other than the cashier, approves the variance: the shift is settled and the
    /// variance is posted to till variance.
    pub fn approve(&mut self, supervisor: &str) -> Result<Decimal, ShiftRefused> {
        self.expect(ShiftStatus::AwaitingApproval, "approve")?;
        if supervisor == self.cashier || supervisor.trim().is_empty() {
            return Err(ShiftRefused::NotSupervisor);
        }
        self.approved_by = Some(supervisor.to_owned());
        self.status = ShiftStatus::Settled;
        Ok(self.variance().unwrap_or_default())
    }
}
