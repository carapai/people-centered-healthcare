//! Pricing at central (pricing-and-insurance.md; ADR 0031; #120), where Price Books and rules
//! are edited and approved, so prices cannot change at a facility while it is offline.
//!
//! - **Drafting** ([`Pricing::draft_book`], [`Pricing::draft_rule`]): a new draft is the next
//!   version of its book or rule; while the latest version is still a draft, drafting again
//!   replaces it.
//! - **Approving** ([`Pricing::approve_book`], [`Pricing::approve_rule`]): one finance approver
//!   who is not the author; the domain refuses overlapping versions, malformed prices and
//!   conflicting rules.
//! - **Retiring** ([`Pricing::retire_book`], [`Pricing::retire_rule`]): an approved version ends
//!   from a time not yet passed.
//! - **Pricing** ([`Pricing::price`], [`Pricing::check`]): a charge priced through the six
//!   stages from every version kept, and the authority's check of the versions a charge records.
//!
//! Each step is one transaction. Who may draft and approve is the caller's permission check.

use health_persistence::{Store, StoreError, Transaction, WriteError};
use health_pricing::{
    Applicability, LifecycleError, Mismatch, PriceBook, PriceRule, Priced, PricingError,
    PricingStore, Request, approve_book, approve_rule, check, price, retire_book, retire_rule,
};
use health_time::{Clock, Timestamp};
use std::sync::Arc;

