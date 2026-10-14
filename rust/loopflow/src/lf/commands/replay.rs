//! Replay one self-contained Machine-local capture request through the ordinary harness.

use anyhow::{anyhow, Context, Result};
use std::io::Write;

use crate::agent::check_cli_available;
use crate::agent::run_agent;
use crate::agent::stream::StreamFormat;
use crate::agent::AgentConfig;
use crate::agent::ProcessConfig;
use crate::session_record::{AttributionSource, CaptureHandle, SessionCaptureSpec};

pub fn run(selector: &str) -> Result<()> {
    let home = crate::store::lf_home_dir();
    let artifact_key = replay_at(&home, selector)?;
    println!("replayed {selector} as {artifact_key}");
    Ok(())
}

fn replay_at(home: &std::path::Path, selector: &str) -> Result<String> {
    let (_, source) = crate::session_record::resolve_manifest(home, selector)
        .with_context(|| format!("cannot read capture {selector}"))?;
    anyhow::ensure!(
        source.surface == "headless",
        "capture {} did not record a replayable headless request",
        source.artifact_key
    );
    let mut request = source.process.ok_or_else(|| {
        anyhow!(
            "capture {} did not record a replayable headless request",
            source.artifact_key
        )
    })?;
    if let Some(reason) = request.replay_unavailable_reason() {
        return Err(anyhow!(
            "capture {} cannot be replayed: {reason}",
            source.artifact_key
        ));
    }
    let (harness, model) = crate::config::parse_agent(&request.agent);
    if harness != source.harness || model != source.model {
        return Err(anyhow!(
            "capture {} has inconsistent provider identity",
            source.artifact_key
        ));
    }
    let mut config = AgentConfig {
        system_prompt: request.system_prompt.clone(),
        conversation_context: request.conversation_context.clone(),
        task_prompt: request.task_prompt.clone(),
        skill_invocation: request.skill_invocation.clone(),
        output_schema: request.output_schema.clone(),
        agent: Some(request.agent.clone()),
        provider_account_id: request.account_id.clone(),
        provider_account_authority_home: request.account_id.as_ref().map(|_| home.to_path_buf()),
        max_turns: request.max_turns,
        cwd: Some(source.cwd.clone()),
        write_scope: request.write_scope,
        execution_boundary: request.execution_boundary.clone(),
        skip_permissions: request.skip_permissions,
        chrome: request.chrome,
        ..AgentConfig::default()
    };
    config.apply_agent_override();
    let (selected_harness, model) = crate::config::parse_agent(config.agent());
    if selected_harness != harness {
        config.provider_account_id = None;
        config.provider_account_authority_home = None;
    }
    if !check_cli_available(&selected_harness) {
        return Err(anyhow!(
            "'{selected_harness}' CLI is unavailable on this Machine"
        ));
    }
    crate::agent::pin_provider_account_id_blocking(&mut config).map_err(anyhow::Error::from)?;
    request.agent = config.agent().to_owned();
    request.account_id = config.provider_account_id.clone();
    let spec = SessionCaptureSpec {
        harness: selected_harness,
        model,
        surface: "headless".to_string(),
        cwd: source.cwd,
        repo: source.repo,
        worktree: source.worktree,
        skill: source.skill,
        subjects: source
            .subjects
            .into_iter()
            .map(|mut subject| {
                subject.source = AttributionSource::Inherited;
                subject
            })
            .collect(),
        // A replay re-executes recorded inputs; it is not the Flow occurrence.
        flow: crate::session_record::SessionFlowMembership::Independent,
        work: None,
    };
    let capture = CaptureHandle::begin_replay_at(home, spec, request, source.artifact_key)
        .map_err(|error| anyhow!("failed to publish replay capture before execution: {error}"))?;
    capture.record_input("replay", &config.task_prompt);
    let artifact_key = capture.artifact_key();
    let context_file = if config.system_prompt.trim().is_empty() {
        None
    } else {
        let mut file =
            tempfile::NamedTempFile::new().context("create private replay system-prompt file")?;
        file.write_all(config.system_prompt.as_bytes())
            .context("write replay system-prompt file")?;
        Some(file)
    };
    let process = ProcessConfig {
        auto: true,
        stream: true,
        stream_format: StreamFormat::Human(false),
        capture: Some(capture.clone().into()),
        context_file: context_file.as_ref().map(|file| file.path().to_path_buf()),
        ..ProcessConfig::default()
    };
    let result = run_agent(&config, &process);
    let outcome = match &result {
        Ok(result) if result.exit_code == 0 => "completed",
        Ok(_) | Err(_) => "failed",
    };
    capture
        .finish(outcome)
        .map_err(|error| anyhow!("replay capture did not settle: {error}"))?;
    let result = result.map_err(anyhow::Error::from)?;
    if result.exit_code != 0 {
        return Err(anyhow!(
            "replay provider exited with code {}",
            result.exit_code
        ));
    }
    Ok(artifact_key)
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::replay_at;
    use crate::session_record::{AgentProcessRequest, CaptureHandle, SessionCaptureSpec};

    #[test]
    fn replay_uses_recorded_request_without_the_planning_store() {
        replay_recorded_request("opencode");
    }

    #[test]
    fn claude_replay_preserves_native_output_schema() {
        replay_recorded_request("claude");
    }

    fn replay_recorded_request(harness: &str) {
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let evidence = home.path().join("replay-evidence");
        let claude_source = r#"#!/usr/bin/python3
import json, os, sys
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
schema = json.loads(sys.argv[sys.argv.index('--json-schema') + 1])
assert schema == {'type': 'object', 'properties': {'decision': {'const': 'advance'}}, 'required': ['decision']}
context = open(sys.argv[sys.argv.index('--append-system-prompt-file') + 1]).read()
for line in sys.stdin:
    message = json.loads(line)
    with open(os.environ['LF_TEST_REPLAY_EVIDENCE'], 'w') as evidence:
        json.dump({'capture': os.environ['LF_CAPTURE_KEY'], 'context': context, 'message': message, 'schema': schema}, evidence)
    message['session_id'] = 'replay-thread'
    print(json.dumps(message), flush=True)
    print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'replay-thread'}), flush=True)
    print(json.dumps({'type': 'result', 'uuid': 'replay-result', 'session_id': 'replay-thread', 'subtype': 'success', 'is_error': False, 'result': 'not the answer', 'structured_output': {'decision': 'advance'}}), flush=True)
