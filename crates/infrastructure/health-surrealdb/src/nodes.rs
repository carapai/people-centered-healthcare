//! Central's node registry, provisioning codes and escrowed recovery keys (migrations
//! `0024_nodes`, `0025_recovery_keys`), used by
//! `health-provisioning`: each code and node as JSON, with the columns lookups and the unique
//! indexes need.

use health_persistence::{StoreError, WriteError};

use crate::store::SurrealTx;

impl SurrealTx {
    /// Stores a new provisioning code: its JSON and the hash of its text.
    pub async fn put_provisioning_code(
        &mut self,
        id: &str,
        hash: &str,
        data: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('node_provisioning_code', $id) CONTENT { data: $data, \
             hash: $hash } RETURN VALUE record::id(id)",
            vec![
                ("id", id.to_owned()),
                ("hash", hash.to_owned()),
                ("data", data.to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    /// Replaces a provisioning code's JSON, as when it is redeemed.
    pub async fn update_provisioning_code(
        &mut self,
        id: &str,
        data: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "UPDATE type::record('node_provisioning_code', $id) SET data = $data \
             RETURN VALUE record::id(id)",
            vec![("id", id.to_owned()), ("data", data.to_owned())],
        )
        .await?;
        Ok(())
    }

    /// The JSON of the provisioning code whose text hashes to `hash`, if any.
    pub async fn provisioning_code_by_hash(
        &mut self,
        hash: &str,
    ) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM node_provisioning_code WHERE hash = $hash",
                vec![("hash", hash.to_owned())],
            )
            .await?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
    }

    /// Stores a node.
    pub async fn put_node(
        &mut self,
        id: &str,
        facility: &str,
        short_code: &str,
        code: &str,
        data: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('node', $id) CONTENT { data: $data, facility: $facility, \
             short_code: $short_code, code: $code } RETURN VALUE record::id(id)",
            vec![
                ("id", id.to_owned()),
                ("data", data.to_owned()),
                ("facility", facility.to_owned()),
                ("short_code", short_code.to_owned()),
                ("code", code.to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    /// The JSON of node `id`, if any.
    pub async fn node(&mut self, id: &str) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('node', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
    }

    /// The JSON of every node at `facility`, decommissioned ones too, by id.
    pub async fn nodes_at(&mut self, facility: &str) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM node WHERE facility = $facility ORDER BY id",
                vec![("facility", facility.to_owned())],
            )
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }

    /// The JSON of every node, by id.
    pub async fn nodes(&mut self) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows("SELECT VALUE [data] FROM node ORDER BY id", Vec::new())
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }

    /// Stores a node's recovery key for a volume, `node:volume`: its JSON and the wrapped key.
    pub async fn put_recovery_key(
        &mut self,
        node: &str,
        volume: &str,
        data: &str,
        wrapped: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('node_recovery_key', $id) CONTENT { data: $data, \
             wrapped: $wrapped, node: $node } RETURN VALUE record::id(id)",
            vec![
                ("id", format!("{node}:{volume}")),
                ("data", data.to_owned()),
                ("wrapped", wrapped.to_owned()),
                ("node", node.to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    /// Replaces a recovery key's JSON, as when it is read out; the wrapped key is kept.
    pub async fn update_recovery_key(
        &mut self,
        node: &str,
        volume: &str,
        data: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "UPDATE type::record('node_recovery_key', $id) SET data = $data \
             RETURN VALUE record::id(id)",
            vec![
                ("id", format!("{node}:{volume}")),
                ("data", data.to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    /// A node's recovery key for a volume: its JSON and the wrapped key, if escrowed.
    pub async fn recovery_key(
        &mut self,
        node: &str,
        volume: &str,
    ) -> Result<Option<(String, String)>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data, wrapped] FROM type::record('node_recovery_key', $id)",
                vec![("id", format!("{node}:{volume}"))],
            )
            .await?;
        Ok(rows.into_iter().next().and_then(|row| {
            let mut row = row.into_iter();
            Some((row.next()?, row.next()?))
        }))
    }

    /// The JSON of every escrowed recovery key, without the keys.
    pub async fn recovery_keys(&mut self) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM node_recovery_key ORDER BY id",
                Vec::new(),
            )
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }
}
