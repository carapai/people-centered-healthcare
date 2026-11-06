//! The six stages (pricing-and-insurance.md § Precedence; ADR 0031), run wherever the charge is
//! recorded, with every Price Book and rule version recorded on the result.

use std::collections::BTreeSet;

use health_time::Timestamp;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::book::{Adjustment, Approval, PriceBook, PriceRule, Scope, Stage};

/// Evaluates a rule's FHIRPath applicability over the pricing context of a line (the ChargeItem,
/// encounter, program, coverages and eligibility categories), outside the domain.
pub trait Applicability {
    /// Whether `expression` holds for `line`.
    fn applies(&self, expression: &str, line: &Line) -> Result<bool, String>;
}

/// One item to price.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    /// Its id, e.g. the ChargeItem's.
    pub id: String,
    /// The Healthcare Service or product code.
    pub item: String,
    /// How many.
    pub quantity: Decimal,
}

/// The patient's part of what a coverage pays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "value")]
pub enum Copay {
    /// None.
    None,
    /// A percentage of what the coverage would pay.
    Percent(Decimal),
    /// A fixed amount per line.
    Amount(Decimal),
}

/// One coverage's Benefit Plan terms, as cached at the node (stage 4), in coverage order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageTerms {
    /// The Coverage.
    pub coverage: String,
    /// Items the plan excludes.
    #[serde(default)]
    pub excluded: Vec<String>,
    /// The copay.
    pub copay: Copay,
    /// What remains of the deductible.
    pub deductible_remaining: Decimal,
    /// What remains of the plan's limit, if it has one.
    #[serde(default)]
    pub limit_remaining: Option<Decimal>,
    /// The accumulators are a cached snapshot: the split is an estimate (ADR 0031).
    #[serde(default)]
    pub estimate: bool,
}

/// A manual override of a line's patient share (stage 6). Who may is the caller's permission
/// check; it is audited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Override {
    /// The line.
    pub line: String,
    /// The patient share it sets.
    pub patient_share: Decimal,
    /// Who.
    pub by: String,
    /// Why.
    pub reason: String,
}

/// What to price.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// When the service was given: which versions are in effect.
    pub service_time: Timestamp,
    /// The ledger's functional currency.
    pub currency: String,
    /// Its minor units (UGX 0, USD 2): every amount is rounded to them.
    pub minor_units: u32,
    /// The scopes the charge falls in (its payer contracts, program, department, facility, …).
    pub scopes: Vec<Scope>,
    /// The items.
    pub lines: Vec<Line>,
    /// The coverages, primary first.
    #[serde(default)]
    pub coverages: Vec<CoverageTerms>,
    /// Manual overrides.
    #[serde(default)]
    pub overrides: Vec<Override>,
}

/// A book or rule version used.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Applied {
    /// Its id.
    pub id: String,
    /// Its version.
    pub version: u32,
}

/// What a component is (ChargeItemDefinition `priceComponent.type`, with the platform's
/// detail).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Stage 1.
    Base,
    /// Stage 2.
    Surcharge,
    /// Stage 3.
    Discount,
    /// Stage 3: priced in a bundle line instead.
    Bundled,
    /// Stage 5: taken off the patient share.
    Subsidy,
    /// Stage 6: the change to the patient share.
    Override,
}

/// One component of a line's price.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    /// What it is.
    pub kind: Kind,
    /// The amount: positive adds, negative takes off.
    pub amount: Decimal,
    /// The book or rule version it came from; none for an override.
    #[serde(default)]
    pub from: Option<Applied>,
    /// The bundle line, an override's reason and who gave it.
    #[serde(default)]
    pub note: Option<String>,
}

/// A priced line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricedLine {
    /// The line.
    pub id: String,
    /// Its item.
    pub item: String,
    /// Its quantity.
    pub quantity: Decimal,
    /// Its components, in stage order.
    pub components: Vec<Component>,
    /// The price after stages 1–3.
    pub gross: Decimal,
    /// What each coverage pays (stage 4).
    pub payer_shares: Vec<(String, Decimal)>,
    /// What each coverage's deductible took of the line (stage 4), for its accumulator (#125).
    #[serde(default)]
    pub deductibles: Vec<(String, Decimal)>,
    /// What the patient pays, after stages 5 and 6.
    pub patient_share: Decimal,
    /// The bundle line it is priced in, if bundled.
    #[serde(default)]
    pub bundled_in: Option<String>,
    /// The split used cached accumulators.
    pub estimate: bool,
}

