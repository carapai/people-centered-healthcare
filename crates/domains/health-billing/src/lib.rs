//! Billing's charge capture (billing-and-payments.md § Charge capture; identifiers.md; #121).
//!
//! - **Charge triggers** ([`ChargeTrigger`]) are configuration records mapping clinical events to
//!   charges: an Encounter started at a service point, a specimen collected, a MedicationDispense
//!   or Procedure completed, a bed-day accrued. Clinical Modules only emit events
//!   ([`ClinicalEvent`]); no clinical code contains payment logic.
//! - **Capture** ([`capture`]) turns an event into charges. Each charge id is the UUIDv5 of
//!   `{event id}#{item key}|{trigger canonical, no version}` under the platform namespace, so
//!   replaying an event, on any node, yields the same ids: the application keeps a charge once,
//!   and a replay never double-charges.
//! - **The lifecycle** ([`Charge`]): `planned` when captured, `billable` once priced, `billed` on an
//!   Invoice or Claim. A cancellation of its source event ([`Charge::cancel`]) makes a planned
//!   charge `not-billable`, and a billable one `entered-in-error` with a reversal to post; a
//!   billed charge is corrected on its Invoice, never here.
//! - **Free care**: a trigger may name a funding source (government or donor). Its charges are
//!   priced at the reference tariff and paid wholly by that source, so the patient share is zero
//!   and the consumption is still recorded (§70).
//! - **Financial Gates** ([`FinancialGate`], [`evaluate`]; ADR 0033): Finance's answer when a
//!   workflow reaches a gate. Emergency and urgent care is never blocked; deferred charges stay
//!   owed, and a [`Waiver`] is a separate, permitted and audited decision.

use std::future::Future;

mod gate;

pub use gate::{Deferral, FinancialGate, GateFacts, GateRefused, Waiver, evaluate};

use health_identifiers::{DerivationName, derive_id};
use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// One item of a clinical event, e.g. a dispensed product.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventItem {
    /// Its key in the event, stable across replays (e.g. the dispense line's id).
    pub key: String,
    /// Its code: a Healthcare Service or product.
    pub code: String,
    /// How many.
    pub quantity: Decimal,
}

/// A clinical event a Module emitted after its commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClinicalEvent {
    /// Its id: the source record's id or the op that produced it.
    pub id: String,
    /// What happened, e.g. `encounter.started`, `medication-dispense.completed`.
    pub kind: String,
    /// When the service was given.
    pub at: Timestamp,
    /// The patient.
    pub patient: String,
    /// The encounter, if any.
    #[serde(default)]
    pub encounter: Option<String>,
    /// The episode of care, if any.
    #[serde(default)]
    pub episode: Option<String>,
    /// The service point, if any.
    #[serde(default)]
    pub service_point: Option<String>,
    /// Its items; an event with none (an encounter starting) has one implicit item.
    #[serde(default)]
    pub items: Vec<EventItem>,
}

/// What a trigger charges for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "from", content = "code")]
pub enum ItemSource {
    /// Always this item, e.g. the consultation fee.
    Fixed(String),
    /// Each event item's own code, e.g. the product dispensed.
    Event,
}

/// Which Account a charge belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccountBasis {
    /// The encounter's.
    Encounter,
    /// The episode's: inpatient stays and long programs.
    Episode,
}

/// A charge trigger: a configuration record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChargeTrigger {
    /// Its canonical URL.
    pub url: String,
    /// Its version.
    pub version: String,
    /// The event kind it fires on.
    pub kind: String,
    /// The service points it fires at; empty for any.
    #[serde(default)]
    pub service_points: Vec<String>,
    /// The event item codes it fires for; empty for any.
    #[serde(default)]
    pub codes: Vec<String>,
    /// What it charges for.
    pub item: ItemSource,
    /// The Account basis.
    pub account: AccountBasis,
    /// A funding source paying in full (free care), if any.
    #[serde(default)]
    pub free_care: Option<String>,
}

/// Where a charge is in its life (FHIR `ChargeItem.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChargeStatus {
    /// Captured, not yet priced.
    Planned,
    /// Priced and owed.
    Billable,
    /// On an Invoice or Claim.
    Billed,
    /// Cancelled before it was owed.
    NotBillable,
    /// Cancelled after it was owed: reversed.
    EnteredInError,
}

