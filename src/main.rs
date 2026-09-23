#![warn(clippy::pedantic)]

use snafu::ResultExt;
use tracing::{Level, event};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use crate::chat::{ChatServer, Message};

mod chat;

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

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

    let messages = [Message::Text {
        role: "user".to_owned(),
        content: "Hello there".to_owned(),
    }];

    let msg = server
        .complete(&messages)
        .await
        .with_whatever_context(|_| "complete")?;

    event!(Level::INFO, "msg = {:?}", msg);

    Ok(())
}
