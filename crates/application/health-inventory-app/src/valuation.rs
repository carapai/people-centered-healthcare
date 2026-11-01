//! Stock valuation and inventory-accounting postings (inventory-and-procurement.md § Valuation;
//! ADR 0030; #131). Valuation is a projection rebuilt from the movement ledger; facilities with
//! inventory accounting also post each costed movement to the financial ledger.

use health_financial_ledger::{
    COST_OF_GOODS, Currency, Dimensions, INVENTORY, Line, Money, PAYABLE, Posting,
    STOCK_ADJUSTMENT, Side,
};
use health_inventory::valuation::{Costed, Method, Valuation, value};
use health_inventory::{MovementKind, StockStore, StockUnit};
use health_persistence::Store;
use health_time::Timestamp;
use rust_decimal::Decimal;

use crate::{Inventory, InventoryError};

/// A store's value: each Stock Unit's, and the total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreValuation {
    /// Each Stock Unit with its valuation.
    pub units: Vec<(StockUnit, Valuation)>,
    /// The store's total value.
    pub total: Decimal,
}

impl<S> Inventory<S>
where
    S: Store,
    S::Tx: StockStore,
{
    /// The value of `store` by `method`, rebuilt from its movements.
    pub async fn valuation(
        &self,
        store: &str,
        method: Method,
    ) -> Result<StoreValuation, InventoryError> {
        let mut tx = self.store.begin().await?;
        let mut units = Vec::new();
        let mut total = Decimal::ZERO;
        for unit in tx.units_of_store(store).await? {
            let movements = tx.movements(&unit.key()).await?;
            let valuation = value(&unit, &movements, method);
            total += valuation.value;
            units.push((unit, valuation));
        }
        Ok(StoreValuation { units, total })
    }
}

/// The accounts a costed movement moves value between, debit first; `None` when it stays in
/// inventory (a transfer within the facility, a status change).
fn accounts(kind: MovementKind, increase: bool) -> Option<(&'static str, &'static str)> {
    use MovementKind::{
        Adjustment, CountVariance, Dispense, ExpiryWriteOff, Issue, Receipt, Return, Reversal,
    };
    match (kind, increase) {
        (Receipt, true) => Some((INVENTORY, PAYABLE)),
        (Issue | Dispense, false) => Some((COST_OF_GOODS, INVENTORY)),
        (Return, true) => Some((INVENTORY, COST_OF_GOODS)),
        (ExpiryWriteOff | Adjustment | CountVariance | Reversal, false) => {
            Some((STOCK_ADJUSTMENT, INVENTORY))
        }
        (Adjustment | CountVariance | Reversal, true) => Some((INVENTORY, STOCK_ADJUSTMENT)),
        // Transfers within the facility and status changes keep the value in inventory.
        _ => None,
    }
}

/// The inventory-accounting postings for `unit`'s costed movements at `facility`, in
/// `currency`: one balanced posting per movement whose value rounds to more than zero, with the
/// unit's funding source as a dimension.
pub fn inventory_postings(
    unit: &StockUnit,
    costed: &[Costed],
    facility: &str,
    currency: Currency,
    at: Timestamp,
) -> Vec<Posting> {
    costed
        .iter()
        .filter_map(|c| {
            let (debit, credit) = accounts(c.kind, c.value > Decimal::ZERO)?;
            let amount = Money::rounded(c.value.abs(), currency);
            if amount.is_zero() {
                return None;
            }
            let dimensions = Dimensions {
                facility: facility.to_owned(),
                department: None,
                service: None,
                program: None,
                funding_source: Some(unit.funding_source.clone()),
                fund: None,
                encounter: None,
                price_versions: std::collections::BTreeSet::new(),
            };
            let line = |account: &str, side: Side| Line {
                account: account.to_owned(),
                side,
                amount,
                dimensions: dimensions.clone(),
                original: None,
            };
            Some(Posting {
                id: format!("stock-{}", c.movement),
                at,
                description: format!("{:?} of {} {}", c.kind, unit.product, unit.batch),
                lines: vec![line(debit, Side::Debit), line(credit, Side::Credit)],
                reverses: None,
            })
        })
        .collect()
}
