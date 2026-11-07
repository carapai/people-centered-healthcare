use serde::{Deserialize, Serialize};

use crate::Outcome;

/// Where an outbox item stands (synchronization.md § Item states).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// Committed locally, not yet sent.
    Pending,
    /// In a batch on the wire.
    Sending,
    /// Accepted by the authority.
    Accepted,
    /// Under review at the authority.
    Conflict,
    /// Refused; in the originating node's review queue.
    Rejected,
    /// Waiting on a group that is in conflict or rejected.
    Held,
}

/// What happens to an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    /// It went into a push batch.
    BatchBuilt,
    /// The push failed in transit; it is sent again.
    TransportFailed,
    /// The authority answered.
    Answered(Answer),
    /// A group it depends on is in conflict or rejected.
    DependencyBlocked,
    /// That group was accepted.
    DependencyAccepted,
    /// Its conflict was resolved at the authority, and the resolution pulled down.
    Resolved,
    /// The user corrected it locally, as a new candidate.
    Corrected,
}

/// The authority's answer, without its detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Answer {
    /// Accepted.
    Accepted,
    /// A Conflict Record.
    Conflict,
    /// Rejected.
    Rejected,
}

impl From<&Outcome> for Event {
    fn from(outcome: &Outcome) -> Self {
        Event::Answered(match outcome {
            Outcome::Accepted => Answer::Accepted,
            Outcome::Conflict { .. } => Answer::Conflict,
            Outcome::Rejected { .. } => Answer::Rejected,
        })
    }
}

/// An event the state does not allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{event:?} is not allowed from {from:?}")]
pub struct InvalidTransition {
    /// The state.
    pub from: ItemState,
    /// The event.
    pub event: Event,
}

impl ItemState {
    /// The state after `event`.
    pub fn on(self, event: Event) -> Result<Self, InvalidTransition> {
        use ItemState::{Accepted, Conflict, Held, Pending, Rejected, Sending};
        let next = match (self, event) {
            (Pending, Event::BatchBuilt) => Sending,
            (Pending, Event::DependencyBlocked) => Held,
            (Held, Event::DependencyAccepted) => Pending,
            (Sending, Event::TransportFailed) => Pending,
            (Sending, Event::Answered(Answer::Accepted)) | (Conflict, Event::Resolved) => Accepted,
            (Sending, Event::Answered(Answer::Conflict)) => Conflict,
            (Sending, Event::Answered(Answer::Rejected)) => Rejected,
            (Rejected, Event::Corrected) => Pending,
            (from, event) => return Err(InvalidTransition { from, event }),
        };
        Ok(next)
    }

    /// Whether the authority has not accepted it yet, so pulled data must not overwrite it.
    pub fn is_unaccepted(self) -> bool {
        self != ItemState::Accepted
    }
}
