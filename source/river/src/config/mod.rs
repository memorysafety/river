pub mod cli;
pub mod internal;
pub mod toml;

use clap::Parser;
use cli::Cli;

use crate::config::toml::Toml;

pub fn render_config() -> internal::Config {
    // To begin with, start with the blank internal config. We will layer on top of that.
    let mut config = internal::Config::default();

    // Then, obtain the command line information, as that may
    // change the paths to look for configuration files. It also handles
    // bailing immediately if the user passes `--help`.
    tracing::info!("Parsing CLI options");
    let c = Cli::parse();
    tracing::info!(
        config = ?c,
        "CLI config"
    );

    let toml_opts = if let Some(toml_path) = c.config_toml.as_ref() {
        Some(Toml::from_path(toml_path))
    } else {
        tracing::info!("No TOML file provided");
        None
    };

    // 2.6.7: River MUST give the following priority to configuration:
    //   1. Command Line Options (highest priority)
    //   2. Environment Variable Options
    //   3. Configuration File Options (lowest priority)
    //
    // Apply in reverse order as we are layering.
    if let Some(tf) = toml_opts {
        tracing::info!("Applying TOML options");
        apply_toml(&mut config, &tf);
    }
    tracing::info!("Applying CLI options");
    apply_cli(&mut config, &c);

    tracing::info!(?config, "Full configuration",);
    config
}

fn apply_cli(conf: &mut internal::Config, cli: &Cli) {
    let Cli {
        validate_configs,
        threads_per_service,
        config_toml: _,
    } = cli;

    conf.validate_configs |= validate_configs;
    if let Some(tps) = threads_per_service {
        conf.threads_per_service = *tps;
    }
}

fn apply_toml(conf: &mut internal::Config, toml: &Toml) {
    let Toml {
        threads_per_service,
    } = toml;

    if let Some(tps) = threads_per_service {
        conf.threads_per_service = *tps;
    }
}
