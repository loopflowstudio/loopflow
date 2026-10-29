//! Provider command syntax. Account selection and process attachment belong to the caller.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use super::AgentConfig;
use crate::id::AgentSessionId;

#[derive(Debug)]
pub(crate) enum ProviderInvocation<'a> {
    Terminal {
        session_id: Option<&'a AgentSessionId>,
    },
    Resume {
        session_id: &'a AgentSessionId,
    },
    ClaudeStream {
        session_id: Option<&'a AgentSessionId>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct SessionCommand {
    pub(crate) launch: Option<AgentConfig>,
    pub(crate) input: Option<String>,
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: PathBuf,
    pub(crate) context: Option<crate::context_block::ContextDelivery>,
    pub(crate) remote: Option<NativeConnection>,
    pub(crate) attachment: Option<(String, crate::process::SessionAttachment)>,
}

/// The local relay is the client transport; only upstream identifies the AgentProcess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeConnection {
    pub(crate) relay: PathBuf,
    pub(crate) upstream: PathBuf,
}

pub(crate) fn native_connection_command(
    cwd: PathBuf,
    session_id: Option<&AgentSessionId>,
    remote: NativeConnection,
    attachment: Option<(String, crate::process::SessionAttachment)>,
) -> SessionCommand {
    let mut args = vec![
        "--remote".into(),
        format!("unix://{}", remote.relay.display()),
    ];
    if let Some(id) = session_id {
        args.extend(["resume".into(), "--".into(), id.to_string()]);
    }
    SessionCommand {
        launch: None,
        input: None,
        program: "codex".into(),
        args,
        cwd,
        context: None,
        remote: Some(remote),
        attachment,
    }
}

pub(crate) fn build_session_command(
    config: &AgentConfig,
    invocation: ProviderInvocation<'_>,
    context_file: Option<&Path>,
) -> Result<SessionCommand> {
    let (harness, model) = crate::config::parse_agent(config.agent());
    let cwd = config
        .cwd
        .clone()
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)?;
    let stream = matches!(invocation, ProviderInvocation::ClaudeStream { .. });
    if !stream && (config.max_turns.is_some() || config.output_schema.is_some()) {
        bail!("turn limits and response schemas require a headless Run (-b)");
    }
    if stream && harness != "claude" {
        bail!("stream-json command syntax belongs to Claude");
    }
    let (mut args, prompt) = match invocation {
        ProviderInvocation::Terminal { .. } => {
            let (flags, prompt) = match &config.skill_invocation {
                Some(skill) => skill.terminal_input(&harness, config)?,
                None => (Vec::new(), config.task_prompt.clone()),
            };
            (flags, Some(prompt))
        }
        ProviderInvocation::Resume { .. } => (
            Vec::new(),
            (!config.task_prompt.is_empty()).then(|| config.task_prompt.clone()),
        ),
        ProviderInvocation::ClaudeStream { .. } => (Vec::new(), None),
    };
    let mut native = None;
    let mut input = None;
    match harness.as_str() {
        "claude" => {
            if stream {
                args.extend([
                    "-p".into(),
                    "--replay-user-messages".into(),
                    "--input-format".into(),
                    "stream-json".into(),
                    "--output-format".into(),
                    "stream-json".into(),
                    "--verbose".into(),
                ]);
            }
            args.extend(super::claude_options(
                config,
                model.as_deref(),
                context_file,
                stream,
            ));
            match invocation {
                ProviderInvocation::Terminal {
                    session_id: Some(id),
                } => args.extend(["--session-id".into(), id.to_string()]),
                ProviderInvocation::Resume { session_id: id }
                | ProviderInvocation::ClaudeStream {
                    session_id: Some(id),
                } => args.extend(["--resume".into(), id.to_string()]),
                _ => {}
            }
            if prompt.is_some() {
                args.push("--".into());
                input = prompt;
            }
        }
        "codex" | "opencode" => {
            let mut launch = config.clone();
            launch.task_prompt = prompt.unwrap_or_default();
            if harness == "opencode" {
                super::validate_terminal_turn(&launch.task_prompt)?;
            } else if matches!(invocation, ProviderInvocation::Terminal { .. }) {
                launch.skill_invocation = None;
            }
            if let ProviderInvocation::Resume { session_id } = invocation {
                launch.resume_token = Some(session_id.clone());
            }
            native = Some(launch);
        }
        "pi" => {
            args.extend(crate::harness::pi::options(config, context_file)?);
            if let ProviderInvocation::Resume { session_id } = invocation {
                crate::harness::pi::validate_resume(session_id)?;
                args.extend(["--session".into(), session_id.to_string()]);
            }
            if let Some(prompt) = prompt.filter(|p| !p.is_empty()) {
                super::validate_terminal_turn(&prompt)?;
                args.extend(["--".into(), prompt]);
            }
        }
        _ => bail!("unsupported agent provider: {harness}"),
    }
    Ok(SessionCommand {
        launch: native,
        input,
        program: harness,
        args,
        cwd,
        context: config.conversation_context.clone(),
        remote: None,
        attachment: config.session_attachment.clone(),
    })
}

