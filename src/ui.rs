use std::{
    io::{self, Write},
    rc::Rc,
};

use snafu::ResultExt;
use tracing::{Level, event};

use crate::{
    chat::{ChatHistory, ChatServer, Message, TextPayload},
    tools::ToolDefinition,
};

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

pub async fn main_loop<S, H>(server: S, history: H, tools: &[ToolDefinition]) -> Result<()>
where
    S: ChatServer,
    H: ChatHistory + std::fmt::Debug,
{
    let server = &server;
    let history = Rc::new(history);

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

        println!();

        my_hist.add_message(Message::Text(TextPayload {
            role: "user".to_owned(),
            content: input.to_owned(),
        }))?;

        event!(Level::DEBUG, "history = {:?}", history);

        let msg = server
            .complete(my_hist, tools)
            .await
            .whatever_context("complete")?;

        event!(Level::DEBUG, "msg = {:?}", msg);

        match msg {
            Message::Text(payload) => {
                println!("A> {}", payload.content);
                println!();
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
