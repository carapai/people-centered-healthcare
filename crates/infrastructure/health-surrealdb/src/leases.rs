//! Authority Leases (migration `0053_authority_leases`; #135; write-authority.md § Hand-over
//! rules). An edge keeps the leases it granted, a desktop the ones it holds, and the stock path
//! asks [`StockStore::may_write_store`](health_inventory::StockStore::may_write_store) before a
//! node moves a store's stock as its authority.

use health_authority_leases::{Lease, LeaseState};
use health_persistence::{Store as _, StoreError, Transaction as _, WriteError};
use health_time::Timestamp;

use crate::store::{SurrealStore, SurrealTx};

/// Whose lease a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseRole {
    /// This node granted it to another: the edge's own writes in its scope are refused.
    Granted,
    /// This node holds it: its writes in its scope are allowed only while it is active.
    Held,
}

impl LeaseRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Held => "held",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "granted" => Some(Self::Granted),
            "held" => Some(Self::Held),
            _ => None,
        }
    }
}

fn state(lease: &Lease) -> &'static str {
    match lease.state {
        LeaseState::Active => "active",
        LeaseState::Returning { .. } => "returning",
        LeaseState::Frozen { .. } => "frozen",
        LeaseState::Returned { .. } => "returned",
        LeaseState::Recovered { .. } => "recovered",
    }
}

fn invalid() -> StoreError {
    StoreError::Unavailable("an authority_lease row has an invalid shape".into())
}

impl SurrealTx {
    /// Stores `lease` as `role`, replacing what was there.
    pub async fn put_lease(&mut self, role: LeaseRole, lease: &Lease) -> Result<(), WriteError> {
        let data = serde_json::to_string(lease)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
        // Binds are strings: the stores cross joined by the unit separator, which no store id
        // holds, and are split back into the array.
        let stores = lease
            .stores
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\u{1f}");
        self.run(
            "UPSERT type::record('authority_lease', $id) CONTENT { role: $role, holder: $holder, \
             stores: string::split($stores, $separator), state: $state, data: $data } \
             RETURN VALUE record::id(id)",
            vec![
                ("id", lease.id.to_string()),
                ("role", role.as_str().to_owned()),
                ("holder", lease.holder.clone()),
                ("stores", stores),
                ("separator", "\u{1f}".to_owned()),
                ("state", state(lease).to_owned()),
                ("data", data),
            ],
        )
        .await?;
        Ok(())
    }

    /// The leases kept as `role`.
    pub async fn leases(&mut self, role: LeaseRole) -> Result<Vec<Lease>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM authority_lease WHERE role = $role",
                vec![("role", role.as_str().to_owned())],
            )
            .await?;
        rows.into_iter()
            .map(|row| {
                row.into_iter()
                    .next()
                    .and_then(|data| serde_json::from_str(&data).ok())
                    .ok_or_else(invalid)
            })
            .collect()
    }

    /// The leases, of either role, whose scope covers `store`.
    pub async fn leases_covering(
        &mut self,
        store: &str,
    ) -> Result<Vec<(LeaseRole, Lease)>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [role, data] FROM authority_lease WHERE stores CONTAINS $store",
                vec![("store", store.to_owned())],
            )
            .await?;
        rows.into_iter()
            .map(|row| {
                let [role, data]: [String; 2] = row.try_into().map_err(|_| invalid())?;
                Ok((
                    LeaseRole::parse(&role).ok_or_else(invalid)?,
                    serde_json::from_str(&data).map_err(|_| invalid())?,
                ))
            })
            .collect()
    }

    /// Whether this node may move stock in `store` at `at` as its authority: not while a lease
    /// it granted holds the store, and, for a lease it holds, only while that lease is active.
    pub(crate) async fn may_write_leased(
        &mut self,
        store: &str,
        at: Timestamp,
    ) -> Result<bool, StoreError> {
        for (role, lease) in self.leases_covering(store).await? {
            if !lease.holds() {
                continue;
            }
            let allowed = match role {
                LeaseRole::Granted => false,
                LeaseRole::Held => lease.holder_may_write(&lease.holder, store, at),
            };
            if !allowed {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The holder of a lease this node granted that holds `store`, if any: only its operations
    /// on the store are taken in (#135).
    pub(crate) async fn leased_to(&mut self, store: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .leases_covering(store)
            .await?
            .into_iter()
            .find(|(role, lease)| *role == LeaseRole::Granted && lease.holds())
            .map(|(_, lease)| lease.holder))
    }
}

impl SurrealStore {
    /// Stores `lease` as `role`, in its own transaction.
    pub async fn put_lease(&self, role: LeaseRole, lease: &Lease) -> Result<(), WriteError> {
        let mut tx = self.begin().await?;
        tx.put_lease(role, lease).await?;
        tx.commit().await
    }

    /// The leases kept as `role`.
    pub async fn leases(&self, role: LeaseRole) -> Result<Vec<Lease>, StoreError> {
        let mut tx = self.begin().await?;
        tx.leases(role).await
    }
}
