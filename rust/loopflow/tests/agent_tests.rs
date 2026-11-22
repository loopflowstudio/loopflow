mod support;

use std::collections::BTreeMap;
use std::time::Duration;

use loopflow_execution::agent::{run_agent, AgentConfig, AgentFailure, ProcessConfig};
use loopflow_execution::error::CoreError;
use loopflow_execution::harness::LaunchKind;
use support::EnvGuard;
use tempfile::TempDir;

const HEADLESS: LaunchKind = LaunchKind::Batch;

fn claude_script(body: &str) -> String {
    r#"#!/usr/bin/python3
import json, os, sys, time
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
message = json.loads(sys.stdin.readline())
message['session_id'] = 'fixture-thread'
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'fixture-thread'}), flush=True)
text = ''
failed = False
__BODY__
print(json.dumps({'type': 'assistant', 'message': {'content': [{'type': 'text', 'text': text}]}}), flush=True)
print(json.dumps({'type': 'result', 'uuid': 'fixture-result', 'session_id': 'fixture-thread', 'subtype': 'error_during_execution' if failed else 'success', 'is_error': failed, 'result': text}), flush=True)
for line in sys.stdin:
    pass
"#.replace("__BODY__", body)
}

fn base_launch() -> AgentConfig {
    AgentConfig {
        task_prompt: "prompt".to_string(),
        yolo: true,
        ..AgentConfig::new(loopflow_execution::config::Agent::parse("claude").unwrap())
    }
}

fn base_process() -> ProcessConfig {
    ProcessConfig {
        stream: false,
        ..Default::default()
    }
}

#[test]
fn claude_batch_reads_large_context_without_argv_limits() {
    let _env = EnvGuard::new(&[("claude", &claude_script(
        "open(os.environ['LF_TEST_INPUT'], 'w').write(message['message']['content'][0]['text'])\ntext = 'received'"
    ))]);
    let directory = TempDir::new().expect("input directory");
    let path = directory.path().join("input");
    let prompt = "Preserve the entire context — including newlines.\n".repeat(30_000);
    let launch = AgentConfig {
        task_prompt: prompt.clone(),
        env: BTreeMap::from([("LF_TEST_INPUT".into(), path.display().to_string())]),
        ..base_launch()
    };
    for stream in [false, true] {
        let process = ProcessConfig {
            stream,
            ..base_process()
        };
        let result = run_agent(&launch, &process, HEADLESS).expect("large context launch");
        assert_eq!(result.exit_code, 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), prompt);
    }
}

#[test]
fn library_launch_records_each_provider_under_its_invocation() {
    let _env = EnvGuard::new(&[("claude", &claude_script("text = str(os.getpid())"))]);
    let directory = TempDir::new().unwrap();
    let launch = AgentConfig {
        cwd: Some(directory.path().to_path_buf()),
        ..base_launch()
    };
    for _ in 0..2 {
        let result = run_agent(&launch, &base_process(), HEADLESS).unwrap();
        assert_eq!(result.exit_code, 0);
        let pid: u32 = result
            .stdout
            .trim()
            .parse()
            .unwrap_or_else(|error| panic!("{error}: fixture output {:?}", result.stdout));
        let store = loopflow_execution::store::open_sqlite(
            &loopflow_execution::store::database_path_from_env().unwrap(),
        )
        .unwrap();
        let rows = store.processes_since(0).unwrap();
        let agent = rows
            .iter()
            .filter_map(loopflow_execution::process::ProcessEntry::as_agent)
            .find(|row| row.pid == Some(pid))
            .unwrap();
        assert!(agent.os_started_at.is_some());
        assert!(agent.completed_at.is_some());
        assert!(agent.parent_lf_process_id.is_some());
        assert!(agent.agent_session_id.is_some());
    }
    let store = loopflow_execution::store::open_sqlite(
        &loopflow_execution::store::database_path_from_env().unwrap(),
    )
    .unwrap();
    // Each provider's row is the one a released reader joins; lf records
    // no row of its own.
    let rows = store.processes_since(0).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .filter_map(loopflow_execution::process::ProcessEntry::as_agent)
            .count(),
        2
    );
}

