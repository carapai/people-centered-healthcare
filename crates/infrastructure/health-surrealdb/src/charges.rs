//! Charges (migration `0031_charges`): `health_billing::ChargeStore` over the transaction.

use health_billing::{Charge, ChargeStore};
use health_persistence::{StoreError, WriteError};

use crate::store::SurrealTx;

fn encode(charge: &Charge) -> Result<String, WriteError> {
    serde_json::to_string(charge)
        .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))
}

fn decode(rows: Vec<Vec<String>>) -> Result<Vec<Charge>, StoreError> {
    rows.into_iter()
        .filter_map(|r| r.into_iter().next())
        .map(|data| {
            serde_json::from_str(&data)
                .map_err(|_| StoreError::Unavailable("a charge row has an invalid shape".into()))
        })
        .collect()
}

fn binds(charge: &Charge) -> Result<Vec<(&'static str, String)>, WriteError> {
    Ok(vec![
        ("id", charge.id.clone()),
        ("data", encode(charge)?),
        ("event", charge.event.clone()),
        ("account", charge.account.clone()),
        ("status", charge.status.code().to_owned()),
        ("at", charge.at.to_string()),
    ])
}

impl ChargeStore for SurrealTx {
    async fn add_charge(&mut self, charge: &Charge) -> Result<bool, WriteError> {
        let kept = self
            .rows(
                "SELECT VALUE [record::id(id)] FROM type::record('charge', $id)",
                vec![("id", charge.id.clone())],
            )
            .await?;
        if !kept.is_empty() {
            return Ok(false);
        }
        self.run(
            "CREATE type::record('charge', $id) CONTENT { data: $data, event: $event, \
             account: $account, status: $status, at: <datetime>$at } RETURN VALUE record::id(id)",
            binds(charge)?,
        )
        .await?;
        Ok(true)
    }

    async fn put_charge(&mut self, charge: &Charge) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('charge', $id) CONTENT { data: $data, event: $event, \
             account: $account, status: $status, at: <datetime>$at } RETURN VALUE record::id(id)",
            binds(charge)?,
        )
        .await?;
        Ok(())
    }

    async fn charges_of_event(&mut self, event: &str) -> Result<Vec<Charge>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM charge WHERE event = $event ORDER BY id",
                vec![("event", event.to_owned())],
            )
            .await?;
        decode(rows)
    }

    async fn charges_of_account(&mut self, account: &str) -> Result<Vec<Charge>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM charge WHERE account = $account ORDER BY at, id",
                vec![("account", account.to_owned())],
            )
            .await?;
        decode(rows)
    }
}

/// Deferrals, settlements and waivers (migration `0032_billing_accounts`).
mod accounts {
    use health_billing::{AccountStore, Deferral, Settlement, Waiver};
    use health_persistence::{StoreError, WriteError};
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    use crate::store::SurrealTx;

    fn encode<T: Serialize>(value: &T) -> Result<String, WriteError> {
        serde_json::to_string(value)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))
    }

    fn decode<T: DeserializeOwned>(rows: Vec<Vec<String>>) -> Result<Vec<T>, StoreError> {
        rows.into_iter()
            .filter_map(|r| r.into_iter().next())
            .map(|data| {
                serde_json::from_str(&data).map_err(|_| {
                    StoreError::Unavailable("a billing row has an invalid shape".into())
                })
            })
            .collect()
    }

    impl SurrealTx {
        async fn by_account<T: DeserializeOwned>(
            &mut self,
            table: &str,
            field: &str,
            value: &str,
        ) -> Result<Vec<T>, StoreError> {
            let rows = self
                .rows(
                    &format!("SELECT VALUE [data] FROM {table} WHERE {field} = $value ORDER BY id"),
                    vec![("value", value.to_owned())],
                )
                .await?;
            decode(rows)
        }
    }

    impl AccountStore for SurrealTx {
        async fn add_deferral(&mut self, deferral: &Deferral) -> Result<(), WriteError> {
            self.run(
                "CREATE billing_deferral CONTENT { data: $data, account: $account } \
                 RETURN VALUE record::id(id)",
                vec![
                    ("data", encode(deferral)?),
                    ("account", deferral.account.clone()),
                ],
            )
            .await?;
            Ok(())
        }

        async fn deferrals(&mut self, account: &str) -> Result<Vec<Deferral>, StoreError> {
            self.by_account("billing_deferral", "account", account)
                .await
        }

        async fn add_settlement(&mut self, settlement: &Settlement) -> Result<(), WriteError> {
            self.run(
                "CREATE billing_settlement CONTENT { data: $data, account: $account, \
                 payment: $payment } RETURN VALUE record::id(id)",
                vec![
                    ("data", encode(settlement)?),
                    ("account", settlement.account.clone()),
                    ("payment", settlement.payment.clone()),
                ],
            )
            .await?;
            Ok(())
        }

        async fn settlements_of_account(
            &mut self,
            account: &str,
        ) -> Result<Vec<Settlement>, StoreError> {
            self.by_account("billing_settlement", "account", account)
                .await
        }

        async fn settlements_of_payment(
            &mut self,
            payment: &str,
        ) -> Result<Vec<Settlement>, StoreError> {
            self.by_account("billing_settlement", "payment", payment)
                .await
        }

        async fn add_waiver(&mut self, waiver: &Waiver) -> Result<(), WriteError> {
            self.run(
                "CREATE type::record('billing_waiver', $id) CONTENT { data: $data, \
                 account: $account } RETURN VALUE record::id(id)",
                vec![
                    ("id", waiver.id.clone()),
                    ("data", encode(waiver)?),
                    ("account", waiver.account.clone()),
                ],
            )
            .await?;
            Ok(())
        }

        async fn waivers(&mut self, account: &str) -> Result<Vec<Waiver>, StoreError> {
            self.by_account("billing_waiver", "account", account).await
        }
    }
}
