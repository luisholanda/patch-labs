//! # Database system.
//!
//! This crate provides a generic interface for a key-value
//! database store. The interface is generic over the underlying
//! storage, meaning that application code can be easily tested
//! via in-memory databases.
use std::{
    future::{Future, IntoFuture},
    ops::{Bound, Deref},
    pin::Pin,
};

use pl_resource_name::{ResourceName, ResourceNameView};
use prost::Message;

/// A key-value database generic over its storage system.
pub struct Database<S> {
    storage: S,
}

impl<S> Database<S> {
    pub fn new(storage: S) -> Self {
        Self { storage }
    }
}

impl<S: Storage> Database<S> {
    /// Get a resource from the database given its key.
    ///
    /// Returns `None` if we don't find the key in the storage.
    pub async fn get<R>(&self, key: impl Key) -> Option<R>
    where
        R: Message + Default,
    {
        let bytes = self.storage.get(key.bytes()).await?;

        let resource = R::decode(&*bytes).expect("failed to decode resource");

        Some(resource)
    }

    // TODO: Get key from `resource`
    //
    // This would require a `Resource` trait, but this would require a custom
    // codegen process.
    /// Sets the given resource as the value for the given key.
    ///
    /// If a value is already present in the storage, it will be overriden.
    pub async fn set<R>(&self, key: impl Key, resource: &R)
    where
        R: Message,
    {
        let mut buf = vec![];

        resource.encode(&mut buf).unwrap();

        self.storage.set(key.bytes(), &buf).await;
    }

    /// Remove the given key from the storage.
    ///
    /// If the key is not on it, nothing will be done.
    pub async fn remove(&self, key: impl Key) {
        self.storage.remove(key.bytes()).await
    }

    /// Starts a fluet-style builder for range queries.
    pub fn range(&self) -> RangeQueryBuilder<'_, S, (), ()> {
        RangeQueryBuilder {
            start: Bound::Unbounded,
            end: Bound::Unbounded,
            limit: None,
            reverse: false,
            storage: &self.storage,
        }
    }
}

// TODO: handle errors
/// A database storage system.
///
/// By abstracting over the storage system, we can use multiple
/// databases as our data backend. Any database that supports key-value
/// model (i.e. all of them) can be used as a storage system.
#[async_trait::async_trait]
pub trait Storage {
    /// The type of key returned by the storage system.
    type Key: Deref<Target = [u8]>;
    /// The type of values returned by the storage system.
    type Bytes: Deref<Target = [u8]>;

    /// Associates the key with the given value.
    ///
    /// Should replace any previous value if present.
    async fn set(&self, key: &[u8], value: &[u8]);

    /// Get the value associated with the given key.
    ///
    /// Should return `None` if the key is not found.
    async fn get(&self, key: &[u8]) -> Option<Self::Bytes>;

    /// Remote the given key from the storage.
    ///
    /// Should be a no-op if the key is not present.
    async fn remove(&self, key: &[u8]);

    /// Executes a range query, returning all the key-value pairs
    /// in the specified range.
    ///
    /// # Panics
    ///
    /// This method MUST panic in case of an unbounded query (in any direction).
    async fn range(&self, query: RangeQuery<'_>) -> Vec<(Self::Key, Self::Bytes)>;
}

/// A type that can represent a key in the database.
pub trait Key {
    /// The bytes of this key.
    fn bytes(&self) -> &[u8];
}

impl<K: Key> Key for &'_ K {
    fn bytes(&self) -> &[u8] {
        (*self).bytes()
    }
}

impl Key for ResourceName {
    fn bytes(&self) -> &[u8] {
        self.as_ref().as_bytes()
    }
}

impl Key for ResourceNameView<'_> {
    fn bytes(&self) -> &[u8] {
        self.as_ref().as_bytes()
    }
}

pub struct RangeQuery<'b> {
    pub start: Bound<&'b [u8]>,
    pub end: Bound<&'b [u8]>,
    pub limit: Option<usize>,
    pub reverse: bool,
}

pub struct RangeQueryBuilder<'s, S, Ks, Ke> {
    start: Bound<Ks>,
    end: Bound<Ke>,
    limit: Option<usize>,
    reverse: bool,
    storage: &'s S,
}

impl<'s, S, Ks, Ke> IntoFuture for RangeQueryBuilder<'s, S, Ks, Ke>
where
    S: Storage,
    Ks: Key + 's,
    Ke: Key + 's,
{
    type Output = Vec<(S::Key, S::Bytes)>;

    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + 's>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.storage
                .range(RangeQuery {
                    start: bound_to_key(&self.start),
                    end: bound_to_key(&self.end),
                    limit: self.limit,
                    reverse: self.reverse,
                })
                .await
        })
    }
}

fn bound_to_key(b: &Bound<impl Key>) -> Bound<&[u8]> {
    match b {
        Bound::Unbounded => panic!("unbounded query!"),
        Bound::Included(k) => Bound::Included(k.bytes()),
        Bound::Excluded(k) => Bound::Excluded(k.bytes()),
    }
}

impl<'s, S, Ks, Ke> RangeQueryBuilder<'s, S, Ks, Ke> {
    /// Starts the query from the given key.
    ///
    /// This means that the key will be the first to be returned.
    pub fn from_key<K>(self, key: K) -> RangeQueryBuilder<'s, S, K, Ke> {
        RangeQueryBuilder {
            start: Bound::Included(key),
            end: self.end,
            limit: self.limit,
            reverse: self.reverse,
            storage: self.storage,
        }
    }

    /// Starts the query from the first key _after_ the given key.
    pub fn after_key<K>(self, key: K) -> RangeQueryBuilder<'s, S, K, Ke> {
        RangeQueryBuilder {
            start: Bound::Excluded(key),
            end: self.end,
            limit: self.limit,
            reverse: self.reverse,
            storage: self.storage,
        }
    }

    /// Ends the query _at_ the given key.
    ///
    /// This means that the key will be the last to be returned.
    pub fn to_key<K>(self, key: K) -> RangeQueryBuilder<'s, S, Ks, K> {
        RangeQueryBuilder {
            start: self.start,
            end: Bound::Included(key),
            limit: self.limit,
            reverse: self.reverse,
            storage: self.storage,
        }
    }

    /// Ends the query at the key right _before_ the given key.
    pub fn before_key<K>(self, key: K) -> RangeQueryBuilder<'s, S, Ks, K> {
        RangeQueryBuilder {
            start: self.start,
            end: Bound::Excluded(key),
            limit: self.limit,
            reverse: self.reverse,
            storage: self.storage,
        }
    }

    /// Reverse the results of the query.
    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }

    /// Limit the number of values returned by the query.
    pub fn limit(mut self, size: usize) -> Self {
        self.limit = Some(size);
        self
    }
}
