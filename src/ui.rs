use std::{
    io::{self, Write},
    rc::Rc,
};

use serde_json::json;
use snafu::ResultExt;
use tracing::{Level, event};

use crate::{
    chat::{
        ChatHistory, ChatServer, FunctionCallResultPayload,
        Message::{self, FunctionCallResult},
        TextPayload,
    },
    tools::{ToolDefinition, ToolHandler},
};

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

pub async fn main_loop<S, H>(
    server: S,
    history: H,
    tools: &[ToolDefinition],
    tool_handler: impl ToolHandler,
) -> Result<()>
where
    S: ChatServer,
    H: ChatHistory + std::fmt::Debug,
{
    let server = &server;
    let history = Rc::new(history);

    // TODO add system message

    loop {
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

        history.clone().add_message(Message::Text(TextPayload {
            role: "user".to_owned(),
            content: input.to_owned(),
        }))?;

        loop {
            event!(Level::DEBUG, "history = {:?}", history);

            let results = server
                .complete(history.clone(), tools)
                .await
                .whatever_context("complete")?;

            let mut tool_called = false;

            for msg in results {
                event!(Level::DEBUG, "msg = {:?}", msg);

                if match msg {
                    Message::Text(payload) => {
                        println!("A> {}", payload.content);
                        println!();
                        false
                    }
                    Message::FunctionCall(payload) => {
                        event!(Level::DEBUG, "payload = {:?}", payload);
                        // TODO This is where we perform the function call and then
                        // append the result back onto history
                        let tool_result = match tool_handler
                            .handle(&payload.name, payload.arguments.clone())
                            .await
                        {
                            Ok(result) => result,
                            Err(e) => json!({"error":e.to_string()}),
                        };
                        history.clone().add_message(FunctionCallResult(
                            FunctionCallResultPayload {
                                id: payload.id,
                                name: payload.name,
                                result: tool_result.to_string(),
                            },
                        ))?;
                        true
                    }
                    Message::FunctionCallResult(_) => {
                        panic!()
                    }
                } {
                    tool_called = true;
                }
            }

            // If tools were called, then history was surely altered.
            // Re-run through the model.
            if !tool_called {
                // Otherwise, wait for the next user input
                break;
            }
        }
    }
}
