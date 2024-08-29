use std::{ops::Deref, sync::Arc};

use leaky_bucket::RateLimiter;
use regex::Regex;

use self::{
    multi::{MultiRaterConfig, MultiRequestKeyKind},
    single::{SingleInstanceConfig, SingleRequestKeyKind},
};

//
// We have two kinds of rate limiters:
//
// * "Multi" rate limiters use a cache of buckets. These are used when we remember
//   multiple bucket keys, like tracking all of the source IP addresses
// * "Single" rate limiters use a single bucket, for example `any-matching-uri`,
//   which uses a single bucket for all matching URIs
pub mod multi;
pub mod single;

#[derive(Debug, PartialEq, Clone)]
pub enum AllRateConfig {
    Single {
        kind: SingleRequestKeyKind,
        config: SingleInstanceConfig,
    },
    Multi {
        kind: MultiRequestKeyKind,
        config: MultiRaterConfig,
    },
}

#[derive(Debug, Clone)]
pub struct RegexShim(pub Regex);

impl PartialEq for RegexShim {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str().eq(other.0.as_str())
    }
}

impl Deref for RegexShim {
    type Target = Regex;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl RegexShim {
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self(Regex::new(pattern)?))
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Outcome {
    Approved,
    Declined,
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
    /// Try to get a token immediately from the bucket.
    pub fn now_or_never(self) -> Outcome {
        if self.limiter.try_acquire(1) {
            Outcome::Approved
        } else {
            Outcome::Declined
        }
    }
}