/// OpenCode merges permissions and context in one launch-scoped configuration.
pub(crate) fn session_environment(
    config: &AgentConfig,
    context_file: Option<&Path>,
) -> Result<BTreeMap<String, String>> {
    let mut environment = config.env.clone();
    if crate::config::parse_agent(config.agent()).0 == "opencode" {
        let mut settings: serde_json::Value = environment
            .get("OPENCODE_CONFIG_CONTENT")
            .cloned()
            .or_else(|| std::env::var("OPENCODE_CONFIG_CONTENT").ok())
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .unwrap_or_else(|| serde_json::json!({}));
        if !settings.is_object() {
            bail!("OPENCODE_CONFIG_CONTENT must be a JSON object");
        }
        if config.write_scope == super::AgentWriteScope::Worktree {
            settings["permissions"] = serde_json::json!([
                {"action":"*","resource":"*","effect":"allow"},
                {"action":"external_directory","resource":"*","effect":"deny"}
            ]);
        } else if config.skip_permissions {
            settings["permissions"] =
                serde_json::json!([{ "action":"*", "resource":"*", "effect":"allow" }]);
        }
        if config.conversation_context.is_none() {
            let instructions = if let Some(path) = context_file {
                Some(path.to_owned())
            } else {
                super::write_system_prompt_file(config, "opencode")?
            };
            if let Some(path) = instructions {
                settings
                    .as_object_mut()
                    .expect("configuration is an object")
                    .entry("instructions")
                    .or_insert_with(|| serde_json::json!([]))
                    .as_array_mut()
                    .ok_or_else(|| anyhow::anyhow!("OpenCode instructions must be an array"))?
                    .push(serde_json::json!(path));
            }
        }
        if let Some(context) = &config.conversation_context {
            crate::harness::context::add_opencode_context(
                &mut settings,
                context,
                &super::system_prompt_with_structured_replies(config),
            )?;
        }
        environment.insert("OPENCODE_CONFIG_CONTENT".into(), settings.to_string());
    }
    Ok(environment)
}