/// A priced charge, with every version it used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Priced {
    /// The lines, bundle lines last.
    pub lines: Vec<PricedLine>,
    /// The Price Book versions used.
    pub books: Vec<Applied>,
    /// The rule versions applied.
    pub rules: Vec<Applied>,
}

/// Why a charge could not be priced.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PricingError {
    /// No approved book in effect, in the charge's scopes and currency, prices the item.
    #[error("no Price Book in effect prices {0}")]
    NoPrice(String),
    /// Two equally specific books price the item.
    #[error("{0} is priced by two equally specific Price Books")]
    Ambiguous(String),
    /// A rule's applicability could not be evaluated.
    #[error("the applicability of {rule} failed: {reason}")]
    Applicability {
        /// The rule.
        rule: String,
        /// Why.
        reason: String,
    },
    /// An override with no reason, no author, a negative share, or for no line.
    #[error("the override of {0} needs a line, an author, a reason and a share of zero or more")]
    BadOverride(String),
}

fn approved(approval: &Approval) -> bool {
    matches!(approval, Approval::Approved { .. })
}

fn applied(id: &str, version: u32) -> Applied {
    Applied {
        id: id.to_owned(),
        version,
    }
}

struct Pricer<'a> {
    request: &'a Request,
    rules: Vec<&'a PriceRule>,
    applicability: &'a dyn Applicability,
    used_rules: BTreeSet<Applied>,
    /// Each coverage's deductible and limit still left, consumed line by line across the charge.
    left: Vec<(Decimal, Option<Decimal>)>,
}

impl<'a> Pricer<'a> {
    fn round(&self, amount: Decimal) -> Decimal {
        amount.round_dp_with_strategy(
            self.request.minor_units,
            RoundingStrategy::MidpointAwayFromZero,
        )
    }

    fn percent(&self, of: Decimal, percent: Decimal) -> Decimal {
        self.round(of * percent / Decimal::ONE_HUNDRED)
    }

    fn holds(&self, rule: &PriceRule, line: &Line) -> Result<bool, PricingError> {
        if !rule.names(&line.item) {
            return Ok(false);
        }
        match &rule.applicability {
            None => Ok(true),
            Some(expression) => self
                .applicability
                .applies(expression, line)
                .map_err(|reason| PricingError::Applicability {
                    rule: rule.id.clone(),
                    reason,
                }),
        }
    }

