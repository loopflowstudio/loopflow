use std::process::Command;

use crate::engine::agent::{
    build_opencode_command, build_opencode_env, AgentCapabilities, AgentConfig, ProcessConfig,
};

use super::HarnessCommandBuilder;

#[derive(Debug, Clone)]
struct OpenCodeCommandBuilder {
    model_variant: Option<String>,
}

impl HarnessCommandBuilder for OpenCodeCommandBuilder {
    fn build_command(
        &self,
        _launch: &AgentConfig,
        process: &ProcessConfig,
        _capabilities: &AgentCapabilities,
    ) -> Vec<String> {
        build_opencode_command(process, self.model_variant.as_deref())
    }

    fn apply_env(&self, cmd: &mut Command, process: &ProcessConfig) {
        if let Some(env_val) = build_opencode_env(process) {
            cmd.env("OPENCODE_CONFIG_CONTENT", env_val);
        }
    }
}

pub(crate) fn build(model_variant: Option<String>) -> Box<dyn HarnessCommandBuilder> {
    Box::new(OpenCodeCommandBuilder { model_variant })
}
