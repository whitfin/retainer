use async_cache::Cache;
use async_std::task;
use smol::Timer;

use std::sync::Arc;
use std::time::{Duration, Instant};

#[async_std::test]
async fn test_async_std() {
    // construct our cache
    let cache = Arc::new(Cache::new());
    let clone = cache.clone();

    // spawn the monitor
    task::spawn(async move {
        // don't forget to monitor your cache to evict entries
        clone.monitor(25, 0.25, Duration::from_secs(1)).await
    });

    // execute the set of base tests
    execute_base_test(cache).await
}

#[test]
fn test_smol() {
    smol::block_on(async {
        // construct our cache
        let cache = Arc::new(Cache::new());
        let clone = cache.clone();

        // spawn the monitor
        let handle = smol::spawn(async move {
            // don't forget to monitor your cache to evict entries
            clone.monitor(25, 0.25, Duration::from_secs(1)).await
        });

        // execute the set of base tests
        execute_base_test(cache).await;

        // cancel the monitor
        handle.cancel().await;
    });
}

#[tokio::test]
async fn test_tokio() {
    // construct our cache
    let cache = Arc::new(Cache::new());
    let clone = cache.clone();

    // spawn the monitor
    let monitor = tokio::spawn(async move {
        // don't forget to monitor your cache to evict entries
        clone.monitor(3, 0.25, Duration::from_secs(3)).await
    });

    // execute the set of base tests
    execute_base_test(cache).await;

    // shutdown monitor
    monitor.abort();
}

async fn execute_base_test(cache: Arc<Cache<&str, usize>>) {
    // insert using an `Instant` type to specify expiration
    cache.insert("one", 1, Instant::now()).await;

    // insert using a `Duration` type to wait before expiration
    cache.insert("two", 2, Duration::from_secs(2)).await;

    // insert using a number of milliseconds
    cache.insert("three", 3, 3500).await;

    // insert using a random number of milliseconds
    cache.insert("four", 4, 3500..5000).await;

    // insert without expiration (i.e. manual removal)
    cache.insert_untracked("five", 5).await;

    // wait until the monitor has run once
    Timer::after(Duration::from_millis(3250)).await;

    // the first two keys should have been removed
    assert!(cache.get(&"one").await.is_none());
    assert!(cache.get(&"two").await.is_none());

    // the rest should be there still for now
    assert!(cache.get(&"three").await.is_some());
    assert!(cache.get(&"four").await.is_some());
    assert!(cache.get(&"five").await.is_some());

    // wait until the monitor has run again
    Timer::after(Duration::from_millis(3250)).await;

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
}