impl ChargeStatus {
    /// The FHIR code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Billable => "billable",
            Self::Billed => "billed",
            Self::NotBillable => "not-billable",
            Self::EnteredInError => "entered-in-error",
        }
    }
}

/// A charge's price, as pricing gave it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChargePrice {
    /// The price after surcharges, discounts and bundles.
    pub gross: Decimal,
    /// What the patient pays.
    pub patient_share: Decimal,
    /// What each payer or funding source pays.
    pub payer_shares: Vec<(String, Decimal)>,
    /// The Price Book and rule versions used, as `id|version`.
    pub versions: Vec<String>,
    /// The split used cached accumulators.
    pub estimate: bool,
}

/// A charge (FHIR ChargeItem).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Charge {
    /// The deterministic id.
    pub id: String,
    /// The trigger, `url|version`.
    pub trigger: String,
    /// The source event.
    pub event: String,
    /// The event item it charges for.
    pub event_item: String,
    /// The patient.
    pub patient: String,
    /// The item charged.
    pub item: String,
    /// How many.
    pub quantity: Decimal,
    /// When the service was given.
    pub at: Timestamp,
    /// The Account it belongs to, e.g. `Encounter/123`.
    pub account: String,
    /// The service point, if any.
    #[serde(default)]
    pub service_point: Option<String>,
    /// The funding source paying in full, for free care.
    #[serde(default)]
    pub free_care: Option<String>,
    /// Where it is.
    pub status: ChargeStatus,
    /// Its price, once priced.
    #[serde(default)]
    pub price: Option<ChargePrice>,
    /// Why it was cancelled.
    #[serde(default)]
    pub cancelled: Option<String>,
}

/// Why an event could not be captured.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaptureError {
    /// The trigger charges to an Account the event does not name.
    #[error("{trigger} charges to the {basis:?} Account, which event {event} does not name")]
    NoAccount {
        /// The trigger.
        trigger: String,
        /// The event.
        event: String,
        /// The basis.
        basis: AccountBasis,
    },
}

/// Why a charge cannot take a step.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChargeError {
    /// The step does not fit the charge's status.
    #[error("a {0:?} charge cannot take this step")]
    WrongStatus(ChargeStatus),
    /// A billed charge is corrected on its Invoice.
    #[error("the charge is billed: correct it on its Invoice")]
    Billed,
    /// A price with a negative or inconsistent split.
    #[error("the price's shares do not add up to its gross amount")]
    BadPrice,
}

/// The id of the charge `trigger` makes for `item_key` of event `event`.
pub fn charge_id(event: &str, item_key: &str, trigger_url: &str) -> String {
    derive_id(&DerivationName::rule(
        format!("{event}#{item_key}"),
        trigger_url,
    ))
    .to_string()
}

fn fires(trigger: &ChargeTrigger, event: &ClinicalEvent, code: Option<&str>) -> bool {
    trigger.kind == event.kind
        && (trigger.service_points.is_empty()
            || event
                .service_point
                .as_ref()
                .is_some_and(|sp| trigger.service_points.contains(sp)))
        && (trigger.codes.is_empty()
            || code.is_some_and(|code| trigger.codes.iter().any(|c| c == code)))
}

