use std::{io::ErrorKind, path::Path};

use serde::Deserialize;
use snafu::{FromString, ResultExt, Whatever};
use tracing::{Level, event};

use crate::Result;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    // TODO
    #[serde(rename = "system-prompt")]
    pub system_prompt: Option<String>,
}

impl Config {
    pub fn load<P>(config_path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let config_str = match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                if e.kind() == ErrorKind::NotFound {
                    // No config (yet) and that's OK
                    event!(Level::DEBUG, "using default config");
                    return Ok(Config::default());
                }
                return Err(Whatever::with_source(
                    Box::new(e),
                    format!("reading {}", config_path.as_ref().display()),
                ));
            }
        };
        let config = toml::from_str(&config_str)
            .with_whatever_context(|_| format!("parsing {}", config_path.as_ref().display()))?;
        event!(Level::DEBUG, "config = {:?}", config);
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    #[test]
    fn test_default_config() {
        let config = Config::load("test/no-config.toml").unwrap();
        assert!(config.system_prompt.is_none());
    }

    #[test]
    fn test_basic_config() {
        let config = Config::load("test/pirate.toml").unwrap();
        assert_eq!(
            config.system_prompt,
            Some("You are a helpful assistant that talks like a pirate.".to_owned())
        );
    }
}
