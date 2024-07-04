mod config;
mod proxy;
mod files;


use crate::{files::river_file_server, proxy::river_proxy_service};
use config::internal::{ListenerConfig, ListenerKind};
use pingora::{server::Server, services::Service};

fn main() {
    // Set up tracing, including catching `log` crate logs from pingora crates
    tracing_subscriber::fmt::init();

    // Read from the various configuration files
    let conf = config::render_config();

    // If the user asked to validate the configs - do it. This will also
    // cause pingora to exit immediately when we start
    if conf.validate_configs {
        conf.validate();
    }

    // Start the Server, which we will add services to.
    let mut my_server =
        Server::new_with_opt_and_conf(conf.pingora_opt(), conf.pingora_server_conf());

    tracing::info!("Applying Basic Proxies...");
    let mut services: Vec<Box<dyn Service>> = vec![];

    // At the moment, we only support basic proxy services. These have some path
    // control, but don't support things like load balancing, health checks, etc.
    for beep in conf.basic_proxies {
        tracing::info!("Configuring Basic Proxy: {}", beep.name);
        let service = river_proxy_service(beep, &my_server);
        services.push(service);
    }

    for fs in conf.file_servers {
        tracing::info!("Configuring File Server: {}", fs.name);
        let service = river_file_server(fs, &my_server);
        services.push(service);
    }

    // Now we hand it over to pingora to run forever.
    tracing::info!("Bootstrapping...");
    my_server.bootstrap();
    tracing::info!("Bootstrapped. Adding Services...");
    my_server.add_services(services);
    tracing::info!("Starting Server...");
    my_server.run_forever();
}


pub fn populate_listners<T>(listeners: Vec<ListenerConfig>, service: &mut pingora_core::services::listening::Service<T>) {
    for list_cfg in listeners {
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
                service
                    .add_tls(&addr, cert_path, key_path)
                    .expect("adding TLS listener shouldn't fail");
            }
            ListenerKind::Tcp { addr, tls: None } => {
                service.add_tcp(&addr);
            }
            ListenerKind::Uds(path) => {
                let path = path.to_str().unwrap();
                service.add_uds(path, None); // todo
            }
        }
    }
}
