#![warn(clippy::pedantic)]

use std::io::{self, Write};

use snafu::ResultExt;
use tracing::{Level, event};

use crate::chat::{ChatServer, Message, TextPayload};

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

pub async fn main_loop<S>(server: &S) -> Result<()>
where
    S: ChatServer,
{
    loop {
        print!("U> ");
        io::stdout().flush().whatever_context("flush")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .whatever_context("read_line")?;

        let input = input.trim();

        let messages = [Message::Text(TextPayload {
            role: "user".to_owned(),
            content: input.to_owned(),
        })];

        let msg = server
            .complete(&messages)
            .await
            .whatever_context("complete")?;

        event!(Level::DEBUG, "msg = {:?}", msg);

        match msg {
            Message::Text(payload) => {
                println!("A> {}", payload.content);
            }
            Message::FunctionCall(payload) => {
                event!(Level::DEBUG, "payload = {:?}", payload);
                todo!();
            }
            Message::FunctionCallResult(_) => {
                panic!()
            }
        }
    }
}
