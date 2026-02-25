use std::path::PathBuf;

use crate::engine::agent::AgentConfig;
use crate::engine::config::Config;
use crate::engine::error::CoreError;
use crate::engine::fork::merge_directions;
use crate::engine::prompt::{
    default_gather_sources, drop_native_instruction_docs, format_context_prompt, format_prompt,
    format_task_prompt, gather_context, trim_context_with_breakdown, ContextBreakdown, Document,
    DocumentSource, GatherContextOpts, PromptComponents, PromptFormatMode, Surface,
    DEFAULT_CONTEXT_BUDGET,
};
use crate::engine::structured_reply::{structured_replies_for_context, ClientContext};

/// Optional per-source overrides for context gathering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextSourceOverrides {
    pub lfdocs: Option<bool>,
    pub diff_files: Option<bool>,
    pub diff: Option<bool>,
    pub clipboard: Option<bool>,
}

/// Canonical launch-prep input shared by CLI and session call sites.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchPromptInput {
    pub repo_root: PathBuf,
    pub step: Option<String>,
    pub surface: Surface,
    pub directions: Vec<String>,
    pub area: Option<String>,
    pub wave: Option<String>,
    pub message: Option<String>,
    pub model: Option<String>,
    pub cwd: Option<PathBuf>,
    pub max_turns: Option<u32>,
    pub yolo_mode: bool,
    pub include_config_directions: bool,
    pub include_config_area: bool,
    pub source_overrides: ContextSourceOverrides,
    pub summary: Option<String>,
    pub client_context: ClientContext,
}

/// Canonical launch-prep output.
#[derive(Debug, Clone)]
pub struct PreparedLaunchPrompt {
    pub config: AgentConfig,
    pub components: PromptComponents,
    pub breakdown: ContextBreakdown,
    pub prompt: String,
}

