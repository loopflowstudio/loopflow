use std::path::PathBuf;

use crate::engine::agent::AgentConfig;
use crate::engine::config::{parse_agent, Config};
use crate::engine::error::CoreError;
use crate::engine::flow::Skill;
use crate::engine::prompt::{
    drop_native_instruction_docs, format_claude_system_prompt, format_claude_task_prompt,
    format_prompt, gather_context, Document, DocumentSource, GatherContextOpts, PromptComponents,
    PromptFormatMode, RelatedRepoContext, Surface,
};
use crate::engine::structured_reply::{structured_replies_for_context, ClientContext};

/// Optional per-source overrides for context gathering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextSourceOverrides {
    pub diff_files: Option<bool>,
    pub diff: Option<bool>,
    pub clipboard: Option<bool>,
}

/// Canonical launch-prep input shared by CLI and session call sites.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchPromptInput {
    pub repo_root: PathBuf,
    pub skill: Option<String>,
    pub resolved_skill: Option<Skill>,
    pub surface: Surface,
    pub directions: Vec<String>,
    pub docs: Vec<String>,
    pub wave: Option<String>,
    pub message: Option<String>,
    pub no_loopflow: bool,
    pub agent: Option<String>,
    pub cwd: Option<PathBuf>,
    pub max_turns: Option<u32>,
    pub yolo_mode: bool,
    pub include_config_directions: bool,
    pub source_overrides: ContextSourceOverrides,
    pub summary: Option<String>,
    pub client_context: ClientContext,
    /// Related repos resolved from the edge graph.
    pub related_repos: Vec<RelatedRepoContext>,
}

/// Canonical launch-prep output.
#[derive(Debug, Clone)]
pub struct PreparedLaunchPrompt {
    pub config: AgentConfig,
    pub components: PromptComponents,
    pub deduplicated_docs: Vec<Document>,
    pub prompt: String,
}

fn merge_directions(base: &[String], extra: &[String]) -> Vec<String> {
    let mut combined = base.to_vec();
    for direction in extra {
        if !combined.contains(direction) {
            combined.push(direction.clone());
        }
    }
    combined
}

/// Build context + launch config from canonical launch-prep input.
pub fn prepare_launch_prompt(
    config: &Config,
    input: LaunchPromptInput,
) -> Result<PreparedLaunchPrompt, CoreError> {
    let LaunchPromptInput {
        repo_root,
        skill,
        resolved_skill,
        surface,
        directions: mut requested_directions,
        docs: requested_docs,
        wave,
        message,
        no_loopflow,
        agent,
        cwd,
        max_turns,
        yolo_mode,
        include_config_directions,
        source_overrides,
        summary,
        client_context,
        related_repos,
    } = input;

    if let Some(skill) = resolved_skill.as_ref() {
        requested_directions = merge_directions(&skill.directions, &requested_directions);
    }

    let config_directions: &[String] = if include_config_directions {
        config.direction.as_deref().unwrap_or_default()
    } else {
        &[]
    };
    let directions = merge_directions(config_directions, &requested_directions);

    let mut docs = config.docs.clone();
    docs.extend(requested_docs);
    let diff_files = source_overrides.diff_files.unwrap_or(config.diff_files);
    let diff = source_overrides.diff.unwrap_or(config.diff);
    let clipboard = source_overrides.clipboard.unwrap_or(config.paste);

    let opts = GatherContextOpts {
        repo_root: repo_root.clone(),
        skill: if resolved_skill.is_some() {
            None
        } else {
            skill
        },
        message,
        operate: !no_loopflow,
        surface,
        directions,
        docs,
        files: Vec::new(),
        wave,
        include_diff: diff,
        include_diff_files: diff_files,
        include_clipboard: clipboard,
        related_repos,
    };

    let mut gathered = gather_context(&opts)?;
    if let Some(skill) = resolved_skill {
        gathered.components_mut().skill = Some(skill);
    }
    let deduplicated_docs = drop_native_instruction_docs(gathered.components_mut(), &repo_root);

    if let Some(summary) = summary {
        gathered.components_mut().summaries.push(Document {
            path: "wave-summary".to_string(),
            content: summary,
            source: DocumentSource::Summary,
        });
    }

    let prompt = format_prompt(PromptFormatMode::Full, gathered.components()).into_string();

    let agent = agent
        .or_else(|| {
            gathered
                .skill
                .as_ref()
                .and_then(|skill| skill.agent.clone())
        })
        .or_else(|| config.agent.clone())
        .or_else(|| {
            gathered
                .skill
                .as_ref()
                .and_then(|skill| skill.default_agent.clone())
        })
        .unwrap_or_else(|| "claude:opus".to_string());
    validate_agent_policy(&agent)?;

    // Keep only system-safe sections (operate/surface/directions) in
    // the system prompt. Repo content (docs, diffs, wave, clipboard) goes in the
    // task prompt to avoid triggering third-party app classifiers.
    let system_prompt = format_claude_system_prompt(gathered.components());
    let task_prompt = format_claude_task_prompt(gathered.components());
    let components = gathered.into_components();
    let action_style = components
        .skill
        .as_ref()
        .and_then(|skill| skill.action_style.as_deref());
    let launch = AgentConfig {
        system_prompt,
        task_prompt,
        agent: Some(agent),
        max_turns,
        resume_token: None,
        cwd: Some(cwd.unwrap_or(repo_root)),
        skip_permissions: yolo_mode,
        structured_replies: structured_replies_for_context(&client_context, action_style),
        directive_relay: None,
    };

    Ok(PreparedLaunchPrompt {
        config: launch,
        components,
        deduplicated_docs,
        prompt,
    })
}

