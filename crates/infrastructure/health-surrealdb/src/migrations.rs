use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use surrealdb::types::SurrealValue;

use crate::Connection;

/// One forward-only schema migration, embedded in the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    /// Its number; migrations apply in increasing order.
    pub version: u32,
    /// Its file name without extension, e.g. `0001_shared_tables`.
    pub name: &'static str,
    /// The SurrealQL it runs, inside one transaction with its `_migration` row.
    pub sql: &'static str,
}

impl Migration {
    /// The SHA-256 of the SurrealQL, hex-encoded, recorded in `_migration`.
    pub fn checksum(&self) -> String {
        Sha256::digest(self.sql.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// The platform's migrations, in order. The `_migration` ledger itself is created by [`migrate`].
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "0001_resource_tables",
        sql: include_str!("../migrations/0001_resource_tables.surql"),
    },
    Migration {
        version: 2,
        name: "0002_search_index",
        sql: include_str!("../migrations/0002_search_index.surql"),
    },
    Migration {
        version: 3,
        name: "0003_condition_lock",
        sql: include_str!("../migrations/0003_condition_lock.surql"),
    },
    Migration {
        version: 4,
        name: "0004_reference_index",
        sql: include_str!("../migrations/0004_reference_index.surql"),
    },
    Migration {
        version: 5,
        name: "0005_inbound",
        sql: include_str!("../migrations/0005_inbound.surql"),
    },
    Migration {
        version: 6,
        name: "0006_blocking_index",
        sql: include_str!("../migrations/0006_blocking_index.surql"),
    },
    Migration {
        version: 7,
        name: "0007_registry_review",
        sql: include_str!("../migrations/0007_registry_review.surql"),
    },
    Migration {
        version: 8,
        name: "0008_auth",
        sql: include_str!("../migrations/0008_auth.surql"),
    },
    Migration {
        version: 9,
        name: "0009_break_glass",
        sql: include_str!("../migrations/0009_break_glass.surql"),
    },
    Migration {
        version: 10,
        name: "0010_break_glass_review",
        sql: include_str!("../migrations/0010_break_glass_review.surql"),
    },
    Migration {
        version: 11,
        name: "0011_stock",
        sql: include_str!("../migrations/0011_stock.surql"),
    },
    Migration {
        version: 12,
        name: "0012_phone_sign_in",
        sql: include_str!("../migrations/0012_phone_sign_in.surql"),
    },
    Migration {
        version: 13,
        name: "0013_registration_review",
        sql: include_str!("../migrations/0013_registration_review.surql"),
    },
    Migration {
        version: 14,
        name: "0014_caregiver_grant",
        sql: include_str!("../migrations/0014_caregiver_grant.surql"),
    },
    Migration {
        version: 15,
        name: "0015_stock_transfers",
        sql: include_str!("../migrations/0015_stock_transfers.surql"),
    },
    Migration {
        version: 16,
        name: "0016_stock_counts",
        sql: include_str!("../migrations/0016_stock_counts.surql"),
    },
    Migration {
        version: 17,
        name: "0017_receiving",
        sql: include_str!("../migrations/0017_receiving.surql"),
    },
    Migration {
        version: 18,
        name: "0018_payments",
        sql: include_str!("../migrations/0018_payments.surql"),
    },
    Migration {
        version: 19,
        name: "0019_provenance",
        sql: include_str!("../migrations/0019_provenance.surql"),
    },
    Migration {
        version: 20,
        name: "0020_devices",
        sql: include_str!("../migrations/0020_devices.surql"),
    },
    Migration {
        version: 21,
        name: "0021_pins",
        sql: include_str!("../migrations/0021_pins.surql"),
    },
    Migration {
        version: 22,
        name: "0022_local_admin",
        sql: include_str!("../migrations/0022_local_admin.surql"),
    },
    Migration {
        version: 23,
        name: "0023_provisional",
        sql: include_str!("../migrations/0023_provisional.surql"),
    },
    Migration {
        version: 24,
        name: "0024_nodes",
        sql: include_str!("../migrations/0024_nodes.surql"),
    },
    Migration {
        version: 25,
        name: "0025_recovery_keys",
        sql: include_str!("../migrations/0025_recovery_keys.surql"),
    },
    Migration {
        version: 26,
        name: "0026_node_revocations",
        sql: include_str!("../migrations/0026_node_revocations.surql"),
    },
    Migration {
        version: 27,
        name: "0027_package_lifecycle",
        sql: include_str!("../migrations/0027_package_lifecycle.surql"),
    },
    Migration {
        version: 28,
        name: "0028_workflow",
        sql: include_str!("../migrations/0028_workflow.surql"),
    },
    Migration {
        version: 29,
        name: "0029_workflow_tasks",
        sql: include_str!("../migrations/0029_workflow_tasks.surql"),
    },
    Migration {
        version: 30,
        name: "0030_pricing",
        sql: include_str!("../migrations/0030_pricing.surql"),
    },
    Migration {
        version: 31,
        name: "0031_charges",
        sql: include_str!("../migrations/0031_charges.surql"),
    },
    Migration {
        version: 32,
        name: "0032_billing_accounts",
        sql: include_str!("../migrations/0032_billing_accounts.surql"),
    },
    Migration {
        version: 33,
        name: "0033_encounters_and_queues",
        sql: include_str!("../migrations/0033_encounters_and_queues.surql"),
    },
    Migration {
        version: 34,
        name: "0034_lab_tests",
        sql: include_str!("../migrations/0034_lab_tests.surql"),
    },
    Migration {
        version: 35,
        name: "0035_stock_reservations",
        sql: include_str!("../migrations/0035_stock_reservations.surql"),
    },
    Migration {
        version: 36,
        name: "0036_coverage",
        sql: include_str!("../migrations/0036_coverage.surql"),
    },
    Migration {
        version: 37,
        name: "0037_change_log",
        sql: include_str!("../migrations/0037_change_log.surql"),
    },
    Migration {
        version: 38,
        name: "0038_sync_apply",
        sql: include_str!("../migrations/0038_sync_apply.surql"),
    },
    Migration {
        version: 39,
        name: "0039_sync_peers",
        sql: include_str!("../migrations/0039_sync_peers.surql"),
    },
    Migration {
        version: 40,
        name: "0040_sync_conflicts",
        sql: include_str!("../migrations/0040_sync_conflicts.surql"),
    },
    Migration {
        version: 41,
        name: "0041_sync_horizon",
        sql: include_str!("../migrations/0041_sync_horizon.surql"),
    },
];

/// The ledger of applied migrations, defined before anything else runs.
const LEDGER: &str = "
DEFINE TABLE IF NOT EXISTS _migration SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS name ON _migration TYPE string;
DEFINE FIELD IF NOT EXISTS checksum ON _migration TYPE string;
DEFINE FIELD IF NOT EXISTS applied_at ON _migration TYPE datetime;
";

/// What [`migrate`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Versions already applied, with matching checksums.
    pub already_applied: Vec<u32>,
    /// Versions applied by this run.
    pub applied: Vec<u32>,
}

