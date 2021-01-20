use std::ops::{Deref, Range};
use std::time::{Duration, Instant};

use rand::prelude::*;

pub struct CacheExpiration {
    pub(crate) instant: Instant,
}

impl Deref for CacheExpiration {
    type Target = Instant;

    fn deref(&self) -> &Self::Target {
        self.instant()
    }
}

impl From<Instant> for CacheExpiration {
    fn from(instant: Instant) -> Self {
        Self { instant }
    }
}

impl From<u64> for CacheExpiration {
    fn from(millis: u64) -> Self {
        Duration::from_millis(millis).into()
    }
}

impl From<Duration> for CacheExpiration {
    fn from(duration: Duration) -> Self {
        Instant::now().checked_add(duration).unwrap().into()
    }
}

impl From<Range<u64>> for CacheExpiration {
    fn from(range: Range<u64>) -> Self {
        rand::thread_rng().gen_range(range).into()
    }
}

impl CacheExpiration {
    pub fn instant(&self) -> &Instant {
        &self.instant
    }
}
