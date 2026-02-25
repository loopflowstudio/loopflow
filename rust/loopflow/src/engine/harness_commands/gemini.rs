use std::process::Command;

use crate::engine::agent::{build_gemini_command, AgentCapabilities, AgentConfig, ProcessConfig};

use super::HarnessCommandBuilder;

#[derive(Debug, Clone)]
struct GeminiCommandBuilder {
    model_variant: Option<String>,
}

impl HarnessCommandBuilder for GeminiCommandBuilder {
    fn build_command(
        &self,
        launch: &AgentConfig,
        process: &ProcessConfig,
        _capabilities: &AgentCapabilities,
    ) -> Vec<String> {
        build_gemini_command(launch, process, self.model_variant.as_deref())
    }

    fn apply_env(&self, cmd: &mut Command, process: &ProcessConfig) {
        if let Some(ref context_file) = process.context_file {
            cmd.env(
                "GEMINI_SYSTEM_MD",
                context_file.to_string_lossy().to_string(),
            );
        }
    }
}

pub(crate) fn build(model_variant: Option<String>) -> Box<dyn HarnessCommandBuilder> {
    Box::new(GeminiCommandBuilder { model_variant })
}
