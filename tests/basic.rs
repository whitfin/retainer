use retainer::*;

use std::sync::Arc;
use std::time::Duration;
use tokio::task::yield_now;
use tokio::time::timeout;

#[tokio::test]
async fn test_cache_size_operations() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 2, CacheExpiration::none()).await;
    cache.insert(2, 2, CacheExpiration::none()).await;
    cache.insert(3, 3, CacheExpiration::none()).await;

    assert_eq!(cache.len().await, 3);
    assert_eq!(cache.expired().await, 0);
    assert_eq!(cache.unexpired().await, 3);

    cache.clear().await;

    assert_eq!(cache.len().await, 0);
    assert_eq!(cache.expired().await, 0);
    assert_eq!(cache.unexpired().await, 0);
}

#[tokio::test]
async fn test_cache_update_operations() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 1, CacheExpiration::none()).await;

    assert_eq!(cache.get(&1).await.unwrap().value(), &1);

    cache
        .update(&1, |value| {
            *value = 5;
        })
        .await;

    assert_eq!(cache.get(&1).await.unwrap().value(), &5);
}

#[tokio::test]
async fn test_cache_read_guard_holds_read_lock() {
    let cache = Arc::new(Cache::<u8, u8>::new());
    cache.insert(1, 1, CacheExpiration::none()).await;

    let guard = cache.get(&1).await.unwrap();
    let writer_cache = Arc::clone(&cache);
    let writer = tokio::spawn(async move {
        writer_cache.remove(&1).await;
    });

    yield_now().await;

    assert!(!writer.is_finished());
    assert_eq!(*guard, 1);

    drop(guard);

    timeout(Duration::from_secs(1), writer)
        .await
        .expect("writer remained blocked after the read guard was dropped")
        .expect("writer task panicked");

    assert!(cache.get(&1).await.is_none());
}
