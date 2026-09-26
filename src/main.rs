use serde_json::json;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use crate::tools::ToolDefinition;
use crate::ui::main_loop;

mod chat;
mod tools;
mod ui;

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

#[tokio::main]
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

    main_loop(server, history, &tools).await
}
