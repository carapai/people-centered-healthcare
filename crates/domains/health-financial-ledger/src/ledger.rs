//! The facility ledger (billing-and-payments.md § The ledger; ADR 0030): a chart of accounts,
//! balanced postings appended only, corrections by reversal, and balances derived from postings.

use std::collections::{BTreeMap, BTreeSet};

use health_time::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::money::{Currency, Money, MoneyError};

/// An account's kind, which fixes its normal side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccountKind {
    /// Debit-normal: cash, receivables, payments in transit.
    Asset,
    /// Credit-normal: patient deposits.
    Liability,
    /// Credit-normal: revenue per service class.
    Revenue,
    /// Debit-normal: waivers and subsidies, set against revenue.
    ContraRevenue,
    /// Debit-normal: rounding, till variance.
    Expense,
    /// Credit-normal.
    Equity,
}

impl AccountKind {
    /// Whether a debit increases the balance.
    pub fn debit_normal(self) -> bool {
        matches!(self, Self::Asset | Self::ContraRevenue | Self::Expense)
    }
}

/// An account in the chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// Its code, e.g. `receivable/patient` or `cash/till-1`.
    pub code: String,
    /// Its kind.
    pub kind: AccountKind,
    /// Its name.
    pub name: String,
}

/// The facility's chart of accounts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartOfAccounts {
    accounts: BTreeMap<String, Account>,
}

/// The chart of accounts refuses an account.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("account {0:?} is defined twice")]
pub struct DuplicateAccount(pub String);

/// What a country's chart-of-accounts template is instantiated for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChartTemplate {
    /// Funding sources (`patient`, an insurer, a program, a donor), each with a receivable and a
    /// waiver account.
    pub funding_sources: Vec<String>,
    /// Service classes, each with a revenue account.
    pub service_classes: Vec<String>,
    /// Tills, each with a cash account.
    pub tills: Vec<String>,
}

/// The rounding account, where split and conversion residue goes.
pub const ROUNDING: &str = "rounding";
/// Patient deposits (a liability).
pub const DEPOSITS: &str = "deposits/patient";
/// Payments in transit (clearing).
pub const IN_TRANSIT: &str = "clearing/in-transit";
/// Till variance.
pub const TILL_VARIANCE: &str = "variance/till";

impl ChartOfAccounts {
    /// An empty chart.
    pub fn new() -> Self {
        Self::default()
    }

    /// The chart the template gives (billing-and-payments.md § The ledger): receivables and
    /// waivers per funding source, patient deposits, payments in transit, cash per till, revenue
    /// per service class, rounding and till variance.
    pub fn from_template(template: &ChartTemplate) -> Result<Self, DuplicateAccount> {
        let mut chart = Self::new();
        for source in &template.funding_sources {
            chart.add(Account {
                code: format!("receivable/{source}"),
                kind: AccountKind::Asset,
                name: format!("Receivable from {source}"),
            })?;
            chart.add(Account {
                code: format!("waiver/{source}"),
                kind: AccountKind::ContraRevenue,
                name: format!("Waivers and subsidies for {source}"),
            })?;
        }
        for class in &template.service_classes {
            chart.add(Account {
                code: format!("revenue/{class}"),
                kind: AccountKind::Revenue,
                name: format!("Revenue from {class}"),
            })?;
        }
        for till in &template.tills {
            chart.add(Account {
                code: format!("cash/{till}"),
                kind: AccountKind::Asset,
                name: format!("Cash in {till}"),
            })?;
        }
        for (code, kind, name) in [
            (DEPOSITS, AccountKind::Liability, "Patient deposits"),
            (IN_TRANSIT, AccountKind::Asset, "Payments in transit"),
            (ROUNDING, AccountKind::Expense, "Rounding"),
            (TILL_VARIANCE, AccountKind::Expense, "Till variance"),
        ] {
            chart.add(Account {
                code: code.to_owned(),
                kind,
                name: name.to_owned(),
            })?;
        }
        Ok(chart)
    }

    /// Adds `account`.
    pub fn add(&mut self, account: Account) -> Result<(), DuplicateAccount> {
        if self.accounts.contains_key(&account.code) {
            return Err(DuplicateAccount(account.code));
        }
        self.accounts.insert(account.code.clone(), account);
        Ok(())
    }

    /// The account with `code`.
    pub fn account(&self, code: &str) -> Option<&Account> {
        self.accounts.get(code)
    }
}

