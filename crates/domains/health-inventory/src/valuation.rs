//! Stock valuation (inventory-and-procurement.md § Valuation; #131): a projection rebuilt from
//! the movement ledger, never stored as truth.
//!
//! - **Weighted average** (the default) or **FIFO** receipt layers, set per store.
//! - Cost is recorded on each receipt; donor-funded stock is received at the donor-declared
//!   price, so it is valued like any other.
//! - **Consignment** stock is not owned: it carries no value.
//! - An increase recorded without a cost (a transfer-in, a return, a positive count) comes in
//!   at the unit's current cost.
//!
//! Every movement gets its cost ([`Costed`]), so inventory-accounting postings follow from the
//! same projection.

use rust_decimal::Decimal;

use crate::{Movement, MovementKind, Ownership, StockUnit, stock_card};

/// How a store values its stock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Method {
    /// The weighted average cost of what is on hand.
    #[default]
    WeightedAverage,
    /// First in, first out: issues take the oldest receipts' costs.
    Fifo,
}

/// A FIFO receipt layer still on hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layer {
    /// How many are left.
    pub quantity: i64,
    /// Their unit cost.
    pub unit_cost: Decimal,
}

/// A movement with its cost: positive for an increase in value, negative for a decrease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Costed {
    /// The movement's id.
    pub movement: String,
    /// Its kind.
    pub kind: MovementKind,
    /// Its signed quantity.
    pub quantity: i64,
    /// Its signed value.
    pub value: Decimal,
}

/// A Stock Unit's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Valuation {
    /// The quantity on hand by the ledger.
    pub quantity: i64,
    /// Its value.
    pub value: Decimal,
    /// The unit cost: the weighted average, or the value per unit of the FIFO layers.
    pub unit_cost: Decimal,
    /// The FIFO layers left, oldest first (empty for weighted average).
    pub layers: Vec<Layer>,
    /// Every movement with its cost, in stock-card order.
    pub costed: Vec<Costed>,
    /// Receipts recorded without a cost while nothing was on hand to cost them by.
    pub uncosted: i64,
}

fn average(value: Decimal, quantity: i64) -> Decimal {
    if quantity > 0 {
        value / Decimal::from(quantity)
    } else {
        Decimal::ZERO
    }
}

/// The value of `unit` from its `movements`, by `method`. The result depends only on the
/// movements, so it is the same however often, and in whatever order, they are given.
pub fn value(unit: &StockUnit, movements: &[Movement], method: Method) -> Valuation {
    let owned = unit.ownership == Ownership::Owned;
    let mut quantity: i64 = 0;
    let mut total = Decimal::ZERO;
    let mut layers: Vec<Layer> = Vec::new();
    let mut costed = Vec::with_capacity(movements.len());
    let mut uncosted = 0;
    for (movement, _) in stock_card(movements) {
        let q = movement.quantity;
        let current = match method {
            Method::WeightedAverage => average(total, quantity),
            Method::Fifo => layers
                .last()
                .map_or(average(total, quantity), |l| l.unit_cost),
        };
        let change = if !owned {
            Decimal::ZERO
        } else if q > 0 {
            let cost = match movement.unit_cost {
                Some(cost) => cost,
                None => {
                    if quantity <= 0 && movement.kind == MovementKind::Receipt {
                        uncosted += q;
                    }
                    current
                }
            };
            if method == Method::Fifo {
                layers.push(Layer {
                    quantity: q,
                    unit_cost: cost,
                });
            }
            cost * Decimal::from(q)
        } else {
            let mut out = -q;
            match method {
                Method::WeightedAverage => -(average(total, quantity) * Decimal::from(out)),
                Method::Fifo => {
                    let mut cost = Decimal::ZERO;
                    let mut last = current;
                    while out > 0 {
                        let Some(layer) = layers.first_mut() else {
                            // Beyond what was received: at the last cost.
                            cost += last * Decimal::from(out);
                            break;
                        };
                        let take = out.min(layer.quantity);
                        cost += layer.unit_cost * Decimal::from(take);
                        last = layer.unit_cost;
                        layer.quantity -= take;
                        out -= take;
                        if layer.quantity == 0 {
                            layers.remove(0);
                        }
                    }
                    -cost
                }
            }
        };
        quantity += q;
        total += change;
        if quantity <= 0 {
            // Nothing (or less than nothing) on hand has no value left.
            total = Decimal::ZERO;
        }
        costed.push(Costed {
            movement: movement.id.clone(),
            kind: movement.kind,
            quantity: q,
            value: change,
        });
    }
    Valuation {
        quantity,
        value: total,
        unit_cost: average(total, quantity),
        layers,
        costed,
        uncosted,
    }
}