/// Build context + launch config from canonical launch-prep input.
pub fn prepare_launch_prompt(
    config: &Config,
    input: LaunchPromptInput,
) -> Result<PreparedLaunchPrompt, CoreError> {
    let LaunchPromptInput {
        repo_root,
        step,
        surface,
        directions: requested_directions,
        area,
        wave,
        message,
        model,
        cwd,
        max_turns,
        yolo_mode,
        include_config_directions,
        include_config_area,
        source_overrides,
        summary,
        client_context,
    } = input;

    let config_directions: &[String] = if include_config_directions {
        config.direction.as_deref().unwrap_or_default()
    } else {
        &[]
    };
    let directions = merge_directions(config_directions, &requested_directions);

    let area = area.or_else(|| {
        if include_config_area {
            config.area.clone()
        } else {
            None
        }
    });

    let lfdocs = source_overrides.lfdocs.unwrap_or(config.lfdocs);
    let diff_files = source_overrides.diff_files.unwrap_or(config.diff_files);
    let diff = source_overrides.diff.unwrap_or(config.diff);
    let clipboard = source_overrides.clipboard.unwrap_or(config.paste);

    let opts = GatherContextOpts {
        repo_root: repo_root.clone(),
        step,
        message,
        surface,
        directions,
        files: Vec::new(),
        sources: default_gather_sources(lfdocs, diff_files || diff, clipboard),
        area,
        wave,
    };

    let mut gathered = gather_context(&opts)?;
    drop_native_instruction_docs(gathered.components_mut(), &repo_root);

    if let Some(summary) = summary {
        gathered.components_mut().summaries.push(Document {
            path: "wave-summary".to_string(),
            content: summary,
            source: DocumentSource::Summary,
        });
    }

    let budgeted = trim_context_with_breakdown(gathered, DEFAULT_CONTEXT_BUDGET);
    let prompt = format_prompt(PromptFormatMode::Full, &budgeted).into_string();
    let system_prompt = format_context_prompt(&budgeted);
    let task_prompt = format_task_prompt(&budgeted);

    let model = model
        .or_else(|| budgeted.step.as_ref().and_then(|step| step.model.clone()))
        .unwrap_or_else(|| config.agent_model.clone());
    let (components, breakdown) = budgeted.into_parts();
    let action_style = components
        .step
        .as_ref()
        .and_then(|step| step.action_style.as_deref());
    let launch = AgentConfig {
        system_prompt,
        task_prompt,
        model: Some(model),
        max_turns,
        cwd: Some(cwd.unwrap_or(repo_root)),
        skip_permissions: yolo_mode,
        structured_replies: structured_replies_for_context(&client_context, action_style),
    };

    Ok(PreparedLaunchPrompt {
        config: launch,
        components,
        breakdown,
        prompt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn create_repo_fixture() -> tempfile::TempDir {
        let tmp = tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join(".lf/steps")).expect("steps dir");
        fs::create_dir_all(tmp.path().join(".lf/directions")).expect("directions dir");
        fs::write(
            tmp.path().join(".lf/steps/test.md"),
            r#"---
model: codex:o3
directions: [thorough]
action_style: procedural
---
Test step body.
"#,
        )
        .expect("write step");
        fs::write(
            tmp.path().join(".lf/directions/thorough.md"),
            "Be thorough.",
        )
        .expect("write direction");
        fs::write(
            tmp.path().join(".lf/directions/config.md"),
            "Config direction.",
        )
        .expect("write config direction");
        tmp
    }

    fn default_test_config() -> Config {
        Config {
            agent_model: "claude:opus".to_string(),
            lfdocs: false,
            diff_files: false,
            diff: false,
            paste: false,
            ..Config::default()
        }
    }

    #[test]
    fn prepare_launch_prompt_prefers_step_model_when_no_override() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                step: Some("test".to_string()),
                surface: Surface::Headless,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        assert_eq!(prepared.config.model.as_deref(), Some("codex:o3"));
    }

    #[test]
    fn prepare_launch_prompt_prefers_explicit_model_override() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                step: Some("test".to_string()),
                model: Some("claude:sonnet".to_string()),
                surface: Surface::Headless,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        assert_eq!(prepared.config.model.as_deref(), Some("claude:sonnet"));
    }

    #[test]
    fn prepare_launch_prompt_merges_config_directions_when_enabled() {
        let tmp = create_repo_fixture();
        let config = Config {
            direction: Some(vec!["config".to_string()]),
            ..default_test_config()
        };

        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                surface: Surface::Headless,
                directions: vec!["thorough".to_string()],
                include_config_directions: true,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        let names: Vec<String> = prepared
            .components
            .directions
            .iter()
            .map(|direction| direction.name.clone())
            .collect();
        assert_eq!(names, vec!["config".to_string(), "thorough".to_string()]);
    }

    #[test]
    fn prepare_launch_prompt_uses_config_area_when_enabled() {
        let tmp = create_repo_fixture();
        fs::create_dir_all(tmp.path().join("docs")).expect("docs dir");
        fs::write(tmp.path().join("docs/README.md"), "area doc").expect("write area doc");
        let config = Config {
            area: Some("docs".to_string()),
            ..default_test_config()
        };

        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                include_config_area: true,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        assert_eq!(prepared.components.area.as_deref(), Some("docs"));
    }

    #[test]
    fn prepare_launch_prompt_injects_structured_replies_for_ui_context() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                step: Some("test".to_string()),
                client_context: ClientContext {
                    has_ui: true,
                    compact: true,
                },
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        assert_eq!(prepared.config.structured_replies.len(), 1);
        assert_eq!(
            prepared.config.structured_replies[0].name,
            "suggest_actions"
        );
        assert!(
            prepared.config.structured_replies[0]
                .guidance
                .contains("up to 3"),
            "compact UI should cap suggestions to 3"
        );
        assert!(
            prepared.config.structured_replies[0]
                .guidance
                .contains("workflow forward"),
            "step action_style should shape guidance"
        );
    }
}
