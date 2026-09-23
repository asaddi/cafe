#![warn(clippy::pedantic)]

use bon::bon;
use reqwest::{Client, RequestBuilder};
use serde_json::{Value, json};
use snafu::{FromString, Whatever, prelude::*};
use tracing::{Level, event};

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

pub trait ChatServer {
    async fn complete(&self, messages: &[Message]) -> Result<Message>;
}

#[derive(Debug)]
pub enum Message {
    Text {
        role: String,
        content: String,
    },
    FunctionCall {
        id: String,
        name: String,
        arguments: Value,
    },
    FunctionCallResult {
        id: String,
        name: String,
        result: String,
    },
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

    fn to_json(message: &Message) -> Value {
        match message {
            Message::Text { role, content } => {
                json!({
                    "role":role,
                    "content":content
                })
            }
            _ => panic!("not yet implemented"),
        }
    }
}

impl ChatServer for OpenAICompatChatServer {
    async fn complete(&self, messages: &[Message]) -> Result<Message> {
        let mut messages_json: Vec<Value> = Vec::new();
        for msg in messages {
            messages_json.push(Self::to_json(msg));
        }
        let request_json = json!({"model":&self.model,"messages":messages_json});
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

        let role = response_msg
            .get("role")
            .and_then(|r| r.as_str())
            .ok_or_else(|| Whatever::without_source("JSON decode".to_string()))?;
        let content = response_msg
            .get("content")
            .and_then(|r| r.as_str())
            .ok_or_else(|| Whatever::without_source("JSON decode".to_string()))?;

        Ok(Message::Text {
            role: role.to_owned(),
            content: content.to_owned(),
        })
    }
}
