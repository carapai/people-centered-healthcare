use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use surrealdb::Surreal;
use surrealdb::engine::any::{self, Any};
use surrealdb::opt::auth::Root;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// The most transactions one connection runs at once. Each runs in a server session of its own
/// (the SDK's `begin` takes a cloned handle, and every clone attaches a session), and SurrealDB
/// refuses more than `SURREAL_WEBSOCKET_MAX_ATTACHED_SESSIONS` (256) on one WebSocket with
/// "Method not allowed". Sessions are pooled and reused, and past this many a transaction waits
/// for another to finish.
pub const MAX_CONCURRENT_TRANSACTIONS: usize = 128;

/// Where the node's database lives.
#[derive(Clone)]
pub enum Target {
    /// Embedded SurrealKV in this process (Facility Edge Node, standalone desktop; ADR 0060).
    /// Always opened with `sync=every`, so a commit's write-ahead log is on disk before it is
    /// acknowledged.
    SurrealKv {
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
            Self::SurrealKv { path } => f.debug_struct("SurrealKv").field("path", path).finish(),
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
            Self::SurrealKv { path } => format!("surrealkv://{}?sync=every", path.display()),
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

/// An open database handle, cheap to clone: clones share one session, and transactions borrow
/// theirs from a bounded pool (see [`MAX_CONCURRENT_TRANSACTIONS`]).
#[derive(Debug, Clone)]
pub struct Connection {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    db: Surreal<Any>,
    idle: Mutex<Vec<Surreal<Any>>>,
    permits: Arc<Semaphore>,
}

/// A transaction's hold on a pooled session; dropping it frees the slot.
#[derive(Debug)]
pub(crate) struct Lease {
    inner: Arc<Inner>,
    _permit: OwnedSemaphorePermit,
}

impl Lease {
    /// Returns a session whose transaction has finished, for the next one to reuse.
    pub(crate) fn give_back(self, db: Surreal<Any>) {
        self.inner
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(db);
    }
}

impl Connection {
    /// Opens the database and selects the namespace and database.
    pub async fn open(settings: &Settings) -> Result<Self, ConnectionError> {
        let engine_enabled = match &settings.target {
            Target::SurrealKv { .. } => cfg!(feature = "embedded").then_some(()).ok_or("SurrealKV"),
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
        Ok(Self {
            inner: Arc::new(Inner {
                db,
                idle: Mutex::new(Vec::new()),
                permits: Arc::new(Semaphore::new(MAX_CONCURRENT_TRANSACTIONS)),
            }),
        })
    }

    /// The SDK handle, for this crate's repositories and migrations.
    pub(crate) fn db(&self) -> &Surreal<Any> {
        &self.inner.db
    }

    /// A session for one transaction: an idle one, or a new clone while under the cap; waits
    /// when every slot is taken.
    pub(crate) async fn session(&self) -> (Surreal<Any>, Lease) {
        let permit = Arc::clone(&self.inner.permits)
            .acquire_owned()
            .await
            .expect("the semaphore is never closed");
        let idle = self
            .inner
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop();
        let db = idle.unwrap_or_else(|| self.inner.db.clone());
        (
            db,
            Lease {
                inner: Arc::clone(&self.inner),
                _permit: permit,
            },
        )
    }

    /// A snapshot-consistent logical export of this database as SurrealQL (backup-and-recovery.md).
    /// Supported by the embedded engines (edges, desktops, tests), not over WebSocket.
    pub async fn export_sql(&self) -> Result<Vec<u8>, surrealdb::Error> {
        use futures_util::StreamExt;
        let mut stream = self.inner.db.export(()).await?;
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk?);
        }
        Ok(bytes)
    }

    /// Restores an [`export_sql`](Self::export_sql) into this (empty) database. The text is run
    /// in memory, so a decrypted backup never touches the disk.
    pub async fn import_sql(&self, sql: &str) -> Result<(), surrealdb::Error> {
        self.inner.db.query(sql).await?.check()?;
        Ok(())
    }

    /// Runs SurrealQL and returns the raw response, for tests and tooling such as
    /// `INFO FOR DB` comparisons.
    pub async fn query(&self, sql: &str) -> Result<surrealdb::IndexedResults, surrealdb::Error> {
        self.inner.db.query(sql).await?.check()
    }
}

/// Opens the node's database (see [`Connection::open`]).
pub async fn connect(settings: &Settings) -> Result<Connection, ConnectionError> {
    Connection::open(settings).await
}
