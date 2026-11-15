//! Equipment (ADR 0064; GLOSSARY.md): a physical, individually identified item a facility holds
//! and uses again and again without using it up, such as a BP machine, an analyser or a fridge.
//! Anything used up is Stock instead.
//!
//! - **[`Equipment`]**: one record per unit, named by its Product and serial number or asset tag,
//!   at one facility, in one location, with its [`Status`].
//! - **[`Equipment::received`]**: the record a goods receipt creates for one unit of a Product
//!   tracked as Equipment, in the receipt's transaction. No stock balance is posted for it.
//! - **[`identity`]**: a unit's id follows from its Product and serial, so the same unit cannot
//!   be registered twice.
//! - **[`Equipment::set_status`]** and **[`Equipment::move_to`]**: the facility's changes, each
//!   kept as the unit's latest [`Change`]. Its maintenance history is the record's versions.
//!   A retired unit changes no more.
//! - **Transfers between facilities** (ADR 0064): [`Equipment::dispatch`] sends a unit to
//!   another facility, and it changes no more where it was. Central, the authority for the
//!   hand-over, then gives the record to the receiving facility ([`Equipment::hand_over`]), which
//!   [`Equipment::receive`]s it into one of its locations. Until then the unit is in transit.
//! - **Valuation** (#453): a unit keeps its [`Acquisition`] cost from the goods receipt, and its
//!   book value ([`Equipment::book_value`]) is that cost less straight-line depreciation over its
//!   Product's useful life: a projection, never stored. A retired unit is worth nothing.

use health_time::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The namespace of Equipment ids (UUIDv5 of `product|serial`).
const NAMESPACE: Uuid = Uuid::from_u128(0x0193_6a2e_4f1c_7d55_9b0e_3c8a_e4d1_2f60);

/// The longest serial number or asset tag accepted.
pub const SERIAL_MAX: usize = 64;

/// What state a unit is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Working and ready to use.
    Available,
    /// Broken, waiting for repair.
    Faulty,
    /// Being serviced or repaired.
    UnderMaintenance,
    /// Taken out of service for good.
    Retired,
}

impl Status {
    /// Its code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Status::Available => "available",
            Status::Faulty => "faulty",
            Status::UnderMaintenance => "under-maintenance",
            Status::Retired => "retired",
        }
    }

    /// The status a code names.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        [
            Status::Available,
            Status::Faulty,
            Status::UnderMaintenance,
            Status::Retired,
        ]
        .into_iter()
        .find(|s| s.code() == code)
    }
}

/// One unit of Equipment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Equipment {
    /// Its id ([`identity`]).
    pub id: Uuid,
    /// The Product it is, by code.
    pub product: String,
    /// What a person reads: the Product's name.
    pub name: String,
    /// Its serial number or asset tag.
    pub serial: String,
    /// The facility that holds it, and the only writer of its record.
    pub facility: String,
    /// Where it is, e.g. `Location/pharmacy`.
    pub location: String,
    /// Its state.
    pub status: Status,
    /// The goods received note it arrived on, if it arrived on one.
    pub received: Option<String>,
    /// The latest change, if it was changed since it was registered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<Change>,
    /// The transfer it is on, while it is in transit to another facility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<Transfer>,
    /// What it cost and when it was acquired, if it arrived on a costed receipt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquired: Option<Acquisition>,
}

/// A unit's acquisition: its cost (in the facility's currency) and when it was received.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acquisition {
    /// What one unit cost.
    pub cost: Decimal,
    /// When it was received, from when it depreciates.
    pub at: Timestamp,
}

/// Seconds in a year of 365.25 days, the depreciation year.
const YEAR: i64 = 31_557_600;

/// A unit's transfer to another facility: dispatched by `from`, handed over by central to `to`,
/// and over once `to` receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transfer {
    /// The facility that dispatched it.
    pub from: String,
    /// The facility it is going to.
    pub to: String,
    /// Who dispatched it.
    pub by: String,
    /// When.
    pub at: Timestamp,
}

