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
