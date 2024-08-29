//! Rate Limiting
//!
//! This is an implementation of request rate limiting.
//!
//! See the [`Rater`] structure for more details

use std::{fmt::Debug, hash::Hash, net::IpAddr, sync::Arc, time::Duration};

use concread::arcache::{ARCache, ARCacheBuilder};
use leaky_bucket::RateLimiter;
use pandora_module_utils::pingora::SocketAddr;
use pingora_proxy::Session;

use crate::proxy::rate_limiting::Ticket;

use super::RegexShim;

#[derive(Debug, Clone, PartialEq)]
pub struct MultiRaterInstanceConfig {
    pub rater_cfg: MultiRaterConfig,
    pub kind: MultiRequestKeyKind,
}

/// Configuration for the [`Rater`]
#[derive(Debug, PartialEq, Clone)]
pub struct MultiRaterConfig {
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

#[derive(Debug, Clone, PartialEq)]
pub enum MultiRequestKeyKind {
    SourceIp,
    Uri { pattern: RegexShim },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MultiRequestKey {
    Source(IpAddr),
    Uri(String),
}

#[derive(Debug)]
pub struct MultiRaterInstance {
    pub rater: Rater<MultiRequestKey>,
    pub kind: MultiRequestKeyKind,
}

impl MultiRaterInstance {
    pub fn new(config: MultiRaterConfig, kind: MultiRequestKeyKind) -> Self {
        Self {
            rater: Rater::new(config),
            kind,
        }
    }

    pub fn get_ticket(&self, session: &Session) -> Option<Ticket> {
        let key = self.get_key(session)?;
        Some(self.rater.get_ticket(key))
    }

    pub fn get_key(&self, session: &Session) -> Option<MultiRequestKey> {
        match &self.kind {
            MultiRequestKeyKind::SourceIp => {
                let src = session.downstream_session.client_addr()?;
                let src_ip = match src {
                    SocketAddr::Inet(addr) => addr.ip(),
                    SocketAddr::Unix(_) => return None,
                };
                Some(MultiRequestKey::Source(src_ip))
            }
            MultiRequestKeyKind::Uri { pattern } => {
                let uri_path = session.downstream_session.req_header().uri.path();
                if pattern.is_match(uri_path) {
                    Some(MultiRequestKey::Uri(uri_path.to_string()))
                } else {
                    None
                }
            }
        }
    }
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
    /// See [`MultiRaterConfig`] for configuration options.
    pub fn new(config: MultiRaterConfig) -> Self {
        let MultiRaterConfig {
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

#[cfg(test)]
mod test {
    use crate::proxy::rate_limiting::Outcome;

    use super::*;
    use std::time::Instant;
    use tokio::time::interval;

    #[tokio::test]
    async fn smoke() {
        let _ = tracing_subscriber::fmt::try_init();
        let config = MultiRaterConfig {
            threads: 2,
            max_buckets: 5,
            max_tokens_per_bucket: 3,
            refill_interval_millis: 10,
            refill_qty: 1,
        };

        let rater = Arc::new(Rater::new(config.clone()));
        let mut sleeper = interval(Duration::from_millis(6));
        let start = Instant::now();
        let mut approved = 0;
        for i in 0..100 {
            sleeper.tick().await;
            let ticket = rater.get_ticket("bob".to_string());

            match ticket.now_or_never() {
                Outcome::Approved => {
                    approved += 1;
                    tracing::info!("Approved {i}!")
                }
                Outcome::Declined => tracing::info!("Declined {i}!"),
            }
        }
        let duration = start.elapsed();
        let duration = duration.as_secs_f64();
        let approved = approved as f64;

        let expected_rate = 1000.0f64 / config.refill_interval_millis as f64;
        let expected_ttl = (duration * expected_rate) + config.max_tokens_per_bucket as f64;

        // Did we get +/-10% of the expected number of approvals?
        tracing::info!(expected_ttl, actual_ttl = approved, "Rates");
        assert!(approved > (expected_ttl * 0.9f64));
        assert!(approved < (expected_ttl * 1.1f64));
    }
}
