//! Configuration sourced from the CLI

use clap::Parser;
use std::path::PathBuf;

/// River: A reverse proxy from Prossimo
#[derive(Parser, Debug)]
pub struct Cli {
    /// Validate all configuration data
    #[arg(long)]
    pub validate_configs: bool,

    /// Path to the configuration file
    #[arg(long)]
    pub config_toml: Option<PathBuf>,

    #[arg(long)]
    pub threads_per_service: Option<usize>,
}
