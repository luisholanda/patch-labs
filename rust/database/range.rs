use std::{
    borrow::Cow,
    future::{Future, IntoFuture},
    marker::PhantomData,
    ops::Bound,
    pin::Pin,
};

use pl_api_resource_name::ResourceName;

use crate::{
    storage::{StorageTransaction, Transaction},
    Entity,
};

pub struct RangeQuery<'b> {
    pub start: Bound<&'b ResourceName>,
    pub end: Bound<&'b ResourceName>,
    pub limit: Option<usize>,
    pub reverse: bool,
}

pub struct RangeQueryBuilder<'s, E, T> {
    start: Bound<&'s ResourceName>,
    end: Bound<Cow<'s, ResourceName>>,
    limit: Option<usize>,
    reverse: bool,
    transaction: &'s Transaction<T>,
    _marker: PhantomData<E>,
}

impl<'s, E: Entity, T: StorageTransaction> IntoFuture for RangeQueryBuilder<'s, E, T> {
    type Output = Result<Vec<E>, T::Error>;

    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + 's>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.transaction
                .get_range(RangeQuery {
                    start: self.start,
                    end: match &self.end {
                        Bound::Unbounded => Bound::Unbounded,
                        Bound::Excluded(k) => Bound::Excluded(&**k),
                        Bound::Included(k) => Bound::Included(&**k),
                    },
                    limit: self.limit,
                    reverse: self.reverse,
                })
                .await
        })
    }
}

impl<'s, E, T> RangeQueryBuilder<'s, E, T> {
    pub(crate) fn new(transaction: &'s Transaction<T>) -> Self {
        Self {
            start: Bound::Unbounded,
            end: Bound::Unbounded,
            limit: None,
            reverse: false,
            transaction,
            _marker: PhantomData,
        }
    }

    /// Starts the query from the given key.
    ///
    /// This means that the key will be the first to be returned.
    pub fn from_key<'k>(self, key: &'k ResourceName) -> RangeQueryBuilder<'k, E, T>
    where
        's: 'k,
    {
        RangeQueryBuilder {
            start: Bound::Included(key),
            ..self
        }
    }

    /// Starts the query from the first key _after_ the given key.
    pub fn after_key<'k>(self, key: &'k ResourceName) -> RangeQueryBuilder<'k, E, T>
    where
        's: 'k,
    {
        RangeQueryBuilder {
            start: Bound::Excluded(key),
            ..self
        }
    }

    /// Ends the query _at_ the given key.
    ///
    /// This means that the key will be the last to be returned.
    pub fn to_key<'k>(self, key: &'k ResourceName) -> RangeQueryBuilder<'k, E, T>
    where
        's: 'k,
    {
        RangeQueryBuilder {
            end: Bound::Included(Cow::Borrowed(key)),
            ..self
        }
    }

    /// Ends the query at the key right _before_ the given key.
    pub fn before_key<'k>(self, key: &'k ResourceName) -> RangeQueryBuilder<'k, E, T>
    where
        's: 'k,
    {
        RangeQueryBuilder {
            end: Bound::Excluded(Cow::Borrowed(key)),
            ..self
        }
    }

    /// All elements that start with the given key.
    pub fn beginning_with<'k>(self, key: &'k ResourceName) -> RangeQueryBuilder<'k, E, T>
    where
        's: 'k,
    {
        let end = key.next_bytewise();

        RangeQueryBuilder {
            start: Bound::Excluded(key),
            end: Bound::Excluded(Cow::Owned(end)),
            ..self
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
