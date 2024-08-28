use std::collections::BTreeMap;

use async_trait::async_trait;
use cidr::IpCidr;
use pingora::ErrorType;
use pingora_core::{protocols::l4::socket::SocketAddr, Error, Result};
use pingora_proxy::Session;

use crate::proxy::{extract_val, RiverContext};

/// This is a single-serving trait for modifiers that provide actions for
/// [ProxyHttp::request_filter] methods
#[async_trait]
pub trait RequestFilterMod: Send + Sync {
    /// See [ProxyHttp::request_filter] for more details
    async fn request_filter(&self, session: &mut Session, ctx: &mut RiverContext) -> Result<bool>;
}

pub struct CidrRangeFilter {
    blocks: Vec<IpCidr>,
}

impl CidrRangeFilter {
    /// Create from the settings field
    pub fn from_settings(mut settings: BTreeMap<String, String>) -> Result<Self> {
        let mat = extract_val("addrs", &mut settings)?;

        let addrs = mat.split(',');

        let mut blocks = vec![];
        for addr in addrs {
            let addr = addr.trim();
            match addr.parse::<IpCidr>() {
                Ok(a) => {
                    blocks.push(a);
                }
                Err(_) => {
                    tracing::error!("Failed to parse '{addr}' as a valid CIDR notation range");
                    return Err(Error::new(ErrorType::Custom("Invalid configuration")));
                }
            };
        }

        Ok(Self { blocks })
    }
}

#[async_trait]
impl RequestFilterMod for CidrRangeFilter {
    async fn request_filter(&self, session: &mut Session, _ctx: &mut RiverContext) -> Result<bool> {
        let Some(addr) = session.downstream_session.client_addr() else {
            // Unable to determine source address, assuming it should be blocked
            session.downstream_session.respond_error(401).await;
            return Ok(true);
        };
        let SocketAddr::Inet(addr) = addr else {
            // CIDR filters don't apply to UDS
            return Ok(false);
        };
        let ip_addr = addr.ip();

        if self.blocks.iter().any(|b| b.contains(&ip_addr)) {
            session.downstream_session.respond_error(401).await;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
