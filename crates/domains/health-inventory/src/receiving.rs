//! Receiving, requisitions and reorder (inventory-and-procurement.md § Receiving, requisitions
//! and reorder; #130).
//!
//! - **Goods received notes** are recorded at the facility authority against a supplier
//!   delivery: each line a receipt of a Stock Unit at its unit cost.
//! - **GS1 DataMatrix** ([`parse_gs1`]) fills a line from a barcode where one exists; it is
//!   never required.
//! - **Requisitions** within the facility (ward → pharmacy) are SupplyRequests, fulfilled by a
//!   transfer.
//! - **Reorder** ([`suggest`]): min/max levels per store and product from the average monthly
//!   consumption; when stock falls to the minimum, order up to the maximum.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{Ownership, Status, StockUnit};

/// One line of a delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceivedLine {
    /// The product.
    pub product: String,
    /// The batch.
    pub batch: String,
    /// The expiry (`YYYY-MM-DD`).
    pub expiry: String,
    /// Who paid for it.
    pub funding_source: String,
    /// Owned or consignment.
    pub ownership: Ownership,
    /// How many, in base units.
    pub quantity: i64,
    /// The pack size it came in.
    pub pack_size: u32,
    /// The cost of one base unit.
    pub unit_cost: Decimal,
}

impl ReceivedLine {
    /// The Stock Unit it is received into at `store`.
    pub fn unit(&self, store: &str) -> StockUnit {
        StockUnit {
            store: store.to_owned(),
            product: self.product.clone(),
            batch: self.batch.clone(),
            expiry: self.expiry.clone(),
            funding_source: self.funding_source.clone(),
            ownership: self.ownership,
            status: Status::Usable,
        }
    }
}

/// A goods received note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoodsReceived {
    /// The note's id.
    pub id: String,
    /// The receiving store.
    pub store: String,
    /// The supplier, e.g. `NMS` or `JMS`.
    pub supplier: String,
    /// The supplier's delivery or invoice number.
    pub delivery: String,
    /// What arrived.
    pub lines: Vec<ReceivedLine>,
}

/// Why a note, requisition or barcode was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReceivingRefused {
    /// The note or requisition is incomplete.
    #[error("{0}")]
    Invalid(String),
    /// The step does not apply in the requisition's status.
    #[error("cannot {action} a requisition that is {status:?}")]
    WrongState {
        /// The requisition's status.
        status: RequisitionStatus,
        /// The step.
        action: &'static str,
    },
    /// The barcode is not GS1 element strings this parser knows.
    #[error("not a GS1 barcode: {0}")]
    Barcode(&'static str),
}

impl GoodsReceived {
    /// Checks a note before it is received: a supplier and delivery, and lines with a product,
    /// batch and expiry, a positive quantity and pack size, and a cost of zero or more.
    pub fn validate(&self) -> Result<(), ReceivingRefused> {
        let invalid = |m: &str| Err(ReceivingRefused::Invalid(m.to_owned()));
        if self.supplier.trim().is_empty() || self.delivery.trim().is_empty() {
            return invalid("a goods received note names the supplier and the delivery");
        }
        if self.lines.is_empty() {
            return invalid("a goods received note has at least one line");
        }
        for (i, line) in self.lines.iter().enumerate() {
            if line.product.trim().is_empty()
                || line.batch.trim().is_empty()
                || line.expiry.trim().is_empty()
            {
                return invalid(&format!("line {i} needs its product, batch and expiry"));
            }
            if line.quantity <= 0 || line.pack_size == 0 {
                return invalid(&format!("line {i} needs a positive quantity and pack size"));
            }
            if line.unit_cost.is_sign_negative() {
                return invalid(&format!("line {i} has a negative cost"));
            }
        }
        Ok(())
    }
}

/// What a GS1 DataMatrix holds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Gs1 {
    /// AI 01: the GTIN (14 digits).
    pub gtin: Option<String>,
    /// AI 17: the expiry, `YYYY-MM-DD` (a day of `00` is the month's last day).
    pub expiry: Option<String>,
    /// AI 10: the batch or lot.
    pub batch: Option<String>,
    /// AI 21: the serial number.
    pub serial: Option<String>,
}

/// The group separator (FNC1) that ends a variable-length element.
const GS: char = '\u{1d}';

