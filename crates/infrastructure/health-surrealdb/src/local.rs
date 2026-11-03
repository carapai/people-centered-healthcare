//! Offline administration on an edge (migration `0022_local_admin`), used by `health-auth`'s
//! SurrealDB store: local grants, local revocations and the local configuration, as JSON.

use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;

use crate::store::SurrealTx;

impl SurrealTx {
    /// Stores a local grant: its JSON, its signed JWS, and the columns lookups need.
    pub async fn put_local_grant(
        &mut self,
        id: &str,
        account: &str,
        facility: &str,
        not_after: Timestamp,
        data: &str,
        jws: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('auth_local_grant', $id) CONTENT { data: $data, jws: $jws, \
             account: $account, facility: $facility, not_after: <datetime>$not_after } \
             RETURN VALUE record::id(id)",
            vec![
                ("id", id.to_owned()),
                ("data", data.to_owned()),
                ("jws", jws.to_owned()),
                ("account", account.to_owned()),
                ("facility", facility.to_owned()),
                ("not_after", not_after.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    /// The JSON of the local grants at `facility` unexpired at `at`, of `account` when given.
    pub async fn local_grants(
        &mut self,
        facility: &str,
        account: Option<&str>,
        at: Timestamp,
    ) -> Result<Vec<String>, StoreError> {
        let mut binds = vec![("facility", facility.to_owned()), ("at", at.to_string())];
        let by_account = match account {
            Some(account) => {
                binds.push(("account", account.to_owned()));
                " AND account = $account"
            }
            None => "",
        };
        let rows = self
            .rows(
                &format!(
                    "SELECT VALUE [data] FROM auth_local_grant WHERE facility = $facility \
                     AND not_after > <datetime>$at{by_account} ORDER BY id"
                ),
                binds,
            )
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }

    /// Records a local revocation of `account`.
    pub async fn put_local_revocation(
        &mut self,
        account: &str,
        data: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('auth_local_revocation', $account) CONTENT { data: $data } \
             RETURN VALUE record::id(id)",
            vec![("account", account.to_owned()), ("data", data.to_owned())],
        )
        .await?;
        Ok(())
    }

    /// The JSON of `account`'s local revocation, if any.
    pub async fn local_revocation(&mut self, account: &str) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('auth_local_revocation', $account)",
                vec![("account", account.to_owned())],
            )
            .await?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
    }

    /// The JSON of every local revocation.
    pub async fn local_revocations(&mut self) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM auth_local_revocation ORDER BY id",
                Vec::new(),
            )
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }

    /// Records a provisional account.
    pub async fn put_local_provisional(
        &mut self,
        account: &str,
        data: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('auth_local_provisional', $account) CONTENT { data: $data } \
             RETURN VALUE record::id(id)",
            vec![("account", account.to_owned()), ("data", data.to_owned())],
        )
        .await?;
        Ok(())
    }

    /// The JSON of `account`'s provisional record, if it has one.
    pub async fn local_provisional(&mut self, account: &str) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('auth_local_provisional', $account)",
                vec![("account", account.to_owned())],
            )
            .await?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
    }

    /// Replaces the local configuration.
    pub async fn put_local_config(&mut self, data: &str) -> Result<(), WriteError> {
        self.run(
            "UPSERT auth_local_config:current CONTENT { data: $data } RETURN VALUE record::id(id)",
            vec![("data", data.to_owned())],
        )
        .await?;
        Ok(())
    }

    /// The local configuration's JSON, if central provisioned one.
    pub async fn local_config(&mut self) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM auth_local_config:current",
                Vec::new(),
            )
            .await?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
    }
}