/// Why the database cannot be brought to the binary's schema. Every variant stops startup.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    /// The embedded list is not strictly increasing.
    #[error("migrations must be listed in strictly increasing order; {0} is out of order")]
    OutOfOrder(u32),
    /// An applied migration's SurrealQL was changed after it ran.
    #[error(
        "migration {version} ({name}) was modified after it was applied: recorded {recorded}, embedded {embedded}"
    )]
    Modified {
        /// The migration number.
        version: u32,
        /// Its name.
        name: &'static str,
        /// The checksum recorded when it ran.
        recorded: String,
        /// The checksum of the embedded SurrealQL.
        embedded: String,
    },
    /// The database has a migration this binary does not know: the binary is older than the data.
    #[error("the database has migration {version} ({name}), which this binary does not include")]
    Unknown {
        /// The migration number.
        version: u32,
        /// The recorded name.
        name: String,
    },
    /// A migration's SurrealQL failed; nothing from it was applied.
    #[error("migration {version} ({name}) failed: {source}")]
    Failed {
        /// The migration number.
        version: u32,
        /// Its name.
        name: &'static str,
        /// The database error.
        source: Box<surrealdb::Error>,
    },
    /// The ledger could not be created or read.
    #[error("cannot read the migration ledger: {0}")]
    Ledger(Box<surrealdb::Error>),
}

impl From<surrealdb::Error> for MigrationError {
    fn from(error: surrealdb::Error) -> Self {
        Self::Ledger(Box::new(error))
    }
}

#[derive(Debug, SurrealValue)]
struct Applied {
    version: i64,
    name: String,
    checksum: String,
}

/// Applies every pending migration in order, each in its own transaction with its ledger row.
/// It first checks that every applied migration still matches the binary, so a modified or
/// unknown migration stops startup before anything changes.
pub async fn migrate(
    connection: &Connection,
    migrations: &[Migration],
) -> Result<MigrationReport, MigrationError> {
    for pair in migrations.windows(2) {
        if let [a, b] = pair
            && b.version <= a.version
        {
            return Err(MigrationError::OutOfOrder(b.version));
        }
    }
    let db = connection.db();
    db.query(LEDGER).await?.check()?;
    let applied: Vec<Applied> = db
        .query("SELECT record::id(id) AS version, name, checksum FROM _migration ORDER BY version")
        .await?
        .check()?
        .take(0)?;
    let applied: BTreeMap<u32, Applied> = applied
        .into_iter()
        .filter_map(|row| u32::try_from(row.version).ok().map(|v| (v, row)))
        .collect();

    let embedded: BTreeMap<u32, &Migration> = migrations.iter().map(|m| (m.version, m)).collect();
    let mut report = MigrationReport::default();
    for (version, row) in &applied {
        let Some(migration) = embedded.get(version) else {
            return Err(MigrationError::Unknown {
                version: *version,
                name: row.name.clone(),
            });
        };
        let checksum = migration.checksum();
        if row.checksum != checksum {
            return Err(MigrationError::Modified {
                version: *version,
                name: migration.name,
                recorded: row.checksum.clone(),
                embedded: checksum,
            });
        }
        report.already_applied.push(*version);
    }

    for migration in migrations
        .iter()
        .filter(|m| !applied.contains_key(&m.version))
    {
        let failed = |source: surrealdb::Error| MigrationError::Failed {
            version: migration.version,
            name: migration.name,
            source: Box::new(source),
        };
        let sql = format!(
            "BEGIN TRANSACTION;\n{}\n;CREATE _migration:{} SET name = $name, checksum = $checksum, applied_at = time::now();\nCOMMIT TRANSACTION;",
            migration.sql, migration.version
        );
        db.query(sql)
            .bind(("name", migration.name))
            .bind(("checksum", migration.checksum()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(failed)?;
        report.applied.push(migration.version);
    }
    Ok(report)
}