fn expiry(yymmdd: &str) -> Result<String, ReceivingRefused> {
    let bad = || ReceivingRefused::Barcode("the expiry (17) is YYMMDD");
    if yymmdd.len() != 6 || !yymmdd.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    let year = 2000 + yymmdd[0..2].parse::<i32>().map_err(|_| bad())?;
    let month: u32 = yymmdd[2..4].parse().map_err(|_| bad())?;
    let mut day: u32 = yymmdd[4..6].parse().map_err(|_| bad())?;
    if !(1..=12).contains(&month) {
        return Err(bad());
    }
    let last = match month {
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 31,
    };
    if day == 0 {
        day = last;
    } else if day > last {
        return Err(bad());
    }
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

/// Parses GS1 element strings: as scanned (`]d2` symbology prefix optional, variable-length
/// elements ended by the group separator) or as printed (`(01)…(17)…(10)…`). AIs 01, 17, 10 and
/// 21 are read; others are refused.
pub fn parse_gs1(data: &str) -> Result<Gs1, ReceivingRefused> {
    let data = data.trim();
    let data = data.strip_prefix("]d2").unwrap_or(data);
    // The printed form: turn `(AI)` into separators between elements.
    let normalized: String = if data.starts_with('(') {
        let mut out = String::new();
        for part in data.split('(').filter(|p| !p.is_empty()) {
            let (ai, value) = part
                .split_once(')')
                .ok_or(ReceivingRefused::Barcode("an unclosed AI"))?;
            if !out.is_empty() {
                out.push(GS);
            }
            out.push_str(ai);
            out.push_str(value);
        }
        out
    } else {
        data.to_owned()
    };
    let mut gs1 = Gs1::default();
    let mut rest = normalized.as_str();
    while !rest.is_empty() {
        rest = rest.trim_start_matches(GS);
        if rest.len() < 2 {
            if rest.is_empty() {
                break;
            }
            return Err(ReceivingRefused::Barcode("a truncated AI"));
        }
        let (ai, after) = rest.split_at(2);
        match ai {
            "01" | "17" => {
                let len = if ai == "01" { 14 } else { 6 };
                if after.len() < len {
                    return Err(ReceivingRefused::Barcode(
                        "a truncated fixed-length element",
                    ));
                }
                let (value, next) = after.split_at(len);
                if ai == "01" {
                    if !value.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(ReceivingRefused::Barcode("the GTIN (01) is 14 digits"));
                    }
                    gs1.gtin = Some(value.to_owned());
                } else {
                    gs1.expiry = Some(expiry(value)?);
                }
                rest = next;
            }
            "10" | "21" => {
                let end = after.find(GS).unwrap_or(after.len());
                let value = &after[..end];
                if value.is_empty() || value.len() > 20 {
                    return Err(ReceivingRefused::Barcode(
                        "a batch or serial is 1 to 20 characters",
                    ));
                }
                if ai == "10" {
                    gs1.batch = Some(value.to_owned());
                } else {
                    gs1.serial = Some(value.to_owned());
                }
                rest = &after[end..];
            }
            _ => {
                return Err(ReceivingRefused::Barcode(
                    "an AI other than 01, 17, 10 or 21",
                ));
            }
        }
    }
    Ok(gs1)
}

/// Where a requisition is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequisitionStatus {
    /// Asked for.
    Requested,
    /// Approved by the supplying store.
    Approved,
    /// Sent, by a transfer.
    Fulfilled,
    /// Refused.
    Rejected,
}

/// One product asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestedLine {
    /// The product.
    pub product: String,
    /// How many, in base units.
    pub quantity: i64,
}

/// A requisition within the facility (a FHIR SupplyRequest): a ward asking the pharmacy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requisition {
    /// The requisition's id.
    pub id: String,
    /// The store asking (`SupplyRequest.deliverTo`).
    pub requesting_store: String,
    /// The store asked (`SupplyRequest.deliverFrom`).
    pub supplying_store: String,
    /// What is asked for.
    pub lines: Vec<RequestedLine>,
    /// Where it is.
    pub status: RequisitionStatus,
    /// Who asked.
    pub requested_by: String,
    /// Who approved or rejected it.
    pub decided_by: Option<String>,
    /// The transfer that fulfilled it.
    pub transfer: Option<String>,
}

