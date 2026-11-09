//! Central's publication lifecycle (migration `0027_package_lifecycle`; #88), used by
//! `health-publication`: each lifecycle, its records and each published package as JSON, with
//! the columns lookups need. Tarballs are blobs, outside the database.
//!
//! Package activation records (migration `0049_package_activation`; #89) are kept here too: on
//! central, where publication issues them, and on every node they reach by sync.

use health_persistence::{StoreError, WriteError};

use crate::store::SurrealTx;

fn first(rows: Vec<Vec<String>>) -> Option<String> {
    rows.into_iter().next().and_then(|r| r.into_iter().next())
}

fn all(rows: Vec<Vec<String>>) -> Vec<String> {
    rows.into_iter()
        .filter_map(|r| r.into_iter().next())
        .collect()
}

impl SurrealTx {
    /// Creates lifecycle `id` (`name@version`) of `package`, at `stage`, at revision 0. Fails if
    /// it exists.
    pub async fn create_package_lifecycle(
        &mut self,
        id: &str,
        package: &str,
        stage: &str,
        data: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('package_lifecycle', $id) CONTENT { data: $data, \
             package: $package, stage: $stage, revision: 0 } RETURN VALUE record::id(id)",
            vec![
                ("id", id.to_owned()),
                ("data", data.to_owned()),
                ("package", package.to_owned()),
                ("stage", stage.to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    /// Replaces lifecycle `id` if it is still at revision `before`, taking the next. False if
    /// another change came first.
    pub async fn update_package_lifecycle(
        &mut self,
        id: &str,
        stage: &str,
        data: &str,
        before: u64,
    ) -> Result<bool, WriteError> {
        let updated = self
            .run(
                "UPDATE type::record('package_lifecycle', $id) SET data = $data, stage = $stage, \
                 revision = <int>$before + 1 WHERE revision = <int>$before \
                 RETURN VALUE record::id(id)",
                vec![
                    ("id", id.to_owned()),
                    ("data", data.to_owned()),
                    ("stage", stage.to_owned()),
                    ("before", before.to_string()),
                ],
            )
            .await?;
        Ok(!updated.is_empty())
    }

    /// The JSON of lifecycle `id` and its revision, if any.
    pub async fn package_lifecycle(
        &mut self,
        id: &str,
    ) -> Result<Option<(String, u64)>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data, <string>revision] FROM type::record('package_lifecycle', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .and_then(|row| match row.as_slice() {
                [data, revision] => Some((data.clone(), revision.parse().ok()?)),
                _ => None,
            }))
    }

    /// The JSON of every lifecycle not yet published, by id.
    pub async fn open_package_lifecycles(&mut self) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM package_lifecycle WHERE stage != 'published' ORDER BY id",
                Vec::new(),
            )
            .await?;
        Ok(all(rows))
    }

    /// Appends record `id` (a UUIDv7) to lifecycle `lifecycle`.
    pub async fn append_package_lifecycle_record(
        &mut self,
        id: &str,
        lifecycle: &str,
        data: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('package_lifecycle_record', $id) CONTENT { data: $data, \
             lifecycle: $lifecycle } RETURN VALUE record::id(id)",
            vec![
                ("id", id.to_owned()),
                ("data", data.to_owned()),
                ("lifecycle", lifecycle.to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    /// The JSON of lifecycle `lifecycle`'s records, oldest first.
    pub async fn package_lifecycle_records(
        &mut self,
        lifecycle: &str,
    ) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM package_lifecycle_record WHERE lifecycle = $lifecycle \
                 ORDER BY id",
                vec![("lifecycle", lifecycle.to_owned())],
            )
            .await?;
        Ok(all(rows))
    }

    /// Records published package `id` (`name@version`) of `package`. A second publication of
    /// the same id fails.
    pub async fn create_published_package(
        &mut self,
        id: &str,
        package: &str,
        data: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('published_package', $id) CONTENT { data: $data, \
             package: $package } RETURN VALUE record::id(id)",
            vec![
                ("id", id.to_owned()),
                ("data", data.to_owned()),
                ("package", package.to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    /// The JSON of published package `id`, if any.
    pub async fn published_package(&mut self, id: &str) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('published_package', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        Ok(first(rows))
    }

    /// Keeps activation record `id` of `package`, issued at `issued_at` (RFC 3339), and logs it
    /// for the nodes below, unless it is here already: records are never changed. `true` when
    /// it is new.
    pub async fn put_package_activation(
        &mut self,
        id: &str,
        package: &str,
        issued_at: &str,
        data: &str,
    ) -> Result<bool, WriteError> {
        let held = self
            .rows(
                "SELECT VALUE [<string>record::id(id)] FROM type::record('package_activation', $id)",
                vec![("id", id.to_owned())],
            )
            .await
            .map_err(WriteError::Store)?;
        if !held.is_empty() {
            return Ok(false);
        }
        self.run(
            "CREATE type::record('package_activation', $id) CONTENT { package: $package, \
             issued_at: <datetime>$issued_at, data: $data } RETURN VALUE record::id(id)",
            vec![
                ("id", id.to_owned()),
                ("package", package.to_owned()),
                ("issued_at", issued_at.to_owned()),
                ("data", data.to_owned()),
            ],
        )
        .await?;
        self.log_op(crate::sync::ACTIVATIONS, id, data);
        Ok(true)
    }

    /// The JSON of every activation record held here, oldest issued first.
    pub async fn package_activations(&mut self) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM package_activation ORDER BY issued_at, id",
                vec![],
            )
            .await?;
        Ok(all(rows))
    }

    /// The JSON of every published version of `package`.
    pub async fn published_versions(&mut self, package: &str) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM published_package WHERE package = $package ORDER BY id",
                vec![("package", package.to_owned())],
            )
            .await?;
        Ok(all(rows))
    }
}
