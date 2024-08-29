use std::ops::Deref;

use regex::Regex;

pub mod multi;

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

#[derive(Debug, Clone, PartialEq)]
pub enum AllRequestKeyKind {
    SourceIp,
    Uri { pattern: RegexShim },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RaterInstanceConfig {
    pub rater_cfg: RaterConfig,
    pub kind: AllRequestKeyKind,
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