#[test]
fn library_launch_reuses_the_enclosing_invocation() {
    let _env = EnvGuard::new(&[("claude", &claude_script("text = 'done'"))]);
    let directory = TempDir::new().unwrap();
    let launch = AgentConfig {
        cwd: Some(directory.path().to_path_buf()),
        ..base_launch()
    };
    loopflow_execution::journal::with_runtime(directory.path(), &["fixture".into()], || {
        for _ in 0..2 {
            let result = run_agent(&launch, &base_process(), HEADLESS)?;
            assert_eq!(result.exit_code, 0);
        }
        let store = loopflow_execution::store::open_sqlite(
            &loopflow_execution::store::database_path_from_env()?,
        )?;
        let rows = store.processes_since(0)?;
        let invocation = loopflow_execution::journal::current_lf_process_id().unwrap();
        let agents = rows
            .iter()
            .filter_map(loopflow_execution::process::ProcessEntry::as_agent)
            .collect::<Vec<_>>();
        assert_eq!(agents.len(), 2);
        for agent in agents {
            assert_eq!(agent.parent_lf_process_id.as_ref(), Some(&invocation));
            assert!(agent.completed_at.is_some());
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn launch_returns_exit_code() {
    let _env = EnvGuard::new(&[("claude", &claude_script("text = 'done'"))]);
    let result = run_agent(&base_launch(), &base_process(), HEADLESS).expect("launch");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn launch_captures_stdout() {
    let _env = EnvGuard::new(&[("claude", &claude_script("text = 'hello'"))]);
    let result = run_agent(&base_launch(), &base_process(), HEADLESS).expect("launch");
    assert!(result.stdout.contains("hello"));
}

#[test]
fn launch_captures_stderr() {
    let _env = EnvGuard::new(&[(
        "claude",
        &claude_script("text = 'fixture error'; failed = True"),
    )]);
    let result = run_agent(&base_launch(), &base_process(), HEADLESS).expect("launch");
    assert!(result.stderr.contains("error"));
}

#[test]
fn model_unavailable_result_falls_through_or_stops_without_spending_quota() {
    for (case, wait_for_capacity, fallbacks, expected_launches, stop) in [
        (
            "fallback",
            true,
            vec!["claude:opus"],
            "fable\nopus\n",
            false,
        ),
        ("pinned", true, vec![], "fable\n", true),
        (
            "exhausted",
            true,
            vec!["claude:opus"],
            "fable\nopus\n",
            true,
        ),
        // A command outside a Graph step moves the same way.
        (
            "command",
            false,
            vec!["claude:opus"],
            "fable\nopus\n",
            false,
        ),
    ] {
        let home = TempDir::new().unwrap();
        let script = claude_script(
            r#"model = sys.argv[sys.argv.index('--model') + 1]
with open(os.environ['LF_TEST_LAUNCHES'], 'a') as launches:
    launches.write(model + '\n')
failed = model == 'fable' or os.environ['LF_TEST_CASE'] == 'exhausted'
text = 'Fable 5 is currently unavailable. Switch to another model.' if failed else 'finished on ' + model"#,
        );
        let _env = EnvGuard::with_lf_home(&[("claude", &script)], home.path());
        let _directory = support::InDirectory::enter(home.path());
        let launches = home.path().join("launches");
        let launch = AgentConfig {
            agent: loopflow_execution::config::Agent::parse("claude:fable").unwrap(),
            fallback_agents: fallbacks
                .into_iter()
                .map(|agent| loopflow_execution::config::Agent::parse(agent).unwrap())
                .collect(),
            cwd: Some(home.path().to_path_buf()),
            env: BTreeMap::from([
                ("LF_TEST_LAUNCHES".into(), launches.display().to_string()),
                ("LF_TEST_CASE".into(), case.into()),
            ]),
            ..base_launch()
        };
        let process = ProcessConfig {
            wait_for_capacity,
            ..base_process()
        };
        let result = run_agent(&launch, &process, HEADLESS).unwrap();
        assert_eq!(
            std::fs::read_to_string(launches).unwrap(),
            expected_launches
        );
        if stop {
            assert_ne!(result.exit_code, 0);
            assert_eq!(result.failure, Some(AgentFailure::ModelUnavailable));
        } else {
            assert_eq!(result.exit_code, 0, "{}", result.stderr);
            assert_eq!(result.stdout, "finished on opus");
        }
    }
}

/// LOO-974's refusal, 2026-11-18, as Codex's app-server words it: the cause
/// is in `codexErrorInfo`, and the sentence's apostrophe is U+2019.
#[test]
fn codex_usage_limit_goes_on_under_the_next_agent() {
    const REFUSAL: &str = r#"{"message":"You\u2019ve hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/settings/usage to purchase more credits or try again at 3:36 PM.","codexErrorInfo":"usageLimitExceeded","additionalDetails":null,"misalignment":null}"#;
    let codex = support::codex_socket_script(
        &r#"#!/bin/sh
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r initialized
read -r thread_start
echo '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"0197f00d-aaaa-7000-8000-000000000001"}}}'
read -r turn_start
echo '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn"}}}'
echo '{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"0197f00d-aaaa-7000-8000-000000000001","turn":{"id":"turn","status":"inProgress"}}}'
echo '{"jsonrpc":"2.0","method":"error","params":{"threadId":"0197f00d-aaaa-7000-8000-000000000001","turnId":"turn","error":__REFUSAL__,"willRetry":false}}'
echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"0197f00d-aaaa-7000-8000-000000000001","turn":{"id":"turn","status":"failed","error":__REFUSAL__}}}'
if [ -n "$LF_TEST_CODEX_STDIO" ]; then exit 0; fi
while read -r line; do :; done
"#
        .replace("__REFUSAL__", REFUSAL),
    );
    let home = TempDir::new().unwrap();
    let claude = claude_script("text = 'finished on claude'");
    let _env = EnvGuard::with_lf_home(&[("codex", &codex), ("claude", &claude)], home.path());
    let _directory = support::InDirectory::enter(home.path());
    let launch = AgentConfig {
        agent: loopflow_execution::config::Agent::parse("codex:gpt-6.2-astra").unwrap(),
        fallback_agents: vec![loopflow_execution::config::Agent::parse("claude:fable").unwrap()],
        cwd: Some(home.path().to_path_buf()),
        ..base_launch()
    };
    let process = ProcessConfig {
        wait_for_capacity: true,
        ..base_process()
    };
    let result = run_agent(&launch, &process, HEADLESS).unwrap();
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert_eq!(result.stdout, "finished on claude");
}

#[test]
fn launch_scopes_process_environment_to_child() {
    let _env = EnvGuard::new(&[(
        "claude",
        &claude_script("text = os.environ['LF_TEST_SCOPED_ENV']"),
    )]);
    let launch = AgentConfig {
        env: BTreeMap::from([("LF_TEST_SCOPED_ENV".to_string(), "owned".to_string())]),
        ..base_launch()
    };

    let result =
        run_agent(&launch, &base_process(), HEADLESS).expect("launch with scoped environment");

    assert_eq!(result.stdout, "owned");
}

#[test]
fn launch_nonzero_exit() {
    let _env = EnvGuard::new(&[("claude", &claude_script("sys.exit(7)"))]);
    let result = run_agent(&base_launch(), &base_process(), HEADLESS).expect("launch");
    assert_ne!(result.exit_code, 0);
}

#[test]
fn launch_missing_binary_returns_error() {
    let _env = EnvGuard::new_isolated(&[]);
    let result = run_agent(&base_launch(), &base_process(), HEADLESS);
    assert!(matches!(
        result,
        Err(CoreError::IoError(_)) | Err(CoreError::ExecutionFailed(_))
    ));
}

#[test]
fn launch_with_cwd() {
    let _env = EnvGuard::new(&[("claude", &claude_script("text = os.getcwd()"))]);
    let cwd = TempDir::new().expect("cwd");
    let mut launch = base_launch();
    launch.cwd = Some(cwd.path().to_path_buf());
    let result = run_agent(&launch, &base_process(), HEADLESS).expect("launch");
    assert!(result.stdout.contains(&cwd.path().display().to_string()));
}

#[test]
fn launch_streaming_mode() {
    let _env = EnvGuard::new(&[("claude", &claude_script("text = 'first\\nsecond'"))]);
    let process = ProcessConfig {
        stream: true,
        ..Default::default()
    };
    let result = run_agent(&base_launch(), &process, HEADLESS).expect("launch");
    assert!(result.stdout.contains("first"));
    assert!(result.stdout.contains("second"));
}

#[test]
fn launch_batch_times_out() {
    let _env = EnvGuard::new(&[("claude", &claude_script("time.sleep(2); text = 'late'"))]);
    let process = ProcessConfig {
        stream: false,
        timeout: Some(Duration::from_millis(100)),
        ..Default::default()
    };

    let result = run_agent(&base_launch(), &process, HEADLESS);
    assert!(
        matches!(result, Err(CoreError::ExecutionFailed(ref message)) if message.contains("timed out")),
        "expected timeout error, got: {result:?}"
    );
}

#[test]
fn launch_streaming_times_out() {
    let _env = EnvGuard::new(&[("claude", &claude_script("time.sleep(2); text = 'late'"))]);
    let process = ProcessConfig {
        stream: true,
        timeout: Some(Duration::from_millis(100)),
        ..Default::default()
    };

    let result = run_agent(&base_launch(), &process, HEADLESS);
    assert!(
        matches!(result, Err(CoreError::ExecutionFailed(ref message)) if message.contains("timed out")),
        "expected timeout error, got: {result:?}"
    );
}
