#![warn(clippy::pedantic)]

use bon::bon;
use serde_json::Value;

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

trait ChatServer {
    fn complete(&self, messages: &[Message]) -> Result<Message>;
}

enum Message {
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
    base_url: String,
    api_key: Option<String>,
}

#[bon]
impl OpenAICompatChatServer {
    #[builder]
    pub fn new(base_url: &str, api_key: Option<&str>) -> Self {
        Self {
            base_url: base_url.to_owned(),
            api_key: api_key.map(ToOwned::to_owned),
        }
    }
}

impl ChatServer for OpenAICompatChatServer {
    fn complete(&self, messages: &[Message]) -> Result<Message> {
        Ok(Message::Text {
            role: "assistant".to_owned(),
            content: "Hello there".to_owned(),
        })
    }
}
