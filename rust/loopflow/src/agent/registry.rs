use crate::agent::anthropic::ToolDefinition;
use crate::chat::AgentEvent;

/// Result of a tool invocation, returned to the turn loop.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    /// Text result sent back to the model.
    pub output: String,
    /// Optional event emitted to consumers (e.g. for boundary tools like send_message).
    pub event: Option<AgentEvent>,
}

/// A tool the agent can invoke during a turn.
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self) -> ToolDefinition;
    fn call(&self, input: &serde_json::Value) -> ToolResult;
}

/// Registry of tools available to the agent during a turn.
#[derive(Debug, Default)]
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl std::fmt::Debug for dyn Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tool({})", self.name())
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    /// Dispatch a tool call by name. Returns `None` if the tool is not registered.
    pub fn dispatch(&self, name: &str, input: &serde_json::Value) -> Option<ToolResult> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.call(input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;

    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "echo".to_string(),
                description: "Echo back the input".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" }
                    },
                    "required": ["text"]
                }),
            }
        }

        fn call(&self, input: &serde_json::Value) -> ToolResult {
            let text = input
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("(empty)");
            ToolResult {
                output: text.to_string(),
                event: None,
            }
        }
    }

    #[test]
    fn registry_dispatches_registered_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));

        let result = registry
            .dispatch("echo", &serde_json::json!({"text": "hello"}))
            .expect("tool should be found");

        assert_eq!(result.output, "hello");
        assert!(result.event.is_none());
    }

    #[test]
    fn registry_returns_none_for_unknown_tool() {
        let registry = ToolRegistry::new();
        assert!(registry
            .dispatch("nonexistent", &serde_json::json!({}))
            .is_none());
    }

    #[test]
    fn registry_collects_definitions() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));

        let defs = registry.definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "echo");
    }
}