fn validate_agent_policy(agent: &str) -> Result<(), CoreError> {
    let (harness, variant) = parse_agent(agent);
    if harness != "opencode" {
        return Ok(());
    }

    // `parse_agent` resolves bare `opencode` to the Loopflow-owned
    // `opencode/glm-5.2` default, so a variant is always present for OpenCode.
    // Validate explicit selections against the supported set.
    let variant = variant.expect("parse_agent resolves a default model for the opencode harness");
    if is_supported_opencode_model_variant(&variant) {
        return Ok(());
    }

    Err(CoreError::ExecutionFailed(format!(
        "unsupported OpenCode model '{}': supported variants are 'opencode/*' (excluding claude/codex families) and 'moonshotai/kimi*'",
        variant
    )))
}

fn is_supported_opencode_model_variant(variant: &str) -> bool {
    let variant = variant.trim().to_ascii_lowercase();
    let Some((provider, model_id)) = variant.split_once('/') else {
        return false;
    };
    if model_id.is_empty() {
        return false;
    }

    match provider {
        "moonshotai" => model_id.starts_with("kimi"),
        "opencode" => !model_id.contains("claude") && !model_id.contains("codex"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn create_repo_fixture() -> tempfile::TempDir {
        let tmp = tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join(".lf/skills")).expect("skills dir");
        fs::create_dir_all(tmp.path().join(".lf/directions")).expect("directions dir");
        fs::write(
            tmp.path().join(".lf/skills/test.md"),
            r#"---
agent: codex:o3
directions: [thorough]
action_style: procedural
---
Test skill body.
"#,
        )
        .expect("write skill");
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
            agent: Some("claude:opus".to_string()),
            diff_files: false,
            diff: false,
            paste: false,
            ..Config::default()
        }
    }

    #[test]
    fn prepare_launch_prompt_prefers_skill_agent_when_no_override() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                surface: Surface::Headless,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("codex:o3"));
    }

    #[test]
    fn prepare_launch_prompt_prefers_explicit_agent_override() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                agent: Some("claude:sonnet".to_string()),
                surface: Surface::Headless,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("claude:sonnet"));
    }

    #[test]
    fn prepare_launch_prompt_includes_loopflow_by_default() {
        let tmp = create_repo_fixture();
        let config = default_test_config();

        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                surface: Surface::Headless,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare prompt");
        assert!(prepared.prompt.contains("<lf:loopflow>"));
        assert!(prepared.config.system_prompt.contains("lf commit"));
        assert!(prepared.config.system_prompt.contains("Execute Here First"));
        assert!(!prepared.config.system_prompt.contains("lf pm show"));
        assert!(!prepared.config.system_prompt.contains("lf loop <flow>"));
        assert!(!prepared.config.system_prompt.contains("tmux attach -r"));
    }

    #[test]
    fn prepare_launch_prompt_omits_loopflow_when_disabled() {
        let tmp = create_repo_fixture();
        let config = default_test_config();

        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                surface: Surface::Headless,
                no_loopflow: true,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare prompt");
        assert!(!prepared.prompt.contains("<lf:loopflow>"));
        assert!(!prepared.config.system_prompt.contains("lf commit"));
    }

    #[test]
    fn prepare_launch_prompt_uses_default_agent_when_no_user_config() {
        let tmp = tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join(".lf/skills")).expect("skills dir");
        fs::write(
            tmp.path().join(".lf/skills/test.md"),
            r#"---
default_agent: gemini:2.5-pro
---
Test skill body.
"#,
        )
        .expect("write skill");

        let mut config = default_test_config();
        config.agent = None;
        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                surface: Surface::Headless,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("gemini:2.5-pro"));
    }

    #[test]
    fn prepare_launch_prompt_user_config_overrides_default_agent() {
        let tmp = tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join(".lf/skills")).expect("skills dir");
        fs::write(
            tmp.path().join(".lf/skills/test.md"),
            r#"---
default_agent: gemini:2.5-pro
---
Test skill body.
"#,
        )
        .expect("write skill");

        let mut config = default_test_config();
        config.agent = Some("codex:o3".to_string());
        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                surface: Surface::Headless,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("codex:o3"));
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
    fn prepare_launch_prompt_omits_config_directions_when_disabled() {
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
                include_config_directions: false,
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
        assert_eq!(names, vec!["thorough".to_string()]);
    }

    #[test]
    fn prepare_launch_prompt_uses_config_docs() {
        let tmp = create_repo_fixture();
        fs::create_dir_all(tmp.path().join("docs")).expect("docs dir");
        fs::write(tmp.path().join("docs/README.md"), "docs content").expect("write docs");
        let config = Config {
            docs: vec!["docs".to_string()],
            ..default_test_config()
        };

        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        assert!(prepared
            .components
            .docs
            .iter()
            .any(|doc| doc.path == "docs/README.md" && doc.content == "docs content"));
    }

    #[test]
    fn prepare_launch_prompt_injects_structured_replies_for_ui_context() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
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
            "skill action_style should shape guidance"
        );
    }

    #[test]
    fn prepare_launch_prompt_uses_resolved_skill_skill_without_loading_by_name() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("npx/skill-creator".to_string()),
                resolved_skill: Some(Skill {
                    name: "npx/skill-creator".to_string(),
                    agent: Some("codex:o3".to_string()),
                    default_agent: None,
                    directions: vec!["thorough".to_string()],
                    action_style: Some("procedural".to_string()),
                    interactive: Some(true),
                    feedback: false,
                    content: Some("Skill body".to_string()),
                }),
                surface: Surface::Headless,
                ..LaunchPromptInput::default()
            },
        )
        .expect("prepare launch prompt");

        assert_eq!(
            prepared
                .components
                .skill
                .as_ref()
                .map(|skill| skill.name.as_str()),
            Some("npx/skill-creator")
        );
        assert_eq!(prepared.config.agent.as_deref(), Some("codex:o3"));
        assert!(prepared
            .components
            .directions
            .iter()
            .any(|direction| direction.name == "thorough"));
    }

    #[test]
    fn prepare_launch_prompt_rejects_unsupported_opencode_variants() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let err = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                agent: Some("opencode:anthropic/claude-sonnet-4-5".to_string()),
                surface: Surface::Headless,
                ..LaunchPromptInput::default()
            },
        )
        .expect_err("unsupported OpenCode model should fail");

        assert!(err
            .to_string()
            .contains("unsupported OpenCode model 'anthropic/claude-sonnet-4-5'"));
    }

    #[test]
    fn prepare_launch_prompt_accepts_supported_opencode_variants() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                agent: Some("opencode:moonshotai/kimi-k2".to_string()),
                surface: Surface::Headless,
                ..LaunchPromptInput::default()
            },
        )
        .expect("supported OpenCode model should pass");

        assert_eq!(
            prepared.config.agent.as_deref(),
            Some("opencode:moonshotai/kimi-k2")
        );
    }

    #[test]
    fn prepare_launch_prompt_accepts_bare_opencode_default() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_launch_prompt(
            &config,
            LaunchPromptInput {
                repo_root: tmp.path().to_path_buf(),
                agent: Some("opencode".to_string()),
                surface: Surface::Headless,
                ..LaunchPromptInput::default()
            },
        )
        .expect("bare OpenCode should resolve to the Loopflow-owned default");

        // The bare agent string is preserved; `parse_agent` resolves the
        // `opencode/glm-5.2` default at consumption time so explicit selections
        // still win.
        assert_eq!(prepared.config.agent.as_deref(), Some("opencode"));
    }
}