/// Explicit continuation reloads policy at the saved checkout, retaining native identity.
pub(crate) fn resolve_session_config(
    session: &crate::session::LfSession,
    cli: &crate::lf::Cli,
) -> Result<AgentConfig> {
    let current = crate::config::load_config(Some(&session.cwd))?.unwrap_or_default();
    let provider = session
        .provider
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Session has no provider"))?;
    let selected = cli.agent.clone().or_else(crate::config::agent_override);
    let model = if let Some(agent) = current.agent.as_deref() {
        let (harness, model) = crate::config::parse_agent(agent);
        if harness == provider {
            model
        } else {
            session.model.clone()
        }
    } else {
        session.model.clone()
    };
    let (agent, importing) =
        match crate::config::resume_choice(provider, model.as_deref(), selected.as_deref()) {
            crate::config::Resume::Native(model) => (
                model.map_or_else(
                    || provider.to_string(),
                    |model| format!("{provider}:{model}"),
                ),
                false,
            ),
            crate::config::Resume::Import { agent } => (agent, true),
        };
    let mut config = AgentConfig {
        agent: Some(agent),
        cwd: Some(session.cwd.clone()),
        skip_permissions: cli.yolo || crate::config::yolo_override() || current.yolo,
        chrome: cli.chrome_setting().unwrap_or(current.chrome),
        output_schema: cli.output_schema.clone(),
        ..Default::default()
    };
    let request = if let Some(key) = session.artifact_key.as_deref() {
        match crate::session_record::resolve_manifest(&crate::store::lf_home_dir(), key) {
            Ok((_, saved)) => saved.process,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        }
    } else {
        None
    };
    let mut needs_execution_boundary = false;
    if let Some(request) = request {
        if !importing && provider == "codex" {
            config.task_prompt = request.codex_initial_input();
        }
        if !importing {
            config.provider_account_id = request.account_id;
        }
        config.system_prompt = request.system_prompt;
        config.conversation_context = request.conversation_context;
        config.write_scope = request.write_scope;
        needs_execution_boundary = request.execution_boundary.is_some();
    }
    let store = crate::store::sqlite::SqliteStore::new(&crate::store::database_path_from_env()?)?;
    if cli.batch {
        if let Some(task) = session
            .task_id
            .as_ref()
            .map(|id| store.task_by_issue(id.as_str()))
            .transpose()?
            .flatten()
        {
            if task.worktree()?.canonicalize()? == session.cwd.canonicalize()? {
                config.write_scope = super::AgentWriteScope::Worktree;
                needs_execution_boundary = true;
            }
        }
    }
    if needs_execution_boundary {
        config.execution_boundary = Some(super::checkout_execution_boundary(
            &session.cwd,
            config.agent(),
        )?);
    }
    if !importing {
        if let Some(native) = session
            .artifact_key
            .as_deref()
            .map(|key| store.input_provider_session(key))
            .transpose()?
            .flatten()
        {
            config.resume_token = Some(native.agent_session);
            config.provider_account_id = native.account_id.or(config.provider_account_id);
        }
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::{build_session_command, session_environment, ProviderInvocation};
    use crate::agent::{build_codex_thread_start_params, AgentConfig, AgentWriteScope};

    #[test]
    fn claude_context_uses_the_file_when_present_and_omits_blank_inline_text() {
        let cwd = tempfile::tempdir().unwrap();
        let file = cwd.path().join("context with spaces.md");
        for (instructions, context_file) in [
            ("", None),
            ("  ", None),
            ("inline instructions", None),
            ("inline instructions", Some(file.as_path())),
        ] {
            let config = AgentConfig {
                agent: Some("claude".into()),
                cwd: Some(cwd.path().into()),
                system_prompt: instructions.into(),
                ..Default::default()
            };
            for invocation in [
                ProviderInvocation::Terminal { session_id: None },
                ProviderInvocation::Resume {
                    session_id: &"saved".into(),
                },
                ProviderInvocation::ClaudeStream { session_id: None },
            ] {
                let command = build_session_command(&config, invocation, context_file).unwrap();
                assert_eq!(
                    command.args.contains(&"--append-system-prompt".into()),
                    context_file.is_none() && !instructions.trim().is_empty(),
                );
                assert_eq!(
                    command.args.contains(&"--append-system-prompt-file".into()),
                    context_file.is_some(),
                );
                if context_file.is_some() {
                    assert!(command.args.contains(&file.display().to_string()));
                    assert!(!command.args.contains(&instructions.into()));
                } else if !instructions.trim().is_empty() {
                    assert!(command.args.contains(&instructions.into()));
                }
            }
        }
    }

    #[test]
    fn write_scope_survives_yolo_in_terminal_resume_and_stream() {
        let cwd = tempfile::tempdir().unwrap();
        for provider in ["claude", "codex", "opencode"] {
            let config = AgentConfig {
                agent: Some(provider.into()),
                cwd: Some(cwd.path().to_path_buf()),
                skip_permissions: true,
                write_scope: AgentWriteScope::Worktree,
                ..Default::default()
            };
            for invocation in [
                ProviderInvocation::Terminal { session_id: None },
                ProviderInvocation::Resume {
                    session_id: &"saved".into(),
                },
            ] {
                let command = build_session_command(&config, invocation, None).unwrap();
                assert!(!command
                    .args
                    .contains(&"--dangerously-skip-permissions".into()));
                assert!(!command
                    .args
                    .contains(&"--dangerously-bypass-approvals-and-sandbox".into()));
                assert!(!command.args.contains(&"--add-dir".into()));
                if provider == "claude" {
                    assert!(command
                        .args
                        .iter()
                        .any(|arg| arg.contains("failIfUnavailable")));
                }
                if provider == "codex" {
                    let params = build_codex_thread_start_params(command.launch.as_ref().unwrap());
                    assert_eq!(params["sandbox"], "workspace-write");
                    assert_eq!(params["approvalPolicy"], "never");
                }
            }
            if provider == "codex" {
                let params = build_codex_thread_start_params(&config);
                assert_eq!(params["sandbox"], "workspace-write");
                assert_eq!(params["approvalPolicy"], "never");
            } else if provider == "claude" {
                let command = build_session_command(
                    &config,
                    ProviderInvocation::ClaudeStream { session_id: None },
                    None,
                )
                .unwrap();
                assert!(!command
                    .args
                    .contains(&"--dangerously-skip-permissions".into()));
                assert!(command
                    .args
                    .iter()
                    .any(|arg| arg.contains("failIfUnavailable")));
            } else {
                let environment = session_environment(&config, None).unwrap();
                let settings: serde_json::Value =
                    serde_json::from_str(&environment["OPENCODE_CONFIG_CONTENT"]).unwrap();
                assert_eq!(
                    settings["permissions"][1],
                    serde_json::json!({"action":"external_directory","resource":"*","effect":"deny"})
                );
            }
        }
    }

    #[test]
    fn claude_stream_retains_chrome_turn_limit_schema_and_resume() {
        let cwd = tempfile::tempdir().unwrap();
        let config = AgentConfig {
            agent: Some("claude:sonnet".into()),
            cwd: Some(cwd.path().into()),
            chrome: true,
            max_turns: Some(7),
            output_schema: Some(serde_json::json!({"type":"object"})),
            ..Default::default()
        };
        for token in [None, Some(&"saved".into())] {
            let command = build_session_command(
                &config,
                ProviderInvocation::ClaudeStream { session_id: token },
                None,
            )
            .unwrap();
            assert!(command.args.contains(&"--chrome".into()));
            assert!(command
                .args
                .windows(2)
                .any(|pair| pair == ["--max-turns", "7"]));
            assert!(command
                .args
                .windows(2)
                .any(|pair| pair == ["--json-schema", r#"{"type":"object"}"#]));
            assert_eq!(command.args.contains(&"--resume".into()), token.is_some());
        }
    }

    #[test]
    fn terminal_launch_and_resume_reject_turn_limits_and_schemas_independently() {
        for provider in ["claude", "codex", "opencode"] {
            for (max_turns, output_schema) in [
                (Some(7), None),
                (None, Some(serde_json::json!({"type": "object"}))),
            ] {
                let config = AgentConfig {
                    agent: Some(provider.into()),
                    max_turns,
                    output_schema,
                    ..Default::default()
                };
                for invocation in [
                    ProviderInvocation::Terminal { session_id: None },
                    ProviderInvocation::Resume {
                        session_id: &"saved".into(),
                    },
                ] {
                    let error = build_session_command(&config, invocation, None).unwrap_err();
                    assert!(error.to_string().contains("require a headless Run"));
                }
            }
        }
    }
}