/// One change to a unit: what was done or found, by whom and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// What was done or found, e.g. "cuff replaced" or "screen cracked".
    pub note: String,
    /// Who made the change.
    pub by: String,
    /// When.
    pub at: Timestamp,
}

/// Why a unit is not valid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EquipmentRefused {
    /// A required field is empty.
    #[error("equipment needs its {0}")]
    Missing(&'static str),
    /// The serial number is too long or has control characters.
    #[error("a serial number or asset tag is at most {SERIAL_MAX} printable characters")]
    Serial,
    /// A retired unit changes no more.
    #[error("{0} is retired")]
    Retired(String),
    /// The change changes nothing.
    #[error("{0} is already {1}")]
    Unchanged(String, String),
    /// Taking a unit out of use needs a note of why.
    #[error("say why {0} is {1}")]
    NoteRequired(String, String),
    /// A unit in transit changes no more until the receiving facility receives it.
    #[error("{0} is in transit to {1}")]
    InTransit(String, String),
    /// Only a unit handed over to the facility is received by it.
    #[error("{0} is not in transit to this facility")]
    NotInTransit(String),
    /// A unit is dispatched to another facility.
    #[error("{0} is already at {1}")]
    SameFacility(String, String),
}

/// The id of the unit of `product` with `serial`: a UUIDv5, so it is the same at every node.
#[must_use]
pub fn identity(product: &str, serial: &str) -> Uuid {
    Uuid::new_v5(&NAMESPACE, format!("{product}|{serial}").as_bytes())
}

/// Whether `serial` is a usable serial number or asset tag.
#[must_use]
pub fn valid_serial(serial: &str) -> bool {
    !serial.is_empty()
        && serial.len() <= SERIAL_MAX
        && serial == serial.trim()
        && !serial.chars().any(char::is_control)
}

impl Equipment {
    /// The record for one unit of `product` (named `name`) with `serial`, received on `note`
    /// into `location` at `facility`: available.
    pub fn received(
        product: &str,
        name: &str,
        serial: &str,
        facility: &str,
        location: &str,
        note: &str,
    ) -> Result<Self, EquipmentRefused> {
        let unit = Self {
            id: identity(product, serial),
            product: product.to_owned(),
            name: name.to_owned(),
            serial: serial.to_owned(),
            facility: facility.to_owned(),
            location: location.to_owned(),
            status: Status::Available,
            received: Some(note.to_owned()),
            change: None,
            transfer: None,
            acquired: None,
        };
        unit.validate()?;
        Ok(unit)
    }

    /// The unit, acquired at `cost` on `at`.
    #[must_use]
    pub fn with_acquisition(mut self, cost: Decimal, at: Timestamp) -> Self {
        self.acquired = Some(Acquisition { cost, at });
        self
    }

    /// Its book value on `on`: its cost less straight-line depreciation over `life_years`, never
    /// below zero; its cost when its Product has no useful life set; nothing once retired.
    /// `None` when its cost is not known.
    #[must_use]
    pub fn book_value(&self, life_years: Option<u32>, on: Timestamp) -> Option<Decimal> {
        let acquired = self.acquired?;
        if self.status == Status::Retired {
            return Some(Decimal::ZERO);
        }
        let Some(life) = life_years.filter(|years| *years > 0) else {
            return Some(acquired.cost);
        };
        let elapsed = (on.as_second() - acquired.at.as_second()).max(0);
        let life = i64::from(life) * YEAR;
        if elapsed >= life {
            return Some(Decimal::ZERO);
        }
        let remaining = Decimal::from(life - elapsed) / Decimal::from(life);
        Some((acquired.cost * remaining).round_dp(2))
    }

