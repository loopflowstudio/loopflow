//! Replay one self-contained Home-local Run request through the ordinary harness.

use anyhow::{anyhow, Context, Result};
use std::io::Write;

use crate::engine::{
    check_cli_available, launch_agent, AgentCapabilities, AgentConfig, ProcessConfig, StreamFormat,
};
use crate::run_record::{AttributionSource, CaptureHandle, RunSpec};

pub fn run(selector: &str) -> Result<()> {
    let home = crate::store::observability_home_dir();
    let run_id = replay_at(&home, selector)?;
    println!("replayed {selector} as {run_id}");
    Ok(())
}

fn replay_at(home: &std::path::Path, selector: &str) -> Result<crate::durable::RunId> {
    let (_, source) = crate::run_record::resolve_manifest(home, selector)
        .with_context(|| format!("cannot read Run {selector}"))?;
    let launch = source.launch.clone().ok_or_else(|| {
        anyhow!(
            "Run {} did not record a replayable headless request",
            source.run_id
        )
    })?;
    if let Some(reason) = launch.replay_unavailable_reason() {
        return Err(anyhow!(
            "Run {} cannot be replayed: {reason}",
            source.run_id
        ));
    }
    let (harness, model) = crate::engine::parse_agent(&launch.agent);
    if harness != source.harness || model != source.model {
        return Err(anyhow!(
            "Run {} has inconsistent launch identity",
            source.run_id
        ));
    }
    if !check_cli_available(&harness) {
        return Err(anyhow!("'{harness}' CLI is unavailable on this Home"));
    }
    let mut config = AgentConfig {
        system_prompt: launch.system_prompt.clone(),
        task_prompt: launch.task_prompt.clone(),
        agent: Some(launch.agent.clone()),
        provider_account_id: launch.account_id.clone(),
        provider_account_authority_home: launch.account_id.as_ref().map(|_| home.to_path_buf()),
        max_turns: launch.max_turns,
        cwd: Some(source.cwd.clone()),
        write_scope: launch.write_scope,
        execution_boundary: launch.execution_boundary.clone(),
        skip_permissions: launch.skip_permissions,
        ..AgentConfig::default()
    };
    crate::engine::agent::pin_provider_account_id_blocking(&mut config)
        .map_err(anyhow::Error::from)?;
    let mut replay_launch = launch;
    replay_launch.account_id = config.provider_account_id.clone();
    let spec = RunSpec {
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
    };
    let capture = CaptureHandle::begin_replay_at(home, spec, replay_launch.clone(), source.run_id)
        .map_err(|error| anyhow!("failed to publish replay Run before launch: {error}"))?;
    capture.record_input("replay", &replay_launch.task_prompt);
    let run_id = capture.run_id();
    let mut context_file = if replay_launch.system_prompt.trim().is_empty() {
        None
    } else {
        let mut file =
            tempfile::NamedTempFile::new().context("create private replay system-prompt file")?;
        file.write_all(replay_launch.system_prompt.as_bytes())
            .context("write replay system-prompt file")?;
        Some(file)
    };
    let process = ProcessConfig {
        auto: true,
        stream: true,
        stream_format: StreamFormat::Human(false),
        capture: Some(capture.clone().into()),
        context_file: context_file.as_mut().map(|file| file.path().to_path_buf()),
        ..ProcessConfig::default()
    };
    let result = launch_agent(
        &config,
        &process,
        &AgentCapabilities {
            chrome: replay_launch.chrome,
        },
    );
    let outcome = match &result {
        Ok(result) if result.exit_code == 0 => "completed",
        Ok(_) | Err(_) => "failed",
    };
    capture
        .finish(outcome)
        .map_err(|error| anyhow!("replay Run did not settle: {error}"))?;
    let result = result.map_err(anyhow::Error::from)?;
    if result.exit_code != 0 {
        return Err(anyhow!(
            "replay provider exited with code {}",
            result.exit_code
        ));
    }
    Ok(run_id)
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::replay_at;
    use crate::run_record::{CaptureHandle, RunLaunchRequest, RunSpec};

    struct EnvironmentRestore(Vec<(&'static str, Option<std::ffi::OsString>)>);

    impl EnvironmentRestore {
        fn capture(keys: &[&'static str]) -> Self {
            Self(
                keys.iter()
                    .map(|key| (*key, std::env::var_os(key)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvironmentRestore {
        fn drop(&mut self) {
            for (key, value) in self.0.drain(..).rev() {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

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
r#"#!/bin/sh
context_file=$(printf '%s' "$OPENCODE_CONFIG_CONTENT" | sed -n 's/.*"instructions":\["\([^"]*\)"\].*/\1/p')
printf '%s\n' "$LF_RUN_ID|$LF_PARENT_RUN_ID|$(cat "$context_file")|$*" > "$LF_TEST_REPLAY_EVIDENCE"
printf '%s\n' '{"type":"result","subtype":"success","usage":{"input_tokens":5,"output_tokens":2}}'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();

        let keys = [
            "PATH",
            "LF_BIN",
            "LF_HOME",
            "LF_DB_PATH",
            "LF_TEST_REPLAY_EVIDENCE",
            crate::store::CONTROL_HOME_ENV,
            crate::store::CONTROL_DB_PATH_ENV,
        ];
        let _environment = EnvironmentRestore::capture(&keys);
        std::env::set_var(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_TEST_REPLAY_EVIDENCE", &evidence);
        let decoy_home = home.path().join("decoy-home");
        std::fs::create_dir(&decoy_home).unwrap();
        std::env::set_var(crate::store::CONTROL_HOME_ENV, &decoy_home);
        std::env::remove_var(crate::store::CONTROL_DB_PATH_ENV);
        let registry = home.path().join("unreadable-registry");
        std::fs::create_dir(&registry).unwrap();
        std::env::set_var("LF_DB_PATH", &registry);

        let request = RunLaunchRequest {
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
        let source = CaptureHandle::begin_with_launch(
            RunSpec {
                harness: "opencode".to_string(),
                model: Some("opencode/glm-5.2".to_string()),
                surface: "headless".to_string(),
                cwd: home.path().to_path_buf(),
                repo: Some(home.path().to_path_buf()),
                worktree: Some(home.path().to_path_buf()),
                skill: Some("implement".to_string()),
                subjects: Vec::new(),
            },
            request.clone(),
        )
        .unwrap();
        let source_id = source.run_id();
        source.finish("completed").unwrap();

        let child_id = replay_at(home.path(), source_id.as_str()).unwrap();

        let evidence = std::fs::read_to_string(&evidence).unwrap();
        assert!(evidence.contains(child_id.as_str()));
        assert!(evidence.contains(source_id.as_str()));
        assert!(evidence.contains("recorded system"));
        assert!(evidence.contains("recorded task"));
        let (child_dir, child) =
            crate::run_record::resolve_manifest(home.path(), child_id.as_str()).unwrap();
        assert_eq!(child.parent_run_id.as_ref(), Some(&source_id));
        assert_eq!(child.launch.as_ref(), Some(&request));
        assert!(child_dir.join("terminal.json").is_file());
        assert!(!child_dir.join("owner.json").exists());
        assert!(!decoy_home.join("runs").exists());
        assert!(registry.is_dir());
    }
}
