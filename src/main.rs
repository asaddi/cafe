use clap::Parser;
use schemars::{JsonSchema, schema_for};
use serde::Deserialize;
use serde_json::{Value, json};
use snafu::prelude::*;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use crate::tools::{ToolDefinition, ToolHandler};
use crate::ui::main_loop;

mod chat;
mod tools;
mod ui;

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

#[derive(Debug, Deserialize, JsonSchema)]
struct RollDiceParams {
    #[schemars(range(min = 1))]
    /// The number of sides of each die, e.g. 6 is a standard six-sided die.
    faces: u64,

    #[schemars(range(min = 1))]
    /// The number of dice to roll.
    number: u64,
}

struct MyTools;

impl ToolHandler for MyTools {
    #[allow(
        clippy::unused_async_trait_impl,
        reason = "random number generation isn't async, but this is a generic trait"
    )]
    async fn handle(&self, name: &str, arguments: Value) -> Result<Value> {
        let value = match name {
            "roll_dice" => {
                let args: RollDiceParams =
                    serde_json::from_value(arguments).whatever_context("bad arguments")?;

                let mut total: u64 = 0;

                for _ in 0..args.number {
                    total += rand::random_range(1..=args.faces);
                }

                json!(total)
            }
            _ => {
                unimplemented!("tool: {name}");
            }
        };

        Ok(value)
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

    let mut tools = vec![];
    tools.push(
        ToolDefinition::builder()
            .name("roll_dice")
            .description("Roll a number of dice (with the specified number of faces), returning the total result.")
            .parameters(schema_for!(RollDiceParams).into())
            .build()
    );

    main_loop(server, history, &tools, MyTools).await
}
