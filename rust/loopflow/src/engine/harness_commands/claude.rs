use crate::engine::agent::{build_claude_command, AgentCapabilities, AgentConfig, ProcessConfig};

use super::HarnessCommandBuilder;

#[derive(Debug, Clone)]
pub(crate) struct ClaudeCommandBuilder {
    model_variant: Option<String>,
}

impl ClaudeCommandBuilder {
    fn new(model_variant: Option<String>) -> Self {
        Self { model_variant }
    }
}

impl HarnessCommandBuilder for ClaudeCommandBuilder {
    fn build_command(
        &self,
        launch: &AgentConfig,
        process: &ProcessConfig,
        capabilities: &AgentCapabilities,
    ) -> Vec<String> {
        build_claude_command(launch, process, capabilities, self.model_variant.as_deref())
    }
}

pub(crate) fn build(_model: &str, model_variant: Option<String>) -> Box<dyn HarnessCommandBuilder> {
    Box::new(ClaudeCommandBuilder::new(model_variant))
}

pub(crate) fn build_fallback(model: &str) -> Box<dyn HarnessCommandBuilder> {
    Box::new(ClaudeCommandBuilder::new(Some(model.to_string())))
}
