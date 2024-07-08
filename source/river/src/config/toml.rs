//! Configuration sourced from a TOML file

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use pingora::upstreams::peer::HttpPeer;
use serde::{Deserialize, Serialize};

use super::internal::UpstreamOptions;

/// Configuration used for TOML formatted files
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Toml {
    /// System-wide configuration valies
    #[serde(default)]
    pub system: System,

    /// Configuration for each Basic Proxy instance
    #[serde(default = "Vec::new")]
    pub basic_proxy: Vec<ProxyConfig>,
}

impl Toml {
    pub fn from_path<P>(path: &P) -> Self
    where
        P: AsRef<Path> + core::fmt::Debug + ?Sized,
    {
        tracing::info!("Loading TOML from {path:?}");
        let f = std::fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("Failed to load file at {path:?}"));
        let t = ::toml::from_str(&f).expect("failed to deserialize");
        tracing::info!("TOML file contents: {t:?}");
        t
    }
}

//
// System Config
//

/// System level configuration options
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct System {
    #[serde(default = "System::default_threads_per_service")]
    pub threads_per_service: usize,
}

impl Default for System {
    fn default() -> Self {
        System {
            threads_per_service: Self::default_threads_per_service(),
        }
    }
}

impl System {
    fn default_threads_per_service() -> usize {
        8
    }
}

/// Add Path Control Modifiers
///
/// Note that we use `BTreeMap` and NOT `HashMap`, as we want to maintain the
/// ordering from the configuration file.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct PathControl {
    #[serde(default = "Vec::new")]
    pub upstream_request_filters: Vec<BTreeMap<String, String>>,
    #[serde(default = "Vec::new")]
    pub upstream_response_filters: Vec<BTreeMap<String, String>>,
}

