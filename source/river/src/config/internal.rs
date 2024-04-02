//! This is the *actual* internal configuration structure.
//!
//! It is ONLY used for the internal configuration, and should not ever
//! be exposed as the public API for CLI, Env vars, or via Serde.
//!
//! This is used as the buffer between any external stable UI, and internal
//! impl details which may change at any time.

use pingora::server::configuration::{Opt as PingoraOpt, ServerConf as PingoraServerConf};

/// River's internal configuration
#[derive(Debug, PartialEq)]
pub struct Config {
    pub validate_configs: bool,
    pub threads_per_service: usize,
}


impl Default for Config {
    fn default() -> Self {
        Self {
            validate_configs: false,
            threads_per_service: 8,
        }
    }
}

impl Config {
    /// Get the [`Opt`][PingoraOpt] field for Pingora
    pub fn pingora_opt(&self) -> PingoraOpt {
        // TODO
        PingoraOpt {
            upgrade: false,
            daemon: false,
            nocapture: false,
            test: self.validate_configs,
            conf: None,
        }
    }

    /// Get the [`ServerConf`][PingoraServerConf] field for Pingora
    pub fn pingora_server_conf(&self) -> PingoraServerConf {
        PingoraServerConf {
            daemon: false,
            error_log: None,
            pid_file: String::from("./target/pidfile"),
            upgrade_sock: String::from("./target/upgrade"),
            user: None,
            group: None,
            threads: self.threads_per_service,
            work_stealing: true,
            ca_file: None,
            ..PingoraServerConf::default()
        }
    }

    pub fn validate(&self) {
        // TODO: validation logic
    }
}

