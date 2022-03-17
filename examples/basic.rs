use retainer::*;
use simple_logger::SimpleLogger;

use std::time::Duration;

#[tokio::main]
async fn main() {
    // enable logs for example purposes
    SimpleLogger::new().init().unwrap();

    // create our new cache
    let cache = Cache::new();

    // insert 100K entries
    for i in 0..100000 {
        cache.insert(i, i, Duration::from_millis(i)).await;
    }

    // spawn a monitor using Redis config; 20 keys every 100ms
    cache.monitor(20, 0.25, Duration::from_millis(100)).await;
}
