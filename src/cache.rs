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
use std::marker::PhantomData;
use std::time::{Duration, Instant};

use async_lock::{RwLock, RwLockUpgradableReadGuard};
use async_timer::Interval;
use log::{debug, log_enabled, trace, Level};
use rand::prelude::*;

use crate::entry::{CacheEntry, CacheEntryReadGuard, CacheExpiration};

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
  K: Ord + Clone + std::fmt::Debug,
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
    self
      .store
      .read()
      .await
      .iter()
      .filter(|(_, entry)| entry.is_expired())
      .count()
  }

  /// Retrieve a key expiration from cache.
  pub async fn expiration(&self, k: &K) -> Option<Instant> {
    match self.store.read().await.get(k) {
      Some(entry) => match &entry.expiration {
        Some(expiration) => Some(*expiration.instant()),
        None => None,
      },
      None => None,
    }
  }

  /// Retrieve a reference to a value inside the cache.
  ///
  /// The returned reference is bound inside a `RwLockReadGuard`.
  pub async fn get(&self, k: &K) -> Option<CacheEntryReadGuard<'_, V>> {
    let guard = self.store.read().await;

    unpack!(guard.get(k)?).map(|entry| CacheEntryReadGuard {
      entry,
      marker: PhantomData,
    })
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
  /// For expiration logic, please see `Cache::purge`, as this is used under the hood.
  pub async fn monitor(
    &self,
    sample: usize,
    threshold: usize,
    frequency: Duration,
    on_expire: &dyn Fn(&Vec<K>),
  ) {
    let mut interval = Interval::platform_new(frequency);
    loop {
      interval.as_mut().await;
      self.purge(sample, threshold, on_expire).await;
    }
  }

  /// Cleanses the cache of expired entries.
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
  pub async fn purge(&self, sample: usize, threshold: usize, on_expire: &dyn Fn(&Vec<K>)) {
    let start = Instant::now();

    let mut locked = Duration::from_nanos(0);
    let mut removed = 0;

    loop {
      // lock the store and grab a generator
      let store = self.store.upgradable_read().await;

      // once we're empty, no point carrying on
      if store.is_empty() {
        break;
      }

      // determine the sample size of the batch
      let total = store.len();
      let sample = cmp::min(sample, total);

      // counter to track removed keys
      let mut gone = 0;

      // create our temporary key store and index tree
      let mut keys = Vec::with_capacity(sample);
      let mut indices: BTreeSet<usize> = BTreeSet::new();

      {
        // fetch `sample` keys at random
        let mut rng = rand::thread_rng();
        while indices.len() < sample {
          indices.insert(rng.gen_range(0..total));
        }
      }
      {
        // tracker for previous index
        let mut prev = 0;

        // boxed iterator to allow us to iterate a single time for all indices
        let mut iter: Box<dyn Iterator<Item = (&K, &CacheEntry<V>)>> = Box::new(store.iter());

        // walk our index list
        for idx in indices {
          // calculate how much we need to shift the iterator
          let offset = idx
            .checked_sub(prev)
            .and_then(|idx| idx.checked_sub(1))
            .unwrap_or(0);

          // shift and mark the current index
          iter = Box::new(iter.skip(offset));
          prev = idx;

          // fetch the next pair (at our index)
          let (key, entry) = iter.next().unwrap();

          // skip if not expired
          if !entry.is_expired() {
            continue;
          }

          // otherwise mark for removal
          keys.push(key.to_owned());

          // and increment remove count
          gone += 1;
        }
      }

      if keys.len() == 0 {
        return;
      }

      on_expire(&keys);

      {
        // upgrade to a write guard so that we can make our changes
        let acquired = Instant::now();
        let mut store = RwLockUpgradableReadGuard::upgrade(store).await;

        // remove all expired keys
        for key in &keys {
          store.remove(key);
        }

        // increment the lock timer tracking directly
        locked = locked.checked_add(acquired.elapsed()).unwrap();
      }

      // log out now many of the sampled keys were removed
      if log_enabled!(Level::Trace) {
        trace!(
          "removed {} / {} ({:.2}%) of the sampled keys",
          gone,
          sample,
          (gone as f64 / sample as f64) * 100f64,
        );
      }

      // bump total remove count
      removed += gone;

      // break the loop if we don't meet thresholds
      if gone < threshold {
        break;
      }
    }

    // log out the completion as well as the time taken in millis
    if log_enabled!(Level::Debug) {
      debug!(
        "purge loop removed {} entries in {:.0?} ({:.0?} locked)",
        removed,
        start.elapsed(),
        locked
      );
    }
  }

  /// Remove an entry from the cache and return any stored value.
  pub async fn remove(&self, k: &K) -> Option<CacheEntry<V>> {
    self
      .store
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
    self
      .store
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

    self
      .store
      .write()
      .await
      .insert(k, entry)
      .and_then(|entry| unpack!(entry))
  }
}

/// Default implementation.
impl<K, V> Default for Cache<K, V>
where
  K: Ord + Clone + std::fmt::Debug,
{
  fn default() -> Self {
    Cache::new()
  }
}
