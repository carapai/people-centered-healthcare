use std::fmt;
use std::path::PathBuf;

use surrealdb::Surreal;
use surrealdb::engine::any::{self, Any};
use surrealdb::opt::auth::Root;

/// Where the node's database lives.
#[derive(Clone)]
pub enum Target {
    /// Embedded RocksDB in this process (Facility Edge Node, standalone desktop). Always opened
    /// with `sync=every`, so a commit is on disk before it is acknowledged.
    ///
    /// The RocksDB block cache is capped through the service environment
    /// (`SURREAL_ROCKSDB_BLOCK_CACHE_SIZE`, in bytes), set by the edge and desktop installers.
    RocksDb {
        /// The data directory.
        path: PathBuf,
    },
    /// A SurrealDB server (central), e.g. `ws://127.0.0.1:8000`.
    Remote {
        /// The server endpoint.
        endpoint: String,
        /// The root user.
        username: String,
        /// The root password.
        password: String,
    },
    /// The in-memory engine, for tests only.
    Memory,
}

impl fmt::Debug for Target {
    // Never prints credentials.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RocksDb { path } => f.debug_struct("RocksDb").field("path", path).finish(),
            Self::Remote { endpoint, .. } => f
                .debug_struct("Remote")
                .field("endpoint", endpoint)
                .finish_non_exhaustive(),
            Self::Memory => f.write_str("Memory"),
        }
    }
}

impl Target {
    /// The engine address passed to the SDK.
    pub fn address(&self) -> String {
        match self {
            Self::RocksDb { path } => format!("rocksdb://{}?sync=every", path.display()),
            Self::Remote { endpoint, .. } => endpoint.clone(),
            Self::Memory => "mem://".to_owned(),
        }
    }
}

/// Connection settings: one namespace per tenant and one database per node (persistence.md).
#[derive(Debug, Clone)]
pub struct Settings {
    /// Where the database lives.
    pub target: Target,
    /// The tenant namespace, e.g. `ug`.
    pub namespace: String,
    /// The node's database, normally `main`.
    pub database: String,
}

/// Why the database could not be opened.
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    /// The engine named by the target is not compiled into this binary.
    #[error("the {0} engine is not enabled in this build")]
    EngineNotEnabled(&'static str),
    /// The SDK failed to connect, authenticate or select the namespace.
    #[error("cannot open the database: {0}")]
    Open(#[from] surrealdb::Error),
}

/// An open database handle, cheap to clone.
#[derive(Debug, Clone)]
pub struct Connection {
    db: Surreal<Any>,
}

impl Connection {
    /// Opens the database and selects the namespace and database.
    pub async fn open(settings: &Settings) -> Result<Self, ConnectionError> {
        let engine_enabled = match &settings.target {
            Target::RocksDb { .. } => cfg!(feature = "embedded").then_some(()).ok_or("RocksDB"),
            Target::Remote { .. } => cfg!(feature = "remote").then_some(()).ok_or("remote"),
            Target::Memory => Ok(()),
        };
        engine_enabled.map_err(ConnectionError::EngineNotEnabled)?;
        let db = any::connect(settings.target.address()).await?;
        if let Target::Remote {
            username, password, ..
        } = &settings.target
        {
            db.signin(Root {
                username: username.clone(),
                password: password.clone(),
            })
            .await?;
        }
        db.use_ns(settings.namespace.as_str())
            .use_db(settings.database.as_str())
            .await?;
        Ok(Self { db })
    }

    /// The SDK handle, for this crate's repositories and migrations.
    pub(crate) fn db(&self) -> &Surreal<Any> {
        &self.db
    }

    /// Runs SurrealQL and returns the raw response, for tests and tooling such as
    /// `INFO FOR DB` comparisons.
    pub async fn query(&self, sql: &str) -> Result<surrealdb::IndexedResults, surrealdb::Error> {
        self.db.query(sql).await?.check()
    }
}

/// Opens the node's database (see [`Connection::open`]).
pub async fn connect(settings: &Settings) -> Result<Connection, ConnectionError> {
    Connection::open(settings).await
}
