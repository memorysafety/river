mod config;
mod proxy;

use pingora::{server::Server, services::Service};

use crate::{config::internal::ListenerKind, proxy::Modifiers};

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

        let modifiers = Modifiers::from_conf(&beep.path_control).unwrap();

        let mut my_proxy = pingora_proxy::http_proxy_service_with_name(
            &my_server.configuration,
            proxy::MyProxy {
                upstream: beep.upstream,
                modifiers,
            },
            &beep.name,
        );

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
                    my_proxy
                        .add_tls(&addr, cert_path, key_path)
                        .expect("adding TLS listener shouldn't fail");
                }
                ListenerKind::Tcp { addr, tls: None } => {
                    my_proxy.add_tcp(&addr);
                }
                ListenerKind::Uds(path) => {
                    let path = path.to_str().unwrap();
                    my_proxy.add_uds(path, None); // todo
                }
            }
        }

        services.push(Box::new(my_proxy));
    }

    tracing::info!("Bootstrapping...");
    my_server.bootstrap();
    tracing::info!("Bootstrapped. Adding Services...");
    my_server.add_services(services);
    tracing::info!("Starting Server...");
    my_server.run_forever();
}
