//! Proxy handling
//!
//! This module contains the primary proxying logic for River. At the moment,
//! this includes creation of HTTP proxy services, as well as Path Control
//! modifiers.

use async_trait::async_trait;

use pingora::server::Server;
use pingora_core::{services::listening::Service, upstreams::peer::HttpPeer, Result};
use pingora_http::RequestHeader;
use pingora_proxy::{HttpProxy, ProxyHttp, Session};

use crate::{
    config::internal::{ListenerKind, PathControl, ProxyConfig},
    proxy::request_modifiers::{RemoveHeaderKeyRegex, RequestModifyMod, UpsertHeader},
};

pub mod request_modifiers;

/// The [RiverProxyService] is intended to capture the behaviors used to extend
/// the [HttpProxy] functionality by providing a [ProxyHttp] trait implementation.
///
/// The [ProxyHttp] trait allows us to provide callback-like control of various stages
/// of the [request/response lifecycle].
///
/// [request/response lifecycle]: https://github.com/cloudflare/pingora/blob/7ce6f4ac1c440756a63b0766f72dbeca25c6fc94/docs/user_guide/phase_chart.md
pub struct RiverProxyService {
    /// Our single upstream server
    pub upstream: HttpPeer,
    /// All modifiers used when implementing the [ProxyHttp] trait.
    pub modifiers: Modifiers,
}

impl RiverProxyService {
    /// Create a new [RiverProxyService] from the given [ProxyConfig]
    pub fn from_basic_conf(conf: ProxyConfig, server: &Server) -> Service<HttpProxy<Self>> {
        let modifiers = Modifiers::from_conf(&conf.path_control).unwrap();

        let mut my_proxy = pingora_proxy::http_proxy_service_with_name(
            &server.configuration,
            Self {
                upstream: conf.upstream,
                modifiers,
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

        my_proxy
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
}

impl Modifiers {
    /// Build all modifiers from the provided [PathControl]
    pub fn from_conf(conf: &PathControl) -> Result<Self> {
        let mut conf = conf.clone();

        let mut upstream_request_filters: Vec<Box<dyn RequestModifyMod>> = vec![];
        for mut filter in conf.upstream_request_filters.drain(..) {
            let kind = filter.remove("kind").unwrap();
            let f: Box<dyn RequestModifyMod> = match kind.as_str() {
                "remove-header-key-regex" => {
                    Box::new(RemoveHeaderKeyRegex::from_settings(filter).unwrap())
                }
                "upsert-header" => Box::new(UpsertHeader::from_settings(filter).unwrap()),
                _ => panic!(),
            };
            upstream_request_filters.push(f);
        }

        Ok(Self {
            upstream_request_filters,
        })
    }
}

/// Per-peer context. Not currently used
pub struct RiverContext {}

#[async_trait]
impl ProxyHttp for RiverProxyService {
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
        // For now, we only support one upstream
        Ok(Box::new(self.upstream.clone()))
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
}
