use serde_json::Value;

use crate::Result;

pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    // TODO Will be lazy for now and just represent entire JSON-schema as JSON
    pub parameters: Option<Value>,
    // strict? Will most likely always be enabled.
}

#[bon::bon]
impl ToolDefinition {
    #[builder]
    pub fn new(name: &str, description: Option<&str>, parameters: Option<Value>) -> Self {
        Self {
            name: name.to_owned(),
            description: description.map(ToOwned::to_owned),
            parameters,
        }
    }
}

// This is technically more of a dispatcher, but we'll go with this for
// now.
pub trait ToolHandler {
    async fn handle(&self, name: &str, arguments: Value) -> Result<Value>;
}
