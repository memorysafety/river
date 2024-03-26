use clap::Parser;
use std::path::PathBuf;

/// River: A reverse proxy from Prossimo
#[derive(Parser, Debug)]
pub struct Cli {
    /// This will go away soon
    name: Option<String>,

    /// Path to the configuration file
    #[arg(long)]
    config: Option<PathBuf>,
}
