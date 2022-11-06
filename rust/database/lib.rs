use std::{future::Future, ops::Deref, path::Path};

use foundationdb::RangeOption;
use pl_database_error::{DbResult, InfallibleDbResult, StorageError};
use pl_database_storages_sled::{SledDatabase, SledTransaction};
use sled::IVec;

/// An abstract key-value database.
///
/// How the database is implemented depends on what storage implementation is
/// choosen, currently we support sled and FoundationDB.
pub struct Db(DbInner);

enum DbInner {
    Embedded(SledDatabase),
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
                        let tx = Tx(TxInner::Embedded(tx));
                        let fut = f(tx);

                        async move {
                            let (val, tx) = fut.await?;

                            match tx.0 {
                                TxInner::Embedded(tx) => Ok((val, tx)),
                            }
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
}

impl Tx {
    /// Get a value of a key from the database and pass it to the given closure.
    ///
    /// We pass the value via closure to prevent having to copy the bytes returned
    /// by the storage into a temporary buffer. Layers are responsible to implement
    /// the correct decoding.
    pub async fn get(&self, key: &[u8]) -> InfallibleDbResult<Option<IBytes>> {
        match &self.0 {
            TxInner::Embedded(sled_tx) => {
                let Some(bytes) = sled_tx.get(key)? else {
                    return Ok(None)
                };

                Ok(Some(IBytes::embedded(bytes)))
            }
        }
    }

    /// Get a range of key value pairs given the query options.
    ///
    /// Same as [`Tx::get`], we pass a closure to prevent having to copy the storage's bytes.
    pub async fn get_range<'b>(
        &'b self,
        opts: &RangeOption<'_>,
    ) -> Box<dyn Iterator<Item = InfallibleDbResult<(IBytes, IBytes)>> + 'b> {
        match &self.0 {
            TxInner::Embedded(sled_tx) => {
                let range = sled_tx.get_range(opts);
                let kvs = range
                    .map(move |res| res.map(|(k, v)| (IBytes::embedded(k), IBytes::embedded(v))));

                Box::new(kvs)
            }
        }
    }

    /// Set a value of a given key.
    ///
    /// If the key wasn't present in the database, it will be added. If it was, its
    /// value will be replaced.
    pub async fn set(&mut self, key: &[u8], value: &[u8]) {
        match &mut self.0 {
            TxInner::Embedded(sled_tx) => sled_tx.set(key, value),
        }
    }

    /// Clear a key from the database.
    ///
    /// If the key didn't exist, nothing will be done.
    pub async fn clear(&mut self, key: &[u8]) {
        match &mut self.0 {
            TxInner::Embedded(sled_tx) => sled_tx.clear(key),
        }
    }
}

/// A buffer from the database.
pub struct IBytes(IBytesBuf);

impl IBytes {
    fn embedded(bytes: IVec) -> Self {
        Self(IBytesBuf::Embedded(bytes))
    }
}

impl Deref for IBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            IBytesBuf::Embedded(b) => b,
        }
    }
}

enum IBytesBuf {
    Embedded(IVec),
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
                tx.set(b"foo/1", b"1").await;
                tx.set(b"foo/2", b"2").await;

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
