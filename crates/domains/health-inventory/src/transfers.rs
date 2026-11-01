//! Transfers (inventory-and-procurement.md § Transfers): two steps, a transfer-out at the source
//! and a transfer-in with counted quantities at the destination, with the stock in transit in
//! between.
//!
//! ```text
//! Picked → InTransit → Received
//!                    ↘ Reconciling (short / damaged / excess) → Closed
//! ```
//!
//! Within a facility, a one-step direct transfer is allowed only when one user both sends and
//! receives, and it is marked. Between facilities each side records its own movement at its own
//! Authority Node, and central matches the dispatch with the receipt ([`discrepancies`]).

use serde::{Deserialize, Serialize};

use crate::reconciliation::Discrepancy;
use crate::{Ownership, Status, StockUnit};

/// One product batch sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferLine {
    /// The product.
    pub product: String,
    /// The batch.
    pub batch: String,
    /// The expiry.
    pub expiry: String,
    /// Its funding source.
    pub funding_source: String,
    /// Owned or consignment.
    pub ownership: Ownership,
    /// How many, in base units.
    pub quantity: i64,
}

impl TransferLine {
    fn unit(&self, store: &str, status: Status) -> StockUnit {
        StockUnit {
            store: store.to_owned(),
            product: self.product.clone(),
            batch: self.batch.clone(),
            expiry: self.expiry.clone(),
            funding_source: self.funding_source.clone(),
            ownership: self.ownership,
            status,
        }
    }
}

/// What the destination counted for one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counted {
    /// The line, by its index in [`Transfer::lines`].
    pub line: usize,
    /// Arrived usable.
    pub good: i64,
    /// Arrived damaged; received into quarantine.
    pub damaged: i64,
}

/// Where a transfer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransferStatus {
    /// Picked at the source; nothing has moved.
    Picked,
    /// Sent: the stock left the source and is in transit.
    InTransit,
    /// Received as sent.
    Received,
    /// Received with discrepancies, each a Reconciliation Record.
    Reconciling,
    /// Its Reconciliation Records are resolved.
    Closed,
}

/// Why a transfer step was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransferRefused {
    /// A transfer needs lines with positive quantities, between two different stores.
    #[error("{0}")]
    Invalid(&'static str),
    /// The step does not apply in the transfer's status.
    #[error("cannot {action} a transfer that is {status:?}")]
    WrongState {
        /// The transfer's status.
        status: TransferStatus,
        /// The step.
        action: &'static str,
    },
    /// A direct transfer is within one facility, by the one user who sends and receives.
    #[error("a direct transfer is within one facility only")]
    NotDirect,
    /// The counts do not cover each line once, or are negative.
    #[error("count each line once, with no negative quantity")]
    BadCount,
}

/// A movement a transfer step makes: a signed quantity of a Stock Unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leg {
    /// The Stock Unit.
    pub unit: StockUnit,
    /// The signed quantity: negative out of the source, positive into the destination.
    pub quantity: i64,
}

/// A discrepancy found on receipt, for a Reconciliation Record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The Stock Unit at the destination ([`StockUnit::key`]).
    pub unit: String,
    /// What.
    pub discrepancy: Discrepancy,
    /// By how much: negative for missing or damaged stock, positive for excess.
    pub quantity: i64,
}

/// A transfer between two stores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transfer {
    /// The transfer's id.
    pub id: String,
    /// The source store.
    pub from_store: String,
    /// The destination store.
    pub to_store: String,
    /// Between facilities: the source and destination facilities. Within one: `None`.
    pub between: Option<(String, String)>,
    /// What is sent.
    pub lines: Vec<TransferLine>,
    /// Where it is.
    pub status: TransferStatus,
    /// Who sent it.
    pub dispatched_by: Option<String>,
    /// Who received it.
    pub received_by: Option<String>,
    /// What was counted on receipt.
    pub counted: Vec<Counted>,
    /// A one-step direct transfer.
    pub direct: bool,
}

/// The discrepancies between what was sent and what was counted (central's matching of a
/// dispatch with a receipt uses the same comparison).
pub fn discrepancies(lines: &[TransferLine], to_store: &str, counted: &[Counted]) -> Vec<Found> {
    let mut found = Vec::new();
    for count in counted {
        let Some(line) = lines.get(count.line) else {
            continue;
        };
        let usable = line.unit(to_store, Status::Usable).key();
        let arrived = count.good + count.damaged;
        if count.damaged > 0 {
            found.push(Found {
                unit: line.unit(to_store, Status::Quarantine).key(),
                discrepancy: Discrepancy::Damaged,
                quantity: -count.damaged,
            });
        }
        if arrived < line.quantity {
            found.push(Found {
                unit: usable,
                discrepancy: Discrepancy::Short,
                quantity: arrived - line.quantity,
            });
        } else if arrived > line.quantity {
            found.push(Found {
                unit: usable,
                discrepancy: Discrepancy::Excess,
                quantity: arrived - line.quantity,
            });
        }
    }
    found
}

