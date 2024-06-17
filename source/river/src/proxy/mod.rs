//! Proxy handling
//!
//! This module contains the primary proxying logic for River. At the moment,
//! this includes creation of HTTP proxy services, as well as Path Control
//! modifiers.

use std::collections::BTreeMap;

use async_trait::async_trait;

use pingora::{server::Server, Error};
use pingora_core::{upstreams::peer::HttpPeer, Result};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_load_balancing::{
    selection::{BackendIter, BackendSelection},
    LoadBalancer,
};
use pingora_proxy::{ProxyHttp, Session};

use crate::{
    config::internal::{ListenerKind, PathControl, ProxyConfig},
    proxy::request_modifiers::RequestModifyMod,
};

use self::response_modifiers::ResponseModifyMod;

pub mod request_modifiers;
pub mod response_modifiers;

/// The [RiverProxyService] is intended to capture the behaviors used to extend
/// the [HttpProxy] functionality by providing a [ProxyHttp] trait implementation.
///
/// The [ProxyHttp] trait allows us to provide callback-like control of various stages
/// of the [request/response lifecycle].
///
/// [request/response lifecycle]: https://github.com/cloudflare/pingora/blob/7ce6f4ac1c440756a63b0766f72dbeca25c6fc94/docs/user_guide/phase_chart.md
pub struct RiverProxyService<BS: BackendSelection> {
    /// All modifiers used when implementing the [ProxyHttp] trait.
    pub modifiers: Modifiers,
    /// Load Balancer
    pub load_balancer: LoadBalancer<BS>,
}

impl<BS> RiverProxyService<BS>
where
    BS: BackendSelection + Send + Sync + 'static,
    BS::Iter: BackendIter,
{
    /// Create a new [RiverProxyService] from the given [ProxyConfig]
    pub fn from_basic_conf(
        conf: ProxyConfig,
        server: &Server,
    ) -> Box<dyn pingora::services::Service> {
        let modifiers = Modifiers::from_conf(&conf.path_control).unwrap();

        let upstreams =
            LoadBalancer::<BS>::try_from_iter(conf.upstreams.iter().map(|u| u._address.clone()))
                .unwrap();

        let mut my_proxy = pingora_proxy::http_proxy_service_with_name(
            &server.configuration,
            Self {
                modifiers,
                load_balancer: upstreams,
            },
            &conf.name,
        );

        for list_cfg in conf.listeners {
            // NOTE: See https://github.com/cloudflare/pingora/issues/182 for tracking "paths aren't
            // always UTF-8 strings".
            //
            // See also https://github.com/cloudflare/pingora/issues/183 for tracking "ip addrs shouldn't
            // be strings"
            match list_cfg.source {
                ListenerKind::Tcp {
                    addr,
                    tls: Some(tls_cfg),
                } => {
                    let cert_path = tls_cfg
                        .cert_path
                        .to_str()
                        .expect("cert path should be utf8");
                    let key_path = tls_cfg.key_path.to_str().expect("key path should be utf8");
                    my_proxy
                        .add_tls(&addr, cert_path, key_path)
                        .expect("adding TLS listener shouldn't fail");
                }
                ListenerKind::Tcp { addr, tls: None } => {
                    my_proxy.add_tcp(&addr);
                }
                ListenerKind::Uds(path) => {
                    let path = path.to_str().unwrap();
                    my_proxy.add_uds(path, None); // todo
                }
            }
        }

        Box::new(my_proxy)
    }
}

//
// MODIFIERS
//
// This section implements "Path Control Modifiers". As an overview of the initially
// planned control points:
//
//             ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐  ┌ ─ ─ ─ ─ ─ ─ ┐
//                  ┌───────────┐    ┌───────────┐    ┌───────────┐
//             │    │  Request  │    │           │    │  Request  │    │  │             │
// Request  ═══════▶│  Arrival  │═══▶│Which Peer?│═══▶│ Forwarded │═══════▶
//             │    │           │    │           │    │           │    │  │             │
//                  └───────────┘    └───────────┘    └───────────┘
//             │          │                │                │          │  │             │
//                        │                │                │
//             │          ├───On Error─────┼────────────────┤          │  │  Upstream   │
//                        │                │                │
//             │          │          ┌───────────┐    ┌───────────┐    │  │             │
//                        ▼          │ Response  │    │ Response  │
//             │                     │Forwarding │    │  Arrival  │    │  │             │
// Response ◀════════════════════════│           │◀═══│           │◀═══════
//             │                     └───────────┘    └───────────┘    │  │             │
//               ┌────────────────────────┐
//             └ ┤ Simplified Phase Chart │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘  └ ─ ─ ─ ─ ─ ─ ┘
//               └────────────────────────┘
//
// At the moment, "Request Forwarded" corresponds with "upstream_request_filters".
//