    fn of_stage(&self, stage: Stage) -> Vec<&'a PriceRule> {
        self.rules
            .iter()
            .copied()
            .filter(|r| r.stage == stage)
            .collect()
    }

    fn adjust(
        &mut self,
        line: &mut PricedLine,
        stage: Stage,
        kind: Kind,
    ) -> Result<(), PricingError> {
        let probe = Line {
            id: line.id.clone(),
            item: line.item.clone(),
            quantity: line.quantity,
        };
        for rule in self.of_stage(stage) {
            if line.bundled_in.is_some() || !self.holds(rule, &probe)? {
                continue;
            }
            let base = if stage == Stage::Subsidy {
                line.patient_share
            } else {
                line.gross
            };
            let amount = match &rule.adjustment {
                Adjustment::Percent { percent } => self.percent(base, *percent),
                Adjustment::Amount { amount } => self.round(*amount),
                Adjustment::Bundle { .. } => continue,
            };
            // A discount or subsidy takes off at most what there is.
            let signed = if stage == Stage::Surcharge {
                amount
            } else {
                -amount.min(base)
            };
            if stage == Stage::Subsidy {
                line.patient_share += signed;
            } else {
                line.gross += signed;
            }
            self.used_rules.insert(applied(&rule.id, rule.version));
            line.components.push(Component {
                kind,
                amount: signed,
                from: Some(applied(&rule.id, rule.version)),
                note: None,
            });
        }
        Ok(())
    }

    /// Stage 3 bundles: each bundle whose items are all on the charge prices them together.
    fn bundle(&mut self, lines: &mut Vec<PricedLine>) -> Result<(), PricingError> {
        for rule in self.of_stage(Stage::Discount) {
            let Adjustment::Bundle { items, price, item } = &rule.adjustment else {
                continue;
            };
            let mut members: Vec<usize> = Vec::new();
            for wanted in items {
                let found = (0..lines.len()).find(|&i| {
                    &lines[i].item == wanted
                        && lines[i].bundled_in.is_none()
                        && !members.contains(&i)
                });
                match found {
                    Some(index) => members.push(index),
                    None => break,
                }
            }
            if members.len() != items.len() {
                continue;
            }
            let first = &lines[members[0]];
            let probe = Line {
                id: first.id.clone(),
                item: first.item.clone(),
                quantity: first.quantity,
            };
            if !self.holds(rule, &probe)? {
                continue;
            }
            let bundle_id = format!("bundle:{}:{}", rule.id, first.id);
            for &index in &members {
                let line = &mut lines[index];
                line.components.push(Component {
                    kind: Kind::Bundled,
                    amount: -line.gross,
                    from: Some(applied(&rule.id, rule.version)),
                    note: Some(bundle_id.clone()),
                });
                line.gross = Decimal::ZERO;
                line.bundled_in = Some(bundle_id.clone());
            }
            let price = self.round(*price);
            lines.push(PricedLine {
                id: bundle_id,
                item: item.clone(),
                quantity: Decimal::ONE,
                components: vec![Component {
                    kind: Kind::Base,
                    amount: price,
                    from: Some(applied(&rule.id, rule.version)),
                    note: None,
                }],
                gross: price,
                payer_shares: Vec::new(),
                deductibles: Vec::new(),
                patient_share: Decimal::ZERO,
                bundled_in: None,
                estimate: false,
            });
            self.used_rules.insert(applied(&rule.id, rule.version));
        }
        Ok(())
    }

    /// Stage 4: each coverage in order pays its part of what remains; the patient pays the rest.
    /// Deductibles and limits run down across the charge's lines.
    fn split(&mut self, line: &mut PricedLine) {
        let mut remaining = line.gross;
        for (index, terms) in self.request.coverages.iter().enumerate() {
            if terms.excluded.iter().any(|i| i == &line.item) || remaining <= Decimal::ZERO {
                continue;
            }
            let (deductible_left, limit_left) = self.left[index];
            let deductible = deductible_left.max(Decimal::ZERO).min(remaining);
            let after = remaining - deductible;
            let copay = match &terms.copay {
                Copay::None => Decimal::ZERO,
                Copay::Percent(percent) => self.percent(after, *percent),
                Copay::Amount(amount) => self.round(*amount),
            }
            .min(after);
            let mut pays = after - copay;
            if let Some(limit) = limit_left {
                pays = pays.min(limit.max(Decimal::ZERO));
            }
            self.left[index] = (deductible_left - deductible, limit_left.map(|l| l - pays));
            line.estimate |= terms.estimate;
            line.payer_shares.push((terms.coverage.clone(), pays));
            line.deductibles.push((terms.coverage.clone(), deductible));
            remaining -= pays;
        }
        line.patient_share = remaining;
    }
}

