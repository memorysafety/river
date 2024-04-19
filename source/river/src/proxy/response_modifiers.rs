use std::collections::BTreeMap;

use pingora_core::{Error, Result};
use pingora_http::ResponseHeader;
use pingora_proxy::Session;
use regex::Regex;

use super::{ensure_empty, extract_val, RiverContext};

/// This is a single-serving trait for modifiers that provide actions for
/// [ProxyHttp::upstream_response_filter] methods
pub trait ResponseModifyMod: Send + Sync {
    /// See [ProxyHttp::upstream_response_filter] for more details
    fn upstream_response_filter(
        &self,
        session: &mut Session,
        header: &mut ResponseHeader,
        ctx: &mut RiverContext,
    );
}

// Remove header by key
//
//

/// Removes a header if the key matches a given regex
pub struct RemoveHeaderKeyRegex {
    regex: Regex,
}

impl RemoveHeaderKeyRegex {
    /// Create from the settings field
    pub fn from_settings(mut settings: BTreeMap<String, String>) -> Result<Self> {
        let mat = extract_val("pattern", &mut settings)?;

        let reg = Regex::new(&mat).map_err(|e| {
            tracing::error!("Bad pattern: '{mat}': {e:?}");
            Error::new_str("Error building regex")
        })?;

        ensure_empty(&settings)?;

        Ok(Self { regex: reg })
    }
}

impl ResponseModifyMod for RemoveHeaderKeyRegex {
    fn upstream_response_filter(
        &self,
        _session: &mut Session,
        header: &mut ResponseHeader,
        _ctx: &mut RiverContext,
    ) {
        // Find all the headers that have keys that match the regex...
        let headers = header
            .headers
            .keys()
            .filter_map(|k| {
                if self.regex.is_match(k.as_str()) {
                    tracing::debug!("Removing header: {k:?}");
                    Some(k.to_owned())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // ... and remove them
        for h in headers {
            assert!(header.remove_header(&h).is_some());
        }
    }
}

// Upsert Header
//
//

/// Adds or replaces a given header key and value
pub struct UpsertHeader {
    key: String,
    value: String,
}

impl UpsertHeader {
    /// Create from the settings field
    pub fn from_settings(mut settings: BTreeMap<String, String>) -> Result<Self> {
        let key = extract_val("key", &mut settings)?;
        let value = extract_val("value", &mut settings)?;
        Ok(Self { key, value })
    }
}

impl ResponseModifyMod for UpsertHeader {
    fn upstream_response_filter(
        &self,
        _session: &mut Session,
        header: &mut ResponseHeader,
        _ctx: &mut RiverContext,
    ) {
        if let Some(h) = header.remove_header(&self.key) {
            tracing::debug!("Removed header: {h:?}");
        }
        let _ = header.append_header(self.key.clone(), &self.value);
        tracing::debug!("Inserted header: {}: {}", self.key, self.value);
    }
}
