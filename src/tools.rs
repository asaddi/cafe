use std::collections::HashMap;

use async_trait::async_trait;
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, ClientCapabilities, Implementation, InitializeRequestParams},
    service::RunningService,
    transport::StreamableHttpClientTransport,
};
use schemars::{JsonSchema, schema_for};
use serde::Deserialize;
use serde_json::Value;
use snafu::ResultExt;
use tracing::{Level, event};

use crate::Result;

#[derive(Debug)]
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
#[async_trait]
pub trait ToolHandler {
    fn get_tools(&self) -> Vec<ToolDefinition>;
    fn is_handled(&self, name: &str) -> bool; // TODO I don't like this
    async fn handle(&self, name: &str, arguments: Value) -> Result<String>;
}

#[derive(Debug)]
pub struct McpToolHandler {
    client: RunningService<RoleClient, InitializeRequestParams>,
    tools: HashMap<String, rmcp::model::Tool>,
}

impl McpToolHandler {
    pub async fn new(uri: &str) -> Result<Self> {
        // TODO I have no idea what I'm doing here
        // "if it compiles, it's correct"
        let transport = StreamableHttpClientTransport::from_uri(uri);
        let config = InitializeRequestParams::new(
            ClientCapabilities::default(),
            Implementation::new("cafe", env!("CARGO_PKG_VERSION")),
        );
        let client = config
            .serve(transport)
            .await
            .whatever_context("rmcp serve")?;

        let tools = &client
            .list_all_tools()
            .await
            .whatever_context("list_all_tools")?;
        let mut tools_map = HashMap::new();
        for tool in tools {
            tools_map.insert(tool.name.to_string(), tool.clone());
        }

        Ok(McpToolHandler {
            client,
            tools: tools_map,
        })
    }
}

#[async_trait]
impl ToolHandler for McpToolHandler {
    fn get_tools(&self) -> Vec<ToolDefinition> {
        let mut tools = Vec::new();

        for mcp_tool in self.tools.values() {
            let params = if mcp_tool.input_schema.is_empty() {
                None
            } else {
                Some(Value::Object((*mcp_tool.input_schema).clone()))
            };
            let tool = ToolDefinition::builder()
                .name(&mcp_tool.name)
                .maybe_description(mcp_tool.description.as_deref())
                .maybe_parameters(params)
                .build();
            tools.push(tool);
        }

        tools
    }

    async fn handle(&self, name: &str, arguments: Value) -> Result<String> {
        if self.tools.contains_key(name) {
            let call_result = self
                .client
                .call_tool(
                    CallToolRequestParams::new(name.to_string())
                        .with_arguments((*arguments.as_object().unwrap()).clone()),
                )
                .await
                .whatever_context("call_tool")?;
            event!(Level::TRACE, "call_result = {:?}", call_result);

            let mut texts = Vec::new();
            for cb in call_result.content {
                if let Some(text_content) = cb.as_text() {
                    texts.push(text_content.text.clone());
                } else {
                    event!(Level::WARN, "can't handle MCP response of type {:?}", cb);
                }
            }
            // FIXME No idea what to do if there's multiple blocks, just join them
            Ok(texts.join("\n"))
        } else {
            unimplemented!()
        }
    }

    fn is_handled(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RollDiceParams {
    #[schemars(range(min = 1))]
    /// The number of sides of each die, e.g. 6 is a standard six-sided die.
    faces: u64,

    #[schemars(range(min = 1))]
    /// The number of dice to roll.
    number: u64,
}

pub struct BuiltinTools;

#[async_trait]
impl ToolHandler for BuiltinTools {
    fn get_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition::builder()
                .name("roll_dice")
                .description("Roll a number of dice (with the specified number of faces), returning the total result.")
                .parameters(schema_for!(RollDiceParams).into())
                .build()
        ]
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "random number generation isn't async, but this is a generic trait"
    )]
    async fn handle(&self, name: &str, arguments: Value) -> Result<String> {
        let value = match name {
            "roll_dice" => {
                let args: RollDiceParams =
                    serde_json::from_value(arguments).whatever_context("bad arguments")?;

                let mut total: u64 = 0;

                for _ in 0..args.number {
                    total += rand::random_range(1..=args.faces);
                }

                format!("{total}")
            }
            _ => {
                unimplemented!("tool: {name}");
            }
        };

        Ok(value)
    }

    fn is_handled(&self, name: &str) -> bool {
        matches!(name, "roll_dice")
    }
}
