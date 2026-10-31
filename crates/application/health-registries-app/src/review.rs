//! The registry review queue's use cases (patient-identity.md; ADR 0028): registration
//! matching that links only what it may, a queue of potential matches and survivorship
//! disagreements, reviewers' decisions, and the 7-day ageing alert.
//!
//! **A pair a reviewer separated is never linked again automatically:** an unlink or a
//! "different people" decision leaves an adjudication marker on the golden record, and
//! [`RegistryReview::register`] skips every golden record that carries one for the source.

use std::sync::Arc;

use health_client_registry::matching::MatchingRules;
use health_client_registry::{Decision, ReviewEntry, ReviewItem, ReviewKind, ReviewQueue};
use health_identifiers::ResourceId;
use health_persistence::{BlockingIndex, ResourceType, Store, Transaction};
use health_time::{Clock, Timestamp};
use serde_json::Value;

use crate::fhir::{golden_record, linked};
use crate::matching::Matcher;
use crate::{AuditTrail, ClientRegistry, RegistryError};

/// What registration matching did with a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Registered {
    /// Linked to an existing golden record without review.
    AutoLinked {
        /// The golden record.
        golden: ResourceId,
    },
    /// Possible matches went to review; the source stays unlinked (and usable for care).
    Queued {
        /// The review items.
        reviews: Vec<String>,
    },
    /// Nobody matched: a new golden record and ECID.
    NewIdentity {
        /// The new golden record.
        golden: ResourceId,
    },
}

fn patient_type() -> ResourceType {
    "Patient"
        .parse()
        .unwrap_or_else(|_| unreachable!("Patient is a resource type"))
}

/// The review queue's use cases.
pub struct RegistryReview<S, A> {
    registry: ClientRegistry<S, A>,
    matcher: Matcher<S>,
    clock: Arc<dyn Clock>,
}

fn survivorship(golden: ResourceId, items: Vec<ReviewItem>, now: Timestamp) -> Vec<ReviewEntry> {
    items
        .into_iter()
        .map(|item| ReviewEntry {
            id: uuid::Uuid::now_v7().to_string(),
            kind: ReviewKind::Survivorship {
                golden: golden.to_string(),
                field: item.field,
                current: item.current,
                incoming: item.incoming,
            },
            created_at: now,
            decided: None,
        })
        .collect()
}