impl Requisition {
    /// A requisition from `requesting_store` to `supplying_store`.
    pub fn new(
        id: &str,
        requesting_store: &str,
        supplying_store: &str,
        lines: Vec<RequestedLine>,
        by: &str,
    ) -> Result<Self, ReceivingRefused> {
        if lines.is_empty()
            || lines
                .iter()
                .any(|l| l.quantity <= 0 || l.product.trim().is_empty())
        {
            return Err(ReceivingRefused::Invalid(
                "a requisition needs products with positive quantities".into(),
            ));
        }
        if requesting_store == supplying_store {
            return Err(ReceivingRefused::Invalid(
                "a requisition is between two different stores".into(),
            ));
        }
        Ok(Self {
            id: id.to_owned(),
            requesting_store: requesting_store.to_owned(),
            supplying_store: supplying_store.to_owned(),
            lines,
            status: RequisitionStatus::Requested,
            requested_by: by.to_owned(),
            decided_by: None,
            transfer: None,
        })
    }

    fn expect(
        &self,
        status: RequisitionStatus,
        action: &'static str,
    ) -> Result<(), ReceivingRefused> {
        if self.status == status {
            Ok(())
        } else {
            Err(ReceivingRefused::WrongState {
                status: self.status,
                action,
            })
        }
    }

    /// The supplying store approves it.
    pub fn approve(&mut self, by: &str) -> Result<(), ReceivingRefused> {
        self.expect(RequisitionStatus::Requested, "approve")?;
        self.decided_by = Some(by.to_owned());
        self.status = RequisitionStatus::Approved;
        Ok(())
    }

    /// The supplying store refuses it.
    pub fn reject(&mut self, by: &str) -> Result<(), ReceivingRefused> {
        self.expect(RequisitionStatus::Requested, "reject")?;
        self.decided_by = Some(by.to_owned());
        self.status = RequisitionStatus::Rejected;
        Ok(())
    }

    /// Fulfilled by `transfer` from the supplying store to the requesting one.
    pub fn fulfil(&mut self, transfer: &str) -> Result<(), ReceivingRefused> {
        self.expect(RequisitionStatus::Approved, "fulfil")?;
        self.transfer = Some(transfer.to_owned());
        self.status = RequisitionStatus::Fulfilled;
        Ok(())
    }
}

/// How a store's min/max levels follow consumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReorderPolicy {
    /// The minimum, in months of average consumption: reorder at or below it.
    pub min_months: Decimal,
    /// The maximum, in months of average consumption: order up to it.
    pub max_months: Decimal,
}

impl Default for ReorderPolicy {
    /// Reorder at one month's stock, up to three months'.
    fn default() -> Self {
        Self {
            min_months: Decimal::ONE,
            max_months: Decimal::from(3),
        }
    }
}

/// A suggested order for one product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The product.
    pub product: String,
    /// Usable stock on hand, in base units.
    pub on_hand: i64,
    /// The average monthly consumption, in base units.
    pub monthly: Decimal,
    /// The minimum level.
    pub min: i64,
    /// The maximum level.
    pub max: i64,
    /// What to order: up to the maximum, rounded up; zero above the minimum.
    pub order: i64,
}

fn ceil(value: Decimal) -> i64 {
    i64::try_from(value.ceil()).unwrap_or(i64::MAX)
}

/// The suggestion for `product` with `on_hand` in stock and `consumed` base units used over
/// `days` days. With no consumption there is nothing to suggest from, so nothing is ordered.
pub fn suggest(
    product: &str,
    on_hand: i64,
    consumed: i64,
    days: u32,
    policy: ReorderPolicy,
) -> Suggestion {
    let monthly = if days == 0 {
        Decimal::ZERO
    } else {
        Decimal::from(consumed.max(0)) * Decimal::from(30) / Decimal::from(days)
    };
    let min = ceil(monthly * policy.min_months);
    let max = ceil(monthly * policy.max_months);
    let order = if monthly > Decimal::ZERO && on_hand <= min {
        (max - on_hand).max(0)
    } else {
        0
    };
    Suggestion {
        product: product.to_owned(),
        on_hand,
        monthly: monthly.round_dp(2),
        min,
        max,
        order,
    }
}
