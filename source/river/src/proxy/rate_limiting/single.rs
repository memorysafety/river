use std::{sync::Arc, time::Duration};

use leaky_bucket::RateLimiter;
use pingora_proxy::Session;

use super::{RegexShim, Ticket};

#[derive(Debug, PartialEq, Clone)]
pub struct SingleInstanceConfig {
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
pub enum SingleRequestKeyKind {
    UriGroup { pattern: RegexShim },
}

#[derive(Debug)]
pub struct SingleInstance {
    pub limiter: Arc<RateLimiter>,
    pub kind: SingleRequestKeyKind,
}

impl SingleInstance {
    /// Create a new rate limiter with the given configuration.
    ///
    /// See [`SingleInstanceConfig`] for configuration options.
    pub fn new(config: SingleInstanceConfig, kind: SingleRequestKeyKind) -> Self {
        let SingleInstanceConfig {
            max_tokens_per_bucket,
            refill_interval_millis,
            refill_qty,
        } = config;

        let limiter = RateLimiter::builder()
            .initial(max_tokens_per_bucket)
            .max(max_tokens_per_bucket)
            .interval(Duration::from_millis(refill_interval_millis as u64))
            .refill(refill_qty)
            .fair(true)
            .build();
        let limiter = Arc::new(limiter);

        Self { limiter, kind }
    }

    pub fn get_ticket(&self, session: &Session) -> Option<Ticket> {
        match &self.kind {
            SingleRequestKeyKind::UriGroup { pattern } => {
                let uri_path = session.downstream_session.req_header().uri.path();
                if pattern.is_match(uri_path) {
                    Some(Ticket {
                        limiter: self.limiter.clone(),
                    })
                } else {
                    None
                }
            }
        }
    }
}
