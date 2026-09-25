use serde_json::Value;

pub struct ToolDefinition {
    name: String,
    description: Option<String>,
    // TODO Will be lazy for now and just represent entire JSON-schema as JSON
    parameters: Option<Value>,
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
