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

#[cfg(test)]
mod tests {
    use super::{resolve_definition, DefinitionKind, Target};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn ordinary_skill_discovery_ignores_legacy_review_instructions() {
        let _lock = crate::journal::test_env_lock();
        let _environment = crate::test_ambient::EnvGuard::clear(&["LF_HUMAN_SESSION"]);
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        fs::write(
            repo.path().join(".lf/skills/review.md"),
            "Current instructions",
        )
        .unwrap();
        let mut captured = crate::engine::Skill::named("review");
        captured.content = Some("Retired captured instructions".into());
        std::env::set_var(
            "LF_HUMAN_SESSION",
            serde_json::json!({
                "kind": "flow",
                "token": {
                    "task_id": crate::work::task::TaskId::new(),
                    "invocation_id": "legacy",
                    "flow": "legacy",
                    "node_id": "review",
                    "skill": captured,
                    "iteration": 0
                }
            })
            .to_string(),
        );

        let Target::Skill(skill) =
            resolve_definition(repo.path(), "review", Some(DefinitionKind::Skill)).unwrap()
        else {
            panic!("explicit skill must resolve to a skill");
        };
        assert_eq!(skill.content.as_deref(), Some("Current instructions"));
    }

    #[test]
    fn operate_resolves_to_repo_and_wave_operate_stays_explicit() {
        let tmp = TempDir::new().unwrap();
        for (name, expected) in [
            ("operate", "repo/operate"),
            ("repo/operate", "repo/operate"),
            ("wave/operate", "wave/operate"),
        ] {
            let Target::Skill(skill) = resolve_definition(tmp.path(), name, None).unwrap() else {
                panic!("expected a skill for {name}");
            };
            assert_eq!(skill.name, expected);
        }

        let skills = tmp.path().join(".lf/skills/repo");
        fs::create_dir_all(&skills).unwrap();
        fs::write(skills.join("operate.md"), "Repository operation override").unwrap();
        for name in ["operate", "repo/operate"] {
            let Target::Skill(skill) = resolve_definition(tmp.path(), name, None).unwrap() else {
                panic!("expected a skill for {name}");
            };
            assert_eq!(skill.name, "repo/operate");
            assert_eq!(
                skill.content.as_deref(),
                Some("Repository operation override")
            );
        }
    }

    #[test]
    fn untyped_names_resolve_available_definitions() {
        let tmp = TempDir::new().expect("tempdir");
        for name in ["design", "launch-plan", "debug", "unbreak", "implement"] {
            assert!(
                matches!(
                    resolve_definition(tmp.path(), name, None).unwrap(),
                    Target::Skill(_)
                ),
                "{name}"
            );
        }
        for name in ["pursue", "incident", "vsm-operate"] {
            assert!(
                matches!(
                    resolve_definition(tmp.path(), name, None).unwrap(),
                    Target::Flow(_)
                ),
                "{name}"
            );
        }
        // A workflow is traversed by `lf task run`, never run as a Flow.
        for name in ["feature", "code"] {
            let error = resolve_definition(tmp.path(), name, None).unwrap_err();
            assert!(matches!(error, crate::engine::LoadError::Workflow(_)));
        }
    }
}
