use async_trait::async_trait;
use clap::Parser;
use serde_json::Value;
use snafu::ResultExt;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use crate::tools::{BuiltinTools, McpToolHandler, ToolDefinition, ToolHandler};
use crate::ui::main_loop;

mod chat;
mod tools;
mod ui;

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

struct MyTools {
    builtins: BuiltinTools,
    handlers: Vec<Box<dyn ToolHandler + Sync>>,
}

impl MyTools {
    fn new(handlers: Vec<Box<dyn ToolHandler + Sync>>) -> Self {
        Self {
            builtins: BuiltinTools,
            handlers,
        }
    }
}

#[async_trait]
impl ToolHandler for MyTools {
    fn get_tools(&self) -> Vec<ToolDefinition> {
        let mut tools = Vec::new();
        tools.extend(self.builtins.get_tools());
        for handler in &self.handlers {
            tools.extend(handler.get_tools());
        }
        // TODO cache this, maybe a simple TTL cache
        tools
    }

    async fn handle(&self, name: &str, arguments: Value) -> Result<String> {
        if self.builtins.is_handled(name) {
            self.builtins.handle(name, arguments).await
        } else {
            for handler in &self.handlers {
                if handler.is_handled(name) {
                    return handler.handle(name, arguments).await;
                }
            }
            panic!()
        }
    }

    fn is_handled(&self, _name: &str) -> bool {
        true
    }
}

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Base URL of OpenAI-compatible API endpoint
    #[arg(
        short,
        long,
        env = "CAFE_BASE_URL",
        default_value = "http://localhost:8080/v1"
    )]
    base_url: String,

    /// Optional API key
    #[arg(long, env = "CAFE_API_KEY", hide_env_values = true)]
    api_key: Option<String>,

    /// Model to use
    #[arg(
        short,
        long,
        env = "CAFE_MODEL",
        default_value = "mistral-nemo-instruct-2407" // an oldie, but goodie
    )]
    model: String,

    /// Optional MCP server URL for tools
    #[arg(long, env = "CAFE_MCP_SERVER")]
    mcp_server: Option<String>,
}

#[tokio::main]
#[snafu::report]
async fn main() -> Result<()> {
    let args: Args = Args::parse();

    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    let server = chat::OpenAICompatChatServer::builder()
        .base_url(&args.base_url)
        .maybe_api_key(args.api_key.as_deref())
        .model(&args.model)
        .build();

    let history = chat::OpenAICompatChatHistory::new();

    // TODO maybe use a builder to make this cleaner
    let mut extra_handlers: Vec<Box<dyn ToolHandler + Sync>> = Vec::new();
    if let Some(mcp_server) = args.mcp_server {
        let mcp = McpToolHandler::new(&mcp_server)
            .await
            .whatever_context("initializing MCP server")?;
        extra_handlers.push(Box::new(mcp));
    }

    let tools = MyTools::new(extra_handlers);

    println!("Tools available:");
    for tool in &tools.get_tools() {
        println!(" - {}", tool.name);
    }
    println!();

    main_loop(server, history, tools).await
}
