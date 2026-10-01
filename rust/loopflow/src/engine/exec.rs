use std::path::PathBuf;

use crate::engine::agent::AgentConfig;
use crate::engine::config::{default_agent, parse_agent, Config};
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

/// Canonical Exec preparation input shared by CLI and Work-runner call sites.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecPromptInput {
    pub repo_root: PathBuf,
    pub skill: Option<String>,
    pub resolved_skill: Option<Skill>,
    pub surface: Surface,
    pub docs: Vec<String>,
    pub wave: Option<String>,
    /// Wave memory already resolved by the Work layer.
    pub wave_memory: Option<String>,
    pub message: Option<String>,
    pub no_loopflow: bool,
    pub agent: Option<String>,
    pub cwd: Option<PathBuf>,
    pub max_turns: Option<u32>,
    pub yolo_mode: bool,
    pub source_overrides: ContextSourceOverrides,
    pub summary: Option<String>,
    pub client_context: ClientContext,
    /// Related repos resolved from the edge graph.
    pub related_repos: Vec<RelatedRepoContext>,
}

/// Canonical Exec preparation output.
#[derive(Debug, Clone)]
pub struct PreparedExecPrompt {
    pub config: AgentConfig,
    pub components: PromptComponents,
    pub deduplicated_docs: Vec<Document>,
    pub prompt: String,
}

