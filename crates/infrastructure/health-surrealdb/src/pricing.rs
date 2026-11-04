//! Price Books and Price Rules (migration `0030_pricing`): `health_pricing::PricingStore` over
//! the transaction.

use health_persistence::{StoreError, WriteError};
use health_pricing::{Approval, PriceBook, PriceRule, PricingStore};
use serde::de::DeserializeOwned;

use crate::store::SurrealTx;

fn encode<T: serde::Serialize>(value: &T) -> Result<String, WriteError> {
    serde_json::to_string(value)
        .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))
}

fn decode<T: DeserializeOwned>(rows: Vec<Vec<String>>) -> Result<Vec<T>, StoreError> {
    rows.into_iter()
        .filter_map(|r| r.into_iter().next())
        .map(|data| {
            serde_json::from_str(&data)
                .map_err(|_| StoreError::Unavailable("a pricing row has an invalid shape".into()))
        })
        .collect()
}

fn status(approval: &Approval) -> &'static str {
    match approval {
        Approval::Draft => "draft",
        Approval::Approved { .. } => "approved",
    }
}

impl PricingStore for SurrealTx {
    async fn put_book(&mut self, book: &PriceBook) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('price_book', $key) CONTENT { data: $data, book: $book, \
             version: <int>$version, status: $status } RETURN VALUE record::id(id)",
            vec![
                ("key", format!("{}|{}", book.id, book.version)),
                ("data", encode(book)?),
                ("book", book.id.clone()),
                ("version", book.version.to_string()),
                ("status", status(&book.approval).to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn books(&mut self) -> Result<Vec<PriceBook>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM price_book ORDER BY book, version",
                vec![],
            )
            .await?;
        decode(rows)
    }

    async fn put_rule(&mut self, rule: &PriceRule) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('price_rule', $key) CONTENT { data: $data, rule: $rule, \
             version: <int>$version, status: $status } RETURN VALUE record::id(id)",
            vec![
                ("key", format!("{}|{}", rule.id, rule.version)),
                ("data", encode(rule)?),
                ("rule", rule.id.clone()),
                ("version", rule.version.to_string()),
                ("status", status(&rule.approval).to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn rules(&mut self) -> Result<Vec<PriceRule>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM price_rule ORDER BY rule, version",
                vec![],
            )
            .await?;
        decode(rows)
    }
}
