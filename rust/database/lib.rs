use std::{future::Future, ops::Deref, path::Path};

use foundationdb::{future::FdbSlice, RangeOption};
use futures_util::TryStreamExt;
use pl_database_error::StorageError;
use pl_database_storages_foundationdb::{FdbDatabase, FdbTransaction};
use pl_database_storages_sled::{SledDatabase, SledTransaction};
use sled::IVec;

pub use pl_database_error::{DbError, DbResult, InfallibleDbResult};

/// An abstract key-value database.
///
/// How the database is implemented depends on what storage implementation is
/// choosen, currently we support sled and FoundationDB.
pub struct Db(DbInner);

enum DbInner {
    Embedded(SledDatabase),
    Fdb(FdbDatabase),
}

impl Db {
    /// Opens an embedded key-value database in the given path.
    ///
    /// If the path already exists, it will re-use the pre-existent data.
    /// If the path doesn't exists, it will be created.
    ///
    /// # Panics
    ///
    /// Panics if any I/O erorr occurs while opening the database.
    pub fn embedded(path: &Path) -> Self {
        Self(DbInner::Embedded(SledDatabase::open(path)))
    }

    /// Opens a temporary key-value database.
    ///
    /// This can be used in tests to improve isolation and performance.
    pub fn temporary() -> Self {
        Self(DbInner::Embedded(SledDatabase::temporary()))
    }

    /// Opens a database connected to a FoundationDB cluster.
    ///
    /// Configure how to connect to the cluster via the `FDB_*` environment
    /// variables.
    ///
    /// # Safety
    ///
    /// This function MUST only be called once per program execution. Multiple
    /// calls to the function may result in undefined behavior.
    ///
    /// Callers must ensure that the database instance is dropped before exiting
    /// the program.
    pub unsafe fn foundation() -> Self {
        let fdb = unsafe { FdbDatabase::new() };

        Self(DbInner::Fdb(fdb))
    }
}

impl Db {
    /// Executes a future inside a transaction.
    ///
    /// The behavior of the transaction is the same as a FoundationDB's transaction with
    /// snapshot reads, i.e. the transaction is serializable, but not strictly.
    pub async fn transaction<T, E, F, Fut>(&self, f: F) -> Result<T, E>
    where
        F: Fn(Tx) -> Fut,
        Fut: Future<Output = DbResult<(T, Tx), E>>,
        E: From<StorageError> + std::error::Error,
    {
        match &self.0 {
            DbInner::Embedded(sled_db) => {
                sled_db
                    .transaction(move |tx| {
                        let fut = f(Tx(TxInner::Embedded(tx)));

                        async move {
                            let (val, Tx(TxInner::Embedded(tx))) = fut.await? else {
                                unreachable!("invalid transaction type in sled database");
                            };

                            Ok((val, tx))
                        }
                    })
                    .await
            }
            DbInner::Fdb(fdb) => {
                fdb.transact(move |tx| {
                    let fut = f(Tx(TxInner::Fdb(tx)));

                    async move {
                        let (val, Tx(TxInner::Fdb(tx))) = fut.await? else {
                            unreachable!("invalid transaction type in fdb database");
                        };

                        Ok((val, tx))
                    }
                })
                .await
            }
        }
    }
}

/// A serializable database transaction.
///
/// It is not expected users to directly use methods in this type. Instead,
/// developers should just pass it to the layer they're using. The API of
/// this type is implemented doing the least amount of of work possible,
/// causing it to be highly complicated.
pub struct Tx(TxInner);

enum TxInner {
    Embedded(SledTransaction),
    Fdb(FdbTransaction),
}

impl Tx {
    /// Get a value of a key from the database and pass it to the given closure.
    pub async fn get(&self, key: &[u8]) -> InfallibleDbResult<Option<IBytes>> {
        let bytes = match &self.0 {
            TxInner::Embedded(sled_tx) => sled_tx.get(key)?.map(IBytes::embedded),
            TxInner::Fdb(fdb_tx) => fdb_tx.get(key).await?.map(IBytes::foundation),
        };

        Ok(bytes)
    }

