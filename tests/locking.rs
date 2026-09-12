use retainer::entry::CacheReadGuard;
use retainer::{Cache, CacheExpiration};

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot::channel;
use tokio::time::timeout;

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PanicOnClone(u8);

impl Clone for PanicOnClone {
    fn clone(&self) -> Self {
        panic!("cache lookup cloned its key")
    }
}

#[tokio::test]
async fn test_cache_read_guard_holds_read_lock() {
    let cache = Arc::new(Cache::<u8, u8>::new());

    cache.insert(1, 1, CacheExpiration::none()).await;
    assert_eq!(cache.len().await, 1);

    let guard = cache.get(&1).await.unwrap();
    let writer_cache = Arc::clone(&cache);
    let (started_tx, started_rx) = channel();

    let mut writer = tokio::spawn(async move {
        started_tx.send(()).unwrap();
        writer_cache.remove(&1).await;
    });

    started_rx.await.unwrap();

    assert!(
        timeout(Duration::from_millis(50), &mut writer)
            .await
            .is_err(),
        "writer completed while the read guard was still held"
    );
    assert_eq!(*guard, 1);

    drop(guard);

    timeout(Duration::from_secs(1), writer)
        .await
        .expect("writer remained blocked after the read guard was dropped")
        .expect("writer task panicked");

    assert!(cache.get(&1).await.is_none());
}

#[tokio::test]
async fn test_cache_allows_multiple_read_guards() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 10, CacheExpiration::none()).await;
    cache.insert(2, 20, CacheExpiration::none()).await;
    assert_eq!(cache.len().await, 2);

    let first = cache.get(&1).await.unwrap();
    let second = timeout(Duration::from_secs(1), cache.get(&2))
        .await
        .expect("second reader was blocked by the first")
        .unwrap();

    assert_eq!(*first, 10);
    assert_eq!(*second, 20);
}

#[tokio::test]
async fn test_cache_get_does_not_clone_key() {
    let cache = Cache::<PanicOnClone, u8>::new();
    assert_eq!(
        cache
            .insert(PanicOnClone(1), 2, CacheExpiration::none())
            .await,
        None
    );
    assert_eq!(cache.len().await, 1);

    let lookup = PanicOnClone(1);
    let guard = cache.get(&lookup).await.unwrap();
    assert_eq!(*guard, 2);
}

#[test]
fn test_cache_read_guard_is_send_and_sync_for_sync_types() {
    fn assert_send_and_sync<T: Send + Sync>() {}

    assert_send_and_sync::<CacheReadGuard<'static, u8, u8>>();
}

#[tokio::test]
async fn test_cache_read_guard_can_be_read_after_moving_to_another_thread() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 10, CacheExpiration::none()).await;
    assert_eq!(cache.len().await, 1);

    let guard = cache.get(&1).await.unwrap();

    std::thread::scope(|scope| {
        scope
            .spawn(move || assert_eq!(*guard, 10))
            .join()
            .expect("reader thread panicked");
    });
}
