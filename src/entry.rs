//! Small structures based around entries in the cache.
//!
//! Each entry has an associated value and optional expiration,
//! and cache guards provide access functions for both. For convenience,
//! cache guards also dereference to the guarded value.
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut, Range};
use std::time::{Duration, Instant};

use async_lock::{RwLockReadGuard, RwLockWriteGuard};
use rand::prelude::*;

/// Represents an entry inside the cache.
///
/// Each entry has a value and optional expiration associated, with
/// the value being seen through the `Deref` trait for convenience.
#[derive(Debug)]
pub(crate) struct CacheEntry<V> {
    value: V,
    expiration: CacheExpiration,
}

impl<V> CacheEntry<V> {
    /// Create a new cache entry from a value and expiration.
    pub fn new(value: V, expiration: CacheExpiration) -> Self {
        Self { value, expiration }
    }

    /// Retrieve the internal expiration.
    pub fn expiration(&self) -> &CacheExpiration {
        &self.expiration
    }

    /// Retrieve the mutable internal expiration.
    pub(crate) fn expiration_mut(&mut self) -> &mut CacheExpiration {
        &mut self.expiration
    }

    /// Retrieve the internal value.
    pub fn value(&self) -> &V {
        &self.value
    }

    /// Retrieve the mutable internal value.
    pub fn value_mut(&mut self) -> &mut V {
        &mut self.value
    }

    /// Take the internal value.
    pub fn into_inner(self) -> V {
        self.value
    }
}

/// Small structure to represent expiration in a cache.
///
/// Expirations are constructed using the `From` and `Into` traits
/// from the standard library; there are no other functions.
///
/// There are currently several supported conversions:
///
/// * `u64` -> a number of milliseconds to pass before an entry should expire.
/// * `Instant` -> an exact time that an entry should expire.
/// * `Duration` -> a duration to pass before an entry should expire.
/// * `Range<u64>` -> a random range of milliseconds to sample expiry from.
///
/// Other conversions may be added in future, but this should suffice for most
/// cases. Any of these types may be passed to the insertion methods on a cache
/// type when adding entries to a cache.
#[derive(Debug)]
pub struct CacheExpiration {
    instant: Option<Instant>,
}

impl CacheExpiration {
    /// Create an expiration at a given instant.
    pub fn new<I>(instant: I) -> Self
    where
        I: Into<Instant>,
    {
        Self {
            instant: Some(instant.into()),
        }
    }

    /// Create an empty expiration (i.e. no expiration).
    pub fn none() -> Self {
        Self { instant: None }
    }

    /// Retrieve the instant associated with this expiration.
    pub fn instant(&self) -> &Option<Instant> {
        &self.instant
    }

    /// Retrieve whether a cache entry has passed expiration.
    pub fn is_expired(&self) -> bool {
        self.instant()
            .map(|expiration| expiration < Instant::now())
            .unwrap_or(false)
    }

    /// Retrieve the time remaining before expiration.
    pub fn remaining(&self) -> Option<Duration> {
        self.instant
            .map(|i| i.saturating_duration_since(Instant::now()))
    }
}

// Automatic conversation from `Instant`.
impl From<Instant> for CacheExpiration {
    fn from(instant: Instant) -> Self {
        Self::new(instant)
    }
}

// Automatic conversation from `u64`.
impl From<u64> for CacheExpiration {
    fn from(millis: u64) -> Self {
        Duration::from_millis(millis).into()
    }
}

// Automatic conversation from `Duration`.
impl From<Duration> for CacheExpiration {
    fn from(duration: Duration) -> Self {
        Instant::now().checked_add(duration).unwrap().into()
    }
}

// Automatic conversation from `u64`.
impl From<Range<u64>> for CacheExpiration {
    fn from(range: Range<u64>) -> Self {
        rand::rng().random_range(range).into()
    }
}

