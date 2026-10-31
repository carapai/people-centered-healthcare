//! The registry review queue (patient-identity.md; ADR 0028: potential matches go to registry
//! review, with an ageing alert after 7 days).

use std::future::Future;

use health_persistence::{StoreError, WriteError};
use health_time::{SignedDuration, Timestamp};

use crate::survivorship::{Claim, Field};

/// Pending items older than this are overdue, and alert (ADR 0028).
pub const AGEING: SignedDuration = SignedDuration::from_hours(7 * 24);

/// What a reviewer decides.
#[derive(Debug, Clone)]
pub enum ReviewKind {
    /// Matching scored `source` as possibly the person of `golden` (through its linked source
    /// `candidate`).
    PotentialMatch {
        /// The newly registered source Patient.
        source: String,
        /// The existing source Patient it resembles.
        candidate: String,
        /// The candidate's golden record.
        golden: String,
        /// The match weight.
        score: f64,
    },
    /// A linked source disagrees with the golden record at lower trust (§62).
    Survivorship {
        /// The golden record.
        golden: String,
        /// The field.
        field: Field,
        /// The surviving value.
        current: Claim,
        /// The value that did not survive.
        incoming: Claim,
    },
}

impl ReviewKind {
    /// The kind's code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::PotentialMatch { .. } => "potential-match",
            Self::Survivorship { .. } => "survivorship",
        }
    }
}

/// A reviewer's decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Potential match: the same person; link them.
    SamePerson,
    /// Potential match: different people; never link them (an adjudication marker).
    DifferentPeople,
    /// Survivorship: keep the surviving value.
    KeepCurrent,
    /// Survivorship: take the incoming value.
    TakeIncoming,
}

impl Decision {
    /// The decision's code.
    pub fn code(self) -> &'static str {
        match self {
            Self::SamePerson => "same-person",
            Self::DifferentPeople => "different-people",
            Self::KeepCurrent => "keep-current",
            Self::TakeIncoming => "take-incoming",
        }
    }

    /// The decision with `code`.
    pub fn from_code(code: &str) -> Option<Self> {
        [
            Self::SamePerson,
            Self::DifferentPeople,
            Self::KeepCurrent,
            Self::TakeIncoming,
        ]
        .into_iter()
        .find(|d| d.code() == code)
    }
}

/// Who decided what, when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decided {
    /// The decision.
    pub decision: Decision,
    /// The reviewer.
    pub by: String,
    /// When.
    pub at: Timestamp,
}

/// One queue entry.
#[derive(Debug, Clone)]
pub struct ReviewEntry {
    /// The entry id (a UUIDv7).
    pub id: String,
    /// What to decide.
    pub kind: ReviewKind,
    /// When it was queued.
    pub created_at: Timestamp,
    /// The decision, once made.
    pub decided: Option<Decided>,
}

/// Why a decision was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReviewRefused {
    /// Already decided.
    #[error("this review item is already decided")]
    AlreadyDecided,
    /// The decision does not fit the item (a survivorship answer to a potential match).
    #[error("that decision does not apply to this kind of item")]
    WrongDecision,
    /// No reviewer named.
    #[error("a decision needs a named reviewer")]
    NoReviewer,
}

impl ReviewEntry {
    /// Whether the entry is pending past [`AGEING`] at `now`.
    pub fn overdue(&self, now: Timestamp) -> bool {
        self.decided.is_none()
            && self
                .created_at
                .checked_add(AGEING)
                .is_ok_and(|due| now >= due)
    }

