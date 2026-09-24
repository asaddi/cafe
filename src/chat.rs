#![warn(clippy::pedantic)]

use std::{any::Any, cell::RefCell, rc::Rc};

use bon::bon;
use reqwest::{Client, RequestBuilder};
use serde_json::{Value, json};
use snafu::{FromString, Whatever, prelude::*};
use tracing::{Level, event};

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

pub trait ChatHistory: Any {
    fn add_message(&self, message: Message) -> Result<()>;
    fn as_any(&self) -> &dyn Any;
}

pub trait ChatServer {
    async fn complete(&self, messages: Rc<dyn ChatHistory>) -> Result<Message>;
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
    pub fn set_model(&mut self, model: &str) {
        self.model.clone_from(&model.to_owned());
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
            _ => todo!(),
        };
        self.messages.borrow_mut().push(msg);
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ChatServer for OpenAICompatChatServer {
    async fn complete(&self, messages: Rc<dyn ChatHistory>) -> Result<Message> {
        let my_hist = messages
            .as_any()
            .downcast_ref::<OpenAICompatChatHistory>()
            .unwrap();

        let request_json = json!({"model":&self.model,"messages":&my_hist.messages});
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
            .get("choices")
            .and_then(|c| {
                c.as_array()
                    .and_then(|a| a.first().and_then(|f| f.get("message")))
            })
            .ok_or_else(|| Whatever::without_source("JSON decode".to_string()))?;

        event!(Level::DEBUG, "response_msg = {}", response_msg);

        my_hist.messages.borrow_mut().push(response_msg.clone());

        let role = response_msg
            .get("role")
            .and_then(|r| r.as_str())
            .ok_or_else(|| Whatever::without_source("JSON decode".to_string()))?;
        let content = response_msg
            .get("content")
            .and_then(|r| r.as_str())
            .ok_or_else(|| Whatever::without_source("JSON decode".to_string()))?;

        Ok(Message::Text(TextPayload {
            role: role.to_owned(),
            content: content.to_owned(),
        }))
    }
}
