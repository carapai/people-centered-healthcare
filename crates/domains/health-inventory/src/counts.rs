//! Stock counts (inventory-and-procurement.md § Stock counts): Draft → Submitted → Approved, in
//! full, cycle or blind mode, without freezing the store.
//!
//! Each line keeps the balance at the count's start; the variance is what was counted minus
//! that balance and the movements recorded since, so issues during a count are not variances.
//! Approval needs a reason for every variance, and a supervisor for any variance above the
//! store's threshold.

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::{Movement, StockUnit};

/// How a count is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CountMode {
    /// Every Stock Unit of the store.
    Full,
    /// Some of them.
    Cycle,
    /// Every Stock Unit, without showing the counter what the ledger expects.
    Blind,
}

/// Where a count is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CountStatus {
    /// Being counted.
    Draft,
    /// Counted, waiting for approval.
    Submitted,
    /// Approved: its variances are posted.
    Approved,
}

/// One Stock Unit counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountLine {
    /// The Stock Unit.
    pub unit: StockUnit,
    /// Its on-hand balance when the count started.
    pub at_start: i64,
    /// What was counted.
    pub counted: Option<i64>,
    /// The reason code for a variance, e.g. `breakage`, `recount`.
    pub reason: Option<String>,
}

/// One line of the counter's sheet: in blind mode, without the expected quantity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetLine {
    /// The Stock Unit.
    pub unit: StockUnit,
    /// The balance at the start, unless the count is blind.
    pub expected: Option<i64>,
    /// What was counted so far.
    pub counted: Option<i64>,
}

/// A line's variance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variance {
    /// The line.
    pub line: usize,
    /// The Stock Unit.
    pub unit: StockUnit,
    /// The balance at the start plus the movements since.
    pub expected: i64,
    /// What was counted.
    pub counted: i64,
    /// `counted − expected`.
    pub variance: i64,
}

/// Why a count step was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CountRefused {
    /// The step does not apply in the count's status.
    #[error("cannot {action} a count that is {status:?}")]
    WrongState {
        /// The count's status.
        status: CountStatus,
        /// The step.
        action: &'static str,
    },
    /// There is no such line.
    #[error("the count has no line {0}")]
    NoLine(usize),
    /// A count is a quantity of zero or more.
    #[error("a count is zero or more")]
    Negative,
    /// Every line must be counted before it is submitted.
    #[error("line {0} is not counted")]
    Uncounted(usize),
    /// A variance needs a reason code.
    #[error("the variance on line {0} needs a reason")]
    ReasonRequired(usize),
    /// A variance above the threshold needs a supervisor's approval.
    #[error("a variance of {0} needs a supervisor")]
    SupervisorRequired(i64),
    /// A count has at least one line.
    #[error("a count needs at least one stock unit")]
    Empty,
}

/// A stock count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockCount {
    /// The count's id.
    pub id: String,
    /// The store counted.
    pub store: String,
    /// How.
    pub mode: CountMode,
    /// Where it is.
    pub status: CountStatus,
    /// When it started: movements recorded after this are not variances.
    pub started_at: Timestamp,
    /// The units counted.
    pub lines: Vec<CountLine>,
    /// Who submitted it.
    pub counted_by: Option<String>,
    /// Who approved it.
    pub approved_by: Option<String>,
}

impl StockCount {
    /// A count of `units` (each with its balance now) in `store`, started `at`.
    pub fn start(
        id: &str,
        store: &str,
        mode: CountMode,
        units: Vec<(StockUnit, i64)>,
        at: Timestamp,
    ) -> Result<Self, CountRefused> {
        if units.is_empty() {
            return Err(CountRefused::Empty);
        }
        Ok(Self {
            id: id.to_owned(),
            store: store.to_owned(),
            mode,
            status: CountStatus::Draft,
            started_at: at,
            lines: units
                .into_iter()
                .map(|(unit, at_start)| CountLine {
                    unit,
                    at_start,
                    counted: None,
                    reason: None,
                })
                .collect(),
            counted_by: None,
            approved_by: None,
        })
    }

    fn expect(&self, status: CountStatus, action: &'static str) -> Result<(), CountRefused> {
        if self.status == status {
            Ok(())
        } else {
            Err(CountRefused::WrongState {
                status: self.status,
                action,
            })
        }
    }

    /// The counter's sheet.
    pub fn sheet(&self) -> Vec<SheetLine> {
        self.lines
            .iter()
            .map(|line| SheetLine {
                unit: line.unit.clone(),
                expected: (self.mode != CountMode::Blind).then_some(line.at_start),
                counted: line.counted,
            })
            .collect()
    }

    /// Records what was counted on `line`, and the reason for any variance.
    pub fn record(
        &mut self,
        line: usize,
        counted: i64,
        reason: Option<&str>,
    ) -> Result<(), CountRefused> {
        self.expect(CountStatus::Draft, "record")?;
        if counted < 0 {
            return Err(CountRefused::Negative);
        }
        let entry = self.lines.get_mut(line).ok_or(CountRefused::NoLine(line))?;
        entry.counted = Some(counted);
        entry.reason = reason
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .map(str::to_owned);
        Ok(())
    }

    /// Submits it, once every line is counted.
    pub fn submit(&mut self, by: &str) -> Result<(), CountRefused> {
        self.expect(CountStatus::Draft, "submit")?;
        if let Some(line) = self.lines.iter().position(|l| l.counted.is_none()) {
            return Err(CountRefused::Uncounted(line));
        }
        self.counted_by = Some(by.to_owned());
        self.status = CountStatus::Submitted;
        Ok(())
    }

    /// The variances, given the movements recorded since the count started (of any unit; only
    /// the counted ones matter).
    pub fn variances(&self, since: &[Movement]) -> Vec<Variance> {
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                let counted = line.counted?;
                let key = line.unit.key();
                let moved: i64 = since
                    .iter()
                    .filter(|m| m.unit.key() == key && m.recorded_at > self.started_at)
                    .map(|m| m.quantity)
                    .sum();
                let expected = line.at_start + moved;
                Some(Variance {
                    line: index,
                    unit: line.unit.clone(),
                    expected,
                    counted,
                    variance: counted - expected,
                })
            })
            .collect()
    }

    /// Approves it: every variance needs a reason, and any above `threshold` (in base units, in
    /// either direction) a supervisor. Returns the variances to post.
    pub fn approve(
        &mut self,
        by: &str,
        supervisor: bool,
        threshold: i64,
        since: &[Movement],
    ) -> Result<Vec<Variance>, CountRefused> {
        self.expect(CountStatus::Submitted, "approve")?;
        let variances: Vec<Variance> = self
            .variances(since)
            .into_iter()
            .filter(|v| v.variance != 0)
            .collect();
        if let Some(v) = variances
            .iter()
            .find(|v| self.lines[v.line].reason.is_none())
        {
            return Err(CountRefused::ReasonRequired(v.line));
        }
        if let Some(v) = variances.iter().find(|v| v.variance.abs() > threshold)
            && !supervisor
        {
            return Err(CountRefused::SupervisorRequired(v.variance));
        }
        self.approved_by = Some(by.to_owned());
        self.status = CountStatus::Approved;
        Ok(variances)
    }
}