/// Build context + Exec config from canonical Exec preparation input.
pub fn prepare_exec_prompt(
    config: &Config,
    input: ExecPromptInput,
) -> Result<PreparedExecPrompt, CoreError> {
    let ExecPromptInput {
        repo_root,
        skill,
        resolved_skill,
        surface,
        docs: requested_docs,
        wave,
        wave_memory,
        message,
        no_loopflow,
        agent,
        cwd,
        max_turns,
        yolo_mode,
        source_overrides,
        summary,
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
        wave_memory,
        include_diff: diff,
        include_diff_files: diff_files,
        include_clipboard: clipboard,
        related_repos,
    };

    let mut components = gather_context(&opts)?;
    if let Some(skill) = resolved_skill {
        components.skill = Some(skill);
    }
    let deduplicated_docs = drop_native_instruction_docs(&mut components, &repo_root);

    if let Some(summary) = summary {
        components.summaries.push(Document {
            path: "wave-summary".to_string(),
            content: summary,
            source: DocumentSource::Summary,
        });
    }

    crate::engine::context_budget::bound_context(&mut components)?;
    let prompt = format_prompt(PromptFormatMode::Full, &components);

    let agent = resolve_agent(agent.as_deref(), components.skill.as_ref(), config);
    validate_agent_policy(&agent)?;

    // Keep only system-safe sections (operate/surface) in
    // the system prompt. Repo content (docs, diffs, wave, clipboard) goes in the
    // task prompt to avoid triggering third-party app classifiers.
    let system_prompt = format_claude_system_prompt(&components);
    let task_prompt = format_claude_task_prompt(&components);
    let action_style = components
        .skill
        .as_ref()
        .and_then(|skill| skill.action_style.as_deref());
    let launch = AgentConfig {
        chrome: false,
        session_driver: None,
        flow_selection: None,
        system_prompt,
        task_prompt,
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
    crate::engine::context_budget::check_input(
        &crate::engine::agent::system_prompt_with_structured_replies(&launch),
        &launch.task_prompt,
    )?;

    Ok(PreparedExecPrompt {
        config: launch,
        components,
        deduplicated_docs,
        prompt,
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

    #[test]
    fn large_task_launch_stays_within_context_budget_and_preserves_sources() {
        use crate::engine::context_budget::{
            GOAL_TOKENS, INPUT_BYTES, INPUT_TOKENS, MEMORY_TOKENS, SCRATCH_TOKENS,
        };
        use crate::engine::prompt::count_tokens;

        let tmp = create_repo_fixture();
        fs::create_dir_all(tmp.path().join("scratch")).unwrap();
        let evidence =
            "Retain the observed failure and verify the configured user path.\n".repeat(2_000);
        for index in 0..14 {
            fs::write(tmp.path().join(format!("scratch/{index:02}.md")), &evidence).unwrap();
        }
        let message = format!(
            "Task definition\n{}\nLatest direction: preserve the public API",
            (0..384)
                .map(|id| format!(
                    "Comment {id}: {}\n",
                    "A recorded implementation result. ".repeat(100)
                ))
                .collect::<String>()
        );
        assert!(message.len() > 1_048_576);
        let prepared = prepare_exec_prompt(
            &default_test_config(),
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".into()),
                wave: Some("infrastructure".into()),
                wave_memory: Some(evidence.clone()),
                message: Some(message.clone()),
                surface: Surface::Headless,
                ..Default::default()
            },
        )
        .unwrap();
        let config = &prepared.config;
        let bytes = config.system_prompt.len() + config.task_prompt.len();
        let tokens = count_tokens(&config.system_prompt) + count_tokens(&config.task_prompt);
        assert!(bytes <= INPUT_BYTES, "{bytes}");
        assert!(tokens <= INPUT_TOKENS, "{tokens}");
        assert!(count_tokens(prepared.components.message.as_ref().unwrap()) <= GOAL_TOKENS);
        assert!(
            count_tokens(&prepared.components.wave_memory.as_ref().unwrap().content)
                <= MEMORY_TOKENS
        );
        assert!(
            prepared
                .components
                .docs
                .iter()
                .filter(|doc| doc.source == DocumentSource::Scratch)
                .map(|doc| count_tokens(&doc.content))
                .sum::<usize>()
                <= SCRATCH_TOKENS
        );
        assert!(config.task_prompt.contains("Task definition"));
        assert!(config
            .task_prompt
            .contains("Latest direction: preserve the public API"));
        assert!(config.task_prompt.contains("scratch/13.md"));
        let sources: Vec<_> = fs::read_dir(tmp.path().join(".lf/tmp/context"))
            .unwrap()
            .collect();
        assert_eq!(sources.len(), 3);
        assert_eq!(prepared.components.budget_decisions.len(), 3);
        let path = sources
            .iter()
            .map(|entry| entry.as_ref().unwrap().path())
            .find(|path| fs::read_to_string(path).unwrap() == message)
            .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), message);
        assert!(config.task_prompt.contains(path.to_str().unwrap()));
        assert_eq!(
            fs::read_to_string(tmp.path().join("scratch/00.md")).unwrap(),
            evidence
        );
        eprintln!("384-comment launch: {tokens} tokens, {bytes} bytes");
    }

    #[test]
    fn oversized_explicit_instructions_report_local_input_budget() {
        let tmp = create_repo_fixture();
        let error = prepare_exec_prompt(
            &default_test_config(),
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                resolved_skill: Some(Skill {
                    content: Some("Follow this instruction. ".repeat(100_000)),
                    ..Skill::named("large")
                }),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("exceeds the input budget"));
    }

    #[test]
    fn structured_reply_guidance_counts_toward_the_launch_budget() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let input = |content: String, has_ui| ExecPromptInput {
            repo_root: tmp.path().to_path_buf(),
            resolved_skill: Some(Skill {
                content: Some(content),
                ..Skill::named("budget")
            }),
            client_context: ClientContext {
                has_ui,
                compact: false,
            },
            ..Default::default()
        };
        let baseline = prepare_exec_prompt(&config, input(String::new(), false)).unwrap();
        let overhead = crate::engine::prompt::count_tokens(&baseline.config.system_prompt)
            + crate::engine::prompt::count_tokens(&baseline.config.task_prompt);
        let content = " x".repeat(crate::engine::context_budget::INPUT_TOKENS - overhead - 32);
        prepare_exec_prompt(&config, input(content.clone(), false)).unwrap();
        let Err(error) = prepare_exec_prompt(&config, input(content, true)) else {
            panic!("structured reply guidance exceeded the launch budget without rejection");
        };
        assert!(error.to_string().contains("exceeds the input budget"));
    }

    #[test]
    fn implement_launch_treats_kickoff_plan_and_intent_as_references() {
        let tmp = create_repo_fixture();
        fs::create_dir_all(tmp.path().join("scratch/nested")).unwrap();
        let plan = "Jack Heart accepted the design on 2026-09-30. Build Unit 1 first.";
        let intent = "> $kickoff\n> ok just run kickoff here then";
        fs::write(tmp.path().join("scratch/plan.md"), plan).unwrap();
        fs::write(tmp.path().join("scratch/nested/intent.md"), intent).unwrap();
        let prepared = prepare_exec_prompt(
            &default_test_config(),
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("implement".into()),
                agent: Some("codex".into()),
                wave_memory: Some("Jack previously invoked $kickoff.".into()),
                message: Some(
                    "Build the accepted plan.\n<lf:steers>\nJack wrote `$kickoff`.\n</lf:steers>"
                        .into(),
                ),
                ..Default::default()
            },
        )
        .unwrap();
        let submitted = &prepared.config.task_prompt;
        assert!(submitted.contains("<lf:skill:implement>"));
        assert!(submitted.contains("Turn the design doc into working code."));
        assert!(submitted.contains(plan));
        assert!(submitted.contains("scratch/nested/intent.md"));
        assert_eq!(submitted.matches("&#36;kickoff").count(), 3);
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
                Surface::Ide,
                Surface::Mac,
                Surface::Iphone,
                Surface::Headless,
            ] {
                let prepared = prepare_exec_prompt(
                    &default_test_config(),
                    ExecPromptInput {
                        repo_root: tmp.path().to_path_buf(),
                        surface,
                        agent: Some(agent.into()),
                        ..Default::default()
                    },
                )
                .unwrap();
                assert_eq!(prepared.components.user_name.as_deref(), Some("Jack"));
                assert!(prepared.prompt.contains("display name is \"Jack\""));
                assert!(prepared
                    .config
                    .task_prompt
                    .contains("display name is \"Jack\""));
                assert!(!prepared.config.system_prompt.contains("<lf:user>"));
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
    fn prepare_exec_prompt_prefers_skill_agent_when_no_override() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect("prepare Exec prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("codex:o3"));
    }

    #[test]
    fn prepare_exec_prompt_prefers_explicit_agent_override() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                agent: Some("claude:sonnet".to_string()),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect("prepare Exec prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("claude:sonnet"));
    }

    #[test]
    fn prepare_exec_prompt_includes_loopflow_by_default() {
        let tmp = create_repo_fixture();
        let config = default_test_config();

        let prepared = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect("prepare prompt");
        assert!(prepared.prompt.contains("<lf:loopflow>"));
        assert!(prepared
            .config
            .system_prompt
            .contains(crate::engine::builtins::LOOPFLOW_DOC.trim()));
        assert!(!prepared.config.system_prompt.contains("tmux attach -r"));
    }

    #[test]
    fn prepare_exec_prompt_omits_loopflow_when_disabled() {
        let tmp = create_repo_fixture();
        let config = default_test_config();

        let prepared = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                surface: Surface::Headless,
                no_loopflow: true,
                ..ExecPromptInput::default()
            },
        )
        .expect("prepare prompt");
        assert!(!prepared.prompt.contains("<lf:loopflow>"));
        assert!(!prepared
            .config
            .system_prompt
            .contains(crate::engine::builtins::LOOPFLOW_DOC.trim()));
    }

    #[test]
    fn prepare_exec_prompt_uses_default_agent_when_no_user_config() {
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
        let prepared = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect("prepare Exec prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("claude:sonnet"));
    }

    #[test]
    fn unmarked_builtin_skill_defaults_to_codex() {
        let tmp = tempdir().expect("tempdir");
        let prepared = prepare_exec_prompt(
            &Config::default(),
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("implement".to_string()),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect("prepare Exec prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("codex"));
    }

    #[test]
    fn marked_builtin_skill_keeps_claude_default() {
        let tmp = tempdir().expect("tempdir");
        let prepared = prepare_exec_prompt(
            &Config::default(),
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("kickoff".to_string()),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect("prepare Exec prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("claude"));
    }

    #[test]
    fn prepare_exec_prompt_user_config_overrides_default_agent() {
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
        let prepared = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect("prepare Exec prompt");

        assert_eq!(prepared.config.agent.as_deref(), Some("codex:o3"));
    }

    #[test]
    fn prepare_exec_prompt_uses_config_docs() {
        let tmp = create_repo_fixture();
        fs::create_dir_all(tmp.path().join("docs")).expect("docs dir");
        fs::write(tmp.path().join("docs/README.md"), "docs content").expect("write docs");
        let config = Config {
            docs: vec!["docs".to_string()],
            ..default_test_config()
        };

        let prepared = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                ..ExecPromptInput::default()
            },
        )
        .expect("prepare Exec prompt");

        assert!(prepared
            .components
            .docs
            .iter()
            .any(|doc| doc.path == "docs/README.md" && doc.content == "docs content"));
    }

    #[test]
    fn prepare_exec_prompt_injects_structured_replies_for_ui_context() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("test".to_string()),
                client_context: ClientContext {
                    has_ui: true,
                    compact: true,
                },
                ..ExecPromptInput::default()
            },
        )
        .expect("prepare Exec prompt");

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
    fn prepare_exec_prompt_uses_resolved_skill_skill_without_loading_by_name() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                skill: Some("npx/skill-creator".to_string()),
                resolved_skill: Some(Skill {
                    name: "npx/skill-creator".to_string(),
                    agent: Some("codex:o3".to_string()),
                    default_agent: None,
                    action_style: Some("procedural".to_string()),
                    content: Some("Skill body".to_string()),
                }),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect("prepare Exec prompt");

        assert_eq!(
            prepared
                .components
                .skill
                .as_ref()
                .map(|skill| skill.name.as_str()),
            Some("npx/skill-creator")
        );
        assert_eq!(prepared.config.agent.as_deref(), Some("codex:o3"));
    }

    #[test]
    fn prepare_exec_prompt_rejects_unsupported_opencode_variants() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let err = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                agent: Some("opencode:anthropic/claude-sonnet-4-5".to_string()),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect_err("unsupported OpenCode model should fail");

        assert!(err
            .to_string()
            .contains("unsupported OpenCode model 'anthropic/claude-sonnet-4-5'"));
    }

    #[test]
    fn prepare_exec_prompt_rejects_unknown_harnesses() {
        let tmp = create_repo_fixture();
        let err = prepare_exec_prompt(
            &default_test_config(),
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                agent: Some("retired-agent".to_string()),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect_err("unknown harness should fail");

        assert!(err
            .to_string()
            .contains("unsupported agent harness 'retired-agent'"));
    }

    #[test]
    fn prepare_exec_prompt_accepts_supported_opencode_variants() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                agent: Some("opencode:moonshotai/kimi-k2".to_string()),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect("supported OpenCode model should pass");

        assert_eq!(
            prepared.config.agent.as_deref(),
            Some("opencode:moonshotai/kimi-k2")
        );
    }

    #[test]
    fn prepare_exec_prompt_accepts_the_users_opencode_default() {
        let tmp = create_repo_fixture();
        let config = default_test_config();
        let prepared = prepare_exec_prompt(
            &config,
            ExecPromptInput {
                repo_root: tmp.path().to_path_buf(),
                agent: Some("opencode".to_string()),
                surface: Surface::Headless,
                ..ExecPromptInput::default()
            },
        )
        .expect("bare OpenCode should defer to the user's default");

        assert_eq!(prepared.config.agent.as_deref(), Some("opencode"));
    }
}
