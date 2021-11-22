use retainer::Cache;

#[tokio::test]
async fn test_cache_size_operations() {
  /*
      let cache = Cache::<u8, u8>::new();

      cache.insert_untracked(1, 2).await;
      cache.insert_untracked(2, 2).await;
      cache.insert_untracked(3, 3).await;

      assert_eq!(cache.len().await, 3);
      assert_eq!(cache.expired().await, 0);
      assert_eq!(cache.unexpired().await, 3);

      cache.clear().await;

      assert_eq!(cache.len().await, 0);
      assert_eq!(cache.expired().await, 0);
      assert_eq!(cache.unexpired().await, 0);
  */
}

#[tokio::test]
async fn test_cache_update_operations() {
  /*
    let cache = Cache::<u8, u8>::new();

    cache.insert_untracked(1, 1).await;

    assert_eq!(*cache.get(&1).await.unwrap(), 1);

    cache
        .update(&1, |value| {
            *value = 5;
        })
        .await;

    assert_eq!(*cache.get(&1).await.unwrap(), 5);
  */
}
