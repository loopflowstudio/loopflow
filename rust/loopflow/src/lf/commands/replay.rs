//! Replay one self-contained Machine-local capture request through the ordinary harness.

use anyhow::{anyhow, Context, Result};
use std::io::Write;

use crate::engine::{
    check_cli_available, exec_agent, AgentCapabilities, AgentConfig, ProcessConfig, StreamFormat,
};
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
    let mut request = source.exec.ok_or_else(|| {
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
    let (harness, model) = crate::engine::parse_agent(&request.agent);
    if harness != source.harness || model != source.model {
        return Err(anyhow!(
            "capture {} has inconsistent provider identity",
            source.artifact_key
        ));
    }
    if !check_cli_available(&harness) {
        return Err(anyhow!("'{harness}' CLI is unavailable on this Machine"));
    }
    let mut config = AgentConfig {
        system_prompt: request.system_prompt.clone(),
        task_prompt: request.task_prompt.clone(),
        agent: Some(request.agent.clone()),
        provider_account_id: request.account_id.clone(),
        provider_account_authority_home: request.account_id.as_ref().map(|_| home.to_path_buf()),
        max_turns: request.max_turns,
        cwd: Some(source.cwd.clone()),
        write_scope: request.write_scope,
        execution_boundary: request.execution_boundary.clone(),
        skip_permissions: request.skip_permissions,
        ..AgentConfig::default()
    };
    crate::engine::agent::pin_provider_account_id_blocking(&mut config)
        .map_err(anyhow::Error::from)?;
    request.account_id = config.provider_account_id.clone();
    let capabilities = AgentCapabilities {
        chrome: request.chrome,
    };
    let spec = SessionCaptureSpec {
        harness,
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
    let result = exec_agent(&config, &process, &capabilities);
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
    use crate::session_record::{AgentExecRequest, CaptureHandle, SessionCaptureSpec};

    #[test]
    fn replay_uses_recorded_request_without_the_planning_store() {
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let evidence = home.path().join("replay-evidence");
        let provider = bin.join("opencode");
        std::fs::write(
            &provider,
            include_str!("../../../tests/support/opencode_server.py").replace("__WAIT__", "False"),
        )
        .unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();

        let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))
        .unwrap();
        let keys = ["PATH", "LF_BIN", "LF_HOME", "LF_TEST_REPLAY_EVIDENCE"];
        let _environment = crate::test_ambient::EnvGuard::clear(&keys);
        std::env::set_var("PATH", path);
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_TEST_REPLAY_EVIDENCE", &evidence);
        let decoy_home = home.path().join("decoy-home");
        std::fs::create_dir(&decoy_home).unwrap();
        let registry = home.path().join("loopflow.db");

        let request = AgentExecRequest {
            system_prompt: "recorded system".to_string(),
            task_prompt: "recorded task".to_string(),
            agent: "opencode:opencode/glm-5.2".to_string(),
            account_id: None,
            max_turns: Some(3),
            write_scope: crate::engine::AgentWriteScope::Worktree,
            execution_boundary: None,
            skip_permissions: true,
            chrome: false,
        };
        let source = CaptureHandle::begin_with_request(
            SessionCaptureSpec {
                harness: "opencode".to_string(),
                model: Some("opencode/glm-5.2".to_string()),
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

        let session = crate::store::sqlite::SqliteStore::open_execs_read_only(&registry)
            .unwrap()
            .session_for_artifact(&source_id)
            .unwrap()
            .unwrap();
        assert_ne!(session.id, source_id);
        let child_id = replay_at(home.path(), &session.id).unwrap();

        let evidence = std::fs::read_to_string(&evidence).unwrap();
        assert!(evidence.contains(child_id.as_str()));
        assert!(evidence.contains("recorded system"));
        assert!(evidence.contains("recorded task"));
        let (child_dir, child) =
            crate::session_record::resolve_manifest(home.path(), child_id.as_str()).unwrap();
        assert_eq!(child.caller_artifact_key.as_ref(), Some(&source_id));
        assert_eq!(child.exec.as_ref(), Some(&request));
        assert!(child_dir.join("terminal.json").is_file());
        assert!(!child_dir.join("owner.json").exists());
        assert!(!decoy_home.join("runs").exists());
        assert!(registry.is_file());
        let db = rusqlite::Connection::open(&registry).unwrap();
        let tasks: i64 = db
            .query_row("SELECT count(*) FROM tasks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tasks, 0, "replay requires no registered planning Work");
    }
}
