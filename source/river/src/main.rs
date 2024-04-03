mod config;

use pingora::{listeners::Listeners, server::Server};

use crate::config::internal::ListenerKind;

fn main() {
    tracing_subscriber::fmt::init();

    let conf = config::render_config();

    if conf.validate_configs {
        conf.validate();
    }

    let mut my_server =
        Server::new_with_opt_and_conf(conf.pingora_opt(), conf.pingora_server_conf());

    tracing::info!("Applying Basic Proxies...");
    for beep in conf.basic_proxies {
        tracing::info!("Configuring Basic Proxy: {}", beep.name);

        let mut listeners = Listeners::new();
        for list_cfg in beep.listeners {
            // NOTE: See https://github.com/cloudflare/pingora/issues/182 for tracking "paths aren't
            // always UTF-8 strings".
            match list_cfg.source {
                ListenerKind::Tcp { addr, tls: Some(tls_cfg) } => {
                    let cert_path = tls_cfg.cert_path.to_str().expect("cert path should be utf8");
                    let key_path = tls_cfg.key_path.to_str().expect("key path should be utf8");
                    listeners.add_tls(&addr, cert_path, key_path).expect("adding TLS listener shouldn't fail");
                },
                ListenerKind::Tcp { addr, tls: None } => {
                    listeners.add_tcp(&addr);
                },
                ListenerKind::Uds(path) => {
                    let path = path.to_str().unwrap();
                    listeners.add_uds(path, None); // todo
                },
            }
        }

        // pingora_core::services::listening::Service::with_listeners(
        //     beep.name,
        //     listeners,
        //     todo!(),
        // );
    }

    tracing::info!("Bootstrapping...");
    my_server.bootstrap();
    tracing::info!("Bootstrapped.");
    my_server.add_services(vec![]);
    tracing::info!("Starting Server...");
    my_server.run_forever();
}
