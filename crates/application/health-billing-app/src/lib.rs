//! Charge capture at the facility authority (billing-and-payments.md § Charge capture, § The
//! ledger; ADRs 0030, 0031; #121).
//!
//! - **[`Billing::capture`]** takes a clinical event (consumed after its commit, through the
//!   subscription) and, in one transaction, captures its charges under the triggers, prices
//!   them together through the six stages, keeps each charge once by its deterministic id, and
//!   appends a balanced posting per charge: the patient's and each payer's receivable against
//!   the revenue of the item's service class. A replayed event (on this node or any other) finds
//!   its charges already kept and posts nothing: it never double-charges.
//! - **Free care** is priced from the reference-tariff scopes, with the trigger's funding source
//!   paying in full, so the patient share is zero and the consumption is still recorded.
//! - **Bundles:** the charges of one event are priced together, so a bundle among them applies.
//!   The bundled charges are priced at zero, and the bundle line becomes a charge of its own.
//! - **[`Billing::cancel_event`]**: when a source event is cancelled, its charges are cancelled
//!   and each posted one reversed, once.
//!
//! Coverage terms are the caller's (from coverage), each `coverage` naming its funding source.

use std::collections::BTreeMap;

use health_billing::{
    Charge, ChargePrice, ChargeStatus, ChargeStore, ChargeTrigger, ClinicalEvent, Reversal,
    capture, charge_id,
};
use health_financial_ledger::{
    ChartOfAccounts, ChartTemplate, Currency, Dimensions, Ledger, Line, Money, Posting,
    PostingRefused, PostingStore, Side,
};
use health_persistence::{Store, StoreError, Transaction, WriteError};
use health_pricing::{
    Applicability, Copay, CoverageTerms, Line as PriceLine, PricingError, PricingStore, Request,
    Scope, price,
};
use health_time::Timestamp;
use rust_decimal::Decimal;

/// The patient as a funding source.
pub const PATIENT: &str = "patient";

