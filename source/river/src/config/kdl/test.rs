use std::{collections::BTreeMap, net::SocketAddr};

use pingora::upstreams::peer::HttpPeer;

use crate::{
    config::internal::{
        FileServerConfig, ListenerConfig, ListenerKind, ProxyConfig, UpstreamOptions,
    },
    proxy::{
        rate_limiting::{multi::MultiRaterConfig, AllRateConfig, RegexShim},
        request_selector::uri_path_selector,
    },
};

#[test]
fn load_test() {
    let kdl_contents = std::fs::read_to_string("./assets/test-config.kdl").unwrap();

    let doc: ::kdl::KdlDocument = kdl_contents.parse().unwrap_or_else(|e| {
        panic!("Error parsing KDL file: {e:?}");
    });
    let val: crate::config::internal::Config = doc.try_into().unwrap_or_else(|e| {
        panic!("Error rendering config from KDL file: {e:?}");
    });

    let expected = crate::config::internal::Config {
        validate_configs: false,
        threads_per_service: 8,
        basic_proxies: vec![
            ProxyConfig {
                name: "Example1".into(),
                listeners: vec![
                    ListenerConfig {
                        source: crate::config::internal::ListenerKind::Tcp {
                            addr: "0.0.0.0:8080".into(),
                            tls: None,
                            offer_h2: false,
                        },
                    },
                    ListenerConfig {
                        source: crate::config::internal::ListenerKind::Tcp {
                            addr: "0.0.0.0:4443".into(),
                            tls: Some(crate::config::internal::TlsConfig {
                                cert_path: "./assets/test.crt".into(),
                                key_path: "./assets/test.key".into(),
                            }),
                            offer_h2: true,
                        },
                    },
                ],
                upstreams: vec![HttpPeer::new(
                    "91.107.223.4:443",
                    true,
                    String::from("onevariable.com"),
                )],
                path_control: crate::config::internal::PathControl {
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
                    request_filters: vec![BTreeMap::from([
                        ("kind".to_string(), "block-cidr-range".to_string()),
                        (
                            "addrs".to_string(),
                            "192.168.0.0/16, 10.0.0.0/8, 2001:0db8::0/32".to_string(),
                        ),
                    ])],
                },
                upstream_options: UpstreamOptions {
                    selection: crate::config::internal::SelectionKind::Ketama,
                    selector: uri_path_selector,
                    health_checks: crate::config::internal::HealthCheckKind::None,
                    discovery: crate::config::internal::DiscoveryKind::Static,
                },
                rate_limiting: crate::config::internal::RateLimitingConfig {
                    rules: vec![
                        AllRateConfig::Multi {
                            config: MultiRaterConfig {
                                threads: 8,
                                max_buckets: 4000,
                                max_tokens_per_bucket: 10,
                                refill_interval_millis: 10,
                                refill_qty: 1,
                            },
                            kind: crate::proxy::rate_limiting::multi::MultiRequestKeyKind::SourceIp,
                        },
                        AllRateConfig::Multi {
                            config: MultiRaterConfig {
                                threads: 8,
                                max_buckets: 2000,
                                max_tokens_per_bucket: 20,
                                refill_interval_millis: 1,
                                refill_qty: 5,
                            },
                            kind: crate::proxy::rate_limiting::multi::MultiRequestKeyKind::Uri {
                                pattern: RegexShim::new("static/.*").unwrap(),
                            },
                        },
                        AllRateConfig::Single {
                            config: crate::proxy::rate_limiting::single::SingleInstanceConfig {
                                max_tokens_per_bucket: 50,
                                refill_interval_millis: 3,
                                refill_qty: 2,
                            },
                            kind: crate::proxy::rate_limiting::single::SingleRequestKeyKind::UriGroup {
                                pattern: RegexShim::new(r".*\.mp4").unwrap(),
                            },
                        },
                    ],
                },
            },
            ProxyConfig {
                name: "Example2".into(),
                listeners: vec![ListenerConfig {
                    source: crate::config::internal::ListenerKind::Tcp {
                        addr: "0.0.0.0:8000".into(),
                        tls: None,
                        offer_h2: false,
                    },
                }],
                upstreams: vec![HttpPeer::new("91.107.223.4:80", false, String::new())],
                path_control: crate::config::internal::PathControl {
                    upstream_request_filters: vec![],
                    upstream_response_filters: vec![],
                    request_filters: vec![],
                },
                upstream_options: UpstreamOptions::default(),
                rate_limiting: crate::config::internal::RateLimitingConfig { rules: vec![] },
            },
        ],
        file_servers: vec![FileServerConfig {
            name: "Example3".into(),
            listeners: vec![
                ListenerConfig {
                    source: crate::config::internal::ListenerKind::Tcp {
                        addr: "0.0.0.0:9000".into(),
                        tls: None,
                        offer_h2: false,
                    },
                },
                ListenerConfig {
                    source: crate::config::internal::ListenerKind::Tcp {
                        addr: "0.0.0.0:9443".into(),
                        tls: Some(crate::config::internal::TlsConfig {
                            cert_path: "./assets/test.crt".into(),
                            key_path: "./assets/test.key".into(),
                        }),
                        offer_h2: true,
                    },
                },
            ],
            base_path: Some(".".into()),
        }],
        daemonize: false,
        pid_file: Some("/tmp/river.pidfile".into()),
        upgrade_socket: Some("/tmp/river-upgrade.sock".into()),
        upgrade: false,
    };

    assert_eq!(val.validate_configs, expected.validate_configs);
    assert_eq!(val.threads_per_service, expected.threads_per_service);
    assert_eq!(val.basic_proxies.len(), expected.basic_proxies.len());
    assert_eq!(val.file_servers.len(), expected.file_servers.len());

    for (abp, ebp) in val.basic_proxies.iter().zip(expected.basic_proxies.iter()) {
        let ProxyConfig {
            name,
            listeners,
            upstream_options,
            upstreams,
            path_control,
            rate_limiting,
        } = abp;
        assert_eq!(*name, ebp.name);
        assert_eq!(*listeners, ebp.listeners);
        assert_eq!(*upstream_options, ebp.upstream_options);
        upstreams
            .iter()
            .zip(ebp.upstreams.iter())
            .for_each(|(a, e)| {
                assert_eq!(a._address, e._address);
                assert_eq!(a.scheme, e.scheme);
                assert_eq!(a.sni, e.sni);
            });
        assert_eq!(*path_control, ebp.path_control);
        assert_eq!(*rate_limiting, ebp.rate_limiting);
    }

    for (afs, efs) in val.file_servers.iter().zip(expected.file_servers.iter()) {
        let FileServerConfig {
            name,
            listeners,
            base_path,
        } = afs;
        assert_eq!(*name, efs.name);
        assert_eq!(*listeners, efs.listeners);
        assert_eq!(*base_path, efs.base_path);
    }
}

