use std::path::PathBuf;

use clap::Parser;
use directories::ProjectDirs;
use snafu::ResultExt;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use crate::prompt::{BasicSystemPrompt, CharaSystemPrompt, SystemPromptSource};
use crate::tools::{McpToolHandler, ToolDispatcher, ToolHandler};
use crate::ui::main_loop;

mod chat;
mod config;
mod prompt;
mod tools;
mod ui;

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

fn get_project_dirs() -> ProjectDirs {
    ProjectDirs::from("", "", "cafe").expect("no project dirs")
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

    /// Optional character JSON
    #[arg(long)]
    chara: Option<PathBuf>,
}

#[tokio::main]
#[snafu::report]
async fn main() -> Result<()> {
    let args: Args = Args::parse();

    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("warn"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    let config_path = crate::get_project_dirs().config_dir().join("config.toml");
    let config = config::Config::load(&config_path)?;
    let sys_prompt_source: Box<dyn SystemPromptSource> = if let Some(chara) = args.chara {
        let mut chara_prompt = CharaSystemPrompt::new(&config);
        chara_prompt.load_card(chara)?;
        Box::new(chara_prompt)
    } else {
        Box::new(BasicSystemPrompt::new(&config))
    };

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

    let tools = ToolDispatcher::new(extra_handlers);

    println!("Tools available:");
    for tool in &tools.get_tools() {
        println!(" - {}", tool.name);
    }
    println!();

    main_loop(server, history, tools, sys_prompt_source).await
}
