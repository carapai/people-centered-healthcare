//! Dispensing at the store's authority (inventory-and-procurement.md § Dispensing; ADR 0035;
//! #128). **Stock moves only when a dispense is completed.**
//!
//! - **[`Inventory::prepare`]** reserves stock for a MedicationDispense being prepared: the
//!   batches by first expiry (or the pharmacist's choice, with a reason), only those eligible for
//!   the patient and in date for the course. It writes each reservation and its unit's balance
//!   record in one transaction, so two preparations of the last packs conflict, and the rerun sees
//!   the first's reservation. Preparing the same dispense again returns its live reservations.
//! - **[`Inventory::complete_dispense`]** consumes the reservations: each amount given is a
//!   dispense movement, decided by the store's policy and drawn from the dispense's own
//!   reservation, with what was not given released. The same transaction writes the
//!   `inventory.dispensed.v1` event, the charge trigger. A lapsed reservation must be prepared
//!   again.
//! - **[`Inventory::cancel_dispense`]** releases the dispense's reservations. Nothing moves.
//! - **[`Inventory::return_to_quarantine`]** receives a patient's return into quarantine.
//!
//! Reservations count only while live: every balance written takes its `reserved` from the live
//! reservations, so a lapsed one frees its stock at the next write, and `available` reads it
//! the same way.

use std::collections::BTreeMap;

use health_identifiers::{OpId, ResourceId};
use health_inventory::dispensing::{
    BatchRule, Candidate, Eligibility, Need, Pick, Purpose, Reservation, ReservationState, chosen,
    fefo, reserved, returned, settle,
};
use health_inventory::{Balance, Movement, MovementKind, Origin, StockStore, StockUnit};
use health_persistence::{
    OutboxEntry, OutboxPayload, Precondition, RawJson, ResourceType, Store, Transaction,
    WriteError, WriteRequest,
};
use health_time::Timestamp;
use serde_json::json;

use crate::{ATTEMPTS, Attempt, Inventory, InventoryError};

/// The name of the event a completed dispense writes: billing's charge trigger.
pub const DISPENSED_EVENT: &str = "inventory.dispensed.v1";

/// A FHIR resource a dispensing step writes in its own transaction: the MedicationDispense, so
/// it and the stock always agree (ADR 0035).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Its type.
    pub resource_type: ResourceType,
    /// Its id.
    pub id: ResourceId,
    /// Its validated JSON.
    pub json: RawJson,
}

/// Writes `record` over its current head, read in this transaction: a create when there is none,
/// else an update `If-Match` that head, so a concurrent change conflicts and the step reruns.
async fn write_record<T: Transaction>(tx: &mut T, record: &Record) -> Result<(), Attempt> {
    let head = tx.read(&record.resource_type, record.id).await?;
    let precondition = match head {
        Some(head) if !head.version.deleted => Precondition::IfMatch(head.version.version_id),
        _ => Precondition::MustNotExist,
    };
    tx.write(WriteRequest {
        resource_type: record.resource_type.clone(),
        id: record.id,
        json: record.json.clone(),
        op_id: OpId::new(),
        precondition,
    })
    .await
    .map(|_| ())
    .map_err(|error| match error {
        WriteError::PreconditionFailed { .. } => Attempt::Conflict,
        other => Attempt::from(other),
    })
}

/// A dispense to prepare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preparation {
    /// The MedicationDispense, `MedicationDispense/{id}`.
    pub dispense: String,
    /// The store dispensed from.
    pub store: String,
    /// What is needed, for whom, until when.
    pub need: Need,
    /// The pharmacist's own batches and why, instead of first expiry.
    pub choice: Option<(Vec<Pick>, String)>,
    /// When.
    pub at: Timestamp,
    /// The MedicationDispense being prepared, written with the reservations.
    pub record: Option<Record>,
}

/// A dispense being completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// The MedicationDispense, `MedicationDispense/{id}`.
    pub dispense: String,
    /// The patient, for the charge.
    pub patient: String,
    /// What was given from each reservation (by id). A partial dispense gives less.
    pub given: BTreeMap<String, i64>,
    /// Whether the actor holds the emergency-dispense permission.
    pub may_emergency_dispense: bool,
    /// Who.
    pub actor: String,
    /// This node.
    pub node: String,
    /// When.
    pub at: Timestamp,
    /// The completed MedicationDispense, written with the movements.
    pub record: Option<Record>,
}

/// A patient's return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatientReturn {
    /// The return's id.
    pub id: String,
    /// The unit it was dispensed from.
    pub unit: StockUnit,
    /// How much came back, in base units.
    pub quantity: i64,
    /// The dispense it came from, `MedicationDispense/{id}`.
    pub dispense: String,
    /// Who received it.
    pub actor: String,
    /// This node.
    pub node: String,
    /// When.
    pub at: Timestamp,
}

