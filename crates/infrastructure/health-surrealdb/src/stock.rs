//! Stock (migration `0011_stock`): `health_inventory::StockStore` over the transaction.

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
}