/// All modifiers used when implementing the [ProxyHttp] trait.
pub struct Modifiers {
    /// Filters used during the handling of [ProxyHttp::upstream_request_filter]
    pub upstream_request_filters: Vec<Box<dyn RequestModifyMod>>,
    pub upstream_response_filters: Vec<Box<dyn ResponseModifyMod>>,
}

impl Modifiers {
    /// Build all modifiers from the provided [PathControl]
    pub fn from_conf(conf: &PathControl) -> Result<Self> {
        let mut conf = conf.clone();

        let mut upstream_request_filters: Vec<Box<dyn RequestModifyMod>> = vec![];
        for mut filter in conf.upstream_request_filters.drain(..) {
            let kind = filter.remove("kind").unwrap();
            let f: Box<dyn RequestModifyMod> = match kind.as_str() {
                "remove-header-key-regex" => Box::new(
                    request_modifiers::RemoveHeaderKeyRegex::from_settings(filter).unwrap(),
                ),
                "upsert-header" => {
                    Box::new(request_modifiers::UpsertHeader::from_settings(filter).unwrap())
                }
                _ => panic!(),
            };
            upstream_request_filters.push(f);
        }

        let mut upstream_response_filters: Vec<Box<dyn ResponseModifyMod>> = vec![];
        for mut filter in conf.upstream_response_filters.drain(..) {
            let kind = filter.remove("kind").unwrap();
            let f: Box<dyn ResponseModifyMod> = match kind.as_str() {
                "remove-header-key-regex" => Box::new(
                    response_modifiers::RemoveHeaderKeyRegex::from_settings(filter).unwrap(),
                ),
                "upsert-header" => {
                    Box::new(response_modifiers::UpsertHeader::from_settings(filter).unwrap())
                }
                _ => panic!(),
            };
            upstream_response_filters.push(f);
        }

        Ok(Self {
            upstream_request_filters,
            upstream_response_filters,
        })
    }
}

/// Per-peer context. Not currently used
pub struct RiverContext {}

#[async_trait]
impl<BS> ProxyHttp for RiverProxyService<BS>
where
    BS: BackendSelection + Send + Sync + 'static,
    BS::Iter: BackendIter,
{
    type CTX = RiverContext;

    fn new_ctx(&self) -> Self::CTX {
        RiverContext {}
    }

    /// Handle the "upstream peer" phase, where we pick which upstream to proxy to.
    ///
    /// At the moment, we don't support more than one upstream peer, so this choice
    /// is fairly easy!
    async fn upstream_peer(
        &self,
        _session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let backend = self
            .load_balancer
            .select(
                b"", // TODO: Is this configurable?
                256,
            )
            .ok_or_else(|| pingora::Error::new_str("oops"))?;

        // For now, we only support one upstream
        Ok(Box::new(HttpPeer::new(backend, true, "wrong".to_string())))
    }

    /// Handle the "upstream request filter" phase, where we can choose to make
    /// modifications to the request, prior to it being passed along to the
    /// upstream.
    ///
    /// We can also *reject* requests here, though in the future we might do that
    /// via the `request_filter` stage, as that rejection can be done prior to
    /// paying any potential cost `upstream_peer` may incur.
    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        header: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        for filter in &self.modifiers.upstream_request_filters {
            filter.upstream_request_filter(session, header, ctx).await?;
        }
        Ok(())
    }

    /// Handle the "upstream response filter" phase, where we can choose to make
    /// modifications to the response, prior to it being passed along downstream
    ///
    /// We may want to also support `upstream_response` stage, as that may interact
    /// with cache differently.
    fn upstream_response_filter(
        &self,
        session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) {
        for filter in &self.modifiers.upstream_response_filters {
            filter.upstream_response_filter(session, upstream_response, ctx);
        }
    }
}

/// Helper function that extracts the value of a given key.
///
/// Returns an error if the key does not exist
fn extract_val(key: &str, map: &mut BTreeMap<String, String>) -> Result<String> {
    map.remove(key).ok_or_else(|| {
        // TODO: better "Error" creation
        tracing::error!("Missing key: '{key}'");
        Error::new_str("Missing configuration field!")
    })
}

/// Helper function to make sure the map is empty
///
/// This is used to reject unknown configuration keys
fn ensure_empty(map: &BTreeMap<String, String>) -> Result<()> {
    if !map.is_empty() {
        let keys = map.keys().map(String::as_str).collect::<Vec<&str>>();
        let all_keys = keys.join(", ");
        tracing::error!("Extra keys found: '{all_keys}'");
        Err(Error::new_str("Extra settings found!"))
    } else {
        Ok(())
    }
}
