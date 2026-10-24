//! A throwaway Postgres database per test, for the `vitni-db` and `vitni-app` integration tests.
//!
//! One Postgres container is started per Docker container name and kept running between tests and
//! test processes; each [`PostgresTestDb::create`] makes a uniquely named database inside it, and
//! dropping the guard drops that database. Dev-dependency only: it needs a Docker daemon and is
//! never linked into a shipped binary.
//!
//! # Licence
//!
//! `AGPL-3.0-or-later` (ADR 0034). Additional permission under GNU AGPL version 3 section 7: if you
//! modify this Program, or any covered work, by combining it with a WebAssembly component that
//! interacts with the Program solely through the versioned `vitni:host-api` WIT world (or any later
//! version of that world), the licensor grants you additional permission to convey the resulting
//! work. Such a component is not required to be licensed under the GNU AGPL.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use sqlx::{Connection, PgConnection};
use testcontainers::core::IntoContainerPort;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ImageExt, ReuseDirective};
use testcontainers_modules::postgres::Postgres;
use tokio::sync::OnceCell;
use uuid::Uuid;

const IMAGE_TAG: &str = "18-alpine";
const PG_PORT: u16 = 5432;
const SHM_SIZE_BYTES: u64 = 600 * 1024 * 1024;
/// Tuned for speed: durability is irrelevant for a database that lives for one test.
const SERVER_ARGS: [&str; 12] = [
    "-c",
    "max_connections=100",
    "-c",
    "shared_buffers=32MB",
    "-c",
    "work_mem=1MB",
    "-c",
    "maintenance_work_mem=8MB",
    "-c",
    "fsync=off",
    "-c",
    "synchronous_commit=off",
];

/// Admin DSN of the running container, per container name, resolved once per process.
static ADMIN_DSNS: LazyLock<Mutex<HashMap<String, Arc<OnceCell<String>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Why a test database could not be created.
#[derive(Debug, thiserror::Error)]
pub enum TestDbError {
    /// The Postgres container could not be started or its port looked up (is Docker running?).
    #[error("could not start or reach the Postgres container `{container}`: {source}")]
    Container {
        /// The Docker container name asked for.
        container: String,
        /// The testcontainers failure.
        source: testcontainers::TestcontainersError,
    },
    /// Creating the database inside the container failed.
    #[error("could not create test database `{database}`: {source}")]
    CreateDatabase {
        /// The generated database name.
        database: String,
        /// The sqlx failure.
        source: sqlx::Error,
    },
}

/// A freshly created, isolated Postgres database; dropped from the server when this guard drops.
#[derive(Debug)]
pub struct PostgresTestDb {
    admin_dsn: String,
    database: String,
    dsn: String,
}

impl PostgresTestDb {
    /// Creates an empty database in the shared container named `container_name`, starting the
    /// container on first use.
    ///
    /// # Errors
    ///
    /// Returns [`TestDbError`] when Docker is unavailable or the database cannot be created.
    pub async fn create(container_name: &str) -> Result<Self, TestDbError> {
        let admin_dsn = admin_dsn(container_name).await?;
        let database = format!("test_{}", Uuid::now_v7().simple());
        let create_failed = |source| TestDbError::CreateDatabase {
            database: database.clone(),
            source,
        };

        let mut connection = PgConnection::connect(&admin_dsn).await.map_err(create_failed)?;
        sqlx::query(&format!("CREATE DATABASE \"{database}\""))
            .execute(&mut connection)
            .await
            .map_err(create_failed)?;

        let dsn = format!("{}/{database}", admin_dsn.trim_end_matches("/postgres"));
        Ok(Self {
            admin_dsn,
            database,
            dsn,
        })
    }

    /// The connection string of this test database.
    #[must_use]
    pub fn dsn(&self) -> &str {
        &self.dsn
    }
}

impl Drop for PostgresTestDb {
    fn drop(&mut self) {
        let admin_dsn = self.admin_dsn.clone();
        let database = self.database.clone();
        // A dedicated thread and runtime, joined here: the test's own runtime may be shutting down,
        // and a spawned cleanup task would then be lost, leaking the database.
        let cleanup = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
            runtime.block_on(drop_database(&admin_dsn, &database))
        });
        match cleanup.join() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::warn!(database = %self.database, %error, "could not drop test database"),
            Err(_panic) => tracing::warn!(database = %self.database, "test database cleanup panicked"),
        }
    }
}

async fn drop_database(admin_dsn: &str, database: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut connection = PgConnection::connect(admin_dsn).await?;
    sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1")
        .bind(database)
        .execute(&mut connection)
        .await?;
    sqlx::query(&format!("DROP DATABASE \"{database}\""))
        .execute(&mut connection)
        .await?;
    Ok(())
}

async fn admin_dsn(container_name: &str) -> Result<String, TestDbError> {
    let cell = {
        let mut dsns = ADMIN_DSNS.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(dsns.entry(container_name.to_owned()).or_default())
    };
    let dsn = cell
        .get_or_try_init(|| async {
            let container = Postgres::default()
                .with_tag(IMAGE_TAG)
                .with_container_name(container_name)
                .with_cmd(SERVER_ARGS)
                .with_shm_size(SHM_SIZE_BYTES)
                .with_reuse(ReuseDirective::Always)
                .start()
                .await?;
            let port = container.get_host_port_ipv4(PG_PORT.tcp()).await?;
            Ok(format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres"))
        })
        .await
        .map_err(|source| TestDbError::Container {
            container: container_name.to_owned(),
            source,
        })?;
    Ok(dsn.clone())
}
