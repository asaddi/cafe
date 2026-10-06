use std::{
    io::{self, Write},
    rc::Rc,
};

use serde_json::json;
use snafu::ResultExt;
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};
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
    mut tool_handler: impl ToolHandler + Send + Sync,
    sys_prompt_source: Box<dyn SystemPromptSource>,
    username: Option<&str>,
) -> Result<()>
where
    S: ChatServer,
    H: ChatHistory + std::fmt::Debug,
{
    let server = &server;
    let history = Rc::new(history);

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

    let assistant_color = ColorSpec::new().set_fg(Some(Color::Yellow)).to_owned();

    let main_result = loop {
        let mut stdout = StandardStream::stdout(termcolor::ColorChoice::Auto);

        stdout.reset().whatever_context("reset")?;
        write!(&mut stdout, "§ ").whatever_context("write")?;
        stdout.flush().whatever_context("flush")?;

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

        writeln!(&mut stdout).whatever_context("writeln")?;

        history.clone().add_message(Message::Text(TextPayload {
            role: "user".to_owned(),
            content: input.to_owned(),
        }))?;

        let text_handler = |payload: TextPayload| {
            stdout
                .set_color(&assistant_color)
                .whatever_context("set_color")?;
            writeln!(&mut stdout, "{}", payload.content).whatever_context("writeln")?;
            writeln!(&mut stdout).whatever_context("writeln")?;
            Ok(())
        };

        agent_loop(server, history.clone(), text_handler, &tool_handler).await?;
    };

    tool_handler.shutdown().await?;

    main_result
}

async fn agent_loop<S, H, F>(
    server: &S,
    history: Rc<H>,
    mut text_handler: F,
    tool_handler: &(impl ToolHandler + Send + Sync),
) -> Result<()>
where
    S: ChatServer,
    H: ChatHistory + std::fmt::Debug,
    F: FnMut(TextPayload) -> Result<()>,
{
    let tools = tool_handler.get_tools();

    loop {
        event!(Level::TRACE, "history = {:?}", history);

        let results = server
            .complete(history.clone(), &tools)
            .await
            .whatever_context("complete")?;

        let mut tool_called = false;

        // Usually there's only one result.
        // If there are multiple, they will almost certainly be tool
        // calls.
        for msg in results {
            event!(Level::DEBUG, "msg = {:?}", msg);

            // Once true, tool_called should always be true... until we run
            // through the model again (above).
            tool_called = tool_called
                || match msg {
                    Message::Text(payload) => {
                        text_handler(payload)?;
                        false // Not a tool call
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
                        true // Is a tool call
                    }
                    Message::FunctionCallResult(_) => {
                        panic!()
                    }
                };
        }

        // If tools were called, then history was surely altered.
        // Re-run through the model.
        if !tool_called {
            // Otherwise, wait for the next user input
            break Ok(());
        }
    }
}
