// Taken from https://github.com/cloudflare/pingora/blob/5fdf287c4d6a9ddc8a9caf3447cd27575c13a24c/pingora/examples/app/proxy.rs
//
// Copyright 2024 Cloudflare, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use async_trait::async_trait;

use pingora_core::upstreams::peer::HttpPeer;
use pingora_core::Result;
use pingora_http::RequestHeader;
use pingora_proxy::{ProxyHttp, Session};

use crate::config::internal::PathControl;

use self::request_modifiers::{RemoveHeaderKeyRegex, RequestModifyMod, UpsertHeader};

pub mod request_modifiers;

pub struct MyProxy {
    pub upstream: HttpPeer,
    pub modifiers: Modifiers,
}

pub struct Modifiers {
    pub upstream_request_filters: Vec<Box<dyn RequestModifyMod>>,
}

impl Modifiers {
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

pub struct MyCtx {}

#[async_trait]
impl ProxyHttp for MyProxy {
    type CTX = MyCtx;
    fn new_ctx(&self) -> Self::CTX {
        MyCtx {}
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        // For now, we only support one upstream
        Ok(Box::new(self.upstream.clone()))
    }

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
