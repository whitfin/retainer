//! Caching structures for use in an asynchronous context.
//!
//! The main point of this module is the `Cache` type, which offers a small
//! implementation of a cache with time based expiration  support. The underlying
//! structure is nothing more than a map wrapped inside some asynchronous locking
//! mechanisms to avoid blocking the entire async runtime when waiting for a handle.
//!
//! The eviction algorithm has been based on Redis, and essentially just samples
//! the entry set on an interval to prune the inner tree over time. More information
//! on how this works can be seen on the `monitor` method of the `Cache` type.
use std::cmp;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use async_timer::Interval;
use rand::prelude::*;
use tokio::sync::{RwLock, RwLockReadGuard};

use crate::entry::{CacheEntry, CacheExpiration};

// Define small private macro to unpack entry references.
macro_rules! unpack {
    ($entry: expr) => {
        if $entry.is_expired() {
            None
        } else {
            Some($entry)
        }
    };
}

/// Basic caching structure with asynchronous locking support.
///
/// This structure provides asynchronous access wrapped around a standard
/// `BTreeMap` to avoid blocking event loops when a writer cannot gain a
/// handle - which is what would happen with standard locking implementations.
pub struct Cache<K, V> {
    store: RwLock<BTreeMap<K, CacheEntry<V>>>,
}

impl<K, V> Cache<K, V>
where
    K: Ord + Clone,
{
    /// Construct a new `Cache`.
    pub fn new() -> Self {
        Self {
            store: RwLock::new(BTreeMap::new()),
        }
    }

    /// Remove all entries from the cache.
    pub async fn clear(&self) {
        self.store.write().await.clear()
    }

    /// Retrieve the number of expired entries inside the cache.
    ///
    /// Note that this is calculated by walking the set of entries and
    /// should therefore not be used in performance sensitive situations.
    pub async fn expired(&self) -> usize {
        self.store
            .read()
            .await
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .count()
    }

    /// Retrieve a reference to a value inside the cache.
    ///
    /// The returned reference is bound inside a `RwLockReadGuard`.
    pub async fn get(&self, k: &K) -> Option<RwLockReadGuard<'_, CacheEntry<V>>> {
        let guard = self.store.read().await;
        let guard = RwLockReadGuard::try_map(guard, |guard| unpack!(guard.get(k)?));
        guard.ok()
    }

    /// Retrieve the number of entries inside the cache.
    ///
    /// This *does* include entries which may be expired but are not yet evicted. In
    /// future there may be an API addition to find the unexpired count, but as it's
    /// relatively expensive it has been omitted for the time being.
    pub async fn len(&self) -> usize {
        self.store.read().await.len()
    }

    /// Insert a key/value pair into the cache with an associated expiration.
    ///
    /// The third argument controls expiration, which can be provided using any type which
    /// implements `Into<CacheExpiration>`. This allows for various different syntax based
    /// on your use case. If you do not want expiration, see `insert_untracked`.
    pub async fn insert<E>(&self, k: K, v: V, e: E) -> Option<CacheEntry<V>>
    where
        E: Into<CacheExpiration>,
    {
        self.do_insert(k, v, Some(e.into())).await
    }

    /// Insert a key/value pair into the cache with no associated expiration.
    pub async fn insert_untracked(&self, k: K, v: V) -> Option<CacheEntry<V>> {
        self.do_insert(k, v, None).await
    }

    /// Check whether the cache is empty.
    pub async fn is_empty(&self) -> bool {
        self.store.read().await.is_empty()
    }

    /// Retrieve a `Future` used to monitor expired keys.
    ///
    /// This future must be spawned on whatever runtime you are using inside your
    /// application; not doing this will result in keys never being expired.
    ///
    /// Keys are expired using the same logic as the popular caching system Redis:
    ///
    /// 1. Wait until the next tick of `frequency`.
    /// 2. Take a sample of `sample` keys from the cache.
    /// 3. Remove any expired keys from the sample.
    /// 4. Based on `threshold` percentage:
    ///     4a. If more than `threshold` were expired, goto #2.
    ///     4b. If less than `threshold` were expired, goto #1.
    ///
    /// This means that at any point you may have up to `threshold` percent of your
    /// cache storing expired entries (assuming the monitor just ran), so make sure
    /// to tune your frequency, sample size, and threshold accordingly.
    pub async fn monitor(&self, sample: usize, threshold: f64, frequency: Duration) {
        let mut interval = Interval::platform_new(frequency);

        loop {
            interval.as_mut().await;

            let mut store = self.store.write().await;
            let mut rng = rand::thread_rng();

            loop {
                if store.is_empty() {
                    break;
                }

                let count = cmp::min(sample, store.len());

                let mut gone = 0f64;
                let mut keys = Vec::with_capacity(count);
                let mut indices: BTreeSet<usize> = BTreeSet::new();

                while indices.len() < count {
                    indices.insert(rng.gen_range(0..store.len()));
                }

                {
                    let mut prev = 0;
                    let mut iter: Box<dyn Iterator<Item = (&K, &CacheEntry<V>)>> =
                        Box::new(store.iter());

                    for idx in indices {
                        let offset = idx
                            .checked_sub(prev)
                            .and_then(|idx| idx.checked_sub(1))
                            .unwrap_or(0);

                        iter = Box::new(iter.skip(offset));
                        prev = idx;

                        let (key, entry) = iter.next().unwrap();

                        if !entry.is_expired() {
                            continue;
                        }

                        keys.push(key.to_owned());
                        gone += 1.0;
                    }
                }

                for key in &keys {
                    store.remove(key);
                }

                if gone < (sample as f64 * threshold) {
                    break;
                }
            }
        }
    }

    /// Remove an entry from the cache and return any stored value.
    pub async fn remove(&self, k: &K) -> Option<CacheEntry<V>> {
        self.store
            .write()
            .await
            .remove(k)
            .and_then(|entry| unpack!(entry))
    }

    /// Retrieve the number of unexpired entries inside the cache.
    ///
    /// Note that this is calculated by walking the set of entries and
    /// should therefore not be used in performance sensitive situations.
    pub async fn unexpired(&self) -> usize {
        self.store
            .read()
            .await
            .iter()
            .filter(|(_, entry)| !entry.is_expired())
            .count()
    }

    /// Updates an entry in the cache without changing the expiration.
    pub async fn update<F>(&self, k: &K, f: F)
    where
        F: FnOnce(&mut V),
    {
        let mut guard = self.store.write().await;
        if let Some(value) = guard.get_mut(k).and_then(|entry| unpack!(entry)) {
            f(value);
        }
    }

    /// Internal logic for insertion to avoid multiple definitions.
    ///
    /// This is necessary as we have to support storing keys with not attached expiration.
    async fn do_insert(&self, k: K, v: V, e: Option<CacheExpiration>) -> Option<CacheEntry<V>> {
        let entry = CacheEntry {
            value: v,
            expiration: e,
        };

        self.store
            .write()
            .await
            .insert(k, entry)
            .and_then(|entry| unpack!(entry))
    }
}

/// Default implementation.
impl<K, V> Default for Cache<K, V>
where
    K: Ord + Clone,
{
    fn default() -> Self {
        Cache::new()
    }
}
