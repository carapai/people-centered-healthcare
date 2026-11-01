//! Stock (migration `0011_stock`): `health_inventory::StockStore` over the transaction.

use health_inventory::counts::StockCount;
use health_inventory::receiving::{GoodsReceived, Requisition};
use health_inventory::reconciliation::ReconciliationRecord;
use health_inventory::transfers::Transfer;
use health_inventory::{Balance, Movement, StockStore, StockUnit};
use health_persistence::{StoreError, WriteError};

use crate::store::SurrealTx;

fn invalid() -> StoreError {
    StoreError::Unavailable("stock row has an invalid shape".into())
}

impl StockStore for SurrealTx {
    async fn stock_balance(&mut self, key: &str) -> Result<Option<Balance>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [<string>on_hand, <string>reserved, <string>on_hold, <string>flagged] \
                 FROM type::record('stock_balance', $key)",
                vec![("key", key.to_owned())],
            )
            .await?;
        let Some(row) = rows.into_iter().next() else {
            return Ok(None);
        };
        let [on_hand, reserved, on_hold, flagged]: [String; 4] =
            row.try_into().map_err(|_| invalid())?;
        let number = |text: &str| text.parse::<i64>().map_err(|_| invalid());
        Ok(Some(Balance {
            on_hand: number(&on_hand)?,
            reserved: number(&reserved)?,
            on_hold: number(&on_hold)?,
            flagged: flagged == "true",
        }))
    }

    async fn put_stock_balance(
        &mut self,
        unit: &StockUnit,
        balance: &Balance,
    ) -> Result<(), WriteError> {
        let json = serde_json::to_string(unit)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
        self.run(
            "UPSERT type::record('stock_balance', $key) CONTENT { unit: $unit, \
             on_hand: <int>$on_hand, reserved: <int>$reserved, on_hold: <int>$on_hold, \
             flagged: <bool>$flagged } RETURN VALUE record::id(id)",
            vec![
                ("key", unit.key()),
                ("unit", json),
                ("on_hand", balance.on_hand.to_string()),
                ("reserved", balance.reserved.to_string()),
                ("on_hold", balance.on_hold.to_string()),
                ("flagged", balance.flagged.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn append_movement(&mut self, movement: &Movement) -> Result<(), WriteError> {
        let json = serde_json::to_string(movement)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
        self.run(
            "CREATE type::record('stock_movement', $id) CONTENT { unit_key: $unit_key, data: $data, \
             recorded_at: <datetime>$recorded_at } RETURN VALUE record::id(id)",
            vec![
                ("id", movement.id.clone()),
                ("unit_key", movement.unit.key()),
                ("data", json),
                ("recorded_at", movement.recorded_at.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn movements(&mut self, key: &str) -> Result<Vec<Movement>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM stock_movement WHERE unit_key = $key ORDER BY recorded_at, id",
                vec![("key", key.to_owned())],
            )
            .await?;
        rows.into_iter()
            .map(|row| {
                let data = row.into_iter().next().ok_or_else(invalid)?;
                serde_json::from_str(&data).map_err(|_| invalid())
            })
            .collect()
    }

    async fn stock_units(&mut self) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [<string>record::id(id)] FROM stock_balance ORDER BY id",
                vec![],
            )
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }

    async fn put_transfer(&mut self, transfer: &Transfer) -> Result<(), WriteError> {
        let data = serde_json::to_string(transfer)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
        self.run(
            "UPSERT type::record('stock_transfer', $id) CONTENT { data: $data } RETURN VALUE record::id(id)",
            vec![("id", transfer.id.clone()), ("data", data)],
        )
        .await?;
        Ok(())
    }

    async fn transfer(&mut self, id: &str) -> Result<Option<Transfer>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('stock_transfer', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        rows.into_iter()
            .filter_map(|r| r.into_iter().next())
            .next()
            .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
            .transpose()
    }

    async fn put_reconciliation(
        &mut self,
        record: &ReconciliationRecord,
    ) -> Result<(), WriteError> {
        let data = serde_json::to_string(record)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
        self.run(
            "UPSERT type::record('stock_reconciliation', $id) CONTENT { unit_key: $unit, \
             resolved: <bool>$resolved, opened_at: <datetime>$opened_at, data: $data } \
             RETURN VALUE record::id(id)",
            vec![
                ("id", record.id.clone()),
                ("unit", record.unit.clone()),
                ("resolved", record.resolution.is_some().to_string()),
                ("opened_at", record.opened_at.to_string()),
                ("data", data),
            ],
        )
        .await?;
        Ok(())
    }

    async fn reconciliation(
        &mut self,
        id: &str,
    ) -> Result<Option<ReconciliationRecord>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('stock_reconciliation', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        rows.into_iter()
            .filter_map(|r| r.into_iter().next())
            .next()
            .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
            .transpose()
    }

    async fn open_reconciliations(&mut self) -> Result<Vec<ReconciliationRecord>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM stock_reconciliation WHERE resolved = false ORDER BY opened_at",
                vec![],
            )
            .await?;
        rows.into_iter()
            .filter_map(|r| r.into_iter().next())
            .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
            .collect()
    }

    async fn units_of_store(&mut self, store: &str) -> Result<Vec<StockUnit>, StoreError> {
        // A unit's key starts with its store.
        let rows = self
            .rows(
                "SELECT VALUE [unit] FROM stock_balance \
                 WHERE string::starts_with(<string>record::id(id), $prefix) ORDER BY id",
                vec![("prefix", format!("{store}|"))],
            )
            .await?;
        rows.into_iter()
            .filter_map(|r| r.into_iter().next())
            .map(|unit| serde_json::from_str(&unit).map_err(|_| invalid()))
            .collect()
    }

    async fn put_count(&mut self, count: &StockCount) -> Result<(), WriteError> {
        let data = serde_json::to_string(count)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
        self.run(
            "UPSERT type::record('stock_count', $id) CONTENT { data: $data } RETURN VALUE record::id(id)",
            vec![("id", count.id.clone()), ("data", data)],
        )
        .await?;
        Ok(())
    }

    async fn count(&mut self, id: &str) -> Result<Option<StockCount>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('stock_count', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        rows.into_iter()
            .filter_map(|r| r.into_iter().next())
            .next()
            .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
            .transpose()
    }

    async fn put_goods_received(&mut self, note: &GoodsReceived) -> Result<(), WriteError> {
        let data = serde_json::to_string(note)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
        self.run(
            "UPSERT type::record('stock_goods_received', $id) CONTENT { data: $data } RETURN VALUE record::id(id)",
            vec![("id", note.id.clone()), ("data", data)],
        )
        .await?;
        Ok(())
    }

    async fn goods_received(&mut self, id: &str) -> Result<Option<GoodsReceived>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('stock_goods_received', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        rows.into_iter()
            .filter_map(|r| r.into_iter().next())
            .next()
            .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
            .transpose()
    }

    async fn put_requisition(&mut self, requisition: &Requisition) -> Result<(), WriteError> {
        let data = serde_json::to_string(requisition)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
        self.run(
            "UPSERT type::record('stock_requisition', $id) CONTENT { data: $data } RETURN VALUE record::id(id)",
            vec![("id", requisition.id.clone()), ("data", data)],
        )
        .await?;
        Ok(())
    }

    async fn requisition(&mut self, id: &str) -> Result<Option<Requisition>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('stock_requisition', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        rows.into_iter()
            .filter_map(|r| r.into_iter().next())
            .next()
            .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
            .transpose()
    }

    async fn requisitions(&mut self) -> Result<Vec<Requisition>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM stock_requisition ORDER BY id",
                vec![],
            )
            .await?;
        rows.into_iter()
            .filter_map(|r| r.into_iter().next())
            .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
            .collect()
    }
}