    /// Records `decision` by `reviewer`; the caller carries out its effect in the same
    /// transaction.
    pub fn decide(
        &mut self,
        decision: Decision,
        reviewer: &str,
        now: Timestamp,
    ) -> Result<(), ReviewRefused> {
        if self.decided.is_some() {
            return Err(ReviewRefused::AlreadyDecided);
        }
        if reviewer.trim().is_empty() {
            return Err(ReviewRefused::NoReviewer);
        }
        let fits = matches!(
            (&self.kind, decision),
            (
                ReviewKind::PotentialMatch { .. },
                Decision::SamePerson | Decision::DifferentPeople
            ) | (
                ReviewKind::Survivorship { .. },
                Decision::KeepCurrent | Decision::TakeIncoming
            )
        );
        if !fits {
            return Err(ReviewRefused::WrongDecision);
        }
        self.decided = Some(Decided {
            decision,
            by: reviewer.to_owned(),
            at: now,
        });
        Ok(())
    }
}

/// The queue's storage, inside a transaction (the Module's driven port).
pub trait ReviewQueue {
    /// Adds a new entry.
    fn put_review(
        &mut self,
        entry: &ReviewEntry,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// One entry.
    fn review(
        &mut self,
        id: &str,
    ) -> impl Future<Output = Result<Option<ReviewEntry>, StoreError>> + Send;
    /// Pending entries, oldest first.
    fn pending_reviews(
        &mut self,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<ReviewEntry>, StoreError>> + Send;
    /// Records an entry's decision; false if it was already decided (compare-and-set).
    fn record_decision(
        &mut self,
        entry: &ReviewEntry,
    ) -> impl Future<Output = Result<bool, WriteError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivorship::Trust;

    fn at(day: i64) -> Timestamp {
        Timestamp::UNIX_EPOCH
            .checked_add(SignedDuration::from_hours(day * 24))
            .unwrap_or(Timestamp::UNIX_EPOCH)
    }

    fn potential() -> ReviewEntry {
        ReviewEntry {
            id: "r1".into(),
            kind: ReviewKind::PotentialMatch {
                source: "s".into(),
                candidate: "c".into(),
                golden: "g".into(),
                score: 12.0,
            },
            created_at: at(10),
            decided: None,
        }
    }

    #[test]
    fn items_age_after_seven_days_until_decided() {
        let mut entry = potential();
        assert!(!entry.overdue(at(16)));
        assert!(entry.overdue(at(17)));
        entry
            .decide(Decision::DifferentPeople, "reviewer", at(18))
            .expect("decide");
        assert!(!entry.overdue(at(30)));
    }

    #[test]
    fn decisions_fit_their_item_and_are_made_once() {
        let mut entry = potential();
        assert_eq!(
            entry.decide(Decision::TakeIncoming, "r", at(11)),
            Err(ReviewRefused::WrongDecision)
        );
        assert_eq!(
            entry.decide(Decision::SamePerson, " ", at(11)),
            Err(ReviewRefused::NoReviewer)
        );
        assert_eq!(entry.decide(Decision::SamePerson, "r", at(11)), Ok(()));
        assert_eq!(
            entry.decide(Decision::DifferentPeople, "r", at(12)),
            Err(ReviewRefused::AlreadyDecided)
        );
        let claim = |v: &str| Claim {
            value: v.into(),
            trust: Trust::StaffEntered,
            recorded: at(1),
            source: "a".into(),
        };
        let mut survivorship = ReviewEntry {
            kind: ReviewKind::Survivorship {
                golden: "g".into(),
                field: Field::Family,
                current: claim("Okello"),
                incoming: claim("Okelo"),
            },
            ..potential()
        };
        assert_eq!(
            survivorship.decide(Decision::SamePerson, "r", at(11)),
            Err(ReviewRefused::WrongDecision)
        );
        assert_eq!(
            survivorship.decide(Decision::KeepCurrent, "r", at(11)),
            Ok(())
        );
        assert_eq!(survivorship.kind.code(), "survivorship");
        for d in [
            Decision::SamePerson,
            Decision::DifferentPeople,
            Decision::KeepCurrent,
            Decision::TakeIncoming,
        ] {
            assert_eq!(Decision::from_code(d.code()), Some(d));
        }
    }
}
