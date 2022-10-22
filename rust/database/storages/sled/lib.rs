//! # Sled database storage
//!
//! Most of the time during development, there is no need to spin up
//! a large production-grade database. Each test needs to run in isolation
//! and even when running the server, e.g. when developing the web ui, we
//! only need something to store our data on.
//!
//! To improve productivity, make tests easier and faster, and reduce
//! hardware usage, this crate provides a backend for our database infrastructure
//! based on [sled](https://sled.rs). The backend tries its best to
//! simulate how FoundationDB behaves.
//!
//! Although not recommended, the backend also makes spinning-up a small
//! deployment of PL, as there is no need to fiddle with FoundationDB clusters.
use std::{future::Future, path::Path};

use pl_database_error::{DbError, InfallibleDbResult, StorageError};
use sled::{Config, Db};

mod transaction;

#[doc(inline)]
pub use self::transaction::{SledRange, SledTransaction};

/// A database implementation on top of [`sled`].
///
/// This should be used only for testing and development, as
/// we cannot give the same level of guarantees that FoundationDB
/// provides in their transactions.
///
/// [`sled`]: https://docs.rs/sled
pub struct SledDatabase {
    db: Db,
}

/// Openning methods.
impl SledDatabase {
    /// Opens a database at the given path.
    ///
    /// # Panics
    ///
    /// Panics if an I/O error occurs while opening the database.
    #[cold]
    pub fn open(path: &Path) -> Self {
        let db = Config::new()
            .use_compression(true)
            .path(path)
            .open()
            .unwrap_or_else(|err| {
                panic!("failed to open sled database at {}:\n{err}", path.display())
            });

        Self { db }
    }

    /// Creates a temporary isolated database.
    ///
    /// This can be used in tests to prevent conflicts and making things faster.
    ///
    /// # Panics
    ///
    /// Panics if an I/O error occurs while opening the database.
    #[cold]
    pub fn temporary() -> Self {
        let db = Config::new()
            .use_compression(true)
            .temporary(true)
            .create_new(true)
            .mode(sled::Mode::HighThroughput)
            .print_profile_on_drop(true)
            .open()
            .unwrap_or_else(|err| panic!("failed to open temp sled database:\n{err}"));

        Self { db }
    }
}

impl SledDatabase {
    /// Executes an future inside a transaction.
    ///
    /// The behavior of the transaction given as argument is the same
    /// as a FoundatioDB's transaction where all reads are snapshot reads,
    /// i.e. the transaction is not stricty serializable. This is a limitation
    /// of Sled, as it is wasn't designed with this purpose in mind.
    pub async fn transaction<T, E, F, Fut>(&self, f: F) -> Result<T, E>
    where
        F: Fn(&mut SledTransaction) -> Fut,
        Fut: Future<Output = Result<T, DbError<E>>>,
        E: From<StorageError> + std::error::Error,
    {
        let mut tx = SledTransaction::new(&*self.db);

        // No sled error is transient, no need for retry.
        let fut = async {
            let val = f(&mut tx).await?;

            sled_res_to_db_res(tx.commit().await)?;

            Ok(val)
        };

        match fut.await {
            Ok(val) => Ok(val),
            Err(DbError::Abort(err)) => Err(err),
            Err(DbError::Storage(err)) => Err(E::from(err)),
        }
    }
}

pub(crate) fn sled_res_to_db_res<T>(res: Result<T, sled::Error>) -> InfallibleDbResult<T> {
    match res {
        Ok(value) => Ok(value),
        Err(err) => Err(DbError::Storage(Box::new(err))),
    }
}
