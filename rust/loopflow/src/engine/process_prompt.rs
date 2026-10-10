use std::path::PathBuf;

use crate::engine::agent::AgentConfig;
use crate::engine::config::{default_agent, parse_agent, Config};
use crate::engine::error::CoreError;
use crate::engine::flow::Skill;
use crate::engine::prompt::{
    drop_duplicate_docs, format_first_turn, gather_context, Document, DocumentSource,
    GatherContextOpts, PromptComponents, RelatedRepoContext, Surface,
};
use crate::engine::structured_reply::{structured_replies_for_context, ClientContext};

/// Optional per-source overrides for context gathering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextSourceOverrides {
    pub diff_files: Option<bool>,
    pub diff: Option<bool>,
    pub clipboard: Option<bool>,
}

/// Canonical Process preparation input shared by CLI and Work-runner call sites.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessPromptInput {
    pub repo_root: PathBuf,
    pub skill: Option<String>,
    pub resolved_skill: Option<Skill>,
    pub surface: Surface,
    pub docs: Vec<String>,
    pub wave: Option<String>,
    pub message: Option<String>,
    /// Exact invocation arguments; captured Work direction stays in `references`.
    pub skill_arguments: String,
    pub no_loopflow: bool,
    pub agent: Option<String>,
    pub cwd: Option<PathBuf>,
    pub max_turns: Option<u32>,
    pub yolo_mode: bool,
    pub source_overrides: ContextSourceOverrides,
    pub summary: Option<String>,
    /// Complete captured context, listed separately from the live request.
    pub references: Vec<(String, String)>,
    pub client_context: ClientContext,
    /// Related repos resolved from the edge graph.
    pub related_repos: Vec<RelatedRepoContext>,
}

/// Canonical Process preparation output.
#[derive(Debug, Clone)]
pub struct PreparedProcessPrompt {
    pub config: AgentConfig,
    pub components: PromptComponents,
    pub deduplication_decisions: Vec<crate::trace::ContextDecision>,
}

/// Build context + Process config from canonical Process preparation input.
pub fn prepare_process_prompt(
    config: &Config,
    input: ProcessPromptInput,
) -> Result<PreparedProcessPrompt, CoreError> {
    let ProcessPromptInput {
        repo_root,
        skill,
        resolved_skill,
        surface,
        docs: requested_docs,
        wave,
        message,
        skill_arguments,
        no_loopflow,
        agent,
        cwd,
        max_turns,
        yolo_mode,
        source_overrides,
        summary,
        references,
        client_context,
        related_repos,
    } = input;

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
        docs,
        files: Vec::new(),
        wave,
        include_diff: diff,
        include_diff_files: diff_files,
        include_clipboard: clipboard,
        related_repos,
    };

    let mut components = gather_context(&opts)?;
    if let Some(skill) = resolved_skill {
        components.skill = Some(skill);
    }
    let deduplication_decisions = drop_duplicate_docs(&mut components, &repo_root);

    if let Some(summary) = summary {
        components.summaries.push(Document {
            path: "wave-summary".to_string(),
            content: summary,
            source: DocumentSource::Summary,
        });
    }

    components
        .summaries
        .extend(references.into_iter().map(|(path, content)| Document {
            path,
            content,
            source: DocumentSource::Summary,
        }));

    let agent = resolve_agent(agent.as_deref(), components.skill.as_ref(), config);
    validate_agent_policy(&agent)?;

    let skill_invocation = components
        .skill
        .as_ref()
        .filter(|skill| {
            matches!(parse_agent(&agent).0.as_str(), "claude" | "codex")
                && skill.source.as_ref().is_some_and(|source| {
                    source.dialect != crate::engine::skill_catalog::SkillDialect::Loopflow
                })
        })
        .map(|skill| crate::engine::skill_invocation::SkillInvocation {
            skill: skill.clone(),
            arguments: skill_arguments.clone(),
        });
    let system_prompt = crate::engine::prompt::format_system_sections(&components).join("\n\n");
    let task_prompt = if skill_invocation.is_some() {
        [
            components
                .message
                .as_deref()
                .filter(|message| *message != skill_arguments)
                .map(crate::engine::prompt::render_message),
            components
                .clipboard
                .as_deref()
                .map(crate::engine::prompt::format_clipboard),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n\n")
    } else {
        format_first_turn(&components)
    };
    let conversation_context = Some(crate::engine::context_block::ContextDelivery::prepare(
        &components,
    )?);
    let action_style = components
        .skill
        .as_ref()
        .and_then(|skill| skill.action_style.as_deref());
    let launch = AgentConfig {
        chrome: false,
        session_driver: None,
        system_prompt,
        conversation_context,
        task_prompt,
        skill_invocation,
        agent: Some(agent),
        max_turns,
        resume_token: None,
        provider_account_id: None,
        provider_account_authority_home: None,
        cwd: Some(cwd.unwrap_or(repo_root)),
        write_scope: crate::engine::agent::AgentWriteScope::Configured,
        execution_boundary: None,
        skip_permissions: yolo_mode,
        structured_replies: structured_replies_for_context(&client_context, action_style),
        directive_relay: None,
        env: [(
            crate::engine::config::USER_NAME_ENV.to_string(),
            components.user_name.clone().unwrap_or_default(),
        )]
        .into(),
    };
    Ok(PreparedProcessPrompt {
        config: launch,
        components,
        deduplication_decisions,
    })
}

