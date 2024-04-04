//! Configuration sourced from a TOML file

use std::path::{Path, PathBuf};

use pingora::upstreams::peer::BasicPeer;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Toml {
    #[serde(default)]
    pub system: System,
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

//
// Basic Proxy Configuration
//

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ProxyConfig {
    pub name: String,
    #[serde(default = "Vec::new")]
    pub listeners: Vec<ListenerConfig>,
    pub connector: ConnectorConfig,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ConnectorConfig {
    pub proxy_addr: String,
    pub tls_sni: Option<String>,
}

impl From<ConnectorConfig> for BasicPeer {
    fn from(val: ConnectorConfig) -> Self {
        let mut beep = BasicPeer::new(&val.proxy_addr);
        if let Some(sni) = val.tls_sni {
            beep.sni = sni;
        }
        beep
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

#[cfg(test)]
pub mod test {
    use pingora::upstreams::peer::BasicPeer;

    use crate::config::{
        apply_toml, internal,
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
                        proxy_addr: "1.1.1.1:443".into(),
                        tls_sni: Some(String::from("one.one.one.one")),
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
                        proxy_addr: "1.1.1.1:80".into(),
                        tls_sni: None,
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
                    upstream: {
                        let mut beep = BasicPeer::new("1.1.1.1:443");
                        beep.sni = String::from("one.one.one.one");
                        beep
                    },
                },
                internal::ProxyConfig {
                    name: "Example2".into(),
                    listeners: vec![internal::ListenerConfig {
                        source: internal::ListenerKind::Tcp {
                            addr: "0.0.0.0:8000".into(),
                            tls: None,
                        },
                    }],
                    upstream: BasicPeer::new("1.1.1.1:80"),
                },
            ],
        };

        let mut cfg = internal::Config::default();
        apply_toml(&mut cfg, &loaded);

        // These don't impl PartialEq, largely due to `BasicPeer` and `Tracer` not
        // implementing the trait. Since we only need this for testing, this is...
        // sort of acceptable
        assert_eq!(format!("{sys_snapshot:?}"), format!("{cfg:?}"));
    }
}
