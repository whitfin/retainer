use std::ops::{Deref, DerefMut};
use std::time::Instant;

use crate::expiration::SweepingCacheExpiration;

pub struct SweepingCacheEntry<V> {
    pub(crate) value: V,
    pub(crate) expiration: Option<SweepingCacheExpiration>,
}

impl<V> Deref for SweepingCacheEntry<V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        self.value()
    }
}

impl<V> DerefMut for SweepingCacheEntry<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value_mut()
    }
}

impl<V> SweepingCacheEntry<V> {
    pub fn expiration(&self) -> Option<&SweepingCacheExpiration> {
        self.expiration.as_ref()
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expiration) = self.expiration() {
            if expiration.instant() < &Instant::now() {
                return true;
            }
        }
        false
    }

    pub fn value(&self) -> &V {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut V {
        &mut self.value
    }
}