/// Why capture or cancellation failed. Nothing was kept.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BillingError {
    /// The event could not be captured.
    #[error(transparent)]
    Capture(#[from] health_billing::CaptureError),
    /// A charge could not take its step.
    #[error(transparent)]
    Charge(#[from] health_billing::ChargeError),
    /// The charges could not be priced.
    #[error(transparent)]
    Pricing(#[from] PricingError),
    /// The ledger refused a posting.
    #[error(transparent)]
    Ledger(#[from] PostingRefused),
    /// The settings are not usable.
    #[error("{0}")]
    Settings(String),
    /// The store failed, or a concurrent change came first; try again.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for BillingError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for BillingError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

/// A facility's billing settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The facility.
    pub facility: String,
    /// The ledger's functional currency.
    pub currency: Currency,
    /// The chart of accounts.
    pub chart: ChartTemplate,
    /// The scopes this facility's charges are priced in, most specific first.
    pub scopes: Vec<Scope>,
    /// The scopes of the reference tariff free care is priced at.
    pub reference_scopes: Vec<Scope>,
    /// Each item's service class (its revenue account), and the class of the rest.
    pub service_classes: BTreeMap<String, String>,
    /// The class of items not listed.
    pub default_class: String,
}

/// The charge use cases over `store`.
pub struct Billing<S> {
    store: S,
    settings: Settings,
}

fn posting_id(charge: &str) -> String {
    format!("charge-{charge}")
}

impl<S> Billing<S>
where
    S: Store,
    S::Tx: ChargeStore + PostingStore + PricingStore,
{
    /// The use cases over `store` with `settings`.
    pub fn new(store: S, settings: Settings) -> Self {
        Self { store, settings }
    }

    fn ledger(&self) -> Result<Ledger, BillingError> {
        let chart = ChartOfAccounts::from_template(&self.settings.chart)
            .map_err(|e| BillingError::Settings(e.to_string()))?;
        Ok(Ledger::new(
            &self.settings.facility,
            self.settings.currency,
            chart,
        ))
    }

    fn class(&self, item: &str) -> &str {
        self.settings
            .service_classes
            .get(item)
            .map_or(self.settings.default_class.as_str(), String::as_str)
    }

    /// The posting of billable `charge`: each share's receivable debited, the revenue of its
    /// service class credited. None for a charge priced at zero (bundled).
    fn posting(&self, charge: &Charge) -> Option<Posting> {
        let price = charge.price.as_ref()?;
        if price.gross == Decimal::ZERO {
            return None;
        }
        let currency = self.settings.currency;
        let dimensions = |funding: &str| Dimensions {
            facility: self.settings.facility.clone(),
            service: Some(charge.item.clone()),
            funding_source: Some(funding.to_owned()),
            encounter: Some(charge.account.clone()),
            price_versions: price.versions.iter().cloned().collect(),
            ..Dimensions::default()
        };
        let mut lines: Vec<Line> = std::iter::once((PATIENT.to_owned(), price.patient_share))
            .chain(price.payer_shares.iter().cloned())
            .filter(|(_, amount)| *amount > Decimal::ZERO)
            .map(|(funding, amount)| Line {
                account: format!("receivable/{funding}"),
                side: Side::Debit,
                amount: Money::rounded(amount, currency),
                dimensions: dimensions(&funding),
                original: None,
            })
            .collect();
        lines.push(Line {
            account: format!("revenue/{}", self.class(&charge.item)),
            side: Side::Credit,
            amount: Money::rounded(price.gross, currency),
            dimensions: dimensions(charge.free_care.as_deref().unwrap_or(PATIENT)),
            original: None,
        });
        Some(Posting {
            id: posting_id(&charge.id),
            at: charge.at,
            description: format!(
                "Charge {} for {} ({})",
                charge.id, charge.item, charge.account
            ),
            lines,
            reverses: None,
        })
    }

    /// Captures `event` under `triggers`, pricing with the caller's `coverages` (primary first)
    /// and `applicability`. Returns the charges kept now: none for a replay.
    pub async fn capture(
        &self,
        event: &ClinicalEvent,
        triggers: &[ChargeTrigger],
        coverages: &[CoverageTerms],
        applicability: &(dyn Applicability + Sync),
    ) -> Result<Vec<Charge>, BillingError> {
        let captured = capture(event, triggers)?;
        if captured.is_empty() {
            return Ok(Vec::new());
        }
        let mut tx = self.store.begin().await?;
        let books = tx.books().await?;
        let rules = tx.rules().await?;
        let ledger = self.ledger()?;
        let minor_units = self.settings.currency.scale();

        // Priced together, per funding: free care at the reference tariff, the rest as billed.
        let mut groups: BTreeMap<Option<String>, Vec<&Charge>> = BTreeMap::new();
        for charge in &captured {
            groups
                .entry(charge.free_care.clone())
                .or_default()
                .push(charge);
        }
        let mut kept = Vec::new();
        for (free_care, charges) in groups {
            let (scopes, coverages) = match &free_care {
                Some(source) => (
                    self.settings.reference_scopes.clone(),
                    vec![CoverageTerms {
                        coverage: source.clone(),
                        excluded: Vec::new(),
                        copay: Copay::None,
                        deductible_remaining: Decimal::ZERO,
                        limit_remaining: None,
                        estimate: false,
                    }],
                ),
                None => (self.settings.scopes.clone(), coverages.to_vec()),
            };
            let request = Request {
                service_time: event.at,
                currency: self.settings.currency.code().to_owned(),
                minor_units,
                scopes,
                lines: charges
                    .iter()
                    .map(|c| PriceLine {
                        id: c.id.clone(),
                        item: c.item.clone(),
                        quantity: c.quantity,
                    })
                    .collect(),
                coverages,
                overrides: Vec::new(),
            };
            let priced = price(&request, &books, &rules, applicability)?;
            let versions: Vec<String> = priced
                .books
                .iter()
                .chain(&priced.rules)
                .map(|a| format!("{}|{}", a.id, a.version))
                .collect();
            for line in &priced.lines {
                let charge = match charges.iter().find(|c| c.id == line.id) {
                    Some(charge) => (*charge).clone(),
                    // A bundle line: a charge of its own, from the first bundled charge.
                    None => {
                        let first = charges
                            .iter()
                            .find(|c| line.id.ends_with(&format!(":{}", c.id)))
                            .copied()
                            .unwrap_or(charges[0]);
                        let trigger_url = first.trigger.split('|').next().unwrap_or_default();
                        Charge {
                            id: charge_id(&event.id, &line.id, trigger_url),
                            event_item: line.id.clone(),
                            item: line.item.clone(),
                            quantity: line.quantity,
                            ..first.clone()
                        }
                    }
                };
                let billable = charge.priced(ChargePrice {
                    gross: line.gross,
                    patient_share: line.patient_share,
                    payer_shares: line.payer_shares.clone(),
                    versions: versions.clone(),
                    estimate: line.estimate,
                })?;
                if !tx.add_charge(&billable).await? {
                    continue;
                }
                if let Some(posting) = self.posting(&billable) {
                    ledger.check(&posting)?;
                    tx.append_posting(&posting).await?;
                }
                kept.push(billable);
            }
        }
        tx.commit().await?;
        Ok(kept)
    }

    /// Cancels the charges of cancelled event `event`, reversing each posted one once.
    pub async fn cancel_event(
        &self,
        event: &str,
        reason: &str,
        at: Timestamp,
    ) -> Result<Vec<Charge>, BillingError> {
        let mut tx = self.store.begin().await?;
        // The facility's postings so far: a reversal must name one, and only once.
        let mut ledger = self.ledger()?;
        for posting in tx.postings().await? {
            ledger.post(posting)?;
        }
        let mut changed = Vec::new();
        for charge in tx.charges_of_event(event).await? {
            let (cancelled, reversal) = charge.cancel(reason)?;
            if cancelled == charge {
                continue;
            }
            let original = posting_id(&charge.id);
            let posted = ledger.postings().iter().any(|p| p.id == original);
            if reversal == Reversal::Post && posted {
                let reversal =
                    ledger.reversal(&original, &format!("{original}-reversal"), at, reason)?;
                tx.append_posting(&reversal).await?;
                ledger.post(reversal)?;
            }
            tx.put_charge(&cancelled).await?;
            changed.push(cancelled);
        }
        tx.commit().await?;
        Ok(changed)
    }

    /// The charges of `account`, oldest first.
    pub async fn charges(&self, account: &str) -> Result<Vec<Charge>, BillingError> {
        let mut tx = self.store.begin().await?;
        Ok(tx.charges_of_account(account).await?)
    }
}

/// Whether `charge` still counts towards what is owed.
pub fn owed(charge: &Charge) -> bool {
    matches!(charge.status, ChargeStatus::Billable | ChargeStatus::Billed)
}
