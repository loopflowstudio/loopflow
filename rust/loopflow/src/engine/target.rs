use std::path::Path;

use crate::engine::flow::DefinitionLoader;
use crate::engine::{Command, FlowDefinition, LoadError, Skill, Step, XorDef};
use serde::{Deserialize, Serialize};

/// Kind restriction for named-definition lookup; commands are selected by the CLI tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionKind {
    Skill,
    Flow,
}

impl DefinitionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Flow => "flow",
        }
    }
}

/// An executable value shared by CLI selection and authored Flow composition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Target {
    Command(Command),
    Skill(Skill),
    Flow(FlowDefinition),
    Xor(XorDef),
}

impl Target {
    /// Adapt a selected executable only when the caller needs Flow execution.
    pub fn into_flow(self) -> FlowDefinition {
        let name = match &self {
            Self::Flow(flow) => flow.name.clone(),
            Self::Command(command) => command.display_name(),
            Self::Skill(skill) => skill.name.clone(),
            Self::Xor(_) => "xor".to_string(),
        };
        match self {
            Self::Flow(flow) => flow,
            target => FlowDefinition {
                name,
                items: vec![Step::new(target)],
            },
        }
    }
}

/// Resolve local definitions. Only absence permits falling back to a skill.
pub fn resolve_definition(
    repo: &Path,
    name: &str,
    kind: Option<DefinitionKind>,
) -> Result<Target, LoadError> {
    DefinitionLoader::new(repo).resolve(name, kind)
}
