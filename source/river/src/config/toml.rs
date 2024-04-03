//! Configuration sourced from a TOML file

use std::path::{Path, PathBuf};

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
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TlsConfig {
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
        tls: Option<TlsConfig>,
    },
    Uds(PathBuf),
}

#[cfg(test)]
pub mod test {
    use crate::config::{
        apply_toml, internal,
        toml::{ListenerConfig, ProxyConfig, System},
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

        assert_eq!(def, cfg);
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
                                addr: "0.0.0.0:80".into(),
                                tls: None,
                            },
                        },
                        ListenerConfig {
                            source: crate::config::toml::ListenerKind::Tcp {
                                addr: "0.0.0.0:443".into(),
                                tls: Some(crate::config::toml::TlsConfig {
                                    cert_path: "./assets/test.crt".into(),
                                    key_path: "./assets/test.key".into(),
                                }),
                            }
                        },
                    ],
                },
                ProxyConfig {
                    name: "Example2".into(),
                    listeners: vec![],
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
                                addr: "0.0.0.0:80".into(),
                                tls: None,
                            }
                        },
                        internal::ListenerConfig {
                            source: internal::ListenerKind::Tcp {
                                addr: "0.0.0.0:443".into(),
                                tls: Some(internal::TlsConfig {
                                    cert_path: "./assets/test.crt".into(),
                                    key_path: "./assets/test.key".into(),
                                }),
                            }
                        },
                    ],
                },
                internal::ProxyConfig {
                    name: "Example2".into(),
                    listeners: vec![],
                },
            ],
        };

        let mut cfg = internal::Config::default();
        apply_toml(&mut cfg, &loaded);

        assert_eq!(sys_snapshot, cfg);
    }
}
