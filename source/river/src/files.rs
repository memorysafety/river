//! File Serving

use std::ops::{Deref, DerefMut};

use pandora_module_utils::{pingora::SessionWrapper, RequestFilter, RequestFilterResult};
use pingora::{server::Server, upstreams::peer::HttpPeer};
use pingora_core::Result;
use pingora_proxy::{ProxyHttp, Session};
use static_files_module::{StaticFilesConf, StaticFilesHandler};

use crate::{config::internal::FileServerConfig, populate_listners};

/// Create a new file serving service
pub fn river_file_server(
    conf: FileServerConfig,
    server: &Server,
) -> Box<dyn pingora::services::Service> {
    let fsconf = StaticFilesConf {
        root: conf.base_path,
        canonicalize_uri: true,
        index_file: Vec::new().into(),
        page_404: None,
        precompressed: Vec::new().into(),
        ..Default::default()
    };
    let file_server = FileServer {
        server: StaticFilesHandler::try_from(fsconf)
            .expect("Creation of a Static File Service should not fail"),
    };
    let mut my_proxy =
        pingora_proxy::http_proxy_service_with_name(&server.configuration, file_server, &conf.name);

    populate_listners(conf.listeners, &mut my_proxy);

    Box::new(my_proxy)
}

pub struct FileServer {
    pub server: StaticFilesHandler,
}

/// Implementation detail for integrating pingora-web-server's file server
///
/// This wraps the [Session] provided by pingora in a way necessary for pandora.
pub struct SesWrap<'a> {
    extensions: &'a mut http::Extensions,
    session: &'a mut Session,
}

#[async_trait::async_trait]
impl<'a> SessionWrapper for SesWrap<'a> {
    fn extensions(&self) -> &http::Extensions {
        self.extensions
    }

    fn extensions_mut(&mut self) -> &mut http::Extensions {
        self.extensions
    }
}

impl<'a> Deref for SesWrap<'a> {
    type Target = Session;

    fn deref(&self) -> &Self::Target {
        &*self.session
    }
}

impl<'a> DerefMut for SesWrap<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.session
    }
}

/// A small wrapper for delegating requests to a file server
#[async_trait::async_trait]
impl ProxyHttp for FileServer {
    type CTX = http::Extensions;

    fn new_ctx(&self) -> Self::CTX {
        http::Extensions::new()
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        // This should never happen - we fully handle the request at the
        // `request_filter` stage, so no requests should make it to the
        // later `upstream_peer` stage.
        Err(pingora_core::Error::new_str("Request Failed"))
    }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let mut wrap = SesWrap {
            extensions: ctx,
            session,
        };
        match self.server.request_filter(&mut wrap, &mut ()).await? {
            RequestFilterResult::ResponseSent => Ok(true),
            _ => Err(pingora_core::Error::new_str("Request Failed")),
        }
    }
}
