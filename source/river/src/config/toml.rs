//! Configuration sourced from a TOML file

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Toml {
    pub threads_per_service: Option<usize>,
}

impl Toml {
    pub fn from_path<P>(path: &P) -> Self
    where
        P: AsRef<Path> + core::fmt::Debug + ?Sized
    {
        tracing::info!("Loading TOML from {path:?}");
        let f = std::fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("Failed to load file at {path:?}"));
        let t = ::toml::from_str(&f).expect("failed to deserialize");
        tracing::info!("TOML file contents: {t:?}");
        t
    }
}

#[cfg(test)]
pub mod test {
    use crate::config::{apply_toml, internal};

    use super::Toml;

    #[test]
    fn load_example() {
        const SNAPSHOT: Toml = Toml {
            threads_per_service: Some(8),
        };
        let loaded = Toml::from_path("./assets/example-config.toml");
        assert_eq!(SNAPSHOT, loaded);

        let def = internal::Config::default();
        let mut cfg = internal::Config::default();
        apply_toml(&mut cfg, &loaded);

        assert_eq!(def, cfg);
    }
}