/// The balance of `key` with its live reservations at `at` as `reserved`, leaving out `except`.
pub(crate) async fn live_balance<T: StockStore>(
    tx: &mut T,
    key: &str,
    at: Timestamp,
    except: Option<&str>,
) -> Result<(Balance, Vec<Reservation>), Attempt> {
    let balance = tx.stock_balance(key).await?.unwrap_or_default();
    let held = tx.held_reservations(key).await?;
    let counted = reserved(held.iter().filter(|r| Some(r.id.as_str()) != except), at);
    Ok((
        Balance {
            reserved: counted,
            ..balance
        },
        held,
    ))
}

impl<S> Inventory<S>
where
    S: Store,
    S::Tx: StockStore,
{
    /// What is available of the unit with `key` at `at`, counting only live reservations.
    pub async fn available(&self, key: &str, at: Timestamp) -> Result<i64, InventoryError> {
        let mut tx = self.store.begin().await?;
        match live_balance(&mut tx, key, at, None).await {
            Ok((balance, _)) => Ok(balance.available()),
            Err(Attempt::Fail(error)) => Err(error),
            Err(Attempt::Conflict) => Err(InventoryError::Busy),
        }
    }

    /// The reservations made for `dispense` (`MedicationDispense/{id}`), in any state.
    pub async fn reservations(&self, dispense: &str) -> Result<Vec<Reservation>, InventoryError> {
        let mut tx = self.store.begin().await?;
        Ok(tx.reservations_for(dispense).await?)
    }

    /// Reserves stock for `preparation`, by first expiry with `eligibility` and `rule`.
    pub async fn prepare(
        &self,
        preparation: &Preparation,
        eligibility: &Eligibility,
        rule: BatchRule,
    ) -> Result<Vec<Reservation>, InventoryError> {
        let _turn = crate::queue::stores([preparation.store.as_str()]).await;
        for attempt in 0..ATTEMPTS {
            crate::pause(attempt).await;
            let mut tx = self.store.begin().await?;
            let attempt = Self::reserve(&mut tx, preparation, eligibility, rule).await;
            if let Some(done) = Self::settle(tx, attempt).await {
                return done;
            }
        }
        Err(InventoryError::Busy)
    }

    async fn reserve(
        tx: &mut S::Tx,
        preparation: &Preparation,
        eligibility: &Eligibility,
        rule: BatchRule,
    ) -> Result<Vec<Reservation>, Attempt> {
        let at = preparation.at;
        let already: Vec<Reservation> = tx
            .reservations_for(&preparation.dispense)
            .await?
            .into_iter()
            .filter(|r| r.live(at))
            .collect();
        if !already.is_empty() {
            return Ok(already);
        }
        let mut candidates = Vec::new();
        for unit in tx.units_of_store(&preparation.store).await? {
            if unit.product != preparation.need.product {
                continue;
            }
            let (balance, _) = live_balance(tx, &unit.key(), at, None).await?;
            candidates.push(Candidate {
                unit,
                available: balance.available(),
            });
        }
        let picks = match &preparation.choice {
            Some((choice, reason)) => chosen(
                &candidates,
                choice,
                &preparation.need,
                eligibility,
                rule,
                Some(reason),
            ),
            None => fefo(&candidates, &preparation.need, eligibility, rule),
        }
        .map_err(|e| Attempt::Fail(e.into()))?;
        let mut made = Vec::new();
        for (index, pick) in picks.iter().enumerate() {
            let reservation = Reservation::hold(
                &format!("{}-{index}", preparation.dispense.replace('/', "-")),
                &pick.unit,
                pick.quantity,
                Purpose::Dispense,
                &preparation.dispense,
                at,
            );
            let (balance, _) = live_balance(tx, &pick.unit.key(), at, None).await?;
            tx.put_reservation(&reservation).await?;
            // The balance record is the guard: a concurrent preparation of the same unit conflicts.
            tx.put_stock_balance(
                &pick.unit,
                &Balance {
                    reserved: balance.reserved + pick.quantity,
                    ..balance
                },
            )
            .await?;
            made.push(reservation);
        }
        if let Some(record) = &preparation.record {
            write_record(tx, record).await?;
        }
        Ok(made)
    }

    /// Completes a dispense: the movements posted, in reservation order.
    pub async fn complete_dispense(
        &self,
        completion: &Completion,
    ) -> Result<Vec<Movement>, InventoryError> {
        // Its stores are its reservations', read inside the transaction.
        let _turn = crate::queue::node().await;
        for attempt in 0..ATTEMPTS {
            crate::pause(attempt).await;
            let mut tx = self.store.begin().await?;
            let attempt = self.consume(&mut tx, completion).await;
            if let Some(done) = Self::settle(tx, attempt).await {
                return done;
            }
        }
        Err(InventoryError::Busy)
    }

    async fn consume(
        &self,
        tx: &mut S::Tx,
        completion: &Completion,
    ) -> Result<Vec<Movement>, Attempt> {
        let at = completion.at;
        let held: Vec<Reservation> = tx
            .reservations_for(&completion.dispense)
            .await?
            .into_iter()
            .filter(|r| r.state == ReservationState::Held)
            .collect();
        if held.is_empty() {
            return Err(Attempt::Fail(InventoryError::NotFound(format!(
                "a prepared {}",
                completion.dispense
            ))));
        }
        if let Some(lapsed) = held.iter().find(|r| !r.live(at)) {
            return Err(Attempt::Fail(InventoryError::Lapsed(lapsed.id.clone())));
        }
        let settled = settle(&held, &completion.given).map_err(|e| Attempt::Fail(e.into()))?;
        let mut movements = Vec::new();
        for (index, done) in settled.iter().enumerate() {
            let unit = &done.reservation.unit;
            let key = unit.key();
            // The dispense draws from its own reservation: it no longer counts as reserved.
            let (balance, _) = live_balance(tx, &key, at, Some(&done.reservation.id)).await?;
            tx.put_reservation(&done.reservation).await?;
            if done.given == 0 {
                tx.put_stock_balance(unit, &balance).await?;
                continue;
            }
            let id = format!("{}-dispense-{index}", completion.dispense.replace('/', "-"));
            let movement = Movement {
                op_id: id.clone(),
                id,
                unit: unit.clone(),
                kind: MovementKind::Dispense,
                quantity: -done.given,
                pack_size: 1,
                unit_cost: None,
                occurred_at: at,
                recorded_at: at,
                source_document: completion.dispense.clone(),
                actor: completion.actor.clone(),
                node: completion.node.clone(),
                reason: None,
                reverses: None,
                emergency: false,
            };
            let decision = self.policy.decide(
                balance,
                &movement,
                Origin::Authority {
                    may_emergency_dispense: completion.may_emergency_dispense,
                },
            )?;
            let recorded = Movement {
                emergency: decision.emergency,
                ..movement
            };
            tx.append_movement(&recorded).await?;
            tx.put_stock_balance(unit, &decision.balance).await?;
            movements.push(recorded);
        }
        let given: Vec<_> = movements
            .iter()
            .map(|m| json!({ "unit": m.unit.key(), "product": m.unit.product, "quantity": -m.quantity }))
            .collect();
        let event = json!({
            "dispense": completion.dispense, "patient": completion.patient,
            "given": given, "at": at.to_string()
        });
        tx.append_outbox(OutboxEntry {
            op_id: OpId::new(),
            payload: OutboxPayload::DomainEvent {
                name: DISPENSED_EVENT.to_owned(),
                body: RawJson::from_validated(event.to_string()),
            },
        })
        .await
        .map_err(Attempt::from)?;
        if let Some(record) = &completion.record {
            write_record(tx, record).await?;
        }
        Ok(movements)
    }

    /// Releases the reservations of a dispense that will not be completed. Nothing moves.
    /// `record` is the cancelled MedicationDispense, written in the same transaction.
    pub async fn cancel_dispense(
        &self,
        dispense: &str,
        at: Timestamp,
        record: Option<&Record>,
    ) -> Result<usize, InventoryError> {
        let _turn = crate::queue::node().await;
        for attempt in 0..ATTEMPTS {
            crate::pause(attempt).await;
            let mut tx = self.store.begin().await?;
            let attempt = Self::release(&mut tx, dispense, at, record).await;
            if let Some(done) = Self::settle(tx, attempt).await {
                return done;
            }
        }
        Err(InventoryError::Busy)
    }

    async fn release(
        tx: &mut S::Tx,
        dispense: &str,
        at: Timestamp,
        record: Option<&Record>,
    ) -> Result<usize, Attempt> {
        let held: Vec<Reservation> = tx
            .reservations_for(dispense)
            .await?
            .into_iter()
            .filter(|r| r.state == ReservationState::Held)
            .collect();
        for reservation in &held {
            let (balance, _) =
                live_balance(tx, &reservation.unit.key(), at, Some(&reservation.id)).await?;
            tx.put_reservation(&Reservation {
                state: ReservationState::Released,
                ..reservation.clone()
            })
            .await?;
            tx.put_stock_balance(&reservation.unit, &balance).await?;
        }
        if let Some(record) = record {
            write_record(tx, record).await?;
        }
        Ok(held.len())
    }

    /// Receives a patient's return into quarantine, as a return movement.
    pub async fn return_to_quarantine(
        &self,
        back: &PatientReturn,
    ) -> Result<Movement, InventoryError> {
        let movement = Movement {
            id: back.id.clone(),
            op_id: back.id.clone(),
            unit: returned(&back.unit),
            kind: MovementKind::Return,
            quantity: back.quantity,
            pack_size: 1,
            unit_cost: None,
            occurred_at: back.at,
            recorded_at: back.at,
            source_document: back.dispense.clone(),
            actor: back.actor.clone(),
            node: back.node.clone(),
            reason: None,
            reverses: None,
            emergency: false,
        };
        self.record(
            &movement,
            Origin::Authority {
                may_emergency_dispense: false,
            },
        )
        .await?;
        Ok(movement)
    }
}
