use std::time::Duration;

use async_trait::async_trait;
use indexmap::IndexMap;
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
    pub fn new<T, U>(name: T, description: Option<U>, parameters: Option<Value>) -> Self
    where
        T: AsRef<str>,
        U: ToString,
    {
        Self {
            name: name.as_ref().to_owned(),
            description: description.map(|s| s.to_string()),
            parameters,
        }
    }
}

#[async_trait]
pub trait ToolHandler {
    fn name(&self) -> &str;

    fn get_tools(&self) -> Vec<ToolDefinition>;

    fn is_handled(&self, name: &str) -> bool; // TODO I don't like this

    async fn handle(&self, name: &str, arguments: Value) -> Result<String>;

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct McpToolHandler {
    name: String,
    client: RunningService<RoleClient, InitializeRequestParams>,
    tools: IndexMap<String, rmcp::model::Tool>,
}

impl McpToolHandler {
    pub async fn new(uri: &str, name: &str) -> Result<Self> {
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
        let mut tools_map = IndexMap::new();
        for tool in tools {
            tools_map.insert(tool.name.to_string(), tool.clone());
        }

        Ok(McpToolHandler {
            name: name.to_owned(),
            client,
            tools: tools_map,
        })
    }
}

#[async_trait]
impl ToolHandler for McpToolHandler {
    fn name(&self) -> &str {
        &self.name
    }

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
                .maybe_description(mcp_tool.description.clone())
                .maybe_parameters(params)
                .build();
            tools.push(tool);
        }

        tools
    }

    fn is_handled(&self, name: &str) -> bool {
        self.tools.contains_key(name)
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

    async fn shutdown(&mut self) -> Result<()> {
        self.client
            .close_with_timeout(Duration::from_secs(10))
            .await
            .whatever_context("close_with_timeout")?;

        Ok(())
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
    #[expect(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        "Built-in Tools"
    }

    fn get_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition::builder()
                .name("roll_dice")
                .description("Roll a number of dice (with the specified number of faces), returning the total result.")
                .parameters(schema_for!(RollDiceParams).into())
                .build()
        ]
    }

    fn is_handled(&self, name: &str) -> bool {
        matches!(name, "roll_dice")
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
}

pub struct ToolDispatcher {
    builtins: BuiltinTools,
    extra_handlers: Vec<Box<dyn ToolHandler + Send + Sync>>,
}

impl ToolDispatcher {
    pub fn new(extra_handlers: Vec<Box<dyn ToolHandler + Send + Sync>>) -> Self {
        Self {
            builtins: BuiltinTools,
            extra_handlers,
        }
    }
}

#[async_trait]
impl ToolHandler for ToolDispatcher {
    #[expect(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        "Tool Dispatcher"
    }

    fn get_tools(&self) -> Vec<ToolDefinition> {
        let mut tools = Vec::new();
        tools.extend(self.builtins.get_tools());
        for handler in &self.extra_handlers {
            tools.extend(handler.get_tools());
        }
        // TODO cache this, maybe a simple TTL cache
        tools
    }

    fn is_handled(&self, _name: &str) -> bool {
        true
    }

    async fn handle(&self, name: &str, arguments: Value) -> Result<String> {
        if self.builtins.is_handled(name) {
            self.builtins.handle(name, arguments).await
        } else {
            for handler in &self.extra_handlers {
                // TODO There's no real resolution for duplicate tool names
                // other than "first one wins."
                if handler.is_handled(name) {
                    return handler.handle(name, arguments).await;
                }
            }
            panic!()
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        for handler in self.extra_handlers.iter_mut().rev() {
            if let Err(e) = handler.shutdown().await {
                event!(Level::ERROR, "shutting down {}: {e}", handler.name());
            }
        }

        if let Err(e) = self.builtins.shutdown().await {
            // Shouldn't happen, but who knows what tools may be added in the future
            event!(Level::ERROR, "shutting down {}: {e}", self.builtins.name());
        }

        Ok(())
    }
}
