//! Use cases of the Registries Bounded Context: the client registry's links and merges
//! (ADR 0027; patient-identity.md § Golden record model).
//!
//! - [`ClientRegistry::link`] links a source Patient to a golden Patient, creating one with a
//!   fresh ECID when none is named. The golden record takes the source's values by
//!   survivorship; the disagreements come back as review items.
//! - [`ClientRegistry::unlink`] separates a source (a reviewer's audited decision): the golden
//!   record's values are worked out again without it, and it can never be re-linked there.
//! - [`ClientRegistry::merge_preview`] and [`ClientRegistry::merge`]: `$merge` for two sources of
//!   the same facility, after a preview and a person's approval. The duplicate becomes
//!   inactive and `replaced-by` the other; no reference to it is rewritten.
//!
//! Each use case is one transaction with optimistic version checks, and every version it writes
//! goes through the [`AuditTrail`] port in that transaction.

pub mod fhir;
pub mod matching;
pub mod register;
pub mod review;

use std::future::Future;

use health_client_registry::{
    Ecid, GoldenRecord, LinkRefused, MergePlan, MergeRefused, ReviewItem, plan_merge,
};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, StoreError, StoredResource, Transaction,
    WriteError, WriteRequest,
};
use serde_json::{Value, json};

use crate::fhir::{
    RegistryConfig, golden_json, golden_record, is_golden, linked, reconciled, source_record,
    with_links,
};