    /// Sets the unit's status, with a note of what was done or found. A note is needed for any
    /// status but available; a retired unit changes no more.
    pub fn set_status(
        &mut self,
        status: Status,
        note: &str,
        by: &str,
        at: Timestamp,
    ) -> Result<(), EquipmentRefused> {
        self.changeable()?;
        if status == self.status {
            return Err(EquipmentRefused::Unchanged(
                self.serial.clone(),
                status.code().to_owned(),
            ));
        }
        let note = note.trim();
        if note.is_empty() && status != Status::Available {
            return Err(EquipmentRefused::NoteRequired(
                self.serial.clone(),
                status.code().to_owned(),
            ));
        }
        self.status = status;
        self.change = Some(Change {
            note: if note.is_empty() {
                format!("now {}", status.code())
            } else {
                note.to_owned()
            },
            by: by.to_owned(),
            at,
        });
        Ok(())
    }

    /// Moves the unit to `location` within its facility.
    pub fn move_to(
        &mut self,
        location: &str,
        by: &str,
        at: Timestamp,
    ) -> Result<(), EquipmentRefused> {
        self.changeable()?;
        let location = location.trim();
        if location.is_empty() {
            return Err(EquipmentRefused::Missing("location"));
        }
        if location == self.location {
            return Err(EquipmentRefused::Unchanged(
                self.serial.clone(),
                format!("at {location}"),
            ));
        }
        self.change = Some(Change {
            note: format!("moved from {} to {location}", self.location),
            by: by.to_owned(),
            at,
        });
        location.clone_into(&mut self.location);
        Ok(())
    }

    /// Dispatches the unit to facility `to`. It changes no more here: central hands it over to
    /// `to` ([`Equipment::hand_over`]), which receives it.
    pub fn dispatch(&mut self, to: &str, by: &str, at: Timestamp) -> Result<(), EquipmentRefused> {
        self.changeable()?;
        let to = to.trim();
        if to.is_empty() {
            return Err(EquipmentRefused::Missing("facility it goes to"));
        }
        if to == self.facility {
            return Err(EquipmentRefused::SameFacility(
                self.serial.clone(),
                to.to_owned(),
            ));
        }
        self.change = Some(Change {
            note: format!("dispatched to {to}"),
            by: by.to_owned(),
            at,
        });
        self.transfer = Some(Transfer {
            from: self.facility.clone(),
            to: to.to_owned(),
            by: by.to_owned(),
            at,
        });
        Ok(())
    }

    /// Gives a dispatched unit's record to the facility it was dispatched to: central's step of a
    /// transfer, once the dispatch has reached it. `false` when there is nothing to hand over.
    pub fn hand_over(&mut self) -> bool {
        match &self.transfer {
            Some(transfer) if transfer.to != self.facility => {
                self.facility.clone_from(&transfer.to);
                true
            }
            _ => false,
        }
    }

    /// Receives a unit handed over to its facility into `location`, ending its transfer.
    pub fn receive(
        &mut self,
        location: &str,
        by: &str,
        at: Timestamp,
    ) -> Result<(), EquipmentRefused> {
        let Some(transfer) = self
            .transfer
            .take_if(|transfer| transfer.to == self.facility)
        else {
            return Err(EquipmentRefused::NotInTransit(self.serial.clone()));
        };
        let location = location.trim();
        if location.is_empty() {
            self.transfer = Some(transfer);
            return Err(EquipmentRefused::Missing("location"));
        }
        self.change = Some(Change {
            note: format!("received from {} into {location}", transfer.from),
            by: by.to_owned(),
            at,
        });
        location.clone_into(&mut self.location);
        Ok(())
    }

    /// Whether the unit is on a transfer to another facility.
    #[must_use]
    pub fn in_transit(&self) -> bool {
        self.transfer.is_some()
    }

    fn changeable(&self) -> Result<(), EquipmentRefused> {
        if self.status == Status::Retired {
            return Err(EquipmentRefused::Retired(self.serial.clone()));
        }
        if let Some(transfer) = &self.transfer {
            return Err(EquipmentRefused::InTransit(
                self.serial.clone(),
                transfer.to.clone(),
            ));
        }
        Ok(())
    }