/// The analysis dimensions every posting line carries (billing-and-payments.md § The ledger).
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Dimensions {
    /// The facility.
    pub facility: String,
    /// The department.
    #[serde(default)]
    pub department: Option<String>,
    /// The service.
    #[serde(default)]
    pub service: Option<String>,
    /// The program.
    #[serde(default)]
    pub program: Option<String>,
    /// Who pays: the patient, an insurer, a program, a donor, a waiver.
    #[serde(default)]
    pub funding_source: Option<String>,
    /// A restricted grant.
    #[serde(default)]
    pub fund: Option<String>,
    /// The Encounter or EpisodeOfCare (`Encounter/{id}`).
    #[serde(default)]
    pub encounter: Option<String>,
    /// The price version references (`url|version`).
    #[serde(default)]
    pub price_versions: BTreeSet<String>,
}

/// A debit or a credit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Side {
    /// Left.
    Debit,
    /// Right.
    Credit,
}

impl Side {
    fn opposite(self) -> Self {
        match self {
            Self::Debit => Self::Credit,
            Self::Credit => Self::Debit,
        }
    }
}

/// An amount in a foreign currency, kept with the rate used to convert it (ADR 0030).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Original {
    /// The foreign amount.
    pub amount: Money,
    /// Functional currency per unit of the foreign one.
    pub rate: Decimal,
}

/// One line of a posting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    /// The account code.
    pub account: String,
    /// Debit or credit.
    pub side: Side,
    /// The amount, positive, in the ledger's functional currency.
    pub amount: Money,
    /// Its dimensions.
    pub dimensions: Dimensions,
    /// The foreign amount and rate, when it was not in the functional currency.
    #[serde(default)]
    pub original: Option<Original>,
}

/// A balanced set of lines, appended once and never changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posting {
    /// Its id.
    pub id: String,
    /// When it was made.
    pub at: Timestamp,
    /// What it records.
    pub description: String,
    /// Its lines.
    pub lines: Vec<Line>,
    /// The posting this one reverses, if it is a reversal.
    #[serde(default)]
    pub reverses: Option<String>,
}

/// Why a posting was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PostingRefused {
    /// Debits and credits differ.
    #[error("the posting does not balance: debits {debits}, credits {credits}")]
    Unbalanced {
        /// The debit total.
        debits: Decimal,
        /// The credit total.
        credits: Decimal,
    },
    /// Fewer than two lines.
    #[error("a posting needs at least two lines")]
    TooFewLines,
    /// A line's amount is zero or negative.
    #[error("line amounts are positive")]
    NonPositive,
    /// A line is not in the ledger's functional currency.
    #[error("the ledger's functional currency is {0}")]
    WrongCurrency(Currency),
    /// The account is not in the chart.
    #[error("account {0:?} is not in the chart")]
    UnknownAccount(String),
    /// A line names a different facility than the ledger's.
    #[error("the ledger belongs to facility {0}")]
    OtherFacility(String),
    /// The id is already used.
    #[error("posting {0:?} already exists")]
    DuplicateId(String),
    /// The posting to reverse does not exist, or is already reversed, or is itself a reversal.
    #[error("posting {0:?} cannot be reversed")]
    NotReversible(String),
    /// Money arithmetic failed.
    #[error(transparent)]
    Money(#[from] MoneyError),
}

/// One facility's ledger: its chart, its functional currency and its postings, append-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    facility: String,
    currency: Currency,
    chart: ChartOfAccounts,
    postings: Vec<Posting>,
}

impl Ledger {
    /// An empty ledger for `facility` in `currency`.
    pub fn new(facility: &str, currency: Currency, chart: ChartOfAccounts) -> Self {
        Self {
            facility: facility.to_owned(),
            currency,
            chart,
            postings: Vec::new(),
        }
    }

    /// The functional currency.
    pub fn currency(&self) -> Currency {
        self.currency
    }

    /// The postings, in the order they were appended.
    pub fn postings(&self) -> &[Posting] {
        &self.postings
    }

