#![warn(clippy::pedantic)]

use std::{
    io::{self, Write},
    rc::Rc,
    sync::Arc,
};

use snafu::ResultExt;
use tracing::{Level, event};

use crate::chat::{ChatHistory, ChatServer, Message, OpenAICompatChatHistory, TextPayload};

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

pub async fn main_loop<S>(server: &S) -> Result<()>
where
    S: ChatServer,
{
    let history = Rc::new(OpenAICompatChatHistory::new());

    // TODO add system message

    loop {
        let my_hist = history.clone();

        print!("U> ");
        io::stdout().flush().whatever_context("flush")?;

        let mut input = String::new();
        let len = io::stdin()
            .read_line(&mut input)
            .whatever_context("read_line")?;

        if len == 0 {
            break Ok(());
        }

        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        my_hist.add_message(Message::Text(TextPayload {
            role: "user".to_owned(),
            content: input.to_owned(),
        }))?;

        event!(Level::DEBUG, "history = {:?}", history);

        let msg = server
            .complete(my_hist)
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
