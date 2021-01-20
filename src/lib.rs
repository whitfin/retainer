use std::cmp;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use rand::prelude::*;
use tokio::sync::{RwLock, RwLockReadGuard};
use tokio::time;

mod entry;
use entry::SweepingCacheEntry;

mod expiration;
use expiration::SweepingCacheExpiration;

macro_rules! unpack {
    ($entry: expr) => {
        if $entry.is_expired() {
            None
        } else {
            Some($entry)
        }
    };
}

type Lookup<V> = Option<SweepingCacheEntry<V>>;

pub struct SweepingCache<K, V> {
    store: RwLock<BTreeMap<K, SweepingCacheEntry<V>>>,
}

impl<K, V> SweepingCache<K, V>
where
    K: Ord + Clone,
{
    pub fn new() -> Self {
        Self {
            store: RwLock::new(BTreeMap::new()),
        }
    }

    pub async fn clear(&self) {
        self.store.write().await.clear()
    }

    pub async fn get(&self, k: &K) -> Option<RwLockReadGuard<'_, SweepingCacheEntry<V>>> {
        let guard = self.store.read().await;
        let guard = RwLockReadGuard::try_map(guard, |guard| unpack!(guard.get(k)?));

        guard.ok()
    }

    pub async fn len(&self) -> usize {
        self.store.read().await.len()
    }

    pub async fn insert(&self, k: K, v: V) -> Lookup<V> {
        self.do_insert(k, v, None).await
    }

    pub async fn insert_with_expiration<E>(&self, k: K, v: V, e: E) -> Lookup<V>
    where
        E: Into<SweepingCacheExpiration>,
    {
        self.do_insert(k, v, Some(e.into())).await
    }

    pub async fn is_empty(&self) -> bool {
        self.store.read().await.is_empty()
    }

    pub async fn monitor(&self, sample: usize, frequency: Duration) {
        let mut interval = time::interval(frequency);

        loop {
            interval.tick().await;

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
                    let mut iter: Box<dyn Iterator<Item = (&K, &SweepingCacheEntry<V>)>> =
                        Box::new(store.iter());

                    for idx in indices {
                        let offset = cmp::min(0, idx - prev - 1);

                        iter = Box::new(iter.skip(offset));

                        let (key, entry) = iter.next().unwrap();

                        if !entry.is_expired() {
                            continue;
                        }

                        keys.push(key.to_owned());

                        gone += 1.0;
                        prev = idx;
                    }
                }

                for key in &keys {
                    store.remove(key);
                }

                if gone < (sample as f64 * 0.25) {
                    break;
                }
            }
        }
    }

    pub async fn remove(&self, k: &K) -> Lookup<V> {
        self.store
            .write()
            .await
            .remove(k)
            .and_then(|entry| unpack!(entry))
    }

    pub async fn update<F>(&self, k: &K, f: F)
    where
        F: FnOnce(&mut V),
    {
        let mut guard = self.store.write().await;
        if let Some(value) = guard.get_mut(k).and_then(|entry| unpack!(entry)) {
            f(value);
        }
    }

    async fn do_insert(&self, k: K, v: V, e: Option<SweepingCacheExpiration>) -> Lookup<V> {
        let entry = SweepingCacheEntry {
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

impl<K, V> Default for SweepingCache<K, V>
where
    K: Ord + Clone,
{
    fn default() -> Self {
        SweepingCache::new()
    }
}
