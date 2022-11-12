//! # FoundationDB database storage
//!
//! This crate provides a storage backend for `//rust/database` based
//! on Apple's FoundationDB.
//!
//! Using this backend ensures a PL deployment can have high-availability
//! and scalability.
use std::future::Future;

use foundationdb::{
    future::{FdbSlice, FdbValues},
    FdbError, RangeOption, Transaction,
};
use futures_util::{Stream, TryStreamExt};
use pl_database_error::{DbError, DbResult, InfallibleDbResult, StorageError};

/// A database interface into a [FoundationDB] cluster.
///
/// Connection details are configurated via the standard `FDB_*`
/// environment variables.
///
/// [FoundationDB]: https://apple.github.io/foundationdb
pub struct FdbDatabase {
    db: foundationdb::Database,
    _nas: foundationdb::api::NetworkAutoStop,
}

impl FdbDatabase {
    const DEFAULT_RETRY_LIMIT: u8 = 5;

    /// Create a new database connection with FoundationDB.
    ///
    /// This can only be executed once per executable.
    ///
    /// # Safety
    ///
    /// Due to how the C API is implemented, this function may only be
    /// called _once_ per binary. Calling it more than once is illegal
    /// and may result in undefined behavior.
    ///
    /// Also, you must ensure the returned instance is fully dropped before
    /// exiting the program.
    pub unsafe fn new() -> Self {
        let network_auto_stop = unsafe { foundationdb::boot() };

        let db =
            foundationdb::Database::default().expect("failed to build foundationdb connection");

        Self {
            db,
            _nas: network_auto_stop,
        }
    }

    /// Executes a closure inside a FoundationDB transaction.
    ///
    /// The closure _MUST_ be idempotent and may be called more than once
    /// in case of failures. We constraint closures to [`Fn`] exactly for
    /// this reason. Note that this constraint is not enough to catch all
    /// cases, take care to not do uncontrolled side-effects inside such
    /// closures.
    ///
    /// # Errors
    ///
    /// This function returns an error if the closure returns an abort
    /// error, if we fail the transaction more than we can retry or if
    /// the FoundationDB API returns a non-retryable error.
    pub async fn transact<F, T, E, Fut>(&self, f: F) -> DbResult<T, E>
    where
        F: Fn(FdbTransaction) -> Fut,
        Fut: Future<Output = (DbResult<T, E>, FdbTransaction)>,
        E: From<StorageError> + std::error::Error,
    {
        // Reimplement the loop of foundationdb::Database::transact to prevent
        // lifetime issues.
        //
        // The current interface of transact would force either the closure to be 'static,
        // or FdbTransaction to have a lifetime. Both of these are not what we want. To
        // prevent this issue, reimplement the transaction flow, that is quite simple.
        //
        // An advantage of reimplementing the flow is that we will be able to improve database
        // instrumentation later on.
        //
        // ## Implementation Details
        //
        // FoundationDB Transactions know if they can be retried given a specific FdbError,
        // as retries are common when using FDB. The loop thus can use this mechanism to
        // handle most errors.

        let to_db_error = fdb_error_to_db_error::<E>;

        let mut tx = self.db.create_trx().map_err(to_db_error)?;

        let mut remaining_tries = Self::DEFAULT_RETRY_LIMIT;
        let mut can_retry = || {
            remaining_tries -= 1;
            remaining_tries > 0
        };

        loop {
            let (res, FdbTransaction(rtx)) = f(FdbTransaction(tx)).await;
            tx = rtx;

            match res {
                Ok(val) => match tx.commit().await {
                    Ok(_) => break Ok(val),
                    // The commit returned an error, this MAY indicate that we need
                    // to retry the transaction, but we could also be in an unknown
                    // state.
                    Err(err) => {
                        // Here the original loop checks for is_idempotent || !err.is_maybe_committed().
                        // As we assume all of our transactions are idempotent, we can skip the check.
                        if can_retry() {
                            tx = err.on_error().await.map_err(to_db_error)?;
                        } else {
                            break Err(fdb_error_to_db_error(err.into()));
                        }
                    }
                },
                // The user asked us to abort the transaction.
                // We don't need to rollback it manually, the Drop impl for tx
                // takes care of dispatching the rollback.
                Err(DbError::Abort(err)) => break Err(DbError::Abort(err)),
                Err(DbError::Storage(err)) => {
                    let fdb_err = err
                        .downcast::<FdbError>()
                        .expect("Non-FdbError storage error inside FdbTransaction");

                    // Here the original loop checks for is_idempotent || !err.is_maybe_committed().
                    // As we assume all of our transactions are idempotent, we can skip the check.
                    if can_retry() {
                        tx = tx.on_error(*fdb_err).await.map_err(to_db_error)?;
                    } else {
                        break Err(DbError::Storage(fdb_err));
                    }
                }
            }
        }
    }
}

/// A transaction inside a FoundationDB cluster.
///
/// This is a strictly serializable transaction.
pub struct FdbTransaction(Transaction);

impl FdbTransaction {
    /// Get the value of a specific key from the remote database.
    ///
    /// Returns `None` if the key is not present.
    ///
    /// # Errors
    ///
    /// Returns a storage error if something happens while fetching
    /// the data.
    pub async fn get(&self, key: &[u8]) -> InfallibleDbResult<Option<FdbSlice>> {
        self.0.get(key, false).await.map_err(fdb_error_to_db_error)
    }

    /// Get a range of key-value pairs from the remote database.
    ///
    /// This method return chunks of the range. Stop consuming the
    /// stream to stop fetching remaining chunks.
    ///
    /// # Errors
    ///
    /// Returns a storage error if something happens while fetching
    /// the range iterations.
    pub fn get_range<'b>(
        &'b self,
        opts: RangeOption<'b>,
    ) -> impl Stream<Item = InfallibleDbResult<FdbValues>> + Send + Sync + Unpin + 'b {
        let fdb_stream = self.0.get_ranges(opts, false);

        fdb_stream.map_err(fdb_error_to_db_error)
    }

    /// Set a value of a key in the database.
    ///
    /// If the key is present, its value will be replaced. If it is not
    /// present, it will be added.
    pub fn set(&mut self, key: &[u8], value: &[u8]) {
        self.0.set(key, value)
    }

    /// Remove a key from the database.
    ///
    /// This is a no-op if the key is not present.
    pub fn clear(&mut self, key: &[u8]) {
        self.0.clear(key)
    }
}

fn fdb_error_to_db_error<E>(err: FdbError) -> DbError<E> {
    DbError::Storage(Box::new(err))
}

// TODO(fdb): write tests for this crate after we write fdb infrastructure.
