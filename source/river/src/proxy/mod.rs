//! Proxy handling
//!
//! This module contains the primary proxying logic for River. At the moment,
//! this includes creation of HTTP proxy services, as well as Path Control
//! modifiers.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use async_trait::async_trait;
use futures_util::FutureExt;

use pingora::{server::Server, Error, ErrorType};
use pingora_core::{upstreams::peer::HttpPeer, Result};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_load_balancing::{
    discovery,
    selection::{
        consistent::KetamaHashing, BackendIter, BackendSelection, FVNHash, Random, RoundRobin,
    },
    Backend, Backends, LoadBalancer,
};
use pingora_proxy::{ProxyHttp, Session};
use tokio::task::JoinSet;

use crate::{
    config::internal::{PathControl, ProxyConfig, SelectionKind},
    populate_listners,
    proxy::{
        request_modifiers::RequestModifyMod, request_selector::RequestSelector,
        response_modifiers::ResponseModifyMod,
    },
};

use self::{
    rate_limiting::{Rater, RaterInstance, RequestKeyKind, TicketError},
    request_filters::RequestFilterMod,
};

pub mod rate_limiting;
pub mod request_filters;
pub mod request_modifiers;
pub mod request_selector;
pub mod response_modifiers;

pub struct RateLimiters {
    request_filter_stage: Vec<RaterInstance>,
}

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
    pub request_selector: RequestSelector,
    pub rate_limiting_timeout: Duration,
    pub rate_limiters: RateLimiters,
}

/// Create a proxy service, with the type parameters chosen based on the config file
pub fn river_proxy_service(
    conf: ProxyConfig,
    server: &Server,
) -> Box<dyn pingora::services::Service> {
    // Pick the correctly monomorphized function. This makes the functions all have the
    // same signature of `fn(...) -> Box<dyn Service>`.
    type ServiceMaker = fn(ProxyConfig, &Server) -> Box<dyn pingora::services::Service>;

    let service_maker: ServiceMaker = match conf.upstream_options.selection {
        SelectionKind::RoundRobin => RiverProxyService::<RoundRobin>::from_basic_conf,
        SelectionKind::Random => RiverProxyService::<Random>::from_basic_conf,
        SelectionKind::Fnv => RiverProxyService::<FVNHash>::from_basic_conf,
        SelectionKind::Ketama => RiverProxyService::<KetamaHashing>::from_basic_conf,
    };
    service_maker(conf, server)
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

        // TODO: This maybe could be done cleaner? This is a sort-of inlined
        // version of `LoadBalancer::try_from_iter` with the ability to add
        // metadata extensions
        let mut backends = BTreeSet::new();
        for uppy in conf.upstreams {
            let mut backend = Backend::new(&uppy._address.to_string()).unwrap();
            assert!(backend.ext.insert::<HttpPeer>(uppy).is_none());
            backends.insert(backend);
        }
        let disco = discovery::Static::new(backends);
        let upstreams = LoadBalancer::<BS>::from_backends(Backends::new(disco));
        upstreams
            .update()
            .now_or_never()
            .expect("static should not block")
            .expect("static should not error");
        // end of TODO

        let request_filter_stage = conf
            .rate_limiting
            .rules
            .iter()
            .filter_map(|cfg| match &cfg.kind {
                yes @ RequestKeyKind::SourceIp | yes @ RequestKeyKind::Uri { .. } => {
                    Some(RaterInstance {
                        rater: Rater::new(cfg.rater_cfg.clone()),
                        kind: yes.clone(),
                    })
                }
                RequestKeyKind::DestIp => None,
            })
            .collect();

        let mut my_proxy = pingora_proxy::http_proxy_service_with_name(
            &server.configuration,
            Self {
                modifiers,
                load_balancer: upstreams,
                request_selector: conf.upstream_options.selector,
                rate_limiting_timeout: conf
                    .rate_limiting
                    .timeout_ms
                    .map(|v| Duration::from_millis(v as u64))
                    .unwrap_or(Duration::from_secs(1)),
                rate_limiters: RateLimiters {
                    request_filter_stage,
                },
            },
            &conf.name,
        );

        populate_listners(conf.listeners, &mut my_proxy);

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
    /// Filters used during the handling of [ProxyHttp::request_filter]
    pub request_filters: Vec<Box<dyn RequestFilterMod>>,
    /// Filters used during the handling of [ProxyHttp::upstream_request_filter]
    pub upstream_request_filters: Vec<Box<dyn RequestModifyMod>>,
    /// Filters used during the handling of [ProxyHttp::upstream_response_filter]
    pub upstream_response_filters: Vec<Box<dyn ResponseModifyMod>>,
}

