use std::{
    io::{self, Write},
    rc::Rc,
};

use serde_json::json;
use snafu::ResultExt;
use tracing::{Level, event};

use crate::{
    Result,
    chat::{
        ChatHistory, ChatServer, FunctionCallResultPayload,
        Message::{self, FunctionCallResult},
        TextPayload,
    },
    prompt::SystemPromptSource,
    tools::ToolHandler,
};

pub async fn main_loop<S, H>(
    server: S,
    history: H,
    tool_handler: impl ToolHandler,
    sys_prompt_source: Box<dyn SystemPromptSource>,
    username: Option<&str>,
) -> Result<()>
where
    S: ChatServer,
    H: ChatHistory + std::fmt::Debug,
{
    let server = &server;
    let history = Rc::new(history);
    let tools = tool_handler.get_tools();

    // TODO notion of user's identity/persona
    let sys_prompt = sys_prompt_source.generate_system_prompt(username.unwrap_or("User"))?;
    let sys_prompt = sys_prompt.trim();
    if !sys_prompt.is_empty() {
        history.clone().add_message(Message::Text(TextPayload {
            // TODO some models (which?) apparently use "developer" nowadays.
            // How to handle?
            role: "system".to_owned(),
            content: sys_prompt.to_owned(),
        }))?;
    }

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
            event!(Level::TRACE, "history = {:?}", history);

            let results = server
                .complete(history.clone(), &tools)
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
                        let tool_result = match tool_handler
                            .handle(&payload.name, payload.arguments.clone())
                            .await
                        {
                            Ok(result) => result,
                            Err(e) => json!({"error":e.to_string()}).to_string(),
                        };
                        let call_result = FunctionCallResultPayload {
                            id: payload.id,
                            name: payload.name,
                            result: tool_result,
                        };
                        event!(Level::DEBUG, "call_result = {:?}", call_result);
                        history
                            .clone()
                            .add_message(FunctionCallResult(call_result))?;
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
