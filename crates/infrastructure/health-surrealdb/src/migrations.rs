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
