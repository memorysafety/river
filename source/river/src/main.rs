mod config;
mod proxy;

use std::sync::Arc;

use pingora::{listeners::Listeners, server::Server, services::Service};

use crate::{config::internal::ListenerKind, proxy::ProxyApp};

fn main() {
    tracing_subscriber::fmt::init();

    let conf = config::render_config();

    if conf.validate_configs {
        conf.validate();
    }

    let mut my_server =
        Server::new_with_opt_and_conf(conf.pingora_opt(), conf.pingora_server_conf());

    tracing::info!("Applying Basic Proxies...");
    let mut services: Vec<Box<dyn Service>> = vec![];

    for beep in conf.basic_proxies {
        tracing::info!("Configuring Basic Proxy: {}", beep.name);

        let mut listeners = Listeners::new();
        for list_cfg in beep.listeners {
            // NOTE: See https://github.com/cloudflare/pingora/issues/182 for tracking "paths aren't
            // always UTF-8 strings".
            //
            // See also https://github.com/cloudflare/pingora/issues/183 for tracking "ip addrs shouldn't
            // be strings"
            match list_cfg.source {
                ListenerKind::Tcp {
                    addr,
                    tls: Some(tls_cfg),
                } => {
                    let cert_path = tls_cfg
                        .cert_path
                        .to_str()
                        .expect("cert path should be utf8");
                    let key_path = tls_cfg.key_path.to_str().expect("key path should be utf8");
                    listeners
                        .add_tls(&addr, cert_path, key_path)
                        .expect("adding TLS listener shouldn't fail");
                }
                ListenerKind::Tcp { addr, tls: None } => {
                    listeners.add_tcp(&addr);
                }
                ListenerKind::Uds(path) => {
                    let path = path.to_str().unwrap();
                    listeners.add_uds(path, None); // todo
                }
            }
        }

        let upstream = ProxyApp::new(beep.upstream);

        let svc = pingora_core::services::listening::Service::with_listeners(
            beep.name,
            listeners,
            Arc::new(upstream),
        );

        services.push(Box::new(svc));
    }

    tracing::info!("Bootstrapping...");
    my_server.bootstrap();
    tracing::info!("Bootstrapped. Adding Services...");
    my_server.add_services(services);
    tracing::info!("Starting Server...");
    my_server.run_forever();
}
