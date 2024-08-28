use std::{fmt::Debug, hash::Hash, net::IpAddr, sync::Arc, time::Duration};

use concread::arcache::{ARCache, ARCacheBuilder};
use leaky_bucket::RateLimiter;
use pandora_module_utils::pingora::SocketAddr;
use pingora_proxy::Session;
use regex::Regex;

#[derive(Debug, Clone)]
pub enum RequestKeyKind {
    SourceIp,
    DestIp,
    Uri {
        pattern: Regex
    },
}

impl PartialEq for RequestKeyKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::SourceIp, Self::SourceIp) => true,
            (Self::DestIp, Self::DestIp) => true,
            (Self::Uri { pattern: pattern1 }, Self::Uri { pattern: pattern2 }) => {
                pattern1.as_str() == pattern2.as_str()
            }
            _ => false
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestKey {
    Source(IpAddr),
    Dest(IpAddr),
    Uri(String),
}

#[derive(Debug)]
pub struct RaterInstance {
    pub rater: Rater<RequestKey>,
    pub kind: RequestKeyKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RaterInstanceConfig {
    pub rater_cfg: RaterConfig,
    pub kind: RequestKeyKind,
}

impl RaterInstance {
    pub fn get_ticket(&self, session: &mut Session) -> Option<Ticket> {
        let key = self.get_key(session)?;
        Some(self.rater.get_ticket(key))
    }

    pub fn get_key(&self, session: &mut Session) -> Option<RequestKey> {
        match &self.kind {
            RequestKeyKind::SourceIp => {
                let src = session.downstream_session.client_addr().unwrap();
                let src_ip = match src {
                    SocketAddr::Inet(addr) => addr.ip(),
                    SocketAddr::Unix(_) => return None,
                };
                Some(RequestKey::Source(src_ip))
            },
            RequestKeyKind::DestIp => None,
            RequestKeyKind::Uri { pattern } => {
                let uri_path = session.downstream_session.req_header().uri.path();
                if pattern.is_match(uri_path) {
                    Some(RequestKey::Uri(uri_path.to_string()))
                } else {
                    None
                }
            },
        }
    }
}

/// Configuration for the [`Rater`]
#[derive(Debug, PartialEq, Clone)]
pub struct RaterConfig {
    /// The number of expected concurrent threads - should match the number of
    /// tokio threadpool workers
    pub threads: usize,
    /// The peak number of leaky buckets we aim to have live at once
    ///
    /// NOTE: This is not a hard limit of the amount of memory used. See [`ARCacheBuilder`]
    /// for docs on calculating actual memory usage based on these parameters
    pub max_buckets: usize,
    /// The max and initial number of tokens in the leaky bucket - this is the number of
    /// requests that can go through without any waiting if the bucket is full
    pub max_tokens_per_bucket: usize,
    /// The interval between "refills" of the bucket, e.g. the bucket refills `refill_qty`
    /// every `refill_interval_millis`
    pub refill_interval_millis: usize,
    /// The number of tokens added to the bucket every `refill_interval_millis`
    pub refill_qty: usize,
}

/// A concurrent rate limiting structure
///
/// ## Implementation details and notes
///
/// For performance and resource reasons, this provides an *approximation* of exact rate
/// limiting. Currently, there are a few "false positive" cases that can permit more than
/// the expected number of actions to occur.
///
/// Rater is currently modeled as a Least Recently Used (LRU) cache of leaky buckets mapped
/// by a key. This is done to provide a bounded quantity of leaky buckets, without requiring
/// a worker task to "cull" the oldest buckets. Instead, unused buckets will naturally
/// "fall out" of the cache if they are not used.
///
/// ### Too many unique keys at too high of a rate
///
/// If there is a very high diversity of Keys provided, it is possible that keys could
/// be evicted from the cache before they would naturally expire or be refilled. In this
/// case, Rater will appear to not apply rate limiting, as the evicted bucket will be
/// replaced with a new, initially full bucket. This can be mitigated by choosing a
/// bucket storage capacity that is large enough to hold enough buckets to handle the
/// expected requests per second. e.g. if there is room for 1M buckets, and a bucket would
/// refill one token every 100ms, then we would expect to be able to handle at least 100K
/// requests with unique keys per second without evicting the buckets before the bucket
/// would refill anyway.
///
/// ### A burst of previously-unseen keys
///
/// If a number of requests appear around the same time for a Key that is not resident
/// in the cache, it is possible that all worker threads will create a new bucket and
/// attempt to add their buckets to the cache, though only one will be persisted, and
/// the others will be lost.
///
/// For example if there are N worker threads, and N requests with the same key arrive
/// at roughly the same time, it is possible that we will create N new leaky buckets,
/// each that will give one immediately-ready token for the request. However, in the
/// worst case (N - 1) of these tokens won't "count", as (N - 1) of these buckets
/// will be thrown away, and not counted in the one bucket that was persisted
///
/// This worst case is extremely unlikely, as it would require N requests with the same Key
/// to arrive in the time window necessary to write to the cache, and for all N requests
/// to be distributed to N different worker threads that all attempt to find the Key
/// at the same time.
///
/// There is no mitigation for this currently, other than treating the "max tokens per
/// bucket" as an approximate value, with up to "number of worker threads" of false
/// positives as an acceptable bound.
pub struct Rater<Key>
where
    Key: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
{
    cache: ARCache<Key, Arc<RateLimiter>>,
    max_tokens_per_bucket: usize,
    refill_interval_millis: usize,
    refill_qty: usize,
}

impl<Key> Debug for Rater<Key>
where
    Key: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Rater { ... }")
    }
}

