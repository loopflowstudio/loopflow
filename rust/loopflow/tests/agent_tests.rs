mod support;

use std::collections::BTreeMap;
use std::time::Duration;

use loopflow::agent::{run_agent, AgentConfig, AgentFailure, ProcessConfig};
use loopflow::error::CoreError;
use support::EnvGuard;
use tempfile::TempDir;

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
        let result = run_agent(&launch, &process).expect("large context launch");
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
        let result = run_agent(&launch, &base_process()).unwrap();
        assert_eq!(result.exit_code, 0);
        let pid: u32 = result
            .stdout
            .trim()
            .parse()
            .unwrap_or_else(|error| panic!("{error}: fixture output {:?}", result.stdout));
        let store = loopflow::store::sqlite::SqliteStore::new(
            &loopflow::store::database_path_from_env().unwrap(),
        )
        .unwrap();
        let rows = store.processes_since(0).unwrap();
        let agent = rows
            .iter()
            .filter_map(loopflow::process::ProcessEntry::as_agent)
            .find(|row| row.pid == Some(pid))
            .unwrap();
        assert!(agent.os_started_at.is_some());
        assert!(agent.completed_at.is_some());
        let parent = store
            .lf_process(agent.parent_lf_process_id.as_ref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(parent.pid, Some(std::process::id()));
        assert!(parent.completed_at.is_some());
        assert!(agent.agent_session_id.is_some());
    }
    let store = loopflow::store::sqlite::SqliteStore::new(
        &loopflow::store::database_path_from_env().unwrap(),
    )
    .unwrap();
    let rows = store.processes_since(0).unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows.iter()
            .filter_map(loopflow::process::ProcessEntry::as_agent)
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
    loopflow::journal::with_runtime(directory.path(), &["fixture".into()], || {
        for _ in 0..2 {
            let result = run_agent(&launch, &base_process())?;
            assert_eq!(result.exit_code, 0);
        }
        let store =
            loopflow::store::sqlite::SqliteStore::new(&loopflow::store::database_path_from_env()?)?;
        let rows = store.processes_since(0)?;
        let parents = rows
            .iter()
            .filter_map(loopflow::process::ProcessEntry::as_lf)
            .collect::<Vec<_>>();
        assert_eq!(parents.len(), 1);
        assert!(parents[0].completed_at.is_none());
        let agents = rows
            .iter()
            .filter_map(loopflow::process::ProcessEntry::as_agent)
            .collect::<Vec<_>>();
        assert_eq!(agents.len(), 2);
        for agent in agents {
            assert_eq!(agent.parent_lf_process_id.as_ref(), Some(&parents[0].id));
            assert!(agent.completed_at.is_some());
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn launch_returns_exit_code() {
    let _env = EnvGuard::new(&[("claude", &claude_script("text = 'done'"))]);
    let result = run_agent(&base_launch(), &base_process()).expect("launch");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn launch_captures_stdout() {
    let _env = EnvGuard::new(&[("claude", &claude_script("text = 'hello'"))]);
    let result = run_agent(&base_launch(), &base_process()).expect("launch");
    assert!(result.stdout.contains("hello"));
}

#[test]
fn launch_captures_stderr() {
    let _env = EnvGuard::new(&[(
        "claude",
        &claude_script("text = 'fixture error'; failed = True"),
    )]);
    let result = run_agent(&base_launch(), &base_process()).expect("launch");
    assert!(result.stderr.contains("error"));
}

#[test]
fn model_unavailable_result_falls_through_or_stops_without_spending_quota() {
    for (case, fallbacks, expected_launches, stop) in [
        ("fallback", vec!["claude:opus"], "fable\nopus\n", None),
        (
            "pinned",
            vec![],
            "fable\n",
            Some("No fallback remains. Change an `agent` or `-a` pin"),
        ),
        (
            "exhausted",
            vec!["claude:opus"],
            "fable\nopus\n",
            Some("Nothing is left in its order after claude:fable, claude:opus"),
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
            agent: Some("claude:fable".into()),
            fallback_agents: fallbacks.into_iter().map(str::to_owned).collect(),
            cwd: Some(home.path().to_path_buf()),
            env: BTreeMap::from([
                ("LF_TEST_LAUNCHES".into(), launches.display().to_string()),
                ("LF_TEST_CASE".into(), case.into()),
            ]),
            ..base_launch()
        };
        let result = run_agent(&launch, &base_process()).unwrap();
        assert_eq!(
            std::fs::read_to_string(launches).unwrap(),
            expected_launches
        );
        if stop.is_some() {
            assert_ne!(result.exit_code, 0);
            assert_eq!(result.failure, Some(AgentFailure::ModelUnavailable));
        } else {
            assert_eq!(result.exit_code, 0, "{}", result.stderr);
            assert_eq!(result.stdout, "finished on opus");
        }
    }
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

    let result = run_agent(&launch, &base_process()).expect("launch with scoped environment");

    assert_eq!(result.stdout, "owned");
}

#[test]
fn launch_nonzero_exit() {
    let _env = EnvGuard::new(&[("claude", &claude_script("sys.exit(7)"))]);
    let result = run_agent(&base_launch(), &base_process()).expect("launch");
    assert_ne!(result.exit_code, 0);
}

#[test]
fn launch_missing_binary_returns_error() {
    let _env = EnvGuard::new_isolated(&[]);
    let result = run_agent(&base_launch(), &base_process());
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
    let result = run_agent(&launch, &base_process()).expect("launch");
    assert!(result.stdout.contains(&cwd.path().display().to_string()));
}

#[test]
fn launch_streaming_mode() {
    let _env = EnvGuard::new(&[("claude", &claude_script("text = 'first\\nsecond'"))]);
    let process = ProcessConfig {
        auto: true,
        stream: true,
        ..Default::default()
    };
    let result = run_agent(&base_launch(), &process).expect("launch");
    assert!(result.stdout.contains("first"));
    assert!(result.stdout.contains("second"));
}

#[test]
fn launch_batch_times_out() {
    let _env = EnvGuard::new(&[("claude", &claude_script("time.sleep(2); text = 'late'"))]);
    let process = ProcessConfig {
        auto: true,
        stream: false,
        timeout: Some(Duration::from_millis(100)),
        ..Default::default()
    };

    let result = run_agent(&base_launch(), &process);
    assert!(
        matches!(result, Err(CoreError::ExecutionFailed(ref message)) if message.contains("timed out")),
        "expected timeout error, got: {result:?}"
    );
}

#[test]
fn launch_streaming_times_out() {
    let _env = EnvGuard::new(&[("claude", &claude_script("time.sleep(2); text = 'late'"))]);
    let process = ProcessConfig {
        auto: true,
        stream: true,
        timeout: Some(Duration::from_millis(100)),
        ..Default::default()
    };

    let result = run_agent(&base_launch(), &process);
    assert!(
        matches!(result, Err(CoreError::ExecutionFailed(ref message)) if message.contains("timed out")),
        "expected timeout error, got: {result:?}"
    );
}