/// The charges `event` gives under `triggers`, each `planned`, in trigger then item order.
pub fn capture(
    event: &ClinicalEvent,
    triggers: &[ChargeTrigger],
) -> Result<Vec<Charge>, CaptureError> {
    let implicit = [EventItem {
        key: "0".into(),
        code: String::new(),
        quantity: Decimal::ONE,
    }];
    let items: &[EventItem] = if event.items.is_empty() {
        &implicit
    } else {
        &event.items
    };
    let mut charges = Vec::new();
    for trigger in triggers {
        for item in items {
            let code = (!item.code.is_empty()).then_some(item.code.as_str());
            if !fires(trigger, event, code) {
                continue;
            }
            let charged = match &trigger.item {
                ItemSource::Fixed(code) => code.clone(),
                ItemSource::Event => match code {
                    Some(code) => code.to_owned(),
                    None => continue,
                },
            };
            let account = match trigger.account {
                AccountBasis::Encounter => event.encounter.as_ref(),
                AccountBasis::Episode => event.episode.as_ref(),
            }
            .ok_or_else(|| CaptureError::NoAccount {
                trigger: trigger.url.clone(),
                event: event.id.clone(),
                basis: trigger.account,
            })?;
            charges.push(Charge {
                id: charge_id(&event.id, &item.key, &trigger.url),
                trigger: format!("{}|{}", trigger.url, trigger.version),
                event: event.id.clone(),
                event_item: item.key.clone(),
                patient: event.patient.clone(),
                item: charged,
                quantity: item.quantity,
                at: event.at,
                account: account.clone(),
                service_point: event.service_point.clone(),
                free_care: trigger.free_care.clone(),
                status: ChargeStatus::Planned,
                price: None,
                cancelled: None,
            });
        }
    }
    Ok(charges)
}

/// What cancelling a charge needs from the ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reversal {
    /// Nothing was posted: nothing to reverse.
    None,
    /// Its posting must be reversed.
    Post,
}

impl Charge {
    /// Prices a planned charge: it becomes billable. The shares must add up to the gross amount,
    /// and none may be negative; free care leaves the patient nothing.
    pub fn priced(&self, price: ChargePrice) -> Result<Self, ChargeError> {
        if self.status != ChargeStatus::Planned {
            return Err(ChargeError::WrongStatus(self.status));
        }
        let payers: Decimal = price.payer_shares.iter().map(|(_, a)| *a).sum();
        let negative = price.patient_share < Decimal::ZERO
            || price.payer_shares.iter().any(|(_, a)| *a < Decimal::ZERO);
        let free_care_owes = self.free_care.is_some() && price.patient_share != Decimal::ZERO;
        if negative || payers + price.patient_share != price.gross || free_care_owes {
            return Err(ChargeError::BadPrice);
        }
        Ok(Self {
            status: ChargeStatus::Billable,
            price: Some(price),
            ..self.clone()
        })
    }

    /// Puts a billable charge on an Invoice or Claim.
    pub fn billed(&self) -> Result<Self, ChargeError> {
        if self.status != ChargeStatus::Billable {
            return Err(ChargeError::WrongStatus(self.status));
        }
        Ok(Self {
            status: ChargeStatus::Billed,
            ..self.clone()
        })
    }

    /// Cancels the charge because its source event was cancelled: a planned charge is not
    /// billable, and a billable one is entered in error with its posting reversed. Cancelling
    /// again changes nothing.
    pub fn cancel(&self, reason: &str) -> Result<(Self, Reversal), ChargeError> {
        let (status, reversal) = match self.status {
            ChargeStatus::Planned => (ChargeStatus::NotBillable, Reversal::None),
            ChargeStatus::Billable => (ChargeStatus::EnteredInError, Reversal::Post),
            ChargeStatus::Billed => return Err(ChargeError::Billed),
            ChargeStatus::NotBillable | ChargeStatus::EnteredInError => {
                return Ok((self.clone(), Reversal::None));
            }
        };
        Ok((
            Self {
                status,
                cancelled: Some(reason.to_owned()),
                ..self.clone()
            },
            reversal,
        ))
    }
}

/// Charges and charge triggers, inside the unit of work's transaction (billing's driven port).
pub trait ChargeStore {
    /// Adds a charge; false if one with its id is already kept (a replay).
    fn add_charge(
        &mut self,
        charge: &Charge,
    ) -> impl Future<Output = Result<bool, WriteError>> + Send;
    /// Replaces a kept charge.
    fn put_charge(
        &mut self,
        charge: &Charge,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// The charges of a source event.
    fn charges_of_event(
        &mut self,
        event: &str,
    ) -> impl Future<Output = Result<Vec<Charge>, StoreError>> + Send;
    /// The charges of an Account, oldest first.
    fn charges_of_account(
        &mut self,
        account: &str,
    ) -> impl Future<Output = Result<Vec<Charge>, StoreError>> + Send;
}

#[cfg(test)]
mod tests;
