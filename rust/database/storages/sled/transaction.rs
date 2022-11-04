use std::{collections::HashMap, ops::Bound};

use foundationdb::{KeySelector, RangeOption};
use pl_database_error::InfallibleDbResult;
use sled::{Batch, IVec, Tree};

/// A transaction in a Sled tree.
///
/// See [`SledDatabase::transaction`] for more info.
///
/// [`SledDatabase::transaction`]: crate::SledDatabase::transaction
pub struct SledTransaction {
    tree: Tree,
    batch: HashMap<IVec, Option<IVec>>,
}

pub type SledRange<'t> = Box<dyn Iterator<Item = InfallibleDbResult<(IVec, IVec)>> + 't>;

impl SledTransaction {
    /// Get a value of a key from the tree.
    pub fn get(&self, key: &[u8]) -> InfallibleDbResult<Option<IVec>> {
        match self.batch.get(key) {
            Some(v) => Ok(v.clone()),
            None => crate::sled_res_to_db_res(self.tree.get(key)),
        }
    }

    /// Get a range of key-value pairs given the query options.
    pub fn get_range<'t>(&'t self, opts: &RangeOption<'_>) -> SledRange<'t> {
        fn bound_from_selector<'s>(selector: &'s KeySelector<'s>) -> Bound<&'s [u8]> {
            if selector.or_equal() {
                Bound::Included(selector.key())
            } else {
                Bound::Excluded(selector.key())
            }
        }

        let begin = bound_from_selector(&opts.begin);
        let end = bound_from_selector(&opts.end);

        let range = self
            .tree
            .range::<&[u8], _>((begin, end))
            .map(crate::sled_res_to_db_res);

        // Let the caller see its writes
        let read_writes = |res| match res {
            Ok((k, v)) => match self.batch.get(&k).cloned() {
                // Value was updated.
                Some(Some(nv)) => Some(Ok((k, nv))),
                // Key was removed.
                Some(None) => None,
                // No changes applied to the key.
                None => Some(Ok((k, v))),
            },
            Err(err) => Some(Err(err)),
        };
        let range = range.filter_map(read_writes);

        let iter: SledRange = match (opts.limit, opts.reverse) {
            (None, true) => Box::new(range.rev()),
            (None, false) => Box::new(range),
            (Some(n), true) => Box::new(range.rev().take(n)),
            (Some(n), false) => Box::new(range.take(n)),
        };

        iter
    }

    /// Set the value associated with a given key.
    ///
    /// If the key was already present, its value will be overriden.
    pub fn set(&mut self, key: &[u8], value: &[u8]) {
        self.batch.insert(key.into(), Some(value.into()));
    }

    /// Remove a key from the database.
    pub fn clear(&mut self, key: &[u8]) {
        self.batch.insert(key.into(), None);
    }

    pub(crate) fn new(tree: Tree) -> Self {
        Self {
            tree,
            batch: HashMap::default(),
        }
    }

    pub(crate) async fn commit(self) -> Result<(), sled::Error> {
        // First, apply all accumulated writes.
        let mut batch = Batch::default();
        for (k, v) in self.batch {
            if let Some(v) = v {
                batch.insert(k, v);
            } else {
                batch.remove(k);
            }
        }

        self.tree.apply_batch(batch)?;

        // Now, match FoundationDB behavior and flush everyting to disk.
        self.tree.flush_async().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> sled::Db {
        sled::Config::new()
            .use_compression(false)
            .temporary(true)
            .create_new(true)
            .open()
            .unwrap()
    }

    #[test]
    fn test_tree() {
        let db = temp_db();
        db.insert(b"foo/1", b"1").unwrap();
        db.insert(b"foo/2", b"2").unwrap();
        db.insert(b"bar/1", b"1").unwrap();

        let mut tx = SledTransaction::new((*db).clone());

        // Test get from the tree.
        let foo_1 = tx.get(b"foo/1").expect("failed to get foo/1");
        assert_eq!(foo_1, Some(IVec::from("1")));

        // Test get from batch.
        tx.set(b"bar/2", b"2");

        let bar_2 = tx.get(b"bar/2").expect("failed to get bar/2");
        assert_eq!(bar_2, Some(IVec::from("2")));

        // Test range from the tree.
        {
            let mut range = tx.get_range(&RangeOption {
                begin: KeySelector::first_greater_or_equal(&b"foo/"[..]),
                end: KeySelector::last_less_than(&b"fop/"[..]),
                ..Default::default()
            });

            assert_eq!(
                range.next().unwrap().unwrap(),
                (IVec::from("foo/1"), IVec::from(b"1")),
                "failed to get foo/1 from range"
            );
            assert_eq!(
                range.next().unwrap().unwrap(),
                (IVec::from("foo/2"), IVec::from(b"2")),
                "failed to get foo/2 from range"
            );
            assert!(range.next().is_none(), "unnexpected element in range");
        }

        // Test remove
        tx.clear(b"foo/2");
        let foo_2 = tx.get(b"foo/2").expect("failed to get foo/2");
        assert_eq!(foo_2, None, "expected foo/2 to be deleted");
    }

    #[tokio::test]
    async fn test_commit() {
        let db = temp_db();

        db.insert(b"foo/1", b"1").unwrap();
        db.insert(b"foo/2", b"2").unwrap();
        db.insert(b"bar/1", b"1").unwrap();

        let mut tx = SledTransaction::new((*db).clone());

        tx.set(b"bar/2", b"2");
        tx.clear(b"bar/1");

        tx.commit().await.expect("failed to commit");

        let bar_2 = db.get("bar/2").expect("failed to get bar/2");
        assert_eq!(bar_2, Some(IVec::from(b"2")));

        let bar_1 = db.get("bar/1").expect("failed to get bar/2");
        assert_eq!(bar_1, None);
    }
}
