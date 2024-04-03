//! This is the *actual* internal configuration structure.
//!
//! It is ONLY used for the internal configuration, and should not ever
//! be exposed as the public API for CLI, Env vars, or via Serde.
//!
//! This is used as the buffer between any external stable UI, and internal
//! impl details which may change at any time.

use std::path::PathBuf;

use pingora::server::configuration::{Opt as PingoraOpt, ServerConf as PingoraServerConf};

/// River's internal configuration
#[derive(Debug, PartialEq)]
pub struct Config {
    pub validate_configs: bool,
    pub threads_per_service: usize,
    pub basic_proxies: Vec<ProxyConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            validate_configs: false,
            threads_per_service: 8,
            basic_proxies: vec![],
        }
    }
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

//
// Basic Proxy Configuration
//

#[derive(Debug, PartialEq, Clone)]
pub struct ProxyConfig {
    pub(crate) name: String,
    pub(crate) listeners: Vec<ListenerConfig>,
}

impl From<super::toml::ProxyConfig> for ProxyConfig {
    fn from(other: super::toml::ProxyConfig) -> Self {
        Self {
            name: other.name,
            listeners: other.listeners.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TlsConfig {
    pub(crate) cert_path: PathBuf,
    pub(crate) key_path: PathBuf,
}

impl From<super::toml::TlsConfig> for TlsConfig {
    fn from(other: super::toml::TlsConfig) -> Self {
        Self {
            cert_path: other.cert_path,
            key_path: other.key_path,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ListenerConfig {
    pub(crate) source: ListenerKind,
}

impl From<super::toml::ListenerConfig> for ListenerConfig {
    fn from(other: super::toml::ListenerConfig) -> Self {
        Self {
            source: other.source.into(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum ListenerKind {
    Tcp {
        addr: String,
        tls: Option<TlsConfig>,
    },
    Uds(PathBuf),
}

impl From<super::toml::ListenerKind> for ListenerKind {
    fn from(other: super::toml::ListenerKind) -> Self {
        match other {
            super::toml::ListenerKind::Tcp { addr, tls } => ListenerKind::Tcp { addr, tls: tls.map(Into::into) },
            super::toml::ListenerKind::Uds(a) => ListenerKind::Uds(a),
        }
    }
}
