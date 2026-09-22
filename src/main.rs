use serde_json::{Number, Value, json};
use snafu::{FromString, Whatever};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use crate::tools::{ToolDefinition, ToolHandler};
use crate::ui::main_loop;

mod chat;
mod tools;
mod ui;

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

struct MyTools;

impl ToolHandler for MyTools {
    #[allow(
        clippy::unused_async_trait_impl,
        reason = "random number generation isn't async, but this is a generic trait"
    )]
    async fn handle(&self, name: &str, arguments: &Value) -> Result<Value> {
        let value = match name {
            "roll_dice" => {
                let faces = arguments
                    .get("faces")
                    .and_then(|v| v.as_number().and_then(Number::as_u64))
                    .ok_or_else(|| {
                        Whatever::without_source("bad argument for 'faces'".to_owned())
                    })?;
                let number = arguments
                    .get("number")
                    .and_then(|v| v.as_number().and_then(Number::as_u64))
                    .ok_or_else(|| {
                        Whatever::without_source("bad argument for 'number'".to_owned())
                    })?;

                let mut total: u64 = 0;

                for _ in 0..number {
                    total += rand::random_range(1..faces);
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

#[tokio::main]
#[snafu::report]
async fn main() -> Result<()> {
    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    let server = chat::OpenAICompatChatServer::builder()
        .base_url("http://localhost:8069/v1")
        .model("whatever")
        .build();

    let history = chat::OpenAICompatChatHistory::new();

    let mut tools = vec![];
    tools.push(
        ToolDefinition::builder()
            .name("roll_dice")
            .description("Roll a number of dice (with the specified number of faces), returning the total result.")
            .parameters(json!({
                "type":"object",
                "properties":{
                    "faces":{
                        "type":"integer",
                        "description":"The number of sides of each die, e.g. 6 is a standard six-sided die.",
                        "minimum": 1
                    },
                    "number":{
                        "type":"integer",
                        "description":"The number of dice to roll.",
                        "minimum": 1
                    }
                },
                "required":["faces", "number"]
            }))
            .build()
    );

    main_loop(server, history, &tools, MyTools).await
}