impl Modifiers {
    /// Build all modifiers from the provided [PathControl]
    pub fn from_conf(conf: &PathControl) -> Result<Self> {
        let mut conf = conf.clone();

        let mut request_filter_mods: Vec<Box<dyn RequestFilterMod>> = vec![];
        for mut filter in conf.request_filters.drain(..) {
            let kind = filter.remove("kind").unwrap();
            let f: Box<dyn RequestFilterMod> = match kind.as_str() {
                "block-cidr-range" => {
                    Box::new(request_filters::CidrRangeFilter::from_settings(filter).unwrap())
                }
                other => {
                    tracing::warn!("Unknown request filter: '{other}'");
                    return Err(Error::new(ErrorType::Custom("Bad configuration")));
                }
            };
            request_filter_mods.push(f);
        }

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
                other => {
                    tracing::warn!("Unknown upstream request filter: '{other}'");
                    return Err(Error::new(ErrorType::Custom("Bad configuration")));
                }
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
                other => {
                    tracing::warn!("Unknown upstream response filter: '{other}'");
                    return Err(Error::new(ErrorType::Custom("Bad configuration")));
                }
            };
            upstream_response_filters.push(f);
        }

        Ok(Self {
            request_filters: request_filter_mods,
            upstream_request_filters,
            upstream_response_filters,
        })
    }
}

/// Per-peer context. Not currently used
pub struct RiverContext {
    selector_buf: Vec<u8>,
}

#[async_trait]
impl<BS> ProxyHttp for RiverProxyService<BS>
where
    BS: BackendSelection + Send + Sync + 'static,
    BS::Iter: BackendIter,
{
    type CTX = RiverContext;

    fn new_ctx(&self) -> Self::CTX {
        RiverContext {
            selector_buf: Vec::new(),
        }
    }

    /// Handle the "Request filter" stage
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool>
    where
        Self::CTX: Send + Sync,
    {
        // First: do rate limiting at this stage - quickly check if there are none and skip over
        // entirely if so
        if !self.rate_limiters.request_filter_stage.is_empty() {
            // Use a joinset to capture all applicable rate limiters
            //
            // TODO: avoid spawning one task per limiter?
            let mut set: JoinSet<Result<(), TicketError>> = JoinSet::new();
            for limiter in self.rate_limiters.request_filter_stage.iter() {
                if let Some(ticket) = limiter.get_ticket(session) {
                    set.spawn(ticket.wait());
                }
            }

            // This future returns when either ALL tickets have been obtained, OR
            // when any ticket returns an error
            let total_ticket_fut = async move {
                while let Some(res) = set.join_next().await {
                    match res {
                        Ok(Ok(())) => {}
                        Ok(Err(_e)) => return Err(()),
                        Err(_) => return Err(()),
                    }
                }
                Ok(())
            };

            // Cap the waiting time in case all buckets are too full
            //
            // TODO: This is a workaround for not having the ability to immediately bail
            // if buckets are "too full"!
            let res = tokio::time::timeout(self.rate_limiting_timeout, total_ticket_fut).await;
            match res {
                Ok(Ok(())) => {
                    // Rate limiter completed!
                }
                Ok(Err(())) => {
                    tracing::trace!("Rejecting due to rate limiting failure");
                    session.downstream_session.respond_error(429).await;
                    return Ok(true);
                }
                Err(_) => {
                    tracing::trace!("Rejecting due to rate limiting timeout");
                    session.downstream_session.respond_error(429).await;
                    return Ok(true);
                }
            }
        }

        for filter in &self.modifiers.request_filters {
            match filter.request_filter(session, ctx).await {
                // If Ok true: we're done handling this request
                o @ Ok(true) => return o,
                // If Err: we return that
                e @ Err(_) => return e,
                // If Ok(false), we move on to the next filter
                Ok(false) => {}
            }
        }
        Ok(false)
    }

    /// Handle the "upstream peer" phase, where we pick which upstream to proxy to.
    ///
    /// At the moment, we don't support more than one upstream peer, so this choice
    /// is fairly easy!
    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let key = (self.request_selector)(ctx, session);

        let backend = self.load_balancer.select(key, 256);

        // Manually clear the selector buf to avoid accidental leaks
        ctx.selector_buf.clear();

        let backend =
            backend.ok_or_else(|| pingora::Error::new_str("Unable to determine backend"))?;

        // Retrieve the HttpPeer from the associated backend metadata
        backend
            .ext
            .get::<HttpPeer>()
            .map(|p| Box::new(p.clone()))
            .ok_or_else(|| pingora::Error::new_str("Fatal: Missing selected backend metadata"))
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