/// Prices `request` from `books` and `rules` (any versions: only those approved and in effect
/// at the service time count), through the six stages.
pub fn price(
    request: &Request,
    books: &[PriceBook],
    rules: &[PriceRule],
    applicability: &dyn Applicability,
) -> Result<Priced, PricingError> {
    let at = request.service_time;
    let mut active_rules: Vec<&PriceRule> = rules
        .iter()
        .filter(|r| approved(&r.approval) && r.effective.covers(at))
        .collect();
    active_rules.sort_by(|a, b| (a.stage, a.priority, &a.id).cmp(&(b.stage, b.priority, &b.id)));
    let mut pricer = Pricer {
        request,
        rules: active_rules,
        applicability,
        used_rules: BTreeSet::new(),
        left: request
            .coverages
            .iter()
            .map(|c| (c.deductible_remaining, c.limit_remaining))
            .collect(),
    };
    let books: Vec<&PriceBook> = books
        .iter()
        .filter(|b| {
            approved(&b.approval)
                && b.effective.covers(at)
                && b.currency == request.currency
                && request.scopes.contains(&b.scope)
        })
        .collect();

    // Stage 1: the base price, from the most specific book that holds the item.
    let mut used_books = BTreeSet::new();
    let mut lines = Vec::new();
    for line in &request.lines {
        let mut found: Vec<(&PriceBook, &crate::book::Entry)> = books
            .iter()
            .filter_map(|b| {
                b.entries
                    .iter()
                    .find(|e| e.item == line.item)
                    .map(|e| (*b, e))
            })
            .collect();
        found.sort_by_key(|(b, _)| b.scope.rank());
        let Some((book, entry)) = found.first().copied() else {
            return Err(PricingError::NoPrice(line.item.clone()));
        };
        if found
            .get(1)
            .is_some_and(|(b, _)| b.scope.rank() == book.scope.rank())
        {
            return Err(PricingError::Ambiguous(line.item.clone()));
        }
        let unit = entry
            .tiers
            .iter()
            .filter(|t| t.min <= line.quantity)
            .max_by(|a, b| a.min.cmp(&b.min))
            .map_or(entry.unit, |t| t.unit);
        let base = pricer.round(unit * line.quantity);
        used_books.insert(applied(&book.id, book.version));
        lines.push(PricedLine {
            id: line.id.clone(),
            item: line.item.clone(),
            quantity: line.quantity,
            components: vec![Component {
                kind: Kind::Base,
                amount: base,
                from: Some(applied(&book.id, book.version)),
                note: None,
            }],
            gross: base,
            payer_shares: Vec::new(),
            deductibles: Vec::new(),
            patient_share: Decimal::ZERO,
            bundled_in: None,
            estimate: false,
        });
    }

    // Stage 2: surcharges. Stage 3: bundles, then discounts.
    for line in &mut lines {
        pricer.adjust(line, Stage::Surcharge, Kind::Surcharge)?;
    }
    pricer.bundle(&mut lines)?;
    for line in &mut lines {
        pricer.adjust(line, Stage::Discount, Kind::Discount)?;
    }
    // Stage 4: the coverage split. Stage 5: subsidies and waivers on the patient share.
    for line in &mut lines {
        pricer.split(line);
        pricer.adjust(line, Stage::Subsidy, Kind::Subsidy)?;
    }
    // Stage 6: manual overrides of the patient share.
    for change in &request.overrides {
        let line = lines
            .iter_mut()
            .find(|l| l.id == change.line)
            .filter(|_| {
                !change.by.trim().is_empty()
                    && !change.reason.trim().is_empty()
                    && change.patient_share >= Decimal::ZERO
            })
            .ok_or_else(|| PricingError::BadOverride(change.line.clone()))?;
        let share = pricer.round(change.patient_share);
        line.components.push(Component {
            kind: Kind::Override,
            amount: share - line.patient_share,
            from: None,
            note: Some(format!("{}: {}", change.by, change.reason)),
        });
        line.patient_share = share;
    }

    Ok(Priced {
        lines,
        books: used_books.into_iter().collect(),
        rules: pricer.used_rules.into_iter().collect(),
    })
}

/// Why a recorded version does not stand at the authority: the Reconciliation Record's reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Mismatch {
    /// The authority has no approved book or rule of that version.
    #[error("{0:?} is not an approved version here")]
    Unknown(Applied),
    /// It was not in effect at the service time.
    #[error("{0:?} was not in effect at the service time")]
    NotInEffect(Applied),
}

/// The authority's check (ADR 0031): every recorded version must be approved and in effect at
/// `service_time`. A mismatch becomes a Reconciliation Record, never a silent re-price.
pub fn check(
    priced: &Priced,
    service_time: Timestamp,
    books: &[PriceBook],
    rules: &[PriceRule],
) -> Vec<Mismatch> {
    let book_status = |a: &Applied| {
        books
            .iter()
            .find(|b| b.id == a.id && b.version == a.version && approved(&b.approval))
            .map(|b| b.effective.covers(service_time))
    };
    let rule_status = |a: &Applied| {
        rules
            .iter()
            .find(|r| r.id == a.id && r.version == a.version && approved(&r.approval))
            .map(|r| r.effective.covers(service_time))
    };
    let mut found = Vec::new();
    let recorded = priced
        .books
        .iter()
        .map(|a| (a, book_status(a)))
        .chain(priced.rules.iter().map(|a| (a, rule_status(a))));
    for (version, status) in recorded {
        match status {
            None => found.push(Mismatch::Unknown(version.clone())),
            Some(false) => found.push(Mismatch::NotInEffect(version.clone())),
            Some(true) => {}
        }
    }
    found
}
