//! Small structures based around entries in the cache.
//!
//! Each entry has an associated value and optional expiration,
//! and access functions for both. To be more convenient to the
//! called, a `CacheEntry<V>` will also dereference to `V`.
use std::ops::Range;
use std::ops::{Deref, DerefMut};
use std::time::{Duration, Instant};

use rand::prelude::*;

/// Represents an entry inside the cache.
///
/// Each entry has a value and optional expiration associated, with
/// the value being seen through the `Deref` trait for convenience.
#[derive(Debug)]
pub struct CacheEntry<V> {
    pub(crate) value: V,
    pub(crate) expiration: Option<CacheExpiration>,
}

impl<V> CacheEntry<V> {
    /// Retrieve the expiration associated with a cache entry.
    pub fn expiration(&self) -> Option<&CacheExpiration> {
        self.expiration.as_ref()
    }

    /// Retrieve whether a cache entry has passed expiration.
    pub fn is_expired(&self) -> bool {
        if let Some(expiration) = self.expiration() {
            if expiration.instant() < &Instant::now() {
                return true;
            }
        }
        false
    }

    /// Retrieve a reference to a value in a cache entry.
    pub fn value(&self) -> &V {
        &self.value
    }

    /// Retrieve a mutable reference to a value in a cache entry.
    pub fn value_mut(&mut self) -> &mut V {
        &mut self.value
    }
}

impl<V> Deref for CacheEntry<V> {
    type Target = V;

    // Derefs a cache entry to the internal value.
    fn deref(&self) -> &Self::Target {
        self.value()
    }
}

// Derefs a cache entry to the mutable internal value.
impl<V> DerefMut for CacheEntry<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value_mut()
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
/// * `Range<u64>` -> a random range of milliseconds to sample from to
///                   determine when an entry should expire.
///
/// Other conversions may be added in future, but this should suffice for most
/// cases. Any of these types may be passed to the insertion methods on a cache
/// type when adding entries to a cache.
#[derive(Debug)]
pub struct CacheExpiration {
    instant: Instant,
}

impl CacheExpiration {
    /// Retrieve the instant associated with this expiration.
    pub fn instant(&self) -> &Instant {
        &self.instant
    }

    /// Retrieve the time remaning before expiration.
    pub fn remaining(&self) -> Duration {
        self.instant.saturating_duration_since(Instant::now())
    }
}

// Automatic conversation from `Instant`.
impl From<Instant> for CacheExpiration {
    fn from(instant: Instant) -> Self {
        Self { instant }
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
        rand::thread_rng().gen_range(range).into()
    }
}
