use std::{any::Any, cell::RefCell, rc::Rc};

use bon::bon;
use reqwest::{Client, RequestBuilder};
use serde_json::{Value, json};
use snafu::{FromString, Whatever, prelude::*};
use tracing::{Level, event};

use crate::Result;
use crate::chat::Message::FunctionCall;
use crate::tools::ToolDefinition;

pub trait ChatHistory: Any {
    // Note: Expects interior mutability
    fn add_message(&self, message: Message) -> Result<()>;

    // To allow downcasting into specific impls
    fn as_any(&self) -> &dyn Any;
}

pub trait ChatServer {
    async fn complete(
        &self,
        messages: Rc<dyn ChatHistory>,
        tools: &[ToolDefinition],
    ) -> Result<Vec<Message>>;
}

#[derive(Debug, Clone)]
pub struct TextPayload {
    pub role: String,
    pub content: String,
}

#[derive(Debug)]
pub struct FunctionCallPayload {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug)]
pub struct FunctionCallResultPayload {
    pub id: String,
    #[expect(dead_code)]
    pub name: String,
    pub result: String,
}

#[derive(Debug)]
pub enum Message {
    Text(TextPayload),
    FunctionCall(FunctionCallPayload),
    FunctionCallResult(FunctionCallResultPayload),
}

pub struct OpenAICompatChatServer {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
}

trait RequestBuilderExt {
    fn maybe_bearer_auth(self, token: Option<&str>) -> RequestBuilder;
}

impl RequestBuilderExt for RequestBuilder {
    fn maybe_bearer_auth(self, token: Option<&str>) -> RequestBuilder {
        if let Some(tok) = token {
            self.bearer_auth(tok)
        } else {
            self
        }
    }
}

#[bon]
impl OpenAICompatChatServer {
    #[builder]
    pub fn new(base_url: &str, api_key: Option<&str>, model: &str) -> Self {
        let client = Client::builder().build().unwrap();
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.map(ToOwned::to_owned),
            model: model.to_owned(),
        }
    }

    fn endpoint(base_url: &str, path: &str) -> String {
        // TODO there's probably a way to ensure the result is still valid
        let mut result = String::new();
        // Assumes base_url has already been right-stripped of /
        result.push_str(base_url);
        result.push_str(path);
        result
    }

    // TODO Or maybe this should be in ChatServer?
    #[expect(dead_code)]
    pub fn set_model(&mut self, model: &str) {
        self.model.clone_from(&model.to_owned());
    }

    fn tools_to_json(tools: &[ToolDefinition]) -> Value {
        let mut tools_json: Vec<Value> = Vec::new();
        for tool in tools {
            tools_json.push(json!({
                "type":"function",
                "function":{
                    "name":&tool.name,
                    "description":&tool.description,
                    "parameters":&tool.parameters,
                }
            }));
        }
        json!(tools_json)
    }
}

#[derive(Debug)]
pub struct OpenAICompatChatHistory {
    messages: RefCell<Vec<Value>>,
}

impl OpenAICompatChatHistory {
    pub fn new() -> Self {
        Self {
            messages: RefCell::new(Vec::new()),
        }
    }
}

impl ChatHistory for OpenAICompatChatHistory {
    fn add_message(&self, message: Message) -> Result<()> {
        let msg = match message {
            Message::Text(payload) => {
                json!({
                    "role":payload.role,
                    "content":payload.content
                })
            }
            Message::FunctionCallResult(payload) => {
                json!({
                    "role":"tool",
                    "tool_call_id":&payload.id,
                    "content":&payload.result,
                })
            }
            FunctionCall(_) => panic!(),
        };
        self.messages.borrow_mut().push(msg);
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

mod responses {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct ResponseText {
        pub role: String,
        pub content: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseToolCall {
        pub id: String,
        pub function: ResponseToolCallFunction,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseToolCallFunction {
        pub name: String,
        pub arguments: String,
    }
}

impl ChatServer for OpenAICompatChatServer {
    async fn complete(
        &self,
        messages: Rc<dyn ChatHistory>,
        tools: &[ToolDefinition],
    ) -> Result<Vec<Message>> {
        let my_hist = messages
            .as_any()
            .downcast_ref::<OpenAICompatChatHistory>()
            .ok_or_else(|| {
                Whatever::without_source("must use OpenAICompatChatHistory".to_string())
            })?;

        let tools_json = Self::tools_to_json(tools);
        let request_json = json!({
            "model":&self.model,
            "messages":&my_hist.messages,
            "tools":tools_json,
            "stream":false,
        });
        let response = self
            .client
            .post(Self::endpoint(&self.base_url, "/chat/completions"))
            .maybe_bearer_auth(self.api_key.as_deref())
            .json(&request_json)
            .send()
            .await
            .with_whatever_context(|_| "POST /chat/completions")?;

        let response_json: Value = response
            .json()
            .await
            .with_whatever_context(|_| "POST /chat/completions")?;

        event!(Level::TRACE, "response_json = {response_json}");

        // TODO Lots of assumptions here
        let response_msg = response_json
            .pointer("/choices/0/message")
            .ok_or_else(|| Whatever::without_source("JSON decode".to_string()))?;

        event!(
            Level::DEBUG,
            "response_msg = {}",
            serde_json::to_string_pretty(response_msg).unwrap()
        );

        my_hist.messages.borrow_mut().push(response_msg.clone());

        if let Some(tool_calls) = response_msg.get("tool_calls").and_then(Value::as_array) {
            // For now we're assuming that if there are tool_calls, there can
            // be no other content.
            let mut calls = Vec::new();
            for tc in tool_calls {
                let decoded: responses::ResponseToolCall =
                    serde_json::from_value(tc.clone()).whatever_context("JSON decode")?;
                let arguments: Value = serde_json::from_str(&decoded.function.arguments)
                    .whatever_context("JSON parse")?;
                calls.push(FunctionCall(FunctionCallPayload {
                    id: decoded.id,
                    name: decoded.function.name,
                    arguments,
                }));
            }
            Ok(calls)
        } else {
            // We COULD just use TextPayload?
            let decoded: responses::ResponseText =
                serde_json::from_value(response_msg.clone()).whatever_context("JSON decode")?;
            Ok(vec![Message::Text(TextPayload {
                role: decoded.role,
                content: decoded.content,
            })])
        }
    }
}
