//! One transaction spanning work written against a [`Pool`] (ADR 0044 §2).
//!
//! A replace deletes the log, inserts the archive's rows and rebuilds every projection. The rebuild
//! runs through `cqrs-es` repositories that take a `Pool`, not a transaction, so the swap cannot be
//! wrapped in a `sqlx::Transaction`. Instead it runs on a pool of exactly one connection, on which a
//! transaction is opened through the engine's [`TransactionManager`]. `sqlx` counts the open
//! transaction on that connection, so it survives the connection going back to the pool between
//! statements, and every `pool.begin()` inside the work becomes a savepoint, not a second `BEGIN`.

use std::time::Duration;

use sqlx::pool::PoolOptions;
use sqlx::{Database, Pool, TransactionManager};

use crate::store::DbError;

/// How many events a replay over a shared pool reads ahead of the projection writes (the
/// `sqlite-es`/`postgres-es` default).
pub(crate) const SHARED_REPLAY_BUFFER: usize = 200;

/// How many events a replay over an [`ExclusivePool`] reads ahead: all of them. The replay's reader
/// holds the pool's only connection until it has read its last row, while each projection write
/// waits for that same connection, so a bounded read-ahead would stall both once it filled. The
/// value is tokio's largest channel capacity (`Semaphore::MAX_PERMITS`); the channel allocates only
/// for the events actually buffered.
pub(crate) const EXCLUSIVE_REPLAY_BUFFER: usize = usize::MAX >> 3;

/// How long work on an [`ExclusivePool`] waits for its connection. A replay's reader holds it while
/// it reads an aggregate's whole log, which on a large workspace outlasts `sqlx`'s 30-second default;
/// a projection write timing out then is swallowed by `cqrs-es`, leaving that record out of the
/// rebuilt projection.
const EXCLUSIVE_ACQUIRE_TIMEOUT: Duration = Duration::from_hours(24);

/// A one-connection pool to the same database as `pool`, holding one open transaction.
pub(crate) struct ExclusivePool<DB: Database> {
    pool: Pool<DB>,
}

impl<DB: Database> ExclusivePool<DB> {
    /// Connects one more connection to `pool`'s database and opens a transaction on it.
    ///
    /// The connection never expires while idle or by age, so the pool cannot swap it for a fresh one
    /// outside the transaction.
    pub(crate) async fn begin(pool: &Pool<DB>) -> Result<Self, DbError> {
        let options = (*pool.connect_options()).clone();
        let pool = PoolOptions::<DB>::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .acquire_timeout(EXCLUSIVE_ACQUIRE_TIMEOUT)
            .connect_with(options)
            .await
            .map_err(backend("connecting for an exclusive transaction"))?;
        let mut conn = pool
            .acquire()
            .await
            .map_err(backend("acquiring the exclusive connection"))?;
        DB::TransactionManager::begin(&mut conn, None)
            .await
            .map_err(backend("beginning the exclusive transaction"))?;
        drop(conn);
        Ok(Self { pool })
    }

    /// The pool to run the transaction's work on.
    pub(crate) fn pool(&self) -> &Pool<DB> {
        &self.pool
    }

    /// Commits the work when `outcome` is `Ok`, otherwise rolls it back and returns its error.
    ///
    /// # Errors
    ///
    /// `outcome`'s own error, or [`DbError::Backend`] when the connection no longer holds the
    /// transaction (it was lost mid-way) or the commit fails.
    pub(crate) async fn finish<T>(self, outcome: Result<T, DbError>) -> Result<T, DbError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(backend("acquiring the exclusive connection"))?;
        if DB::TransactionManager::get_transaction_depth(&conn) != 1 {
            return Err(DbError::Backend(
                "the exclusive transaction was lost before it could finish".to_owned(),
            ));
        }
        let finished = match outcome {
            Ok(value) => DB::TransactionManager::commit(&mut conn)
                .await
                .map(|()| value)
                .map_err(backend("committing the exclusive transaction")),
            Err(error) => {
                DB::TransactionManager::rollback(&mut conn)
                    .await
                    .map_err(backend("rolling back the exclusive transaction"))?;
                Err(error)
            }
        };
        drop(conn);
        self.pool.close().await;
        finished
    }
}

fn backend(what: &'static str) -> impl Fn(sqlx::Error) -> DbError {
    move |e| DbError::Backend(format!("{what}: {e}"))
}