/// Why a pricing use case failed. Nothing was kept.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PricingAppError {
    /// No such book or rule version.
    #[error("{0} is not known")]
    NotFound(String),
    /// The lifecycle refused it.
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
    /// The charge could not be priced.
    #[error(transparent)]
    Pricing(#[from] PricingError),
    /// A draft with no id or author.
    #[error("a draft needs an id and an author")]
    BadDraft,
    /// The store failed, or a concurrent change came first; try again.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for PricingAppError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for PricingAppError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

/// The pricing use cases over `store`.
pub struct Pricing<S> {
    store: S,
    clock: Arc<dyn Clock>,
}

/// The version a draft takes: the latest version while it is still a draft (replaced), else the
/// next one.
fn next_version(versions: impl Iterator<Item = (u32, bool)>) -> u32 {
    match versions.max_by_key(|(v, _)| *v) {
        Some((version, true)) => version,
        Some((version, false)) => version + 1,
        None => 1,
    }
}

fn is_draft(approval: &health_pricing::Approval) -> bool {
    matches!(approval, health_pricing::Approval::Draft)
}

impl<S> Pricing<S>
where
    S: Store,
    S::Tx: PricingStore,
{
    /// The use cases over `store`, timed by `clock`.
    pub fn new(store: S, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }

    /// Drafts `book` by `author`: the next version, or a replacement of the latest draft.
    pub async fn draft_book(
        &self,
        book: PriceBook,
        author: &str,
    ) -> Result<PriceBook, PricingAppError> {
        if book.id.trim().is_empty() || author.trim().is_empty() {
            return Err(PricingAppError::BadDraft);
        }
        let mut tx = self.store.begin().await?;
        let all = tx.books().await?;
        let version = next_version(
            all.iter()
                .filter(|b| b.id == book.id)
                .map(|b| (b.version, is_draft(&b.approval))),
        );
        let draft = PriceBook {
            version,
            approval: health_pricing::Approval::Draft,
            author: author.to_owned(),
            ..book
        };
        tx.put_book(&draft).await?;
        tx.commit().await?;
        Ok(draft)
    }

    /// Drafts `rule` by `author`: the next version, or a replacement of the latest draft.
    pub async fn draft_rule(
        &self,
        rule: PriceRule,
        author: &str,
    ) -> Result<PriceRule, PricingAppError> {
        if rule.id.trim().is_empty() || author.trim().is_empty() {
            return Err(PricingAppError::BadDraft);
        }
        let mut tx = self.store.begin().await?;
        let all = tx.rules().await?;
        let version = next_version(
            all.iter()
                .filter(|r| r.id == rule.id)
                .map(|r| (r.version, is_draft(&r.approval))),
        );
        let draft = PriceRule {
            version,
            approval: health_pricing::Approval::Draft,
            author: author.to_owned(),
            ..rule
        };
        tx.put_rule(&draft).await?;
        tx.commit().await?;
        Ok(draft)
    }

    /// Approves book `id` version `version` as `by`.
    pub async fn approve_book(
        &self,
        id: &str,
        version: u32,
        by: &str,
    ) -> Result<PriceBook, PricingAppError> {
        let mut tx = self.store.begin().await?;
        let all = tx.books().await?;
        let book = all
            .iter()
            .find(|b| b.id == id && b.version == version)
            .ok_or_else(|| PricingAppError::NotFound(format!("{id}|{version}")))?;
        let approved = approve_book(book, &all, by, self.clock.now())?;
        tx.put_book(&approved).await?;
        tx.commit().await?;
        Ok(approved)
    }

    /// Approves rule `id` version `version` as `by`.
    pub async fn approve_rule(
        &self,
        id: &str,
        version: u32,
        by: &str,
    ) -> Result<PriceRule, PricingAppError> {
        let mut tx = self.store.begin().await?;
        let all = tx.rules().await?;
        let rule = all
            .iter()
            .find(|r| r.id == id && r.version == version)
            .ok_or_else(|| PricingAppError::NotFound(format!("{id}|{version}")))?;
        let approved = approve_rule(rule, &all, by, self.clock.now())?;
        tx.put_rule(&approved).await?;
        tx.commit().await?;
        Ok(approved)
    }

    /// Ends approved book `id` version `version` at `to`.
    pub async fn retire_book(
        &self,
        id: &str,
        version: u32,
        to: Timestamp,
    ) -> Result<PriceBook, PricingAppError> {
        let mut tx = self.store.begin().await?;
        let all = tx.books().await?;
        let book = all
            .iter()
            .find(|b| b.id == id && b.version == version)
            .ok_or_else(|| PricingAppError::NotFound(format!("{id}|{version}")))?;
        let retired = retire_book(book, to, self.clock.now())?;
        tx.put_book(&retired).await?;
        tx.commit().await?;
        Ok(retired)
    }

    /// Ends approved rule `id` version `version` at `to`.
    pub async fn retire_rule(
        &self,
        id: &str,
        version: u32,
        to: Timestamp,
    ) -> Result<PriceRule, PricingAppError> {
        let mut tx = self.store.begin().await?;
        let all = tx.rules().await?;
        let rule = all
            .iter()
            .find(|r| r.id == id && r.version == version)
            .ok_or_else(|| PricingAppError::NotFound(format!("{id}|{version}")))?;
        let retired = retire_rule(rule, to, self.clock.now())?;
        tx.put_rule(&retired).await?;
        tx.commit().await?;
        Ok(retired)
    }

    /// Every version of every book and rule.
    pub async fn catalogue(&self) -> Result<(Vec<PriceBook>, Vec<PriceRule>), PricingAppError> {
        let mut tx = self.store.begin().await?;
        Ok((tx.books().await?, tx.rules().await?))
    }

    /// Prices `request` through the six stages.
    pub async fn price(
        &self,
        request: &Request,
        applicability: &dyn Applicability,
    ) -> Result<Priced, PricingAppError> {
        let (books, rules) = self.catalogue().await?;
        Ok(price(request, &books, &rules, applicability)?)
    }

    /// The authority's check of the versions `priced` records, at `service_time`.
    pub async fn check(
        &self,
        priced: &Priced,
        service_time: Timestamp,
    ) -> Result<Vec<Mismatch>, PricingAppError> {
        let (books, rules) = self.catalogue().await?;
        Ok(check(priced, service_time, &books, &rules))
    }
}
