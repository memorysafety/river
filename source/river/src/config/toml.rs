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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ProxyConfig {
    name: String,
    #[serde(default = "Vec::new")]
    listeners: Vec<ListenerConfig>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct TlsConfig {
    cert_path: PathBuf,
    key_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ListenerConfig {
    source: ListenerKind,
    tls: Option<TlsConfig>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "kind", content = "value")]
pub enum ListenerKind {
    Tcp(String),
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
        let snapshot: Toml = Toml {
            system: System {
                threads_per_service: 8,
            },
            basic_proxy: vec![
                ProxyConfig {
                    name: "Example1".into(),
                    listeners: vec![
                        ListenerConfig {
                            source: crate::config::toml::ListenerKind::Tcp("0.0.0.0:80".into()),
                            tls: None,
                        },
                        ListenerConfig {
                            source: crate::config::toml::ListenerKind::Tcp("0.0.0.0:443".into()),
                            tls: Some(crate::config::toml::TlsConfig {
                                cert_path: "./test.crt".into(),
                                key_path: "./test.pem".into(),
                            }),
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
        assert_eq!(snapshot, loaded);

        let def = internal::Config::default();
        let mut cfg = internal::Config::default();
        apply_toml(&mut cfg, &loaded);

        assert_eq!(def, cfg);
    }
}
