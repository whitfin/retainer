use retainer::*;

use std::time::{Duration, Instant};

fn expired_instant() -> Instant {
    Instant::now().checked_sub(Duration::from_secs(1)).unwrap()
}

#[tokio::test]
async fn test_cache_default_starts_empty() {
    let cache = Cache::<u8, u8>::default();

    assert!(cache.is_empty().await);
}

#[tokio::test]
async fn test_cache_is_empty_tracks_entries() {
    let cache = Cache::<u8, u8>::new();
    assert!(cache.is_empty().await);

    cache.insert(1, 1, CacheExpiration::none()).await;
    assert!(!cache.is_empty().await);

    assert_eq!(cache.remove(&1).await, Some(1));
    assert!(cache.is_empty().await);
}

#[tokio::test]
async fn test_cache_is_not_empty_when_entry_is_expired() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 1, expired_instant()).await;

    assert_eq!(cache.expired().await, 1);
    assert!(!cache.is_empty().await);
}

#[tokio::test]
async fn test_cache_counts_entries_by_expiration() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 2, CacheExpiration::none()).await;
    cache.insert(2, 2, CacheExpiration::none()).await;
    cache.insert(3, 3, expired_instant()).await;

    assert_eq!(cache.len().await, 3);
    assert_eq!(cache.expired().await, 1);
    assert_eq!(cache.unexpired().await, 2);
}

#[tokio::test]
async fn test_cache_clear_removes_all_entries() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 1, CacheExpiration::none()).await;
    cache.insert(2, 2, expired_instant()).await;

    assert_eq!(cache.len().await, 2);
    assert_eq!(cache.unexpired().await, 1);
    assert_eq!(cache.expired().await, 1);

    cache.clear().await;

    assert!(cache.is_empty().await);
    assert_eq!(cache.len().await, 0);
    assert_eq!(cache.unexpired().await, 0);
    assert_eq!(cache.expired().await, 0);
}

#[tokio::test]
async fn test_cache_update_changes_value() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 1, CacheExpiration::none()).await;

    assert_eq!(cache.get(&1).await.unwrap().value(), &1);

    cache.update(&1, |value| *value = 5).await;

    assert_eq!(cache.get(&1).await.unwrap().value(), &5);
}

#[tokio::test]
async fn test_cache_update_ignores_missing_entry() {
    let cache = Cache::<u8, u8>::new();
    let mut called = false;

    assert!(cache.is_empty().await);

    cache.update(&1, |_| called = true).await;

    assert!(!called);
}

#[tokio::test]
async fn test_cache_update_preserves_expiration() {
    let cache = Cache::<u8, u8>::new();
    let expiration = Instant::now().checked_add(Duration::from_secs(60)).unwrap();

    cache.insert(1, 1, expiration).await;

    {
        let guard = cache.get(&1).await.unwrap();
        assert_eq!(*guard, 1);
        assert_eq!(guard.expiration().instant(), &Some(expiration));
    }

    cache.update(&1, |value| *value = 2).await;

    let guard = cache.get(&1).await.unwrap();
    assert_eq!(*guard, 2);
    assert_eq!(guard.expiration().instant(), &Some(expiration));
}

#[tokio::test]
async fn test_cache_get_rejects_missing_entry() {
    let cache = Cache::<u8, u8>::new();

    assert!(cache.get(&1).await.is_none());
}

#[tokio::test]
async fn test_cache_get_rejects_expired_entry_without_removing_it() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 10, expired_instant()).await;

    assert_eq!(cache.len().await, 1);
    assert_eq!(cache.expired().await, 1);

    assert!(cache.get(&1).await.is_none());

    // Reads reject expired entries without physically removing them.
    assert_eq!(cache.len().await, 1);
    assert_eq!(cache.expired().await, 1);
}

#[tokio::test]
async fn test_cache_insert_returns_unexpired_replaced_value() {
    let cache = Cache::<u8, u8>::new();

    assert_eq!(cache.insert(1, 10, CacheExpiration::none()).await, None);
    assert_eq!(cache.insert(1, 20, CacheExpiration::none()).await, Some(10));
}

#[tokio::test]
async fn test_cache_insert_discards_expired_replaced_value() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 30, expired_instant()).await;

    assert_eq!(cache.len().await, 1);
    assert_eq!(cache.expired().await, 1);

    assert_eq!(cache.insert(1, 40, CacheExpiration::none()).await, None);
    assert_eq!(*cache.get(&1).await.unwrap(), 40);
}

#[tokio::test]
async fn test_cache_remove_discards_expired_value() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 10, expired_instant()).await;

    assert_eq!(cache.len().await, 1);
    assert_eq!(cache.expired().await, 1);

    assert_eq!(cache.remove(&1).await, None);
    assert!(cache.is_empty().await);
}

