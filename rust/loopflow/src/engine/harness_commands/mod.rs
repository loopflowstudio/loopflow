use std::process::Command;

use crate::engine::agent::{AgentCapabilities, AgentConfig, ProcessConfig};
use crate::engine::config::parse_model;

mod claude;
mod codex;
mod gemini;
mod opencode;

pub(crate) trait HarnessCommandBuilder: Send + Sync {
    fn build_command(
        &self,
        launch: &AgentConfig,
        process: &ProcessConfig,
        capabilities: &AgentCapabilities,
    ) -> Vec<String>;

    fn apply_env(&self, _cmd: &mut Command, _process: &ProcessConfig) {}
}

type BuilderFactory = fn(model_variant: Option<String>) -> Box<dyn HarnessCommandBuilder>;

const REGISTRY: &[(&str, BuilderFactory)] = &[
    ("claude", claude::build),
    ("codex", codex::build),
    ("gemini", gemini::build),
    ("opencode", opencode::build),
];

pub(crate) fn builder_for_model(model: &str) -> Box<dyn HarnessCommandBuilder> {
    let (harness, variant) = parse_model(model);

    REGISTRY
        .iter()
        .find(|(name, _)| *name == harness)
        .map(|(_, build)| build(variant))
        .unwrap_or_else(|| claude::build_fallback(model))
}
