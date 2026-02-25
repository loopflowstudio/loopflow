use crate::engine::agent::{build_codex_command, AgentCapabilities, AgentConfig, ProcessConfig};

use super::HarnessCommandBuilder;

#[derive(Debug, Clone)]
struct CodexCommandBuilder {
    model_variant: Option<String>,
}

impl HarnessCommandBuilder for CodexCommandBuilder {
    fn build_command(
        &self,
        launch: &AgentConfig,
        process: &ProcessConfig,
        _capabilities: &AgentCapabilities,
    ) -> Vec<String> {
        build_codex_command(launch, process, self.model_variant.as_deref())
    }
}

pub(crate) fn build(_model: &str, model_variant: Option<String>) -> Box<dyn HarnessCommandBuilder> {
    Box::new(CodexCommandBuilder { model_variant })
}
