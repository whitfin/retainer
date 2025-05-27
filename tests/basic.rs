use retainer::*;

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
async fn test_cache_borrow_types() {
    let key = "key".to_string();
    let cache = Cache::<String, bool>::new();

    cache
        .insert(key.clone(), true, CacheExpiration::none())
        .await;

    let lookup: &str = &key;

    assert!(cache.get(lookup).await.unwrap().value());
}