"#;
        // Replay can switch providers; both the source and override must use
        // stand-ins, never a provider installed on the test host.
        for (name, source) in [
            ("claude", claude_source.to_string()),
            (
                "opencode",
                include_str!("../../../tests/support/opencode_server.py")
                    .replace("__WAIT__", "False"),
            ),
        ] {
            let provider = bin.join(name);
            std::fs::write(&provider, source).unwrap();
            std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let path = std::env::join_paths([bin, "/usr/bin".into(), "/bin".into()]).unwrap();
        let keys = [
            "PATH",
            "LF_BIN",
            "LF_HOME",
            "LF_TEST_REPLAY_EVIDENCE",
            crate::config::AGENT_OVERRIDE_ENV,
        ];
        let _environment = crate::test_ambient::EnvGuard::clear(&keys);
        std::env::set_var("PATH", path);
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_TEST_REPLAY_EVIDENCE", &evidence);
        let decoy_home = home.path().join("decoy-home");
        std::fs::create_dir(&decoy_home).unwrap();
        let registry = home.path().join("loopflow.db");

        let account_id = if harness == "claude" {
            let mut account = crate::provider_account::new_account(
                crate::provider_auth::Provider::Claude,
                crate::store::ProviderAccountId::parse("replay-fixture").unwrap(),
                home.path().join("accounts/claude/replay-fixture"),
                Some(crate::profile::EmailAddress::parse("replay@example.com").unwrap()),
            );
            crate::provider_account::identity::tests::write_claude_identity(&mut account);
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let store = crate::store::open_ephemeral_store(
                    &crate::store::StorageConfig::sqlite(registry.clone()),
                )
                .await
                .unwrap();
                store.upsert_provider_account(&account).await.unwrap();
            });
            Some(account.account_id)
        } else {
            None
        };

        let request = AgentProcessRequest {
            conversation_context: None,
            system_prompt: "recorded system".to_string(),
            task_prompt: "recorded task".to_string(),
            skill_invocation: None,
            output_schema: (harness == "claude").then(|| {
                serde_json::json!({
                    "type": "object",
                    "properties": {"decision": {"const": "advance"}},
                    "required": ["decision"]
                })
            }),
            agent: if harness == "claude" {
                "claude"
            } else {
                "opencode:opencode/glm-5.2"
            }
            .to_string(),
            account_id,
            max_turns: Some(3),
            write_scope: crate::agent::AgentWriteScope::Worktree,
            execution_boundary: None,
            skip_permissions: true,
            chrome: false,
        };
        let source = CaptureHandle::begin_with_request(
            SessionCaptureSpec {
                harness: harness.to_string(),
                model: (harness == "opencode").then(|| "opencode/glm-5.2".to_string()),
                surface: "headless".to_string(),
                cwd: home.path().to_path_buf(),
                repo: Some(home.path().to_path_buf()),
                worktree: Some(home.path().to_path_buf()),
                skill: Some("implement".to_string()),
                subjects: Vec::new(),
                flow: crate::session_record::SessionFlowMembership::Independent,
                work: None,
            },
            request.clone(),
        )
        .unwrap();
        let source_id = source.artifact_key();
        source.finish("completed").unwrap();

        let session = crate::store::sqlite::SqliteStore::open_processes_read_only(&registry)
            .unwrap()
            .session_for_artifact(&source_id)
            .unwrap()
            .unwrap();
        assert_ne!(session.id, source_id);
        // The replaying invocation and its capture share this fixture's store.
        let child_id = crate::journal::with_test_ledger(registry.clone(), || {
            replay_at(home.path(), &session.id)
        })
        .unwrap();

        let evidence = std::fs::read_to_string(&evidence).unwrap();
        assert!(evidence.contains(child_id.as_str()));
        assert!(evidence.contains("recorded system"));
        assert!(evidence.contains("recorded task"));
        let (child_dir, child) =
            crate::session_record::resolve_manifest(home.path(), child_id.as_str()).unwrap();
        assert_eq!(child.caller_artifact_key.as_ref(), Some(&source_id));
        assert_eq!(child.process.as_ref(), Some(&request));
        assert!(child_dir.join("terminal.json").is_file());
        assert!(!child_dir.join("owner.json").exists());
        assert!(!decoy_home.join("runs").exists());
        assert!(registry.is_file());
        let db = rusqlite::Connection::open(&registry).unwrap();
        let tasks: i64 = db
            .query_row("SELECT count(*) FROM tasks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tasks, 0, "replay requires no registered planning Work");

        std::env::set_var(crate::config::AGENT_OVERRIDE_ENV, "opencode:opencode/other");
        let overridden_id = crate::journal::with_test_ledger(registry.clone(), || {
            replay_at(home.path(), &session.id)
        })
        .unwrap();
        let (_, overridden) =
            crate::session_record::resolve_manifest(home.path(), &overridden_id).unwrap();
        assert_eq!(overridden.model.as_deref(), Some("opencode/other"));
        let mut expected = request.clone();
        expected.agent = "opencode:opencode/other".into();
        // Claude's selected account cannot transfer to an OpenCode replay.
        expected.account_id = None;
        assert_eq!(overridden.process, Some(expected));
        let (_, retained) =
            crate::session_record::resolve_manifest(home.path(), &source_id).unwrap();
        assert_eq!(retained.process, Some(request));
    }
}
