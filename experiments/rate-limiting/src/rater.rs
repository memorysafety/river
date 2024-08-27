use std::{sync::Arc, time::Duration};
use std::hash::Hash;
use std::fmt::Debug;

use concread::arcache::{ARCache, ARCacheBuilder};
use leaky_bucket::RateLimiter;

pub struct Rater<Key>
where
    Key: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
{
    cache: ARCache<Key, Arc<RateLimiter>>,
    max_tokens_per_bucket: usize,
    refill_interval_millis: usize,
    refill_qty: usize,
}

impl<Key> Rater<Key>
where
    Key: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
{
    pub fn new(
        threads: usize,
        max_buckets: usize,
        max_tokens_per_bucket: usize,
        refill_interval_millis: usize,
        refill_qty: usize,
    ) -> Self {
        let cache = ARCacheBuilder::new()
            .set_expected_workload(max_buckets, threads.max(1), 1, 1, false)
            .build()
            .expect("Creation of rate limiter should not fail");

        Self {
            cache,
            max_tokens_per_bucket,
            refill_interval_millis,
            refill_qty: refill_qty.min(max_tokens_per_bucket),
        }
    }

    pub fn get_limiter(&self, key: Key) -> Arc<RateLimiter> {
        let mut reader = self.cache.read();

        if let Some(find) = reader.get(&key) {
            // Rate limiter DID exist in the cache
            tracing::trace!(?key, "rate limiting cache hit",);
            find.clone()
        } else {
            let new_limiter = Arc::new(
                RateLimiter::builder()
                    .initial(self.max_tokens_per_bucket)
                    .max(self.max_tokens_per_bucket)
                    .interval(Duration::from_millis(self.refill_interval_millis as u64))
                    .refill(self.refill_qty)
                    .fair(true)
                    .build(),
            );

            tracing::debug!(?key, "rate limiting cache miss",);
            reader.insert(key, new_limiter.clone());
            reader.finish();
            new_limiter
        }
    }
}

#[cfg(test)]
mod test {
    use tokio::sync::mpsc::channel;

    use super::*;
    use std::{ops::Add, time::Instant};

    #[tokio::test]
    async fn smoke() {
        let _ = tracing_subscriber::fmt::try_init();

        let rater = Arc::new(Rater::new(2, 5, 3, 10, 1));

        for i in 0..100 {
            let start = Instant::now();
            let bucket = rater.get_limiter("bob".to_string());
            bucket.acquire_owned(1).await;
            tracing::info!("Success {i} took {:?}!", start.elapsed());
        }
    }

    #[tokio::test]
    async fn concurrent_fewer() {
        let _ = tracing_subscriber::fmt::try_init();

        let rater = Arc::new(Rater::new(8, 16, 3, 10, 1));
        let mut handles = vec![];

        for thread in 0..8 {
            let rater = rater.clone();
            let hdl = tokio::task::spawn(async move {
                for i in 1..32 {
                    let name = match i {
                        i if i % 15 == 0 => "fizzbuzz".to_string(),
                        i if i % 3 == 0 => "fizz".to_string(),
                        i if i % 5 == 0 => "buzz".to_string(),
                        i => format!("{i}"),
                    };
                    let start = Instant::now();
                    let bucket = rater.get_limiter(name.clone());
                    bucket.acquire_owned(1).await;
                    tracing::info!("{thread}:{i}:{name} took {:?}!", start.elapsed());
                }
            });
            handles.push(hdl);
        }

        for h in handles.into_iter() {
            h.await.unwrap();
        }
    }

    #[tokio::test]
    async fn concurrent_more() {
        let _ = tracing_subscriber::fmt::try_init();

        let rater = Arc::new(Rater::new(8, 128, 3, 10, 1));
        let (htx, mut hrx) = channel(1024);
        let deadline = Instant::now().add(Duration::from_millis(10));

        for thread in 0..8 {
            let rater = rater.clone();
            let htxin = htx.clone();
            let hdl = tokio::task::spawn(async move {
                for i in 1..32 {
                    let name = match i {
                        i if i % 15 == 0 => "fizzbuzz".to_string(),
                        i if i % 3 == 0 => "fizz".to_string(),
                        i if i % 5 == 0 => "buzz".to_string(),
                        i => format!("{i}"),
                    };
                    let rater = rater.clone();
                    let hdl = tokio::task::spawn(async move {
                        tokio::time::sleep_until(deadline.into()).await;
                        let start = Instant::now();
                        let bucket = rater.get_limiter(name.clone());
                        let res = tokio::time::timeout(Duration::from_millis(100), bucket.acquire_owned(1)).await;
                        match res {
                            Ok(_) => tracing::info!("{thread}:{i}:{name} took {:?}!", start.elapsed()),
                            Err(_) => tracing::error!("{thread}:{i}:{name} gave up after 100ms!"),
                        }
                    });
                    htxin.send(hdl).await.unwrap();
                }
                drop(htxin);
            });
            htx.send(hdl).await.unwrap();
        }
        drop(htx);

        while let Some(hdl) = hrx.recv().await {
            hdl.await.unwrap();
        }
    }
}
