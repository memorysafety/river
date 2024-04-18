//! This is the *actual* internal configuration structure.
//!
//! It is ONLY used for the internal configuration, and should not ever
//! be exposed as the public API for CLI, Env vars, or via Serde.
//!
//! This is used as the buffer between any external stable UI, and internal
//! impl details which may change at any time.

use std::{collections::BTreeMap, path::PathBuf};

use pingora::{
    server::configuration::{Opt as PingoraOpt, ServerConf as PingoraServerConf},
    upstreams::peer::HttpPeer,
};

/// River's internal configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub validate_configs: bool,
    pub threads_per_service: usize,
    pub basic_proxies: Vec<ProxyConfig>,
}

impl Config {
    /// Get the [`Opt`][PingoraOpt] field for Pingora
    pub fn pingora_opt(&self) -> PingoraOpt {
        // TODO
        PingoraOpt {
            upgrade: false,
            daemon: false,
            nocapture: false,
            test: self.validate_configs,
            conf: None,
        }
    }

    /// Get the [`ServerConf`][PingoraServerConf] field for Pingora
    pub fn pingora_server_conf(&self) -> PingoraServerConf {
        PingoraServerConf {
            daemon: false,
            error_log: None,
            pid_file: String::from("./target/pidfile"),
            upgrade_sock: String::from("./target/upgrade"),
            user: None,
            group: None,
            threads: self.threads_per_service,
            work_stealing: true,
            ca_file: None,
            ..PingoraServerConf::default()
        }
    }

    pub fn validate(&self) {
        // TODO: validation logic
    }
}

/// Add Path Control Modifiers
///
/// Note that we use `BTreeMap` and NOT `HashMap`, as we want to maintain the
/// ordering from the configuration file.
#[derive(Debug, Clone)]
pub struct PathControl {
    pub(crate) upstream_request_filters: Vec<BTreeMap<String, String>>,
}

//
// Basic Proxy Configuration
//

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub(crate) name: String,
    pub(crate) listeners: Vec<ListenerConfig>,
    pub(crate) upstream: HttpPeer,
    pub(crate) path_control: PathControl,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TlsConfig {
    pub(crate) cert_path: PathBuf,
    pub(crate) key_path: PathBuf,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ListenerConfig {
    pub(crate) source: ListenerKind,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ListenerKind {
    Tcp {
        addr: String,
        tls: Option<TlsConfig>,
    },
    Uds(PathBuf),
}

//
// Boilerplate trait impls
//

impl Default for Config {
    fn default() -> Self {
        Self {
            validate_configs: false,
            threads_per_service: 8,
            basic_proxies: vec![],
        }
    }
}

impl From<super::toml::ProxyConfig> for ProxyConfig {
    fn from(other: super::toml::ProxyConfig) -> Self {
        Self {
            name: other.name,
            listeners: other.listeners.into_iter().map(Into::into).collect(),
            upstream: other.connector.into(),
            path_control: other.path_control.into(),
        }
    }
}

impl From<super::toml::PathControl> for PathControl {
    fn from(value: super::toml::PathControl) -> Self {
        Self {
            upstream_request_filters: value.upstream_request_filters,
        }
    }
}

impl From<super::toml::ListenerTlsConfig> for TlsConfig {
    fn from(other: super::toml::ListenerTlsConfig) -> Self {
        Self {
            cert_path: other.cert_path,
            key_path: other.key_path,
        }
    }
}

impl From<super::toml::ListenerConfig> for ListenerConfig {
    fn from(other: super::toml::ListenerConfig) -> Self {
        Self {
            source: other.source.into(),
        }
    }
}

impl From<super::toml::ListenerKind> for ListenerKind {
    fn from(other: super::toml::ListenerKind) -> Self {
        match other {
            super::toml::ListenerKind::Tcp { addr, tls } => ListenerKind::Tcp {
                addr,
                tls: tls.map(Into::into),
            },
            super::toml::ListenerKind::Uds(a) => ListenerKind::Uds(a),
        }
    }
}
