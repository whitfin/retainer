# Retainer
[![Build Status](https://img.shields.io/github/workflow/status/whitfin/retainer/CI)](https://github.com/whitfin/retainer/actions)
[![Crates.io](https://img.shields.io/crates/v/retainer.svg)](https://crates.io/crates/retainer)

This crate offers a very small cache with asynchronous bindings, allowing it to be
used in async Rust contexts (Tokio, async-std, smol, etc.) without blocking the
worker thread completely.

It also includes the ability to expire entries in the cache based on their time
inside; this is done by spawning a monitor on your async runtime in order to
perform cleanup tasks periodically. The eviction algorithm is similar to the one
found inside [Redis](https://redis.io/commands/expire), although keys are not
removed on access in order to reduce borrow complexity.

This crate is still a work in progress, so feel free to file any suggestions or
improvements and I'll get to them as soon as possible :).

### Getting Started

This crate is available on [crates.io](https://crates.io/crates/retainer). The
easiest way to use it is to add an entry to your `Cargo.toml` defining the dependency:

```toml
[dependencies]
retainer = "0.1"
```

### Basic Usage

The construction of a cache is very simple, and (currently) requires no options. If
you need to make use of key expiration, you must ensure to either await a monitor or
spawn a monitor on your runtime.

There are many ways to provide an expiration time when inserting into a cache, by
making use of several types implementing the `Into<CacheExpiration>` trait. Below
are some examples of types which are available and some of the typical APIs you
will find yourself using. This code uses the Tokio runtime, but this crate should
be compatible with most of the popular asynchronous runtimes. Currently a small
set of tests are run against async-std, smol and Tokio.

```rust
use retainer::Cache;
use tokio::time::sleep;

use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() {
    // construct our cache
    let cache = Arc::new(Cache::new());
    let clone = cache.clone();

    // don't forget to monitor your cache to evict entries
    let monitor = tokio::spawn(async move {
        clone.monitor(4, Duration::from_secs(3), 0.25).await
    });

    // insert using an `Instant` type to specify expiration
    cache.insert("one", 1usize, Instant::now()).await;

    // insert using a `Duration` type to wait before expiration
    cache.insert("two", 2, Duration::from_secs(2)).await;

    // insert using a number of milliseconds
    cache.insert("three", 3, 3500).await;

    // insert using a random number of milliseconds
    cache.insert("four", 4, 3500..5000).await;

    // insert without expiration (i.e. manual removal)
    cache.insert_untracked("five", 5).await;

    // wait until the monitor has run once
    sleep(Duration::from_millis(3250)).await;

    // the first two keys should have been removed
    assert!(cache.get(&"one").await.is_none());
    assert!(cache.get(&"two").await.is_none());

    // the rest should be there still for now
    assert!(cache.get(&"three").await.is_some());
    assert!(cache.get(&"four").await.is_some());
    assert!(cache.get(&"five").await.is_some());

    // wait until the monitor has run again
    sleep(Duration::from_millis(3250)).await;

    // the other two keys should have been removed
    assert!(cache.get(&"three").await.is_none());
    assert!(cache.get(&"four").await.is_none());

    // the key with no expiration should still exist
    assert!(cache.get(&"five").await.is_some());

    // but we should be able to manually remove it
    assert!(cache.remove(&"five").await.is_some());
    assert!(cache.get(&"five").await.is_none());

    // and now our cache should be empty
    assert!(cache.is_empty().await);

    // shutdown monitor
    monitor.abort();
}

```

In the case this example is not kept up to date, you can look for any types which
implement the `Into<CacheExpiratio>` trait in the documentation for a complete list.

### Cache Monitoring

All key expiration is done on an interval, carried out when you `await` the future
returned by `Cache::monitor`. The basis for how this is done has been lifted roughly
from the implementation found inside Redis, as it's simple but still works well.

When you call `Cache::monitor`, you need to provide 3 arguments:

* sample
* frequency
* threshold

Below is a summarization of the flow of eviction, hopefully in a clear way:

1. Wait until the next tick of `frequency`.
2. Take a batch of `sample` entries from the cache at random.
3. Check for and remove any expired entries found in the batch.
4. If more than `threshold` percent of the entries in the batch were removed,
   immediately goto #2, else goto #1.

This allows the user to control the aggressiveness of eviction quite effectively,
by tweaking the `threshold` and `frequency` values. Naturally a cache uses more
memory on average the higher your threshold is, so please do keep this in mind.
