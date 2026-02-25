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

type BuilderFactory =
    fn(model: &str, model_variant: Option<String>) -> Box<dyn HarnessCommandBuilder>;

#[derive(Debug, Clone, Copy)]
struct HarnessRegistration {
    harness: &'static str,
    build: BuilderFactory,
}

const REGISTRY: &[HarnessRegistration] = &[
    HarnessRegistration {
        harness: "claude",
        build: claude::build,
    },
    HarnessRegistration {
        harness: "codex",
        build: codex::build,
    },
    HarnessRegistration {
        harness: "gemini",
        build: gemini::build,
    },
    HarnessRegistration {
        harness: "opencode",
        build: opencode::build,
    },
];

pub(crate) fn builder_for_model(model: &str) -> Box<dyn HarnessCommandBuilder> {
    let (harness, variant) = parse_model(model);

    REGISTRY
        .iter()
        .find(|registration| registration.harness == harness)
        .map(|registration| (registration.build)(model, variant))
        .unwrap_or_else(|| claude::build_fallback(model))
}
