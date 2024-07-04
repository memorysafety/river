use std::ops::{Deref, DerefMut};

use pandora_module_utils::{pingora::SessionWrapper, OneOrMany, RequestFilter, RequestFilterResult};
use pingora::{server::Server, upstreams::peer::HttpPeer};
use pingora_core::Result;
use pingora_proxy::{ProxyHttp, Session};
use static_files_module::{StaticFilesConf, StaticFilesHandler};

use crate::{config::internal::FileServerConfig, populate_listners};

pub fn river_file_server(
    conf: FileServerConfig,
    server: &Server,
) -> Box<dyn pingora::services::Service> {
    let fsconf = StaticFilesConf {
        root: conf.base_path,
        canonicalize_uri: true,
        index_file: OneOrMany::from(Vec::new()),
        page_404: None,
        precompressed: OneOrMany::from(Vec::new()),
    };
    let file_server = FileServer {
        server: StaticFilesHandler::try_from(fsconf).unwrap(),
    };
    let mut my_proxy = pingora_proxy::http_proxy_service_with_name(
        &server.configuration,
        file_server,
        &conf.name,
    );

    populate_listners(conf.listeners, &mut my_proxy);

    Box::new(my_proxy)
}

pub struct FileServer {
    pub server: StaticFilesHandler,
}

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
        Err(pingora_core::Error::new_str("Request Failed1"))
    }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let mut wrap = SesWrap {
            extensions: ctx,
            session,
        };
        match self.server.request_filter(&mut wrap, &mut ()).await? {
            RequestFilterResult::ResponseSent => Ok(true),
            RequestFilterResult::Handled => {
                Err(pingora_core::Error::new_str("Request Failed2"))
            },
            RequestFilterResult::Unhandled => {
                Err(pingora_core::Error::new_str("Request Failed3"))
            }
        }
    }
}
