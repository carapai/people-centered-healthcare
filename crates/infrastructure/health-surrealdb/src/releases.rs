//! Staged software roll-out (migration `0061_release_rollout`; #136): the platform releases
//! imported at central, and the roll-out records central issues and every node keeps. Records
//! are logged for the nodes below, as activation records are, so an edge passes them on to its
//! desktops.

use health_persistence::{StoreError, WriteError};

use crate::store::SurrealTx;

fn all(rows: Vec<Vec<String>>) -> Vec<String> {
    rows.into_iter()
        .filter_map(|r| r.into_iter().next())
        .collect()
}

impl crate::SurrealStore {
    /// Whether a roll-out record held here names an artifact with sha-256 `hash`: what a node
    /// may download of a release (#136).
    pub async fn release_artifact_named(&self, hash: &str) -> Result<bool, StoreError> {
        let mut tx = health_persistence::Store::begin(self).await?;
        let named = tx
            .rows(
                "SELECT VALUE [<string>record::id(id)] FROM release_rollout \
                 WHERE string::contains(data, $hash) LIMIT 1",
                vec![("hash", hash.to_ascii_lowercase())],
            )
            .await?;
        Ok(hash.len() == 64 && !named.is_empty())
    }
}

impl SurrealTx {
    /// Keeps imported release `version` as `data`, unless it is here already: a release is
    /// imported once. `true` when it is new.
    pub async fn put_platform_release(
        &mut self,
        version: &str,
        data: &str,
    ) -> Result<bool, WriteError> {
        if self
            .platform_release(version)
            .await
            .map_err(WriteError::Store)?
            .is_some()
        {
            return Ok(false);
        }
        self.run(
            "CREATE type::record('platform_release', $version) CONTENT { data: $data, \
             imported_at: time::now() } RETURN VALUE record::id(id)",
            vec![("version", version.to_owned()), ("data", data.to_owned())],
        )
        .await?;
        Ok(true)
    }

    /// Imported release `version`'s JSON, if it is here.
    pub async fn platform_release(&mut self, version: &str) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('platform_release', $version)",
                vec![("version", version.to_owned())],
            )
            .await?;
        Ok(all(rows).into_iter().next())
    }

    /// Every imported release's JSON, oldest imported first.
    pub async fn platform_releases(&mut self) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM platform_release ORDER BY imported_at, id",
                vec![],
            )
            .await?;
        Ok(all(rows))
    }

    /// Keeps roll-out record `id`, issued at `issued_at` (RFC 3339), and logs it for the nodes
    /// below, unless it is here already: records are never changed. `true` when it is new.
    pub async fn put_release_rollout(
        &mut self,
        id: &str,
        issued_at: &str,
        data: &str,
    ) -> Result<bool, WriteError> {
        let held = self
            .rows(
                "SELECT VALUE [<string>record::id(id)] FROM type::record('release_rollout', $id)",
                vec![("id", id.to_owned())],
            )
            .await
            .map_err(WriteError::Store)?;
        if !held.is_empty() {
            return Ok(false);
        }
        self.run(
            "CREATE type::record('release_rollout', $id) CONTENT { \
             issued_at: <datetime>$issued_at, data: $data } RETURN VALUE record::id(id)",
            vec![
                ("id", id.to_owned()),
                ("issued_at", issued_at.to_owned()),
                ("data", data.to_owned()),
            ],
        )
        .await?;
        self.log_op(crate::sync::ROLLOUTS, id, data);
        Ok(true)
    }

    /// The JSON of every roll-out record held here, oldest issued first.
    pub async fn release_rollouts(&mut self) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM release_rollout ORDER BY issued_at, id",
                vec![],
            )
            .await?;
        Ok(all(rows))
    }
}
