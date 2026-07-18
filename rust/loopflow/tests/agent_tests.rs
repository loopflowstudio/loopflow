mod support;

use std::collections::BTreeMap;
use std::time::Duration;

use loopflow::engine::agent::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};
use loopflow::engine::error::CoreError;
use support::EnvGuard;
use tempfile::TempDir;

fn base_launch() -> AgentConfig {
    AgentConfig {
        task_prompt: "prompt".to_string(),
        agent: Some("claude".to_string()),
        skip_permissions: true,
        cwd: None,
        ..Default::default()
    }
}

fn base_process() -> ProcessConfig {
    ProcessConfig {
        auto: true,
        stream: false,
        ..Default::default()
    }
}

#[test]
fn launch_returns_exit_code() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nexit 0\n")]);
    let result = launch_agent(
        &base_launch(),
        &base_process(),
        &AgentCapabilities::default(),
    )
    .expect("launch");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn launch_captures_stdout() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\necho hello\n")]);
    let result = launch_agent(
        &base_launch(),
        &base_process(),
        &AgentCapabilities::default(),
    )
    .expect("launch");
    assert!(result.stdout.contains("hello"));
}

#[test]
fn launch_captures_stderr() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\necho error 1>&2\n")]);
    let result = launch_agent(
        &base_launch(),
        &base_process(),
        &AgentCapabilities::default(),
    )
    .expect("launch");
    assert!(result.stderr.contains("error"));
}

#[test]
fn launch_scopes_process_environment_to_child() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nprintf '%s' \"$LF_TEST_SCOPED_ENV\"\n")]);
    let process = ProcessConfig {
        env: BTreeMap::from([("LF_TEST_SCOPED_ENV".to_string(), "owned".to_string())]),
        ..base_process()
    };

    let result = launch_agent(&base_launch(), &process, &AgentCapabilities::default())
        .expect("launch with scoped environment");

    assert_eq!(result.stdout, "owned");
}

#[test]
fn launch_nonzero_exit() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nexit 7\n")]);
    let result = launch_agent(
        &base_launch(),
        &base_process(),
        &AgentCapabilities::default(),
    )
    .expect("launch");
    assert_eq!(result.exit_code, 7);
}

#[test]
fn launch_missing_binary_returns_error() {
    let _env = EnvGuard::new_isolated(&[]);
    let result = launch_agent(
        &base_launch(),
        &base_process(),
        &AgentCapabilities::default(),
    );
    assert!(matches!(
        result,
        Err(CoreError::IoError(_)) | Err(CoreError::ExecutionFailed(_))
    ));
}

#[test]
fn launch_with_cwd() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\npwd\n")]);
    let cwd = TempDir::new().expect("cwd");
    let mut launch = base_launch();
    launch.cwd = Some(cwd.path().to_path_buf());
    let result =
        launch_agent(&launch, &base_process(), &AgentCapabilities::default()).expect("launch");
    assert!(result.stdout.contains(&cwd.path().display().to_string()));
}

#[test]
fn launch_streaming_mode() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\necho first\necho second\n")]);
    let process = ProcessConfig {
        auto: true,
        stream: true,
        ..Default::default()
    };
    let result =
        launch_agent(&base_launch(), &process, &AgentCapabilities::default()).expect("launch");
    assert!(result.stdout.contains("first"));
    assert!(result.stdout.contains("second"));
}

#[test]
fn launch_batch_times_out() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nsleep 2\necho late\n")]);
    let process = ProcessConfig {
        auto: true,
        stream: false,
        timeout: Some(Duration::from_millis(100)),
        ..Default::default()
    };

    let result = launch_agent(&base_launch(), &process, &AgentCapabilities::default());
    assert!(
        matches!(result, Err(CoreError::ExecutionFailed(ref message)) if message.contains("timed out")),
        "expected timeout error, got: {result:?}"
    );
}

#[test]
fn launch_streaming_times_out() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nsleep 2\necho late\n")]);
    let process = ProcessConfig {
        auto: true,
        stream: true,
        timeout: Some(Duration::from_millis(100)),
        ..Default::default()
    };

    let result = launch_agent(&base_launch(), &process, &AgentCapabilities::default());
    assert!(
        matches!(result, Err(CoreError::ExecutionFailed(ref message)) if message.contains("timed out")),
        "expected timeout error, got: {result:?}"
    );
}
