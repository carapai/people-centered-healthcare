//! Keeping the product master (#128 follow-up), at central.
//!
//! [`Products::save`] writes a product as its Medication, in one transaction:
//! - it is validated first ([`Product::validate`]);
//! - a new product claims its code (`product|{code}`), so no two products share one;
//! - an existing one keeps its code: Stock Units name products by code, so a code never changes;
//! - an update is `If-Match` the head read in the same transaction.

use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    GuardKey, Precondition, RawJson, ResourceType, Store, StoreError, Transaction, WriteError,
    WriteRequest,
};
use health_product_registry::{Product, ProductRefused};

use crate::medication;

/// Why a product could not be saved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProductError {
    /// It is not a valid product.
    #[error(transparent)]
    Refused(#[from] ProductRefused),
    /// Another product has that code.
    #[error("another product has the code {0}")]
    CodeTaken(String),
    /// A product's code never changes.
    #[error("a product's code never changes (it is {0})")]
    CodeChanged(String),
    /// Someone saved it at the same time; read it again.
    #[error("the product changed meanwhile; try again")]
    Conflict,
    /// The store failed.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for ProductError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for ProductError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::Conflict | WriteError::PreconditionFailed { .. } => Self::Conflict,
            WriteError::GuardHeld(key) => {
                Self::CodeTaken(key.0.strip_prefix("product|").unwrap_or(&key.0).to_owned())
            }
            other => Self::Store(other.to_string()),
        }
    }
}

/// The product master over `store`.
pub struct Products<S> {
    store: S,
}

impl<S: Store> Products<S> {
    /// The use cases over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Saves `product` as the Medication `id`.
    pub async fn save(&self, id: ResourceId, product: &Product) -> Result<(), ProductError> {
        product.validate()?;
        let medication_type: ResourceType = "Medication"
            .parse()
            .map_err(|_| ProductError::Store("the Medication type".into()))?;
        let mut tx = self.store.begin().await?;
        let head = tx
            .read(&medication_type, id)
            .await?
            .filter(|h| !h.version.deleted);
        let precondition = match &head {
            Some(head) => {
                let stored: serde_json::Value = serde_json::from_str(head.json.as_str())
                    .map_err(|e| ProductError::Store(e.to_string()))?;
                if let Some(before) = crate::product(&stored)
                    && before.code != product.code
                {
                    return Err(ProductError::CodeChanged(before.code));
                }
                Precondition::IfMatch(head.version.version_id)
            }
            None => {
                tx.claim_guard(GuardKey(format!("product|{}", product.code)))
                    .await?;
                Precondition::MustNotExist
            }
        };
        let json = medication(product, &id.to_string());
        tx.write(WriteRequest {
            resource_type: medication_type,
            id,
            json: RawJson::from_validated(json.to_string()),
            op_id: OpId::new(),
            precondition,
        })
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
