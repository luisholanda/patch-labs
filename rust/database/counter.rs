use std::sync::atomic::{AtomicU64, Ordering};

use pl_api_resource_name::ResourceName;

use crate::storage::StorageTransaction;

/// A counter key-value pair.
///
/// Counters are special entities w.r.t. the fact that they
/// have special support in the storage layer, as in most
/// storage systems they can be implemented more efficiently.
///
/// The implementation also guarantees a fake "read-your-writes"
/// behavior.
pub struct Counter<'t, T> {
    value: AtomicU64,
    key: ResourceName,
    tx: &'t T,
}

impl<'t, T: StorageTransaction> Counter<'t, T> {
    const VALUE_NOT_SET: u64 = u64::MAX;

    pub(crate) fn new(key: ResourceName, tx: &'t T) -> Self {
        Self {
            value: AtomicU64::new(Self::VALUE_NOT_SET),
            key,
            tx,
        }
    }

    /// Get the current transaction value of the counter.
    pub async fn get(&self) -> Result<u64, T::Error> {
        let mut val = self.curr_val();

        if val == Self::VALUE_NOT_SET {
            val = self.tx.counter_get(&self.key).await?.unwrap_or(0);
            self.set_val(val);
        }

        Ok(val)
    }

    /// Increment the counter by a value of 1.
    pub async fn increment_by_one(&self) -> Result<(), T::Error> {
        self.increment(1).await
    }

    /// Decrement the counter by a value of 1.
    pub async fn decrement_once(&self) -> Result<(), T::Error> {
        self.decrement(1).await
    }

    /// Increment the value of the counter by a given amount.
    pub async fn increment(&self, count: u64) -> Result<(), T::Error> {
        self.tx.counter_increment(&self.key, count).await?;

        let val = self.curr_val();
        if val != Self::VALUE_NOT_SET {
            self.set_val(val.saturating_add(count));
        }

        Ok(())
    }

    /// Decrement the value of the counter by a given amount.
    pub async fn decrement(&self, count: u64) -> Result<(), T::Error> {
        self.tx.counter_decrement(&self.key, count).await?;

        let val = self.curr_val();
        if val != Self::VALUE_NOT_SET {
            self.set_val(val.saturating_sub(count));
        }

        Ok(())
    }

    fn curr_val(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    fn set_val(&self, new: u64) {
        self.value.store(new, Ordering::Relaxed)
    }
}
