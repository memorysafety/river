mod config;
mod proxy;

use crate::proxy::RiverProxyService;
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
        let service = RiverProxyService::from_basic_conf(beep, &my_server);
        services.push(Box::new(service));
    }

    // Now we hand it over to pingora to run forever.
    tracing::info!("Bootstrapping...");
    my_server.bootstrap();
    tracing::info!("Bootstrapped. Adding Services...");
    my_server.add_services(services);
    tracing::info!("Starting Server...");
    my_server.run_forever();
}