/// Read guard for references to the inner cache structure.
///
/// This structure retains the cache's read lock, ensuring that the referenced
/// entry cannot be modified or removed while it is in use. It should be dropped
/// as soon as possible so that writers can make progress. It implements `Deref`
/// to expose the inner value.
#[derive(Debug)]
pub struct CacheReadGuard<'a, K, V> {
    pub(crate) entry: *const CacheEntry<V>,
    pub(crate) _lock: RwLockReadGuard<'a, BTreeMap<K, CacheEntry<V>>>,
}

impl<K, V> CacheReadGuard<'_, K, V> {
    /// Retrieve the internal guarded expiration.
    pub fn expiration(&self) -> &CacheExpiration {
        self.entry().expiration()
    }

    /// Retrieve the internal guarded value.
    pub fn value(&self) -> &V {
        self.entry().value()
    }

    /// Retrieve a reference to the internal entry.
    fn entry(&self) -> &CacheEntry<V> {
        unsafe { &*self.entry }
    }
}

impl<K, V> Deref for CacheReadGuard<'_, K, V> {
    type Target = V;

    // Derefs a cache guard to the internal entry.
    fn deref(&self) -> &Self::Target {
        self.value()
    }
}

// The pointer is protected by the retained read guard. Moving or sharing the
// wrapper is safe whenever the complete guarded map is safe to share.
unsafe impl<K, V> Send for CacheReadGuard<'_, K, V>
where
    K: Sync,
    V: Sync,
{
}

unsafe impl<K, V> Sync for CacheReadGuard<'_, K, V>
where
    K: Sync,
    V: Sync,
{
}

/// Write guard for mutable references to the inner cache structure.
///
/// This structure retains the cache's write lock, ensuring exclusive access to
/// the referenced entry while it is in use. It should be dropped as soon as
/// possible and must not be retained across unrelated `.await` points, since no
/// other cache operations can make progress while it exists. It implements
/// `Deref` and `DerefMut` to expose the inner value.
#[derive(Debug)]
pub struct CacheWriteGuard<'a, K, V> {
    pub(crate) entry: *mut CacheEntry<V>,
    pub(crate) _lock: RwLockWriteGuard<'a, BTreeMap<K, CacheEntry<V>>>,
}

impl<K, V> CacheWriteGuard<'_, K, V> {
    /// Retrieve the internal guarded expiration.
    pub fn expiration(&self) -> &CacheExpiration {
        self.entry().expiration()
    }

    /// Retrieve the mutable internal guarded expiration.
    pub fn expiration_mut(&mut self) -> &mut CacheExpiration {
        self.entry_mut().expiration_mut()
    }

    /// Retrieve the internal guarded value.
    pub fn value(&self) -> &V {
        self.entry().value()
    }

    /// Retrieve the mutable internal guarded value.
    pub fn value_mut(&mut self) -> &mut V {
        self.entry_mut().value_mut()
    }

    /// Retrieve a reference to the internal entry.
    fn entry(&self) -> &CacheEntry<V> {
        unsafe { &*self.entry }
    }

    /// Retrieve a mutable reference to the internal entry.
    fn entry_mut(&mut self) -> &mut CacheEntry<V> {
        unsafe { &mut *self.entry }
    }
}

impl<K, V> Deref for CacheWriteGuard<'_, K, V> {
    type Target = V;

    // Derefs a cache guard to the internal entry.
    fn deref(&self) -> &Self::Target {
        self.value()
    }
}

impl<K, V> DerefMut for CacheWriteGuard<'_, K, V> {
    // Mutably derefs a cache guard to the internal entry.
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value_mut()
    }
}

// The pointer is protected by the retained write guard. Moving or sharing the
// wrapper is safe whenever the complete guarded map is safe to move or share.
unsafe impl<K, V> Send for CacheWriteGuard<'_, K, V>
where
    K: Send,
    V: Send,
{
}

unsafe impl<K, V> Sync for CacheWriteGuard<'_, K, V>
where
    K: Sync,
    V: Sync,
{
}