#[tokio::test]
async fn test_cache_remove_returns_unexpired_value() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 10, CacheExpiration::none()).await;

    assert_eq!(*cache.get(&1).await.unwrap(), 10);

    assert_eq!(cache.remove(&1).await, Some(10));
    assert!(cache.is_empty().await);
}

#[tokio::test]
async fn test_cache_remove_missing_entry_returns_none() {
    let cache = Cache::<u8, u8>::new();

    assert!(cache.is_empty().await);
    assert_eq!(cache.remove(&1).await, None);
}

#[tokio::test]
async fn test_cache_update_ignores_expired_entry() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 10, expired_instant()).await;

    assert_eq!(cache.len().await, 1);
    assert_eq!(cache.expired().await, 1);

    let mut called = false;

    cache.update(&1, |_| called = true).await;

    assert!(!called);
}

#[tokio::test]
async fn test_cache_get_reports_expiration_metadata() {
    let cache = Cache::<u8, u8>::new();
    let expiration = Instant::now().checked_add(Duration::from_secs(60)).unwrap();

    cache.insert(1, 10, expiration).await;

    assert_eq!(cache.len().await, 1);

    let guard = cache.get(&1).await.unwrap();

    assert_eq!(guard.expiration().instant(), &Some(expiration));
    assert!(guard.expiration().remaining().unwrap() <= Duration::from_secs(60));
    assert!(!guard.expiration().is_expired());
}

#[tokio::test]
async fn test_cache_purge_removes_all_expired_entries() {
    let cache = Cache::<u8, u8>::new();

    cache.insert(1, 10, expired_instant()).await;
    cache.insert(2, 20, expired_instant()).await;
    cache.insert(3, 30, CacheExpiration::none()).await;

    assert_eq!(cache.expired().await, 2);
    assert_eq!(cache.unexpired().await, 1);

    cache.purge(3, 0.25).await;

    assert_eq!(cache.len().await, 1);
    assert_eq!(cache.expired().await, 0);
    assert_eq!(*cache.get(&3).await.unwrap(), 30);
}

#[tokio::test]
async fn test_cache_purge_accepts_empty_cache() {
    let cache = Cache::<u8, u8>::new();

    assert!(cache.is_empty().await);

    cache.purge(25, 0.25).await;

    assert!(cache.is_empty().await);
}

#[test]
fn test_cache_expiration_from_milliseconds() {
    let started = Instant::now();
    let expiration = CacheExpiration::from(1_000_u64);
    let finished = Instant::now();
    let expiration = expiration.instant().unwrap();
    let earliest = started.checked_add(Duration::from_secs(1)).unwrap();
    let latest = finished.checked_add(Duration::from_secs(1)).unwrap();

    assert!(expiration >= earliest);
    assert!(expiration <= latest);
}

#[test]
fn test_cache_expiration_from_duration() {
    let started = Instant::now();
    let expiration = CacheExpiration::from(Duration::from_secs(1));
    let finished = Instant::now();
    let expiration = expiration.instant().unwrap();
    let earliest = started.checked_add(Duration::from_secs(1)).unwrap();
    let latest = finished.checked_add(Duration::from_secs(1)).unwrap();

    assert!(expiration >= earliest);
    assert!(expiration <= latest);
}

#[test]
fn test_cache_expiration_from_range() {
    let range_started = Instant::now();
    let expiration = CacheExpiration::from(1_000_u64..2_000_u64);
    let range_finished = Instant::now();
    let expiration = expiration.instant().unwrap();

    let earliest_range_expiration = range_started
        .checked_add(Duration::from_millis(1_000))
        .unwrap();

    let latest_range_expiration = range_finished
        .checked_add(Duration::from_millis(2_000))
        .unwrap();

    assert!(expiration >= earliest_range_expiration);
    assert!(expiration < latest_range_expiration);
}

#[test]
fn test_cache_expiration_none_never_expires() {
    let expiration = CacheExpiration::none();

    assert_eq!(expiration.instant(), &None);
    assert_eq!(expiration.remaining(), None);
    assert!(!expiration.is_expired());
}

#[test]
fn test_cache_expiration_preserves_exact_instant() {
    let instant = Instant::now().checked_add(Duration::from_secs(60)).unwrap();

    assert_eq!(CacheExpiration::new(instant).instant(), &Some(instant));
    assert_eq!(CacheExpiration::from(instant).instant(), &Some(instant));
}

#[test]
fn test_expired_cache_expiration_has_no_remaining_time() {
    let expiration = CacheExpiration::new(expired_instant());

    assert_eq!(expiration.remaining(), Some(Duration::from_secs(0)));
}
