//! Configuration sourced from the CLI

use clap::Parser;
use std::path::PathBuf;

/// River: A reverse proxy from Prossimo
#[derive(Parser, Debug)]
pub struct Cli {
    /// Validate all configuration data and exit
    #[arg(long)]
    pub validate_configs: bool,

    /// Path to the configuration file in TOML format
    #[arg(long)]
    pub config_toml: Option<PathBuf>,

    /// Path to the configuration file in KDL format
    #[arg(long)]
    pub config_kdl: Option<PathBuf>,

    /// Number of threads used in the worker pool for EACH service
    #[arg(long)]
    pub threads_per_service: Option<usize>,

    /// Should the server be daemonized after starting?
    #[arg(long)]
    pub daemonize: bool,

    /// Should the server take over an existing server?
    #[arg(long)]
    pub upgrade: bool,

    /// Path to upgrade socket
    #[arg(long)]
    pub upgrade_socket: Option<PathBuf>,

    /// Path to the pidfile, used for upgrade
    #[arg(long)]
    pub pidfile: Option<PathBuf>,
}