//
// Basic Proxy Configuration
//

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct ProxyConfig {
    /// Name of the Service. Used for logging.
    pub name: String,

    /// Listeners - or "downstream" interfaces we listen to
    #[serde(default = "Vec::new")]
    pub listeners: Vec<ListenerConfig>,

    /// Connector - our (currently single) "upstream" server
    pub connector: ConnectorConfig,
    #[serde(default = "Default::default")]

    /// Path Control, for modifying and filtering requests
    pub path_control: PathControl,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ConnectorConfig {
    /// Proxy Address, e.g. `IP:port`
    pub proxy_addr: String,
    /// TLS SNI, if TLS should be used
    pub tls_sni: Option<String>,
}

impl From<ConnectorConfig> for HttpPeer {
    fn from(val: ConnectorConfig) -> Self {
        let (tls, sni) = if let Some(sni) = val.tls_sni {
            (true, sni)
        } else {
            (false, String::new())
        };
        HttpPeer::new(&val.proxy_addr, tls, sni)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ConnectorTlsConfig {
    pub proxy_sni: String,
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ListenerTlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ListenerConfig {
    pub source: ListenerKind,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(tag = "kind", content = "value")]
pub enum ListenerKind {
    Tcp {
        addr: String,
        tls: Option<ListenerTlsConfig>,
    },
    Uds(PathBuf),
}

impl From<ProxyConfig> for super::internal::ProxyConfig {
    fn from(other: ProxyConfig) -> Self {
        Self {
            name: other.name,
            listeners: other.listeners.into_iter().map(Into::into).collect(),
            upstreams: vec![other.connector.into()],
            path_control: other.path_control.into(),
            upstream_options: UpstreamOptions::default(),
        }
    }
}

impl From<PathControl> for super::internal::PathControl {
    fn from(value: PathControl) -> Self {
        Self {
            upstream_request_filters: value.upstream_request_filters,
            upstream_response_filters: value.upstream_response_filters,
        }
    }
}

impl From<ListenerTlsConfig> for super::internal::TlsConfig {
    fn from(other: ListenerTlsConfig) -> Self {
        Self {
            cert_path: other.cert_path,
            key_path: other.key_path,
        }
    }
}

impl From<ListenerConfig> for super::internal::ListenerConfig {
    fn from(other: ListenerConfig) -> Self {
        Self {
            source: other.source.into(),
        }
    }
}

impl From<ListenerKind> for super::internal::ListenerKind {
    fn from(other: ListenerKind) -> Self {
        match other {
            ListenerKind::Tcp { addr, tls } => super::internal::ListenerKind::Tcp {
                addr,
                tls: tls.map(Into::into),
            },
            ListenerKind::Uds(a) => super::internal::ListenerKind::Uds(a),
        }
    }
}

#[cfg(test)]
pub mod test {
    use std::collections::BTreeMap;

    use pingora::upstreams::peer::HttpPeer;

    use crate::config::{
        apply_toml,
        internal::{self, UpstreamOptions},
        toml::{ConnectorConfig, ListenerConfig, ProxyConfig, System},
    };

    use super::Toml;

    #[test]
    fn load_example() {
        let snapshot: Toml = Toml {
            system: System {
                threads_per_service: 8,
            },
            basic_proxy: vec![],
        };
        let loaded = Toml::from_path("./assets/example-config.toml");
        assert_eq!(snapshot, loaded);

        let def = internal::Config::default();
        let mut cfg = internal::Config::default();
        apply_toml(&mut cfg, &loaded);

        // These don't impl PartialEq, largely due to `BasicPeer` and `Tracer` not
        // implementing the trait. Since we only need this for testing, this is...
        // sort of acceptable
        assert_eq!(format!("{def:?}"), format!("{cfg:?}"));
    }

    #[test]
    fn load_test() {
        let toml_snapshot: Toml = Toml {
            system: System {
                threads_per_service: 8,
            },
            basic_proxy: vec![
                ProxyConfig {
                    name: "Example1".into(),
                    listeners: vec![
                        ListenerConfig {
                            source: crate::config::toml::ListenerKind::Tcp {
                                addr: "0.0.0.0:8080".into(),
                                tls: None,
                            },
                        },
                        ListenerConfig {
                            source: crate::config::toml::ListenerKind::Tcp {
                                addr: "0.0.0.0:4443".into(),
                                tls: Some(crate::config::toml::ListenerTlsConfig {
                                    cert_path: "./assets/test.crt".into(),
                                    key_path: "./assets/test.key".into(),
                                }),
                            },
                        },
                    ],
                    connector: ConnectorConfig {
                        proxy_addr: "91.107.223.4:443".into(),
                        tls_sni: Some(String::from("onevariable.com")),
                    },
                    path_control: crate::config::toml::PathControl {
                        upstream_request_filters: vec![
                            BTreeMap::from([
                                ("kind".to_string(), "remove-header-key-regex".to_string()),
                                ("pattern".to_string(), ".*(secret|SECRET).*".to_string()),
                            ]),
                            BTreeMap::from([
                                ("key".to_string(), "x-proxy-friend".to_string()),
                                ("kind".to_string(), "upsert-header".to_string()),
                                ("value".to_string(), "river".to_string()),
                            ]),
                        ],
                        upstream_response_filters: vec![
                            BTreeMap::from([
                                ("kind".to_string(), "remove-header-key-regex".to_string()),
                                ("pattern".to_string(), ".*ETag.*".to_string()),
                            ]),
                            BTreeMap::from([
                                ("key".to_string(), "x-with-love-from".to_string()),
                                ("kind".to_string(), "upsert-header".to_string()),
                                ("value".to_string(), "river".to_string()),
                            ]),
                        ],
                    },
                },
                ProxyConfig {
                    name: "Example2".into(),
                    listeners: vec![ListenerConfig {
                        source: crate::config::toml::ListenerKind::Tcp {
                            addr: "0.0.0.0:8000".into(),
                            tls: None,
                        },
                    }],
                    connector: ConnectorConfig {
                        proxy_addr: "91.107.223.4:80".into(),
                        tls_sni: None,
                    },
                    path_control: crate::config::toml::PathControl {
                        upstream_request_filters: vec![],
                        upstream_response_filters: vec![],
                    },
                },
            ],
        };
        let loaded = Toml::from_path("./assets/test-config.toml");
        assert_eq!(toml_snapshot, loaded);

        let sys_snapshot = internal::Config {
            validate_configs: false,
            threads_per_service: 8,
            basic_proxies: vec![
                internal::ProxyConfig {
                    name: "Example1".into(),
                    listeners: vec![
                        internal::ListenerConfig {
                            source: internal::ListenerKind::Tcp {
                                addr: "0.0.0.0:8080".into(),
                                tls: None,
                            },
                        },
                        internal::ListenerConfig {
                            source: internal::ListenerKind::Tcp {
                                addr: "0.0.0.0:4443".into(),
                                tls: Some(internal::TlsConfig {
                                    cert_path: "./assets/test.crt".into(),
                                    key_path: "./assets/test.key".into(),
                                }),
                            },
                        },
                    ],
                    upstreams: vec![HttpPeer::new(
                        "91.107.223.4:443",
                        true,
                        String::from("onevariable.com"),
                    )],
                    path_control: internal::PathControl {
                        upstream_request_filters: vec![
                            BTreeMap::from([
                                ("kind".to_string(), "remove-header-key-regex".to_string()),
                                ("pattern".to_string(), ".*(secret|SECRET).*".to_string()),
                            ]),
                            BTreeMap::from([
                                ("key".to_string(), "x-proxy-friend".to_string()),
                                ("kind".to_string(), "upsert-header".to_string()),
                                ("value".to_string(), "river".to_string()),
                            ]),
                        ],
                        upstream_response_filters: vec![
                            BTreeMap::from([
                                ("kind".to_string(), "remove-header-key-regex".to_string()),
                                ("pattern".to_string(), ".*ETag.*".to_string()),
                            ]),
                            BTreeMap::from([
                                ("key".to_string(), "x-with-love-from".to_string()),
                                ("kind".to_string(), "upsert-header".to_string()),
                                ("value".to_string(), "river".to_string()),
                            ]),
                        ],
                    },
                    upstream_options: UpstreamOptions::default(),
                },
                internal::ProxyConfig {
                    name: "Example2".into(),
                    listeners: vec![internal::ListenerConfig {
                        source: internal::ListenerKind::Tcp {
                            addr: "0.0.0.0:8000".into(),
                            tls: None,
                        },
                    }],
                    upstreams: vec![HttpPeer::new("91.107.223.4:80", false, String::new())],
                    path_control: internal::PathControl {
                        upstream_request_filters: vec![],
                        upstream_response_filters: vec![],
                    },
                    upstream_options: UpstreamOptions::default(),
                },
            ],
            file_servers: Vec::new(),
        };

        let mut cfg = internal::Config::default();
        apply_toml(&mut cfg, &loaded);

        // These don't impl PartialEq, largely due to `BasicPeer` and `Tracer` not
        // implementing the trait. Since we only need this for testing, this is...
        // sort of acceptable
        assert_eq!(format!("{sys_snapshot:?}"), format!("{cfg:?}"));
    }
}
