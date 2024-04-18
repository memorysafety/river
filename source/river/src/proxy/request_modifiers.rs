use std::collections::HashMap;

use async_trait::async_trait;
use pingora_core::{Error, Result};
use pingora_http::RequestHeader;
use pingora_proxy::Session;
use regex::Regex;

use super::MyCtx;

#[async_trait]
pub trait RequestModifyMod: Send + Sync {
    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        header: &mut RequestHeader,
        ctx: &mut MyCtx,
    ) -> Result<()>;
}

fn extract_val(key: &str, map: &mut HashMap<String, String>) -> Result<String> {
    map.remove(key)
        .ok_or_else(|| {
            // TODO: better "Error" creation
            tracing::error!("Missing key: '{key}'");
            Error::new_str("Missing configuration field!")
        })
}

fn ensure_empty(map: &HashMap<String, String>) -> Result<()> {
    if !map.is_empty() {
        let keys = map.keys().map(String::as_str).collect::<Vec<&str>>();
        let all_keys = keys.join(", ");
        tracing::error!("Extra keys found: '{all_keys}'");
        Err(Error::new_str("Extra settings found!"))
    } else {
        Ok(())
    }
}

// Remove header by key
//
//

pub struct RemoveHeaderKeyRegex {
    regex: Regex,
}

impl RemoveHeaderKeyRegex {
    pub fn from_settings(mut settings: HashMap<String, String>) -> Result<Self> {
        let mat = extract_val("pattern", &mut settings)?;

        let reg = Regex::new(&mat).map_err(|e| {
            tracing::error!("Bad pattern: '{mat}': {e:?}");
            Error::new_str("Error building regex")
        })?;

        ensure_empty(&settings)?;

        Ok(Self { regex: reg })
    }
}

#[async_trait]
impl RequestModifyMod for RemoveHeaderKeyRegex {
    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        header: &mut RequestHeader,
        _ctx: &mut MyCtx,
    ) -> Result<()> {
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

        Ok(())
    }
}

// Upsert Header
//
//

pub struct UpsertHeader {
    key: String,
    value: String,
}

impl UpsertHeader {
    pub fn from_settings(mut settings: HashMap<String, String>) -> Result<Self> {
        let key = extract_val("key", &mut settings)?;
        let value = extract_val("value", &mut settings)?;
        Ok(Self { key, value })
    }
}

#[async_trait]
impl RequestModifyMod for UpsertHeader {
    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        header: &mut RequestHeader,
        _ctx: &mut MyCtx,
    ) -> Result<()> {
        if let Some(h) = header.remove_header(&self.key) {
            tracing::debug!("Removed header: {h:?}");
        }
        header.append_header(self.key.clone(), &self.value)?;
        tracing::debug!("Inserted header: {}: {}", self.key, self.value);
        Ok(())
    }
}