fn validate_agent_policy(agent: &str) -> Result<(), CoreError> {
    let (harness, variant) = parse_agent(agent);
    match harness.as_str() {
        "claude" | "codex" => return Ok(()),
        "opencode" => {}
        _ => {
            return Err(CoreError::ExecutionFailed(format!(
                "unsupported agent harness '{harness}': use 'claude', 'codex', or 'opencode'"
            )));
        }
    }

    let Some(variant) = variant else {
        return Ok(());
    };
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

/// The same precedence applies to direct commands, captured skills and reviews.
pub(crate) fn resolve_agent(
    override_agent: Option<&str>,
    skill: Option<&Skill>,
    config: &Config,
) -> String {
    override_agent
        .map(str::to_owned)
        .or_else(|| skill.and_then(|skill| skill.agent.clone()))
        .or_else(|| config.agent.clone())
        .or_else(|| skill.and_then(|skill| skill.default_agent.clone()))
        .unwrap_or_else(|| default_agent().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn create_repo_fixture() -> tempfile::TempDir {
        let tmp = tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join(".lf/skills")).expect("skills dir");
        fs::write(
            tmp.path().join(".lf/skills/test.md"),
            r#"---
agent: codex:o3
action_style: procedural
---
Test skill body.
"#,
        )
        .expect("write skill");
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

    fn conversation(prepared: &super::PreparedProcessPrompt) -> String {
        prepared
            .config
            .conversation_context
            .as_ref()
            .unwrap()
            .block(crate::engine::context_block::ContextMoment::Start)
            .unwrap()
            .text
    }

    #[test]
    #[cfg(unix)]
    fn context_delivery_keeps_native_and_symlink_sources_single() {
        let tmp = create_repo_fixture();
        fs::write(tmp.path().join("STYLE.md"), "Provider-owned instructions.").unwrap();
        std::os::unix::fs::symlink("STYLE.md", tmp.path().join("AGENTS.md")).unwrap();
        fs::create_dir(tmp.path().join("scratch")).unwrap();
        fs::write(tmp.path().join("scratch/guide.md"), "A shared document.").unwrap();
        std::os::unix::fs::symlink("scratch/guide.md", tmp.path().join("alias.md")).unwrap();
        let prepared = prepare_process_prompt(
            &default_test_config(),
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                docs: vec!["STYLE.md".into(), "AGENTS.md".into(), "alias.md".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!prepared
            .config
            .system_prompt
            .contains("Provider-owned instructions."));
        assert_eq!(
            conversation(&prepared)
                .matches("A shared document.")
                .count(),
            1
        );
        assert!(prepared.deduplication_decisions.iter().any(|decision| {
            decision.source_path.as_deref() == Some("alias.md")
                && decision.decision == crate::trace::ContextDecisionKind::Deduplicated
        }));
        assert!(prepared
            .deduplication_decisions
            .iter()
            .any(|decision| { decision.kind == crate::trace::ContextAssetKind::RepoInstructions }));
    }

    #[test]
    fn context_delivery_keeps_distinct_scopes_and_one_operating_document() {
        let tmp = create_repo_fixture();
        let operating = crate::engine::builtins::LOOPFLOW_DOC;
        fs::write(tmp.path().join("LOOPFLOW.md"), operating).unwrap();
        fs::create_dir_all(tmp.path().join("custom")).unwrap();
        fs::write(
            tmp.path().join("custom/LOOPFLOW.md"),
            "Local operating guidance.",
        )
        .unwrap();
        for scope in ["infrastructure", "infrastructure/release"] {
            let directory = tmp.path().join("wave").join(scope);
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("MEMORY.md"), "Keep rollback available.").unwrap();
        }
        for no_loopflow in [false, true] {
            let prepared = prepare_process_prompt(
                &default_test_config(),
                ProcessPromptInput {
                    repo_root: tmp.path().to_path_buf(),
                    docs: vec![
                        "LOOPFLOW.md".into(),
                        "custom/LOOPFLOW.md".into(),
                        "wave/infrastructure/MEMORY.md".into(),
                        "wave/infrastructure/release/MEMORY.md".into(),
                    ],
                    no_loopflow,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(
                prepared
                    .config
                    .system_prompt
                    .matches(operating.trim())
                    .count(),
                usize::from(!no_loopflow)
            );
            let context = prepared.config.conversation_context.as_ref().unwrap();
            let references = context
                .references
                .iter()
                .map(|path| fs::read_to_string(path).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                references
                    .iter()
                    .filter(|text| text.as_str() == "Keep rollback available.")
                    .count(),
                2
            );
            assert!(references
                .iter()
                .any(|text| text == "Local operating guidance."));
            assert!(!prepared
                .config
                .system_prompt
                .contains("Local operating guidance."));
            assert_eq!(
                prepared.deduplication_decisions.iter().any(|decision| {
                    decision.kind == crate::trace::ContextAssetKind::OperatingInstructions
                        && decision.decision == crate::trace::ContextDecisionKind::Deduplicated
                }),
                !no_loopflow
            );
        }
    }

    #[test]
    fn additions_are_identical_when_repository_request_and_skill_change() {
        let _home = crate::journal::TestLedgerGuard::new();
        let repo = create_repo_fixture();
        fs::create_dir(repo.path().join("scratch")).unwrap();
        let prepare = |agent: &str, skill: Option<&str>, request: &str| {
            prepare_process_prompt(
                &default_test_config(),
                ProcessPromptInput {
                    repo_root: repo.path().into(),
                    agent: Some(agent.into()),
                    skill: skill.map(str::to_owned),
                    message: Some(request.into()),
                    references: vec![("Task brief".into(), "Jack requested a stable API".into())],
                    ..Default::default()
                },
            )
            .unwrap()
        };
        let first = prepare("claude", None, "First request");
        fs::write(repo.path().join("scratch/huge.md"), "BIG".repeat(70_000)).unwrap();
        let second = prepare("codex", Some("test"), "Second request");
        assert_eq!(first.config.system_prompt, second.config.system_prompt);
        assert!(!second.config.system_prompt.contains("BIG"));
        assert!(!second.config.task_prompt.contains("Jack requested"));
        let delivery = second.config.conversation_context.as_ref().unwrap();
        assert_eq!(
            fs::read_to_string(&delivery.references[0]).unwrap(),
            "Jack requested a stable API"
        );
        assert!(
            delivery
                .block(crate::engine::context_block::ContextMoment::Start)
                .unwrap()
                .text
                .len()
                <= 10_000
        );
    }

    #[test]
    fn first_turn_contains_skill_then_exact_request_without_size_reduction() {
        let tmp = create_repo_fixture();
        let message = "😀 request\r\n".repeat(20_000);
        let prepared = prepare_process_prompt(
            &default_test_config(),
            ProcessPromptInput {
                repo_root: tmp.path().to_owned(),
                skill: Some("test".into()),
                message: Some(message.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(prepared.config.task_prompt, format!("<lf:skill:test>\nTest skill body.\n\n</lf:skill:test>\n\n<lf:message>\n{message}\n</lf:message>"));
        assert!(!prepared.config.system_prompt.contains("Test skill body."));
        assert!(!prepared.config.system_prompt.contains(&message));
        assert_eq!(
            prepared.components.message.as_deref(),
            Some(message.as_str())
        );
    }

    #[test]
    fn implement_launch_treats_kickoff_plan_and_intent_as_references() {
        let _home = crate::journal::TestLedgerGuard::new();
        let tmp = create_repo_fixture();
        fs::create_dir_all(tmp.path().join("scratch/nested")).unwrap();
        let plan = "Jack Heart accepted the design on 2026-09-30. Build Unit 1 first.";
        let intent = "> $kickoff\n> ok just run kickoff here then";
        fs::write(tmp.path().join("scratch/plan.md"), plan).unwrap();
        fs::write(tmp.path().join("scratch/nested/intent.md"), intent).unwrap();
        fs::create_dir_all(tmp.path().join("wave/product")).unwrap();
        fs::write(
            tmp.path().join("wave/product/MEMORY.md"),
            "Jack previously invoked $kickoff.",
        )
        .unwrap();
        let prepared = prepare_process_prompt(
            &default_test_config(),
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("implement".into()),
                agent: Some("codex".into()),
                wave: Some("product".into()),
                message: Some(
                    "Build the accepted plan.\n<lf:steers>\nJack wrote `$kickoff`.\n</lf:steers>"
                        .into(),
                ),
                ..Default::default()
            },
        )
        .unwrap();
        let submitted = conversation(&prepared);
        assert!(prepared.config.task_prompt.contains("<lf:skill:implement>"));
        assert!(prepared
            .config
            .task_prompt
            .contains("Turn the design doc into working code."));
        assert!(!prepared
            .config
            .system_prompt
            .contains("<lf:skill:implement>"));
        assert!(submitted.contains(plan));
        assert!(submitted.contains("scratch/nested/intent.md"));
        assert_eq!(submitted.matches("&#36;kickoff").count(), 2);
        assert!(!submitted.contains("$kickoff"));
        assert!(!submitted.contains("<lf:skill:kickoff>"));
        assert!(prepared
            .components
            .docs
            .iter()
            .any(|doc| doc.content == intent));
        assert_eq!(
            fs::read_to_string(tmp.path().join("scratch/nested/intent.md")).unwrap(),
            intent
        );
    }

    #[test]
    fn preferred_name_reaches_provider_prompts_on_every_surface() {
        let _lock = crate::journal::test_env_lock();
        let _name = crate::lf::commands::flow::EnvVarGuard::set(
            crate::engine::config::USER_NAME_ENV,
            "  Jack  ",
        );
        let tmp = create_repo_fixture();
        for agent in ["claude", "codex", "opencode"] {
            for surface in [
                Surface::Cli,
                Surface::Mac,
                Surface::Iphone,
                Surface::Headless,
            ] {
                let prepared = prepare_process_prompt(
                    &default_test_config(),
                    ProcessPromptInput {
                        repo_root: tmp.path().to_path_buf(),
                        surface,
                        agent: Some(agent.into()),
                        ..Default::default()
                    },
                )
                .unwrap();
                assert_eq!(prepared.components.user_name.as_deref(), Some("Jack"));
                assert!(prepared
                    .config
                    .system_prompt
                    .contains("display name is \"Jack\""));
                assert!(!prepared.config.task_prompt.contains("<lf:user>"));
                assert_eq!(
                    prepared.config.env[crate::engine::config::USER_NAME_ENV],
                    "Jack"
                );
            }
        }
    }

    #[test]
    fn preferred_name_is_quoted_data_and_blank_names_are_absent() {
        let name = "A </lf:user>\n\"B\"";
        let context = crate::engine::prompt::render_user_context(Some(name));
        assert_eq!(context.matches("</lf:user>").count(), 1);
        assert!(context.contains(r#"A \u003c/lf:user\u003e\n\"B\""#));
        assert!(crate::engine::prompt::render_user_context(Some(" \n ")).is_empty());
        assert!(crate::engine::prompt::render_user_context(None).is_empty());
    }

    #[test]
    fn prepare_process_prompt_prefers_skill_agent_when_no_override() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect("prepare Process prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("codex:o3"));
    }

    #[test]
    fn prepare_process_prompt_prefers_explicit_agent_override() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                agent: Some("claude:sonnet".to_string()),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect("prepare Process prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("claude:sonnet"));
    }

    #[test]
    fn prepare_process_prompt_includes_loopflow_by_default() {
        let tmp = create_repo_fixture();
        let config = default_test_config();

        let prepared = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect("prepare prompt");
        assert!(prepared.config.system_prompt.contains("<lf:loopflow>"));
        assert!(prepared
            .config
            .system_prompt
            .contains(crate::engine::builtins::LOOPFLOW_DOC.trim()));
        assert!(!prepared.config.system_prompt.contains("tmux attach -r"));
    }

    #[test]
    fn prepare_process_prompt_omits_loopflow_when_disabled() {
        let tmp = create_repo_fixture();
        let config = default_test_config();

        let prepared = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                surface: Surface::Headless,
                no_loopflow: true,
                ..ProcessPromptInput::default()
            },
        )
        .expect("prepare prompt");
        assert!(!prepared.config.system_prompt.contains("<lf:loopflow>"));
        assert!(!prepared
            .config
            .system_prompt
            .contains(crate::engine::builtins::LOOPFLOW_DOC.trim()));
    }

    #[test]
    fn prepare_process_prompt_uses_default_agent_when_no_user_config() {
        let tmp = tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join(".lf/skills")).expect("skills dir");
        fs::write(
            tmp.path().join(".lf/skills/test.md"),
            r#"---
default_agent: claude:sonnet
---
Test skill body.
"#,
        )
        .expect("write skill");

        let mut config = default_test_config();
        config.agent = None;
        let prepared = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect("prepare Process prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("claude:sonnet"));
    }

    #[test]
    fn unmarked_builtin_skill_defaults_to_codex() {
        let tmp = tempdir().expect("tempdir");
        let prepared = prepare_process_prompt(
            &Config::default(),
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("implement".to_string()),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect("prepare Process prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("codex"));
    }

    #[test]
    fn marked_builtin_skill_keeps_claude_default() {
        let tmp = tempdir().expect("tempdir");
        let prepared = prepare_process_prompt(
            &Config::default(),
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("kickoff".to_string()),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect("prepare Process prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("claude"));
    }

    #[test]
    fn prepare_process_prompt_user_config_overrides_default_agent() {
        let tmp = tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join(".lf/skills")).expect("skills dir");
        fs::write(
            tmp.path().join(".lf/skills/test.md"),
            r#"---
default_agent: claude:sonnet
---
Test skill body.
"#,
        )
        .expect("write skill");

        let mut config = default_test_config();
        config.agent = Some("codex:o3".to_string());
        let prepared = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect("prepare Process prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("codex:o3"));
    }

    #[test]
    fn prepare_process_prompt_uses_config_docs() {
        let tmp = create_repo_fixture();
        fs::create_dir_all(tmp.path().join("docs")).expect("docs dir");
        fs::write(tmp.path().join("docs/README.md"), "docs content").expect("write docs");
        let config = Config {
            docs: vec!["docs".to_string()],
            ..default_test_config()
        };

        let prepared = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                ..ProcessPromptInput::default()
            },
        )
        .expect("prepare Process prompt");

        assert!(prepared
            .components
            .docs
            .iter()
            .any(|doc| doc.path == "docs/README.md" && doc.content == "docs content"));
    }

    #[test]
    fn prepare_process_prompt_injects_structured_replies_for_ui_context() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                client_context: ClientContext {
                    has_ui: true,
                    compact: true,
                },
                ..ProcessPromptInput::default()
            },
        )
        .expect("prepare Process prompt");

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
    fn prepare_process_prompt_uses_resolved_skill_skill_without_loading_by_name() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("team/skill-creator".to_string()),
                resolved_skill: Some(Skill {
                    source: None,
                    name: "team/skill-creator".to_string(),
                    agent: Some("codex:o3".to_string()),
                    default_agent: None,
                    action_style: Some("procedural".to_string()),
                    content: Some("Skill body".to_string()),
                }),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect("prepare Process prompt");

        assert_eq!(
            prepared
                .components
                .skill
                .as_ref()
                .map(|skill| skill.name.as_str()),
            Some("team/skill-creator")
        );
        assert_eq!(prepared.config.agent.as_deref(), Some("codex:o3"));
    }

    #[test]
    fn native_skill_uses_the_same_context_channel() {
        let tmp = create_repo_fixture();
        fs::write(tmp.path().join("context.md"), "docs content").unwrap();
        let source = Skill {
            content: Some("Audit $ARGUMENTS".into()),
            source: Some(crate::engine::skill_catalog::SkillOrigin {
                path: tmp.path().join(".claude/skills/audit/SKILL.md"),
                dialect: crate::engine::skill_catalog::SkillDialect::Claude,
                frontmatter: None,
            }),
            ..Skill::named("audit")
        };
        let input = ProcessPromptInput {
            repo_root: tmp.path().into(),
            resolved_skill: Some(source),
            docs: vec!["context.md".into()],
            no_loopflow: true,
            agent: Some("claude".into()),
            ..Default::default()
        };
        let prepared = prepare_process_prompt(&default_test_config(), input.clone()).unwrap();
        assert!(!prepared.config.system_prompt.contains("<lf:loopflow>"));
        assert!(!prepared.config.task_prompt.contains("docs content"));
        assert!(conversation(&prepared).contains("context.md"));
        assert!(!prepared.config.task_prompt.contains("<lf:context-budget>"));

        std::fs::create_dir_all(tmp.path().join("scratch")).unwrap();
        std::fs::write(tmp.path().join("scratch/plan.md"), "Preserve this decision").unwrap();
        let prepared = prepare_process_prompt(&default_test_config(), input).unwrap();
        assert!(!prepared.config.task_prompt.contains("<lf:context-budget>"));
        assert!(conversation(&prepared).contains("Preserve this decision"));
        assert!(!prepared
            .config
            .system_prompt
            .contains("Preserve this decision"));
    }

    #[test]
    fn native_skill_arguments_stay_separate_from_work_direction() {
        let tmp = create_repo_fixture();
        let source = Skill {
            content: Some("Audit $ARGUMENTS using reference.md".into()),
            source: Some(crate::engine::skill_catalog::SkillOrigin {
                path: tmp.path().join(".claude/skills/audit/SKILL.md"),
                dialect: crate::engine::skill_catalog::SkillDialect::Claude,
                frontmatter: Some("\nallowed-tools: Read\n".into()),
            }),
            ..Skill::named("audit")
        };
        for (agent, surface) in [
            ("claude:sonnet", Surface::Headless),
            ("codex", Surface::Headless),
            ("claude", Surface::Cli),
            ("codex", Surface::Cli),
        ] {
            let prepared = prepare_process_prompt(
                &default_test_config(),
                ProcessPromptInput {
                    repo_root: tmp.path().into(),
                    resolved_skill: Some(source.clone()),
                    message: Some("Preserve the Task checkout and return the decision".into()),
                    skill_arguments: "  \"exact arguments\"  ".into(),
                    agent: Some(agent.into()),
                    surface,
                    ..ProcessPromptInput::default()
                },
            )
            .unwrap();
            let invocation = prepared.config.skill_invocation.as_ref().unwrap();
            assert_eq!(invocation.skill, source);
            assert_eq!(invocation.arguments, "  \"exact arguments\"  ");
            assert!(prepared
                .config
                .task_prompt
                .contains("Preserve the Task checkout"));
            assert!(!prepared.config.task_prompt.contains("Audit $ARGUMENTS"));
            assert!(!prepared
                .config
                .system_prompt
                .contains("Preserve the Task checkout"));
            assert!(prepared
                .config
                .skill_invocation
                .as_ref()
                .unwrap()
                .skill
                .content
                .as_deref()
                .unwrap()
                .contains("Audit"));
        }
    }

    #[test]
    fn prepare_process_prompt_rejects_unsupported_opencode_variants() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let err = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                agent: Some("opencode:anthropic/claude-sonnet-4-5".to_string()),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect_err("unsupported OpenCode model should fail");

        assert!(err
            .to_string()
            .contains("unsupported OpenCode model 'anthropic/claude-sonnet-4-5'"));
    }

    #[test]
    fn prepare_process_prompt_rejects_unknown_harnesses() {
        let tmp = create_repo_fixture();
        let err = prepare_process_prompt(
            &default_test_config(),
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                agent: Some("retired-agent".to_string()),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect_err("unknown harness should fail");

        assert!(err
            .to_string()
            .contains("unsupported agent harness 'retired-agent'"));
    }

    #[test]
    fn prepare_process_prompt_accepts_supported_opencode_variants() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                agent: Some("opencode:moonshotai/kimi-k2".to_string()),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect("supported OpenCode model should pass");

        assert_eq!(
            prepared.config.agent.as_deref(),
            Some("opencode:moonshotai/kimi-k2")
        );
    }

    #[test]
    fn prepare_process_prompt_accepts_the_users_opencode_default() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_process_prompt(
            &config,
            ProcessPromptInput {
                repo_root: tmp.path().to_path_buf(),
                agent: Some("opencode".to_string()),
                surface: Surface::Headless,
                ..ProcessPromptInput::default()
            },
        )
        .expect("bare OpenCode should defer to the user's default");

        assert_eq!(prepared.config.agent.as_deref(), Some("opencode"));
    }
}