/// Empty: not allowed
const EMPTY_TEST: &str = "
";

#[test]
fn empty() {
    let doc: ::kdl::KdlDocument = EMPTY_TEST.parse().unwrap_or_else(|e| {
        panic!("Error parsing KDL file: {e:?}");
    });
    let val: Result<crate::config::internal::Config, _> = doc.try_into();
    assert!(val.is_err());
}

/// Empty services: not allowed
const SERVICES_EMPTY_TEST: &str = "
    services {

    }
";

#[test]
fn services_empty() {
    let doc: ::kdl::KdlDocument = SERVICES_EMPTY_TEST.parse().unwrap_or_else(|e| {
        panic!("Error parsing KDL file: {e:?}");
    });
    let val: Result<crate::config::internal::Config, _> = doc.try_into();
    assert!(val.is_err());
}

/// The most minimal config is single services block
const ONE_SERVICE_TEST: &str = r#"
services {
    Example {
        listeners {
            "127.0.0.1:80"
        }
        connectors {
            "127.0.0.1:8000"
        }
    }
}
"#;

#[test]
fn one_service() {
    let doc: ::kdl::KdlDocument = ONE_SERVICE_TEST.parse().unwrap_or_else(|e| {
        panic!("Error parsing KDL file: {e:?}");
    });
    let val: crate::config::internal::Config = doc.try_into().unwrap_or_else(|e| {
        panic!("Error rendering config from KDL file: {e:?}");
    });
    assert_eq!(val.basic_proxies.len(), 1);
    assert_eq!(val.basic_proxies[0].listeners.len(), 1);
    assert_eq!(
        val.basic_proxies[0].listeners[0].source,
        ListenerKind::Tcp {
            addr: "127.0.0.1:80".into(),
            tls: None,
            offer_h2: false,
        }
    );
    assert_eq!(
        val.basic_proxies[0].upstreams[0]._address,
        ("127.0.0.1:8000".parse::<SocketAddr>().unwrap()).into()
    );
}