impl<S, A> RegistryReview<S, A>
where
    S: Store + Clone,
    S::Tx: BlockingIndex + ReviewQueue,
    A: AuditTrail<S::Tx>,
{
    /// The use cases over `registry`, matching with `rules`.
    pub fn new(
        registry: ClientRegistry<S, A>,
        rules: MatchingRules,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let matcher = Matcher::new(registry.store().clone(), rules, registry.config().clone());
        Self {
            registry,
            matcher,
            clock,
        }
    }

    async fn json(tx: &mut S::Tx, id: ResourceId) -> Result<Value, RegistryError> {
        let stored = tx
            .read(&patient_type(), id)
            .await?
            .ok_or_else(|| RegistryError::NotFound(id.to_string()))?;
        serde_json::from_str(stored.json.as_str()).map_err(|e| RegistryError::Store(e.to_string()))
    }

    /// Matches a newly registered or changed source and acts on the result: an auto-link grade
    /// links it, possible matches are queued, and no match creates a new identity. Golden
    /// records a reviewer separated from this source are skipped entirely.
    pub async fn register(
        &self,
        source: ResourceId,
        actor: &str,
    ) -> Result<Registered, RegistryError> {
        let store = self.registry.store();
        let mut read = store.begin().await?;
        let source_json = Self::json(&mut read, source).await?;
        if let Some(golden) = linked(&source_json, "refer").into_iter().next() {
            return Err(RegistryError::AlreadyLinked {
                source_id: source.to_string(),
                golden,
            });
        }
        let matched = self.matcher.find(&source_json, 20, false).await?;
        let mut auto: Option<ResourceId> = None;
        let mut possible: Vec<(String, ResourceId, f64)> = Vec::new();
        for candidate in matched.candidates.iter().filter(|c| c.id != source) {
            let Some(golden) = linked(&candidate.resource, "refer")
                .into_iter()
                .next()
                .and_then(|g| g.parse::<ResourceId>().ok())
            else {
                continue;
            };
            let record = golden_record(&Self::json(&mut read, golden).await?)?;
            if record.separated.contains(&source.to_string())
                || possible.iter().any(|(_, g, _)| *g == golden)
            {
                continue;
            }
            if candidate.evaluation.grade.auto_links() && auto.is_none() {
                auto = Some(golden);
            } else {
                possible.push((candidate.id.to_string(), golden, candidate.evaluation.score));
            }
        }
        drop(read);

        let now = self.clock.now();
        let mut tx = store.begin().await?;
        let result = match auto {
            Some(golden) => {
                let linked = self
                    .registry
                    .link_in(&mut tx, source, Some(golden), actor)
                    .await?;
                for entry in survivorship(golden, linked.review, now) {
                    tx.put_review(&entry).await?;
                }
                Registered::AutoLinked { golden }
            }
            None if !possible.is_empty() => {
                let mut reviews = Vec::new();
                for (candidate, golden, score) in possible {
                    let entry = ReviewEntry {
                        id: uuid::Uuid::now_v7().to_string(),
                        kind: ReviewKind::PotentialMatch {
                            source: source.to_string(),
                            candidate,
                            golden: golden.to_string(),
                            score,
                        },
                        created_at: now,
                        decided: None,
                    };
                    tx.put_review(&entry).await?;
                    reviews.push(entry.id);
                }
                Registered::Queued { reviews }
            }
            None => {
                let linked = self.registry.link_in(&mut tx, source, None, actor).await?;
                Registered::NewIdentity {
                    golden: linked.golden,
                }
            }
        };
        tx.commit().await?;
        Ok(result)
    }

    /// Pending items, oldest first.
    pub async fn pending(&self, limit: usize) -> Result<Vec<ReviewEntry>, RegistryError> {
        let mut tx = self.registry.store().begin().await?;
        Ok(tx.pending_reviews(limit).await?)
    }

    /// Pending items older than 7 days: the ageing alert.
    pub async fn overdue(&self, limit: usize) -> Result<Vec<ReviewEntry>, RegistryError> {
        let now = self.clock.now();
        Ok(self
            .pending(limit)
            .await?
            .into_iter()
            .filter(|e| e.overdue(now))
            .collect())
    }

    /// A reviewer's decision, carried out and recorded in one transaction and audited.
    pub async fn decide(
        &self,
        id: &str,
        decision: Decision,
        reviewer: &str,
    ) -> Result<(), RegistryError> {
        let now = self.clock.now();
        let mut tx = self.registry.store().begin().await?;
        let mut entry = tx
            .review(id)
            .await?
            .ok_or_else(|| RegistryError::NoSuchReview(id.to_owned()))?;
        entry.decide(decision, reviewer, now)?;
        let parse = |id: &str| {
            id.parse::<ResourceId>()
                .map_err(|_| RegistryError::NotFound(id.to_owned()))
        };
        match (&entry.kind, decision) {
            (ReviewKind::PotentialMatch { source, golden, .. }, Decision::SamePerson) => {
                let golden = parse(golden)?;
                let linked = self
                    .registry
                    .link_in(&mut tx, parse(source)?, Some(golden), reviewer)
                    .await?;
                for item in survivorship(golden, linked.review, now) {
                    tx.put_review(&item).await?;
                }
            }
            (ReviewKind::PotentialMatch { source, golden, .. }, Decision::DifferentPeople) => {
                let source_id = parse(source)?;
                self.registry
                    .separate_in(&mut tx, source_id, parse(golden)?, reviewer)
                    .await?;
                // Once no other potential match for this source is pending, it is a new person.
                let others = tx
                    .pending_reviews(1_000)
                    .await?
                    .into_iter()
                    .any(|e| e.id != entry.id && matches!(&e.kind, ReviewKind::PotentialMatch { source: s, .. } if s == source));
                let unlinked = linked(&Self::json(&mut tx, source_id).await?, "refer").is_empty();
                if !others && unlinked {
                    self.registry
                        .link_in(&mut tx, source_id, None, reviewer)
                        .await?;
                }
            }
            (
                ReviewKind::Survivorship {
                    golden,
                    field,
                    incoming,
                    ..
                },
                Decision::TakeIncoming,
            ) => {
                self.registry
                    .override_in(&mut tx, parse(golden)?, *field, incoming.clone(), reviewer)
                    .await?;
            }
            _ => {}
        }
        if !tx.record_decision(&entry).await? {
            return Err(RegistryError::Review(
                health_client_registry::ReviewRefused::AlreadyDecided,
            ));
        }
        self.registry
            .audit()
            .recorded(
                &mut tx,
                reviewer,
                decision.code(),
                &format!("urn:health:registry-review:{id}"),
            )
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

impl<S, A> ClientRegistry<S, A>
where
    S: Store,
    S::Tx: ReviewQueue,
    A: AuditTrail<S::Tx>,
{
    /// A reviewer's link (`$link`): [`link_in`](Self::link_in) in its own transaction, with the
    /// survivorship disagreements queued for review in the same transaction.
    pub async fn link_reviewed(
        &self,
        source: ResourceId,
        golden: Option<ResourceId>,
        reviewer: &str,
        now: Timestamp,
    ) -> Result<crate::Linked, RegistryError> {
        let mut tx = self.store().begin().await?;
        let linked = self.link_in(&mut tx, source, golden, reviewer).await?;
        for entry in survivorship(linked.golden, linked.review.clone(), now) {
            tx.put_review(&entry).await?;
        }
        tx.commit().await?;
        Ok(linked)
    }
}
