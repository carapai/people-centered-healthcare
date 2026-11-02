//! The edge's device registry and pending enrollments (migration `0020_devices`), used by
//! `health-auth`'s SurrealDB store inside one transaction per operation. Rows carry the device
//! or request as JSON; the columns beside it are for lookups and the unique indexes.

use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;

use crate::store::SurrealTx;

/// A device row: its JSON and the columns indexed beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRow {
    /// The device id.
    pub id: String,
    /// Its key's thumbprint, unique.
    pub thumbprint: String,
    /// `active` or `revoked`.
    pub status: &'static str,
    /// Bumped by every change.
    pub version: u64,
    /// The device as JSON.
    pub data: String,
}

impl SurrealTx {
    /// Creates a device. A second device with the same key fails on the unique index.
    pub async fn create_device(&mut self, row: &DeviceRow) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('auth_device', $id) CONTENT { data: $data, thumbprint: $thumbprint, \
             status: $status, version: <int>$version } RETURN VALUE record::id(id)",
            device_binds(row),
        )
        .await?;
        Ok(())
    }

    /// Replaces a device whose stored version is `before`; false if it changed meanwhile.
    pub async fn update_device(
        &mut self,
        row: &DeviceRow,
        before: u64,
    ) -> Result<bool, WriteError> {
        let mut binds = device_binds(row);
        binds.push(("before", before.to_string()));
        let updated = self
            .run(
                "UPDATE type::record('auth_device', $id) SET data = $data, status = $status, \
                 version = <int>$version WHERE version = <int>$before RETURN VALUE record::id(id)",
                binds,
            )
            .await?;
        Ok(!updated.is_empty())
    }

    /// The JSON of the device with key `thumbprint`.
    pub async fn device_by_thumbprint(
        &mut self,
        thumbprint: &str,
    ) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM auth_device WHERE thumbprint = $thumbprint LIMIT 1",
                vec![("thumbprint", thumbprint.to_owned())],
            )
            .await?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
    }

    /// The JSON of the device with `id`.
    pub async fn device(&mut self, id: &str) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('auth_device', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
    }

    /// Every device's JSON, oldest id first.
    pub async fn devices(&mut self) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM auth_device ORDER BY id",
                Vec::new(),
            )
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }

    /// Stores a pending request, replacing any earlier one for the same key and dropping
    /// expired ones first, so their codes are free again. A code another pending request holds
    /// fails on the unique index.
    pub async fn put_enrollment(
        &mut self,
        id: &str,
        thumbprint: &str,
        code: &str,
        expires: Timestamp,
        data: &str,
        now: Timestamp,
    ) -> Result<(), WriteError> {
        self.run(
            "DELETE auth_enrollment WHERE expires <= <datetime>$now OR thumbprint = $thumbprint \
             RETURN VALUE <string>record::id($before.id)",
            vec![
                ("now", now.to_string()),
                ("thumbprint", thumbprint.to_owned()),
            ],
        )
        .await?;
        self.run(
            "CREATE type::record('auth_enrollment', $id) CONTENT { data: $data, thumbprint: \
             $thumbprint, code: $code, expires: <datetime>$expires } RETURN VALUE record::id(id)",
            vec![
                ("id", id.to_owned()),
                ("data", data.to_owned()),
                ("thumbprint", thumbprint.to_owned()),
                ("code", code.to_owned()),
                ("expires", expires.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    /// The JSON of the pending request holding `code`, expired or not.
    pub async fn enrollment_by_code(&mut self, code: &str) -> Result<Option<String>, StoreError> {
        self.first_enrollment("code = $value", code).await
    }

    /// The JSON of the pending request for key `thumbprint`, expired or not.
    pub async fn enrollment_by_thumbprint(
        &mut self,
        thumbprint: &str,
    ) -> Result<Option<String>, StoreError> {
        self.first_enrollment("thumbprint = $value", thumbprint)
            .await
    }

    async fn first_enrollment(
        &mut self,
        condition: &str,
        value: &str,
    ) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                &format!("SELECT VALUE [data] FROM auth_enrollment WHERE {condition} LIMIT 1"),
                vec![("value", value.to_owned())],
            )
            .await?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
    }

    /// The JSON of every request still pending at `now`, soonest to expire first.
    pub async fn enrollments(&mut self, now: Timestamp) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM auth_enrollment WHERE expires > <datetime>$now ORDER BY expires",
                vec![("now", now.to_string())],
            )
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }

    /// Removes a request; false if it was already gone (approved concurrently).
    pub async fn take_enrollment(&mut self, id: &str) -> Result<bool, WriteError> {
        let gone = self
            .run(
                "DELETE type::record('auth_enrollment', $id) RETURN VALUE <string>record::id($before.id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        Ok(!gone.is_empty())
    }
}

fn device_binds(row: &DeviceRow) -> Vec<(&'static str, String)> {
    vec![
        ("id", row.id.clone()),
        ("data", row.data.clone()),
        ("thumbprint", row.thumbprint.clone()),
        ("status", row.status.to_owned()),
        ("version", row.version.to_string()),
    ]
}