/// Records each version a use case writes, in its transaction (the runtime implements it with
/// the audit module).
pub trait AuditTrail<T: Transaction>: Send + Sync {
    /// `json` was written in `tx` by `actor`, for `reason` (`link`, `unlink`, `merge`).
    fn written(
        &self,
        tx: &mut T,
        actor: &str,
        reason: &str,
        json: &RawJson,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// `actor` decided `reason` about the non-FHIR record `what` (a review item) in `tx`.
    fn recorded(
        &self,
        tx: &mut T,
        actor: &str,
        reason: &str,
        what: &str,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
}

/// A registry use case failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// No such Patient (or it is deleted).
    #[error("Patient/{0} is not known")]
    NotFound(String),
    /// The Patient is not the kind the use case needs (a golden record where a source is
    /// expected, or the reverse).
    #[error("{0}")]
    WrongKind(String),
    /// The source is already linked to a golden record.
    #[error("Patient/{source_id} is already linked to Patient/{golden}")]
    AlreadyLinked {
        /// The source.
        source_id: String,
        /// Its golden record.
        golden: String,
    },
    /// The registry's rules refuse the link.
    #[error(transparent)]
    Link(#[from] LinkRefused),
    /// The registry's rules refuse the merge.
    #[error(transparent)]
    Merge(#[from] MergeRefused),
    /// The review queue refuses the decision.
    #[error(transparent)]
    Review(#[from] health_client_registry::ReviewRefused),
    /// No such review item.
    #[error("review item {0} is not known")]
    NoSuchReview(String),
    /// A concurrent change won; rerun the use case.
    #[error("a concurrent change won; retry")]
    Conflict,
    /// The store failed.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for RegistryError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for RegistryError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::Conflict | WriteError::PreconditionFailed { .. } => Self::Conflict,
            other => Self::Store(other.to_string()),
        }
    }
}

impl From<fhir::MappingError> for RegistryError {
    fn from(error: fhir::MappingError) -> Self {
        Self::WrongKind(error.0)
    }
}

/// The result of a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linked {
    /// The golden Patient's id.
    pub golden: ResourceId,
    /// Its ECID.
    pub ecid: Ecid,
    /// Whether this link created the golden record.
    pub created: bool,
    /// Values that did not survive and need a reviewer (§62).
    pub review: Vec<ReviewItem>,
}

/// The client registry's use cases over a store.
pub struct ClientRegistry<S, A> {
    store: S,
    audit: A,
    config: RegistryConfig,
}

fn patient_type() -> ResourceType {
    "Patient"
        .parse()
        .unwrap_or_else(|_| unreachable!("Patient is a resource type"))
}

fn parse(stored: &StoredResource) -> Result<Value, RegistryError> {
    serde_json::from_str(stored.json.as_str()).map_err(|e| RegistryError::Store(e.to_string()))
}

/// 55 random bits for a new ECID, from a UUIDv7's random part.
fn ecid_bits() -> u64 {
    let uuid = uuid::Uuid::now_v7();
    let bytes = uuid.as_bytes();
    bytes[9..16]
        .iter()
        .fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
        & ((1 << 55) - 1)
}

impl<S, A> ClientRegistry<S, A>
where
    S: Store,
    A: AuditTrail<S::Tx>,
{
    /// The registry over `store`, auditing with `audit`.
    pub fn new(store: S, audit: A, config: RegistryConfig) -> Self {
        Self {
            store,
            audit,
            config,
        }
    }

    async fn read(
        &self,
        tx: &mut S::Tx,
        id: ResourceId,
    ) -> Result<(StoredResource, Value), RegistryError> {
        let stored = tx
            .read(&patient_type(), id)
            .await?
            .filter(|s| !s.version.deleted)
            .ok_or_else(|| RegistryError::NotFound(id.to_string()))?;
        let json = parse(&stored)?;
        Ok((stored, json))
    }

    pub(crate) async fn write(
        &self,
        tx: &mut S::Tx,
        actor: &str,
        reason: &str,
        id: ResourceId,
        json: &Value,
        precondition: Precondition,
    ) -> Result<(), RegistryError> {
        let raw = RawJson::from_validated(json.to_string());
        tx.write(WriteRequest {
            resource_type: patient_type(),
            id,
            json: raw,
            op_id: OpId::new(),
            precondition,
        })
        .await?;
        let stored = tx
            .read(&patient_type(), id)
            .await?
            .ok_or_else(|| RegistryError::Store("written Patient is missing".into()))?;
        self.audit.written(tx, actor, reason, &stored.json).await?;
        Ok(())
    }

    fn source_id(json: &Value) -> Result<(), RegistryError> {
        if is_golden(json) {
            return Err(RegistryError::WrongKind(
                "a golden Patient is not a source".into(),
            ));
        }
        Ok(())
    }

    /// Links source `source` to golden `golden`, or to a new golden record when `golden` is
    /// `None`. `actor` is the reviewer or the matching job.
    pub async fn link_in(
        &self,
        tx: &mut S::Tx,
        source: ResourceId,
        golden: Option<ResourceId>,
        actor: &str,
    ) -> Result<Linked, RegistryError> {
        let (source_head, source_json) = self.read(tx, source).await?;
        Self::source_id(&source_json)?;
        if let Some(current) = linked(&source_json, "refer").into_iter().next() {
            return Err(RegistryError::AlreadyLinked {
                source_id: source.to_string(),
                golden: current,
            });
        }
        let record = source_record(&source_json, &self.config)?;
        let (golden_id, mut golden_rec, precondition, created) = match golden {
            Some(id) => {
                let (head, json) = self.read(tx, id).await?;
                (
                    id,
                    golden_record(&json)?,
                    Precondition::IfMatch(head.version.version_id),
                    false,
                )
            }
            None => (
                ResourceId::new(),
                GoldenRecord::new(Ecid::from_random(ecid_bits())),
                Precondition::MustNotExist,
                true,
            ),
        };
        let review = golden_rec.link(&record)?;
        self.write(
            tx,
            actor,
            "link",
            golden_id,
            &golden_json(&golden_id.to_string(), &golden_rec, &self.config),
            precondition,
        )
        .await?;
        let updated = reconciled(&with_links(&source_json, "refer", &[golden_id.to_string()]));
        self.write(
            tx,
            actor,
            "link",
            source,
            &updated,
            Precondition::IfMatch(source_head.version.version_id),
        )
        .await?;
        Ok(Linked {
            golden: golden_id,
            ecid: golden_rec.ecid,
            created,
            review,
        })
    }

    /// [`link_in`](Self::link_in) in its own transaction.
    pub async fn link(
        &self,
        source: ResourceId,
        golden: Option<ResourceId>,
        actor: &str,
    ) -> Result<Linked, RegistryError> {
        let mut tx = self.store.begin().await?;
        let linked = self.link_in(&mut tx, source, golden, actor).await?;
        tx.commit().await?;
        Ok(linked)
    }

    /// Separates `source` from its golden record, a reviewer's decision. The golden record's
    /// values are recomputed from the sources that remain; with none left it becomes inactive.
    pub async fn unlink_in(
        &self,
        tx: &mut S::Tx,
        source: ResourceId,
        reviewer: &str,
    ) -> Result<ResourceId, RegistryError> {
        let (source_head, source_json) = self.read(tx, source).await?;
        Self::source_id(&source_json)?;
        let golden_id: ResourceId = linked(&source_json, "refer")
            .into_iter()
            .next()
            .and_then(|g| g.parse().ok())
            .ok_or(RegistryError::Link(LinkRefused::NotLinked))?;
        let (golden_head, golden_json_value) = self.read(tx, golden_id).await?;
        let mut golden_rec = golden_record(&golden_json_value)?;
        let mut remaining = Vec::new();
        for other in golden_rec
            .sources
            .iter()
            .filter(|s| **s != source.to_string())
        {
            let Ok(id) = other.parse() else { continue };
            let (_, json) = self.read(tx, id).await?;
            remaining.push(source_record(&json, &self.config)?);
        }
        golden_rec.unlink(&source.to_string(), &remaining)?;
        self.write(
            tx,
            reviewer,
            "unlink",
            golden_id,
            &golden_json(&golden_id.to_string(), &golden_rec, &self.config),
            Precondition::IfMatch(golden_head.version.version_id),
        )
        .await?;
        self.write(
            tx,
            reviewer,
            "unlink",
            source,
            &with_links(&source_json, "refer", &[]),
            Precondition::IfMatch(source_head.version.version_id),
        )
        .await?;
        Ok(golden_id)
    }

    /// [`unlink_in`](Self::unlink_in) in its own transaction.
    pub async fn unlink(
        &self,
        source: ResourceId,
        reviewer: &str,
    ) -> Result<ResourceId, RegistryError> {
        let mut tx = self.store.begin().await?;
        let golden = self.unlink_in(&mut tx, source, reviewer).await?;
        tx.commit().await?;
        Ok(golden)
    }

    /// Records that `source` is not the person of golden record `golden` (a reviewer's
    /// decision on a potential match it was never linked to): the adjudication marker means
    /// matching never links them.
    pub async fn separate_in(
        &self,
        tx: &mut S::Tx,
        source: ResourceId,
        golden: ResourceId,
        reviewer: &str,
    ) -> Result<(), RegistryError> {
        let (head, json) = self.read(tx, golden).await?;
        let mut record = golden_record(&json)?;
        if record.sources.contains(&source.to_string()) {
            return Err(RegistryError::WrongKind(
                "the source is linked; unlink it instead".into(),
            ));
        }
        record.separated.insert(source.to_string());
        self.write(
            tx,
            reviewer,
            "separate",
            golden,
            &golden_json(&golden.to_string(), &record, &self.config),
            Precondition::IfMatch(head.version.version_id),
        )
        .await
    }

    /// A reviewer settles a survivorship disagreement in favour of `claim`.
    pub async fn override_in(
        &self,
        tx: &mut S::Tx,
        golden: ResourceId,
        field: health_client_registry::Field,
        claim: health_client_registry::Claim,
        reviewer: &str,
    ) -> Result<(), RegistryError> {
        let (head, json) = self.read(tx, golden).await?;
        let mut record = golden_record(&json)?;
        if !record.sources.contains(&claim.source) {
            return Err(RegistryError::Link(LinkRefused::NotLinked));
        }
        record.claims.insert(field, claim);
        self.write(
            tx,
            reviewer,
            "review",
            golden,
            &golden_json(&golden.to_string(), &record, &self.config),
            Precondition::IfMatch(head.version.version_id),
        )
        .await
    }

    /// The store, for use cases that compose registry steps in one transaction.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// The registry's configuration.
    pub fn config(&self) -> &RegistryConfig {
        &self.config
    }

    /// The audit trail, for use cases that record decisions in the same transaction.
    pub fn audit(&self) -> &A {
        &self.audit
    }

    /// The plan for merging `source` into `target`, for a person to preview and approve.
    pub async fn merge_preview(
        &self,
        source: ResourceId,
        target: ResourceId,
    ) -> Result<MergePlan, RegistryError> {
        let mut tx = self.store.begin().await?;
        let (_, source_json) = self.read(&mut tx, source).await?;
        let (_, target_json) = self.read(&mut tx, target).await?;
        Self::source_id(&source_json)?;
        Self::source_id(&target_json)?;
        Ok(plan_merge(
            &source_record(&source_json, &self.config)?,
            &source_record(&target_json, &self.config)?,
        )?)
    }

    /// Carries out an approved plan, after checking it still holds: the source becomes inactive
    /// and `replaced-by` the target, which `replaces` it. Nothing that references the source is
    /// touched.
    pub async fn merge(&self, plan: &MergePlan) -> Result<(), RegistryError> {
        let reviewer = plan.ready()?.to_owned();
        let parse_id = |id: &str| {
            id.parse::<ResourceId>()
                .map_err(|_| RegistryError::NotFound(id.to_owned()))
        };
        let (source, target) = (parse_id(&plan.source)?, parse_id(&plan.target)?);
        let mut tx = self.store.begin().await?;
        let (source_head, source_json) = self.read(&mut tx, source).await?;
        let (target_head, target_json) = self.read(&mut tx, target).await?;
        let current = plan_merge(
            &source_record(&source_json, &self.config)?,
            &source_record(&target_json, &self.config)?,
        )?;
        if current.facility != plan.facility {
            return Err(RegistryError::Merge(MergeRefused::DifferentFacility));
        }
        let mut retired = with_links(
            &source_json,
            "replaced-by",
            std::slice::from_ref(&plan.target),
        );
        retired["active"] = json!(false);
        let mut replaced = linked(&target_json, "replaces");
        replaced.push(plan.source.clone());
        let survivor = with_links(&target_json, "replaces", &replaced);
        self.write(
            &mut tx,
            &reviewer,
            "merge",
            source,
            &retired,
            Precondition::IfMatch(source_head.version.version_id),
        )
        .await?;
        self.write(
            &mut tx,
            &reviewer,
            "merge",
            target,
            &survivor,
            Precondition::IfMatch(target_head.version.version_id),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
