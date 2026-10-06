use std::{io::ErrorKind, path::Path};

use serde::{Deserialize, Serialize};
use snafu::{FromString, ResultExt, Whatever};
use toml::Table;
use tracing::{Level, event};

use crate::Result;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    // TODO
    #[serde(rename = "system-prompt")]
    pub system_prompt: Option<String>,

    pub persona: Option<String>,

    pub mcp: Option<Vec<McpConfig>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpConfig {
    pub name: String,
    pub url: String,
    // TODO? As this is a CLI/TUI app, it doesn't make sense to support
    // OAuth. Only custom headers (which would include the "Authorization"
    // header).
    pub headers: Option<Table>,
}

impl Config {
    pub fn load<P>(config_path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let config_name = config_path.as_ref().display();

        let config_str = match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                if e.kind() == ErrorKind::NotFound {
                    // No config (yet) and that's OK
                    event!(
                        Level::INFO,
                        "no config found at {config_name}; using default"
                    );
                    return Ok(Config::default());
                }
                return Err(Whatever::with_source(
                    Box::new(e),
                    format!("reading {}", config_path.as_ref().display()),
                ));
            }
        };
        let config = toml::from_str(&config_str)
            .with_whatever_context(|_| format!("parsing {config_name}"))?;
        event!(Level::TRACE, "config = {:?}", config);
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
        assert!(config.mcp.is_none());
    }

    #[test]
    fn test_basic_config() {
        let config = Config::load("test/pirate.toml").unwrap();
        assert_eq!(
            config.system_prompt,
            Some("You are a helpful assistant that talks like a pirate.".to_owned())
        );
        assert!(config.mcp.is_none());
    }

    #[test]
    fn test_mcp_config_1() {
        let config = Config::load("test/mcp.toml").unwrap();
        insta::assert_ron_snapshot!(&config.mcp, @r#"
        Some([
          McpConfig(
            name: "Some MCP server",
            url: "https://mcp.example.com/mcp",
            headers: None,
          ),
        ])
        "#);
    }

    #[test]
    fn test_mcp_config_2() {
        let config = Config::load("test/mcp2.toml").unwrap();
        insta::assert_ron_snapshot!(&config.mcp, @r#"
        Some([
          McpConfig(
            name: "Some MCP server",
            url: "https://mcp.example.com/mcp",
            headers: None,
          ),
          McpConfig(
            name: "Another MCP server",
            url: "https://mcp2.example.com/mcp",
            headers: None,
          ),
        ])
        "#);
    }

    #[test]
    fn test_mcp_config_3() {
        let config = Config::load("test/mcp3.toml").unwrap();
        insta::assert_ron_snapshot!(&config.mcp, @r#"
        Some([
          McpConfig(
            name: "Some MCP server",
            url: "https://mcp.example.com/mcp",
            headers: Some({
              "authorization": "Bearer my-token",
              "x-api-key": "my-api-key",
            }),
          ),
        ])
        "#);
    }
}