impl<Key> Rater<Key>
where
    Key: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
{
    /// Create a new rate limiter with the given configuration.
    ///
    /// See [`RaterConfig`] for configuration options.
    pub fn new(config: RaterConfig) -> Self {
        let RaterConfig {
            threads,
            max_buckets,
            max_tokens_per_bucket,
            refill_interval_millis,
            refill_qty,
        } = config;
        let cache = ARCacheBuilder::new()
            .set_expected_workload(
                // total
                //
                // total number of items you want to have in memory
                max_buckets,
                // threads
                //
                // the number of read threads you expect concurrently (AT LEAST 1)
                threads.max(1),
                // ex_ro_miss
                //
                // the expected average number of cache misses per read operation
                1,
                // ex_rw_miss
                //
                // the expected average number of writes or write cache misses per operation
                1,
                // read_cache
                //
                // ?
                false,
            )
            .build()
            .expect("Creation of rate limiter should not fail");

        Self {
            cache,
            max_tokens_per_bucket,
            refill_interval_millis,
            refill_qty: refill_qty.min(max_tokens_per_bucket),
        }
    }

    /// Obtain a ticket for the given Key.
    ///
    /// If the Key does not exist already, it will be created.
    pub fn get_ticket(&self, key: Key) -> Ticket {
        let mut reader = self.cache.read();

        if let Some(find) = reader.get(&key) {
            // Rate limiter DID exist in the cache
            tracing::trace!(?key, "rate limiting cache hit",);
            Ticket {
                limiter: find.clone(),
            }
        } else {
            let new_limiter = Arc::new(self.new_rate_limiter());
            tracing::debug!(?key, "rate limiting cache miss",);
            reader.insert(key, new_limiter.clone());
            reader.finish();
            Ticket {
                limiter: new_limiter,
            }
        }
    }

    fn new_rate_limiter(&self) -> RateLimiter {
        RateLimiter::builder()
            .initial(self.max_tokens_per_bucket)
            .max(self.max_tokens_per_bucket)
            .interval(Duration::from_millis(self.refill_interval_millis as u64))
            .refill(self.refill_qty)
            .fair(true)
            .build()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum TicketError {
    BurstLimitExceeded,
}

/// A claim ticket for the leaky bucket queue
///
/// You are expected to call [`Ticket::wait()`] to wait for your turn to perform
/// the rate limited option.
#[must_use = "You must call `Ticket::wait()` to wait your turn!"]
pub struct Ticket {
    limiter: Arc<RateLimiter>,
}

impl Ticket {
    /// Wait for our "turn" granted by the leaky bucket
    ///
    /// * If the bucket has a token available, `Ok(())` will be returned immediately
    /// * If the bucket does not have a token available, this function will yield until
    ///   a token is refilled
    /// * If the bucket has too many pending waiters, then `Err(TicketError)` will be
    ///   returned immediately
    ///
    /// NOTE: In the future, we would like to be able to return immediately if there
    /// are too many pending requests at once, instead of queueing requests that are
    /// going to end up timing out anyway.
    ///
    /// However, this is not supported by the [`leaky-bucket`] crate today, enqueueing
    /// will always succeed, and we will handle this one layer up by adding a timeout
    /// to requests.
    ///
    /// This should be fixed in the future as a performance optimization, but for now
    /// we give ourselves the API surface to make this change with minimal fuss in
    /// in the future.
    pub async fn wait(self) -> Result<(), TicketError> {
        self.limiter.acquire_owned(1).await;
        Ok(())
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
        let config = RaterConfig {
            threads: 2,
            max_buckets: 5,
            max_tokens_per_bucket: 3,
            refill_interval_millis: 10,
            refill_qty: 1,
        };

        let rater = Arc::new(Rater::new(config));

        for i in 0..100 {
            let start = Instant::now();
            let bucket = rater.get_ticket("bob".to_string());
            bucket.wait().await.unwrap();
            tracing::info!("Success {i} took {:?}!", start.elapsed());
        }
    }

    #[tokio::test]
    async fn concurrent_fewer() {
        let _ = tracing_subscriber::fmt::try_init();
        let config = RaterConfig {
            threads: 8,
            max_buckets: 16,
            max_tokens_per_bucket: 3,
            refill_interval_millis: 10,
            refill_qty: 1,
        };
        let rater = Arc::new(Rater::new(config));
        let mut handles = vec![];

        for thread in 0..8 {
            let rater = rater.clone();
            let hdl = tokio::task::spawn(async move {
                for i in 1..32 {
                    let name = fizzbuzz(i);
                    let start = Instant::now();
                    let bucket = rater.get_ticket(name.clone());
                    bucket.wait().await.unwrap();
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
        let config = RaterConfig {
            threads: 8,
            max_buckets: 128,
            max_tokens_per_bucket: 3,
            refill_interval_millis: 10,
            refill_qty: 1,
        };
        let rater = Arc::new(Rater::new(config));
        let (htx, mut hrx) = channel(1024);
        let deadline = Instant::now().add(Duration::from_millis(10));

        for thread in 0..8 {
            let rater = rater.clone();
            let htxin = htx.clone();
            let hdl = tokio::task::spawn(async move {
                for i in 1..32 {
                    let name = fizzbuzz(i);
                    let rater = rater.clone();
                    let hdl = tokio::task::spawn(async move {
                        tokio::time::sleep_until(deadline.into()).await;
                        let start = Instant::now();
                        let bucket = rater.get_ticket(name.clone());
                        let res =
                            tokio::time::timeout(Duration::from_millis(100), bucket.wait()).await;
                        match res {
                            Ok(Ok(())) => {
                                tracing::info!("{thread}:{i}:{name} took {:?}!", start.elapsed())
                            }
                            Ok(Err(_)) => unreachable!(),
                            Err(_) => tracing::warn!("{thread}:{i}:{name} gave up after 100ms!"),
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

    fn fizzbuzz(i: usize) -> String {
        match i {
            i if i % 15 == 0 => "fizzbuzz".to_string(),
            i if i % 3 == 0 => "fizz".to_string(),
            i if i % 5 == 0 => "buzz".to_string(),
            i => format!("{i}"),
        }
    }
}
