//! Small structures based around entries in the cache.
//!
//! Each entry has an associated value and optional expiration,
//! and access functions for both. To be more convenient to the
//! called, a `CacheEntry<V>` will also dereference to `V`.
use std::marker::PhantomData;
use std::ops::{Deref, Range};
use std::time::{Duration, Instant};

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
/// This structure is required to return references to the inner cache entries
/// when using locking mechanisms. This structure should be transparent for the
/// most part as it implements `Deref` to convert itself into the inner value.
#[derive(Debug)]
pub struct CacheReadGuard<'a, V> {
    pub(crate) entry: *const CacheEntry<V>,
    pub(crate) marker: PhantomData<&'a CacheEntry<V>>,
}

impl<V> CacheReadGuard<'_, V> {
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

impl<V> Deref for CacheReadGuard<'_, V> {
    type Target = V;

    // Derefs a cache guard to the internal entry.
    fn deref(&self) -> &Self::Target {
        self.value()
    }
}

// Stores a raw pointer to `T`, so if `T` is `Sync`, the lock guard over `T` is `Send`.
unsafe impl<V> Send for CacheReadGuard<'_, V> where V: Sized + Sync {}
unsafe impl<V> Sync for CacheReadGuard<'_, V> where V: Sized + Send + Sync {}