    /// Checks `posting` against the ledger without appending it.
    pub fn check(&self, posting: &Posting) -> Result<(), PostingRefused> {
        if posting.lines.len() < 2 {
            return Err(PostingRefused::TooFewLines);
        }
        if self.postings.iter().any(|p| p.id == posting.id) {
            return Err(PostingRefused::DuplicateId(posting.id.clone()));
        }
        let mut debits = Money::zero(self.currency);
        let mut credits = Money::zero(self.currency);
        for line in &posting.lines {
            if line.amount.currency() != self.currency {
                return Err(PostingRefused::WrongCurrency(self.currency));
            }
            if line.amount.amount() <= Decimal::ZERO {
                return Err(PostingRefused::NonPositive);
            }
            if self.chart.account(&line.account).is_none() {
                return Err(PostingRefused::UnknownAccount(line.account.clone()));
            }
            if line.dimensions.facility != self.facility {
                return Err(PostingRefused::OtherFacility(self.facility.clone()));
            }
            match line.side {
                Side::Debit => debits = debits.plus(line.amount)?,
                Side::Credit => credits = credits.plus(line.amount)?,
            }
        }
        if debits != credits {
            return Err(PostingRefused::Unbalanced {
                debits: debits.amount(),
                credits: credits.amount(),
            });
        }
        if let Some(original) = &posting.reverses {
            let reversible = self
                .postings
                .iter()
                .find(|p| &p.id == original)
                .is_some_and(|p| p.reverses.is_none())
                && !self
                    .postings
                    .iter()
                    .any(|p| p.reverses.as_deref() == Some(original.as_str()));
            if !reversible {
                return Err(PostingRefused::NotReversible(original.clone()));
            }
        }
        Ok(())
    }

    /// Appends `posting` once it checks.
    pub fn post(&mut self, posting: Posting) -> Result<(), PostingRefused> {
        self.check(&posting)?;
        self.postings.push(posting);
        Ok(())
    }

    /// The reversal of posting `original`: the same lines on opposite sides. It is the only way
    /// to correct a posting; reverse once, then post the right one.
    pub fn reversal(
        &self,
        original: &str,
        id: &str,
        at: Timestamp,
        reason: &str,
    ) -> Result<Posting, PostingRefused> {
        let posting = self
            .postings
            .iter()
            .find(|p| p.id == original)
            .ok_or_else(|| PostingRefused::NotReversible(original.to_owned()))?;
        let reversal = Posting {
            id: id.to_owned(),
            at,
            description: format!("Reversal of {original}: {reason}"),
            lines: posting
                .lines
                .iter()
                .map(|line| Line {
                    side: line.side.opposite(),
                    ..line.clone()
                })
                .collect(),
            reverses: Some(original.to_owned()),
        };
        self.check(&reversal)?;
        Ok(reversal)
    }

    /// The balance of `account` over the lines `filter` keeps, on its normal side (an asset's
    /// debits less its credits; a liability's or revenue's credits less its debits).
    pub fn balance(&self, account: &str, filter: impl Fn(&Dimensions) -> bool) -> Money {
        let debit_normal = self
            .chart
            .account(account)
            .is_none_or(|a| a.kind.debit_normal());
        let mut total = Decimal::ZERO;
        for line in self.postings.iter().flat_map(|p| &p.lines) {
            if line.account != account || !filter(&line.dimensions) {
                continue;
            }
            let signed = match (line.side, debit_normal) {
                (Side::Debit, true) | (Side::Credit, false) => line.amount.amount(),
                (Side::Debit, false) | (Side::Credit, true) => -line.amount.amount(),
            };
            total += signed;
        }
        Money::rounded(total, self.currency)
    }

    /// Debits less credits over every line: always zero.
    pub fn trial_balance(&self) -> Decimal {
        self.postings
            .iter()
            .flat_map(|p| &p.lines)
            .map(|l| match l.side {
                Side::Debit => l.amount.amount(),
                Side::Credit => -l.amount.amount(),
            })
            .sum()
    }

    /// What is owed for `encounter`, by funding source: the receivable balances of the lines
    /// that carry it. This is what FHIR `Account.balance[]` projects (ADR 0030).
    pub fn encounter_balances(&self, encounter: &str) -> BTreeMap<String, Money> {
        let mut balances = BTreeMap::new();
        for account in self.chart.accounts.values() {
            let Some(source) = account.code.strip_prefix("receivable/") else {
                continue;
            };
            let owed = self.balance(&account.code, |d| d.encounter.as_deref() == Some(encounter));
            if !owed.is_zero() {
                balances.insert(source.to_owned(), owed);
            }
        }
        balances
    }
}