impl Transfer {
    /// A transfer of `lines` from `from_store` to `to_store`; `between` names the two
    /// facilities when they differ.
    pub fn new(
        id: &str,
        from_store: &str,
        to_store: &str,
        between: Option<(String, String)>,
        lines: Vec<TransferLine>,
    ) -> Result<Self, TransferRefused> {
        if lines.is_empty() || lines.iter().any(|l| l.quantity <= 0) {
            return Err(TransferRefused::Invalid(
                "a transfer needs lines with positive quantities",
            ));
        }
        if from_store == to_store {
            return Err(TransferRefused::Invalid(
                "a transfer is between two different stores",
            ));
        }
        Ok(Self {
            id: id.to_owned(),
            from_store: from_store.to_owned(),
            to_store: to_store.to_owned(),
            between: between.filter(|(from, to)| from != to),
            lines,
            status: TransferStatus::Picked,
            dispatched_by: None,
            received_by: None,
            counted: Vec::new(),
            direct: false,
        })
    }

    fn expect(&self, status: TransferStatus, action: &'static str) -> Result<(), TransferRefused> {
        if self.status == status {
            Ok(())
        } else {
            Err(TransferRefused::WrongState {
                status: self.status,
                action,
            })
        }
    }

    /// Sends it: a transfer-out of each line at the source.
    pub fn dispatch(&mut self, by: &str) -> Result<Vec<Leg>, TransferRefused> {
        self.expect(TransferStatus::Picked, "dispatch")?;
        self.dispatched_by = Some(by.to_owned());
        self.status = TransferStatus::InTransit;
        Ok(self.outs())
    }

    fn outs(&self) -> Vec<Leg> {
        self.lines
            .iter()
            .map(|line| Leg {
                unit: line.unit(&self.from_store, Status::Usable),
                quantity: -line.quantity,
            })
            .collect()
    }

    /// Receives it with what was counted: a transfer-in of the good stock, and of the damaged
    /// stock into quarantine, and the discrepancies for Reconciliation Records.
    pub fn receive(
        &mut self,
        counted: Vec<Counted>,
        by: &str,
    ) -> Result<(Vec<Leg>, Vec<Found>), TransferRefused> {
        self.expect(TransferStatus::InTransit, "receive")?;
        let mut lines: Vec<usize> = counted.iter().map(|c| c.line).collect();
        lines.sort_unstable();
        lines.dedup();
        if lines.len() != counted.len()
            || lines != (0..self.lines.len()).collect::<Vec<_>>()
            || counted.iter().any(|c| c.good < 0 || c.damaged < 0)
        {
            return Err(TransferRefused::BadCount);
        }
        let mut ins = Vec::new();
        for count in &counted {
            let line = &self.lines[count.line];
            if count.good > 0 {
                ins.push(Leg {
                    unit: line.unit(&self.to_store, Status::Usable),
                    quantity: count.good,
                });
            }
            if count.damaged > 0 {
                ins.push(Leg {
                    unit: line.unit(&self.to_store, Status::Quarantine),
                    quantity: count.damaged,
                });
            }
        }
        let found = discrepancies(&self.lines, &self.to_store, &counted);
        self.counted = counted;
        self.received_by = Some(by.to_owned());
        self.status = if found.is_empty() {
            TransferStatus::Received
        } else {
            TransferStatus::Reconciling
        };
        Ok((ins, found))
    }

    /// A one-step direct transfer within one facility, by the user who both sends and
    /// receives: every line out and in at once, marked `direct`.
    pub fn direct(&mut self, by: &str) -> Result<Vec<Leg>, TransferRefused> {
        if self.between.is_some() {
            return Err(TransferRefused::NotDirect);
        }
        self.expect(TransferStatus::Picked, "direct-transfer")?;
        let mut legs = self.outs();
        legs.extend(self.lines.iter().map(|line| Leg {
            unit: line.unit(&self.to_store, Status::Usable),
            quantity: line.quantity,
        }));
        self.counted = (0..self.lines.len())
            .map(|i| Counted {
                line: i,
                good: self.lines[i].quantity,
                damaged: 0,
            })
            .collect();
        self.dispatched_by = Some(by.to_owned());
        self.received_by = Some(by.to_owned());
        self.direct = true;
        self.status = TransferStatus::Received;
        Ok(legs)
    }

    /// Closes it once its Reconciliation Records are resolved (`all_resolved`).
    pub fn close(&mut self, all_resolved: bool) -> Result<(), TransferRefused> {
        self.expect(TransferStatus::Reconciling, "close")?;
        if !all_resolved {
            return Err(TransferRefused::Invalid(
                "a transfer closes once its reconciliation records are resolved",
            ));
        }
        self.status = TransferStatus::Closed;
        Ok(())
    }
}
