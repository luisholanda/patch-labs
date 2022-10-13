use std::{collections::BTreeMap, sync::Mutex};

use pl_database::{Storage, RangeQuery};

/// A in-memory implementation of [`Storage`].
///
/// Used for testing only.
#[derive(Default)]
pub struct InMemoryStorage {
    btree: Mutex<BTreeMap<Vec<u8>, Vec<u8>>>
}

#[async_trait::async_trait]
impl Storage for InMemoryStorage {
    type Key = Vec<u8>;
    type Bytes = Vec<u8>;

    async fn set(&self, key: &[u8], value: &[u8]) {
        self.btree.lock().unwrap().insert(key.to_vec(), value.to_vec());
    }

    async fn get(&self, key: &[u8]) -> Option<Self::Key> {
        self.btree.lock().unwrap().get(key).cloned()
    }

    async fn remove(&self, key: &[u8]) {
        self.btree.lock().unwrap().remove(key);
    }

    async fn range(&self, query: RangeQuery<'_>) -> Vec<(Self::Key, Self::Bytes)> {
        let tree = self.btree.lock().unwrap();
        let iter = tree
            .range::<[u8], _>((query.start, query.end))
            .map(|(k, v)| (k.clone(), v.clone()));

        match (query.reverse, query.limit) {
            (true, None) => iter.rev().collect(),
            (true, Some(n)) => iter.rev().take(n).collect(),
            (false, None) => iter.collect(),
            (false, Some(n)) => iter.take(n).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_storage() {
        let storage = InMemoryStorage::default();

        storage.set(b"foo", b"bar").await;

        assert_eq!(storage.get(b"foo").await, Some(b"bar".to_vec()));

        storage.set(b"foo", b"another-value").await;

        assert_eq!(storage.get(b"foo").await, Some(b"another-value".to_vec()));

        storage.remove(b"foo").await;

        assert_eq!(storage.get(b"foo").await, None);
    }
}
