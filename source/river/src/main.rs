mod cli;

use std::sync::Arc;

use clap::Parser;
use cli::Cli;
use pingora::server::{
    configuration::{Opt, ServerConf},
    Server,
};

fn main() {
    tracing_subscriber::fmt::init();

    tracing::info!("Parsing CLI options");

    let c = Cli::parse();

    tracing::info!(
        config = ?c,
        "CLI config"
    );

    let opt = Opt {
        upgrade: false,
        daemon: false,
        nocapture: false,
        test: false,
        conf: None,
    };

    let mut my_server = Server::new(opt).unwrap();

    // TODO: These options need to be checked
    let mut conf = ServerConf::default();
    conf.daemon = false;
    conf.error_log = None;
    conf.pid_file = String::from("./target/pidfile");
    conf.upgrade_sock = String::from("./target/upgrade");
    conf.user = None;
    conf.group = None;
    conf.threads = 8;
    conf.work_stealing = true;
    conf.ca_file = None;

    // TODO: These are private fields (sort of), see
    // https://github.com/cloudflare/pingora/issues/159
    // for more details
    //
    // conf.version = todo!();
    // conf.client_bind_to_ipv4 = todo!();
    // conf.client_bind_to_ipv6 = todo!();
    // conf.upstream_keepalive_pool_size = todo!();
    // conf.upstream_connect_offload_threadpools = todo!();
    // conf.upstream_connect_offload_thread_per_pool = todo!();

    my_server.configuration = Arc::new(conf);

    tracing::info!("Bootstrapping...");
    my_server.bootstrap();
    tracing::info!("Bootstrapped.");
    my_server.add_services(vec![]);
    tracing::info!("Starting Server...");
    my_server.run_forever();
}
