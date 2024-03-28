mod config;

use pingora::server::Server;

fn main() {
    tracing_subscriber::fmt::init();

    let conf = config::render_config();

    if conf.validate_configs {
        conf.validate();
    }

    let mut my_server =
        Server::new_with_opt_and_conf(conf.pingora_opt(), conf.pingora_server_conf());

    tracing::info!("Bootstrapping...");
    my_server.bootstrap();
    tracing::info!("Bootstrapped.");
    my_server.add_services(vec![]);
    tracing::info!("Starting Server...");
    my_server.run_forever();
}