    pub async fn for_each_in_range<F, E, Fut>(&self, opts: RangeOption<'_>, f: F) -> DbResult<(), E>
    where
        F: FnMut(&[u8], &[u8]) -> Fut,
        Fut: Future<Output = DbResult<bool, E>>,
        E: std::error::Error,
    {
        match &self.0 {
            TxInner::Embedded(sled_tx) => Self::sled_for_each_in_range(sled_tx, &opts, f).await,
            TxInner::Fdb(fdb_tx) => Self::fdb_for_each_in_range(fdb_tx, opts, f).await,
        }
    }

    async fn sled_for_each_in_range<F, E, Fut>(
        sled_tx: &SledTransaction,
        opts: &RangeOption<'_>,
        mut f: F,
    ) -> DbResult<(), E>
    where
        F: FnMut(&[u8], &[u8]) -> Fut,
        Fut: Future<Output = DbResult<bool, E>>,
        E: std::error::Error,
    {
        let range = sled_tx.get_range(opts);

        for next in range {
            let (key, value) = next?;
            if !f(&key, &value).await? {
                break;
            }
        }

        Ok(())
    }

    async fn fdb_for_each_in_range<F, E, Fut>(
        fdb_tx: &FdbTransaction,
        opts: RangeOption<'_>,
        mut f: F,
    ) -> DbResult<(), E>
    where
        F: FnMut(&[u8], &[u8]) -> Fut,
        Fut: Future<Output = DbResult<bool, E>>,
        E: std::error::Error,
    {
        use futures_util::future::{poll_fn, try_maybe_done, FusedFuture, FutureExt};
        use std::task::Poll;

        let mut stream = fdb_tx.get_range(opts);

        let mut curr = stream.try_next().await?;

        // Given that each key-value iteration may take arbitrary long time,
        // and assuming most executions will iterate over most part of the range,
        // it would be better to start fetching the next chunk as soon as we start
        // processing the current one. The poll_fn closure implements this logic.
        while let Some(values) = curr.take() {
            let mut get_next_chunk = stream.try_next();

            // Move values to inside process_curr_chunk to decrease change of having
            // multiple chunks in memory at once.
            let f_ref = &mut f;
            let process_curr_chunk = try_maybe_done(async move {
                for kv in values {
                    if !f_ref(kv.key(), kv.value()).await? {
                        return Ok(false);
                    }
                }

                Ok(true) as DbResult<bool, E>
            });

            futures_util::pin_mut!(process_curr_chunk);

            poll_fn(|cx| {
                // First, try poll current chunk processing. It will tell us if we
                // need to continue polling the next chunk.
                if !process_curr_chunk.is_terminated() {
                    // The result of poll doesn't matter, we only use the TryMaybeDone interface.
                    let _ = process_curr_chunk.poll_unpin(cx)?;
                }

                // We're still processing the current chunk, or we know we should continue
                // into the next one.
                if matches!(
                    process_curr_chunk.as_mut().output_mut().copied(),
                    None | Some(true)
                ) {
                    // XXX: What we should do in case this fails?
                    //
                    // The caller may or may not use this chunk, meaning that
                    // failing the entire loop due to this error doesn't make
                    // much sense.
                    curr = std::task::ready!(get_next_chunk.poll_unpin(cx))?;

                    if !process_curr_chunk.is_terminated() {
                        return Poll::Pending;
                    }
                }

                Poll::Ready(Ok(())) as Poll<DbResult<_, E>>
            })
            .await?;
        }

        Ok(())
    }

    /// Set a value of a given key.
    ///
    /// If the key wasn't present in the database, it will be added. If it was, its
    /// value will be replaced.
    pub fn set(&mut self, key: &[u8], value: &[u8]) {
        match &mut self.0 {
            TxInner::Embedded(sled_tx) => sled_tx.set(key, value),
            TxInner::Fdb(fdb_tx) => fdb_tx.set(key, value),
        }
    }

    /// Clear a key from the database.
    ///
    /// If the key didn't exist, nothing will be done.
    pub fn clear(&mut self, key: &[u8]) {
        match &mut self.0 {
            TxInner::Embedded(sled_tx) => sled_tx.clear(key),
            TxInner::Fdb(fdb_tx) => fdb_tx.clear(key),
        }
    }
}

/// A buffer from the database.
pub struct IBytes(IBytesBuf);

impl IBytes {
    fn embedded(bytes: IVec) -> Self {
        Self(IBytesBuf::Embedded(bytes))
    }

    fn foundation(bytes: FdbSlice) -> Self {
        Self(IBytesBuf::Fdb(bytes))
    }
}

impl Deref for IBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            IBytesBuf::Embedded(b) => b,
            IBytesBuf::Fdb(b) => b,
        }
    }
}

enum IBytesBuf {
    Embedded(IVec),
    Fdb(FdbSlice),
}

#[cfg(test)]
mod tests {
    use pl_api_status::{Status, StatusOr};

    use super::*;

    #[tokio::test]
    async fn test_db() -> StatusOr<()> {
        let db = Db::temporary();

        db.transaction(|mut tx| {
            Box::pin(async move {
                tx.set(b"foo/1", b"1");
                tx.set(b"foo/2", b"2");

                Ok(((), tx)) as DbResult<_, Status>
            })
        })
        .await?;

        db.transaction(|tx| {
            Box::pin(async move {
                let b = tx.get(b"foo/1").await?;

                assert_eq!(b.as_deref(), Some(b"1" as &[u8]));

                Ok(((), tx)) as DbResult<_, Status>
            })
        })
        .await
    }
}