    /// Checks the record is complete.
    pub fn validate(&self) -> Result<(), EquipmentRefused> {
        for (value, field) in [
            (&self.product, "product"),
            (&self.name, "name"),
            (&self.facility, "facility"),
            (&self.location, "location"),
        ] {
            if value.trim().is_empty() {
                return Err(EquipmentRefused::Missing(field));
            }
        }
        if !valid_serial(&self.serial) {
            return Err(EquipmentRefused::Serial);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unit_s_id_follows_from_its_product_and_serial() {
        let a = identity("bp-machine", "BP-0042");
        assert_eq!(a, identity("bp-machine", "BP-0042"));
        assert_ne!(a, identity("bp-machine", "BP-0043"));
        assert_ne!(a, identity("analyser", "BP-0042"));
        assert_eq!(a.get_version_num(), 5);
    }

    #[test]
    fn a_received_unit_is_available_and_complete() {
        let unit = Equipment::received(
            "bp-machine",
            "BP machine",
            "BP-0042",
            "KLA01",
            "Location/opd",
            "grn-1",
        )
        .expect("valid");
        assert_eq!(unit.status, Status::Available);
        assert_eq!(unit.received.as_deref(), Some("grn-1"));
        assert_eq!(
            Equipment::received(
                "bp-machine",
                "BP machine",
                " ",
                "KLA01",
                "Location/opd",
                "g"
            ),
            Err(EquipmentRefused::Serial)
        );
        assert_eq!(
            Equipment::received("bp-machine", "BP machine", "S1", "", "Location/opd", "g"),
            Err(EquipmentRefused::Missing("facility"))
        );
        assert!(!valid_serial(&"x".repeat(SERIAL_MAX + 1)));
        assert!(!valid_serial("A\tB"));
    }

    fn at(second: i64) -> Timestamp {
        Timestamp::from_second(1_800_000_000 + second).expect("time")
    }

    #[test]
    fn a_unit_s_status_changes_with_a_note_until_it_is_retired() {
        let mut unit = Equipment::received(
            "bp-machine",
            "BP machine",
            "BP-1",
            "KLA01",
            "Location/opd",
            "g",
        )
        .expect("valid");
        assert_eq!(
            unit.set_status(Status::Faulty, " ", "nurse-1", at(0)),
            Err(EquipmentRefused::NoteRequired(
                "BP-1".into(),
                "faulty".into()
            ))
        );
        unit.set_status(Status::Faulty, "cuff leaks", "nurse-1", at(0))
            .expect("faulty");
        assert_eq!(
            unit.change,
            Some(Change {
                note: "cuff leaks".into(),
                by: "nurse-1".into(),
                at: at(0)
            })
        );
        assert_eq!(
            unit.set_status(Status::Faulty, "still", "nurse-1", at(1)),
            Err(EquipmentRefused::Unchanged("BP-1".into(), "faulty".into()))
        );
        unit.set_status(Status::Available, "", "tech-1", at(2))
            .expect("repaired");
        assert_eq!(
            unit.change.as_ref().map(|c| c.note.as_str()),
            Some("now available")
        );
        unit.move_to("Location/ward-2", "nurse-1", at(3))
            .expect("moved");
        assert_eq!(unit.location, "Location/ward-2");
        assert_eq!(
            unit.move_to("Location/ward-2", "nurse-1", at(4)),
            Err(EquipmentRefused::Unchanged(
                "BP-1".into(),
                "at Location/ward-2".into()
            ))
        );
        unit.set_status(Status::Retired, "beyond repair", "tech-1", at(5))
            .expect("retired");
        assert_eq!(
            unit.set_status(Status::Available, "", "tech-1", at(6)),
            Err(EquipmentRefused::Retired("BP-1".into()))
        );
        assert_eq!(
            unit.move_to("Location/store", "tech-1", at(6)),
            Err(EquipmentRefused::Retired("BP-1".into()))
        );
    }

    #[test]
    fn a_dispatched_unit_is_handed_over_and_changes_only_once_received() {
        let mut unit = Equipment::received(
            "bp-machine",
            "BP machine",
            "BP-1",
            "KLA01",
            "Location/opd",
            "g",
        )
        .expect("valid");
        assert_eq!(
            unit.dispatch("KLA01", "store-1", at(0)),
            Err(EquipmentRefused::SameFacility(
                "BP-1".into(),
                "KLA01".into()
            ))
        );
        // Not received where it is not in transit to.
        assert_eq!(
            unit.receive("Location/ward", "store-1", at(0)),
            Err(EquipmentRefused::NotInTransit("BP-1".into()))
        );
        unit.dispatch(" GUL02 ", "store-1", at(1))
            .expect("dispatched");
        assert!(unit.in_transit());
        assert_eq!(
            unit.change.as_ref().map(|c| c.note.as_str()),
            Some("dispatched to GUL02")
        );
        // In transit it changes no more, and the sender does not receive it.
        let in_transit = Err(EquipmentRefused::InTransit("BP-1".into(), "GUL02".into()));
        assert_eq!(
            unit.set_status(Status::Faulty, "dropped", "store-1", at(2)),
            in_transit
        );
        assert_eq!(unit.move_to("Location/ward", "store-1", at(2)), in_transit);
        assert_eq!(unit.dispatch("MBR03", "store-1", at(2)), in_transit);
        assert_eq!(
            unit.receive("Location/ward", "store-1", at(2)),
            Err(EquipmentRefused::NotInTransit("BP-1".into()))
        );
        // Central hands it over once.
        assert!(unit.hand_over());
        assert_eq!(unit.facility, "GUL02");
        assert!(!unit.hand_over());
        assert_eq!(
            unit.receive(" ", "store-2", at(3)),
            Err(EquipmentRefused::Missing("location"))
        );
        unit.receive("Location/lab", "store-2", at(3))
            .expect("received");
        assert!(!unit.in_transit());
        assert_eq!(unit.location, "Location/lab");
        assert_eq!(
            unit.change.as_ref().map(|c| c.note.as_str()),
            Some("received from KLA01 into Location/lab")
        );
        unit.set_status(Status::Faulty, "dropped", "store-2", at(4))
            .expect("its own now");
    }

    #[test]
    fn a_unit_depreciates_straight_line_and_is_worth_nothing_retired() {
        let unit = Equipment::received(
            "bp-machine",
            "BP machine",
            "BP-1",
            "KLA01",
            "Location/opd",
            "g",
        )
        .expect("valid");
        assert_eq!(unit.book_value(Some(5), at(0)), None, "no cost known");
        let unit = unit.with_acquisition(Decimal::new(1_000_000, 0), at(0));
        assert_eq!(
            unit.book_value(None, at(10 * YEAR)),
            Some(Decimal::new(1_000_000, 0)),
            "no useful life: at cost"
        );
        assert_eq!(
            unit.book_value(Some(4), at(0)),
            Some(Decimal::new(1_000_000, 0))
        );
        assert_eq!(
            unit.book_value(Some(4), at(YEAR)),
            Some(Decimal::new(750_000, 0))
        );
        assert_eq!(
            unit.book_value(Some(4), at(-YEAR)),
            Some(Decimal::new(1_000_000, 0)),
            "not before it was acquired"
        );
        assert_eq!(unit.book_value(Some(4), at(5 * YEAR)), Some(Decimal::ZERO));
        let mut retired = unit;
        retired
            .set_status(Status::Retired, "beyond repair", "tech-1", at(1))
            .expect("retired");
        assert_eq!(retired.book_value(Some(4), at(2)), Some(Decimal::ZERO));
    }

    #[test]
    fn statuses_round_trip_by_code() {
        for status in [
            Status::Available,
            Status::Faulty,
            Status::UnderMaintenance,
            Status::Retired,
        ] {
            assert_eq!(Status::from_code(status.code()), Some(status));
        }
        assert_eq!(Status::from_code("lost"), None);
    }
}
