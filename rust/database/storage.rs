use std::{error::Error, ops::Deref};

use bytes::{Bytes, BytesMut};
use pl_api_resource_name::ResourceName;

use crate::{
    counter::Counter,
    range::{RangeQuery, RangeQueryBuilder},
    Entity,
};

/// An abstract key-value storage system.
///
/// This trait is only used to control how we transact in
/// the storage system. The logic of how to manipulate data
/// is in [`StorageTransaction`].
#[async_trait::async_trait]
pub trait Storage: Sized + Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;
    type Transaction: StorageTransaction<Error = Self::Error>;

    /// Runs a [`Transactional`] against the storage system.
    ///
    /// Is responsability of this method to control retries and
    /// timeouts. The method should do its best to handle transient
    /// errors from the storage system/network.
    fn transact<T>(&self, transactional: T) -> Result<T::Output, Self::Error>
    where
        T: Transactional;
}

/// An abstract logic transactional operation.
///
/// Note that storages are allowed to assume that transactionals are
/// idempotent under database errors. This means that the operation
/// may be executed _multiple_ times. It also means that logic failures
/// inside the operation _can_ still cause the transaction to be
/// _committed_.
///
/// The recommended way to deal with this constraints is:
/// * Generate any database-independent data when _creating_ the
///   operation.
/// * Do all your data-dependent validations _before_ doing your
///   writes.
#[async_trait::async_trait]
pub trait Transactional {
    /// The output of this operation.
    type Output;

    /// Executes this operation inside the given transaction.
    ///
    /// As noted in the trait documentation, this method may be called
    /// multiple times if previous executions returned a database error.
    async fn transact<T>(&mut self, tx: &Transaction<T>) -> Result<Self::Output, T::Error>
    where
        T: StorageTransaction;
}

/// An abstract transaction.
///
/// This is how a storage system controls how application manipulate
/// data.
#[async_trait::async_trait]
pub trait StorageTransaction: Sized {
    /// The error returned by database operations.
    type Error: Error + Send + Sync + 'static;

    type GetValue: Deref<Target = [u8]>;
    type RangeValue: Deref<Target = [u8]>;
    type RangeValues: IntoIterator<Item = Self::RangeValue>;

    /// Get the given key from the database.
    ///
    /// Should return `None` if the key doesn't exist.
    async fn get(&self, key: &ResourceName) -> Result<Option<Self::GetValue>, Self::Error>;

    /// Get a range of values from the database.
    ///
    /// This method should take care of paginating data.
    async fn get_range(&self, range: RangeQuery<'_>) -> Result<Self::RangeValues, Self::Error>;

    /// Make the given key have the given value in the database.
    ///
    /// If the key was not previously present, it should be inserted.
    async fn set(&self, key: &ResourceName, value: Bytes) -> Result<(), Self::Error>;

    /// Remove the given key from the databsae.
    ///
    /// There should be no effect if the key was not previous present.
    async fn clear(&self, key: &ResourceName) -> Result<(), Self::Error>;

    /// Get the value of a counter.
    ///
    /// If the counter isn't present, the method should return `None`.
    async fn counter_get(&self, key: &ResourceName) -> Result<Option<u64>, Self::Error>;

    /// Atomically increment the value of a counter.
    ///
    /// If the counter didn't exist, this method should set its value to `count`.
    ///
    /// Storages should trive to implement this method in a conflict-free way.
    async fn counter_increment(&self, key: &ResourceName, count: u64) -> Result<(), Self::Error>;

    // TODO: what behavior we want when decrementing a zeroed counter?
    /// Atomically decrement the value of a counter.
    ///
    /// Storages should trive to implement this method in a conflict-free way.
    async fn counter_decrement(&self, key: &ResourceName, count: u64) -> Result<(), Self::Error>;
}

pub struct Transaction<T> {
    storage_transaction: T,
}

impl<T: StorageTransaction> Transaction<T> {
    /// Get the given key from the database.
    ///
    /// Should return `None` if the key doesn't exist.
    pub async fn get<E>(&self, key: &ResourceName) -> Result<Option<E>, T::Error>
    where
        E: Entity,
    {
        let bytes = self.storage_transaction.get(key).await?;

        Ok(bytes.as_deref().map(E::read))
    }

    /// Starts a builder for a range query.
    pub fn range<E>(&self) -> RangeQueryBuilder<'_, E, T> {
        RangeQueryBuilder::new(self)
    }

    /// Make the given key have the given value in the database.
    ///
    /// If the key was not previously present, it should be inserted.
    pub async fn set<E>(&self, key: &ResourceName, value: &E) -> Result<(), T::Error>
    where
        E: Entity,
    {
        let mut bytes = BytesMut::new();

        value.write(&mut bytes);

        self.storage_transaction.set(key, bytes.freeze()).await
    }

    /// Remove the given key from the databsae.
    ///
    /// There should be no effect if the key was not previous present.
    pub async fn clear(&self, key: &ResourceName) -> Result<(), T::Error> {
        self.storage_transaction.clear(key).await
    }

    /// Creates a new counter instance.
    pub fn counter(&self, key: ResourceName) -> Counter<'_, T> {
        Counter::new(key, &self.storage_transaction)
    }

    pub(crate) async fn get_range<E>(&self, range: RangeQuery<'_>) -> Result<Vec<E>, T::Error>
    where
        E: Entity,
    {
        let items = self.storage_transaction.get_range(range).await?;

        Ok(items.into_iter().map(|bytes| E::read(&*bytes)).collect())
    }
}
