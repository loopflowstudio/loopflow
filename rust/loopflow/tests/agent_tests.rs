mod support;

use base64::prelude::*;
use std::collections::BTreeMap;
use std::time::Duration;

use loopflow::agent::{run_agent, AgentConfig, AgentProcessResult, ProcessConfig};
use loopflow::error::CoreError;
use loopflow::profile::{ProviderRoute, RouteScope};
use loopflow::provider_auth::Provider;
use loopflow::store::{
    CredentialState, ProviderAccount, ProviderAccountId, RoutingState, StorageConfig,
};
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

/// A store routing Codex through `first`, then `second`, each with a login.
fn two_codex_accounts(
    home: &std::path::Path,
    first: &str,
    second: &str,
) -> (
    tokio::runtime::Runtime,
    loopflow::store::Store,
    ProviderAccountId,
) {
    let first_id = ProviderAccountId::parse(first).expect("first id");
    let second_id = ProviderAccountId::parse(second).expect("second id");
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let account = |account_id: ProviderAccountId| {
        let path = home.join("accounts/codex").join(account_id.as_str());
        std::fs::create_dir_all(&path).expect("account home");
        let email = format!("{account_id}@example.com");
        let claims = BASE64_URL_SAFE_NO_PAD
            .encode(serde_json::json!({"email":email, "sub":account_id.as_str()}).to_string());
        // The access token names the account so a stand-in can tell which login
        // the shared home carries.
        std::fs::write(path.join("auth.json"), serde_json::json!({"tokens":{"access_token":format!("fixture-{account_id}"), "id_token":format!("h.{claims}.s")}}).to_string()).unwrap();
        ProviderAccount {
            provider: "codex".to_string(),
            account_id,
            home: Some(path),
            login_email: Some(loopflow::profile::EmailAddress::parse(&email).unwrap()),
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: now,
            updated_at: now,
        }
    };
    let runtime = tokio::runtime::Runtime::new().expect("store runtime");
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.join("loopflow.db")),
        ))
        .expect("account store");
    for id in [&first_id, &second_id] {
        runtime
            .block_on(store.upsert_provider_account(&account(id.clone())))
            .expect("account");
    }
    runtime
        .block_on(store.set_provider_route(&ProviderRoute {
            scope: RouteScope::Default,
            provider: Provider::Codex,
            accounts: vec![first_id.clone(), second_id],
            created_at: now,
            updated_at: now,
        }))
        .expect("codex route");
    (runtime, store, first_id)
}

/// A Codex provider whose every launch keeps one thread with a transcript in
/// its home. `$LIMITED` lists the accounts that hit a limit on their turn,
/// each resetting `$RESET_IN` seconds from then, once.
const LIMITED_CODEX: &str = r#"#!/bin/sh
account=$(sed -n 's/.*"access_token":"fixture-\([^"]*\)".*/\1/p' "$CODEX_HOME/auth.json")
thread="0199a213-81c0-7800-8aa1-bbab2a035a53"
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r initialized
read -r thread_open
case "$thread_open" in
  *'"method":"thread/start"'*) echo "start $account" >> "$LAUNCHES";;
  *'"method":"thread/resume"'*) echo "resume $account" >> "$LAUNCHES";;
esac
mkdir -p "$CODEX_HOME/sessions/2026/11/05"
echo '{"type":"session_meta"}' >> "$CODEX_HOME/sessions/2026/11/05/rollout-2026-11-05T10-00-00-$thread.jsonl"
echo '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"'"$thread"'"}}}'
read -r turn_start
echo '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-test"}}}'
echo '{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"'"$thread"'","turn":{"id":"turn-test","status":"inProgress"}}}'
case " $LIMITED " in
  *" $account "*)
    if [ ! -e "$CODEX_HOME/limited-once-$account" ]; then
      touch "$CODEX_HOME/limited-once-$account"
      reset=$(( $(date +%s) + RESET_IN ))
      echo '{"jsonrpc":"2.0","method":"account/rateLimits/updated","params":{"rateLimits":{"primary":{"usedPercent":100,"resetsAt":'"$reset"'},"rateLimitReachedType":"rate_limit_reached"}}}'
      echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"'"$thread"'","turn":{"id":"turn-test","status":"failed"}}}'
      while read -r line; do :; done
      exit 0
    fi;;
esac
echo '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"'"$thread"'","turnId":"turn-test","itemId":"message-test","delta":"finished on '"$account"'"}}'
echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"'"$thread"'","turn":{"id":"turn-test","status":"completed"}}}'
if [ -n "$LF_TEST_CODEX_STDIO" ]; then exit 0; fi
while read -r line; do :; done
"#;

struct RestoreEnv(&'static str, Option<std::ffi::OsString>);

impl RestoreEnv {
    fn set(name: &'static str, value: &str) -> Self {
        let restore = Self(name, std::env::var_os(name));
        std::env::set_var(name, value);
        restore
    }
}

impl Drop for RestoreEnv {
    fn drop(&mut self) {
        match &self.1 {
            Some(value) => std::env::set_var(self.0, value),
            None => std::env::remove_var(self.0),
        }
    }
}

fn branch_lf() -> RestoreEnv {
    RestoreEnv::set("LF_BIN", env!("CARGO_BIN_EXE_lf"))
}

fn run_limited_step(
    home: &std::path::Path,
    limited: &str,
    reset_in: u32,
) -> (AgentProcessResult, String) {
    let launches = home.join("launches");
    let launch = AgentConfig {
        task_prompt: "finish the operation".to_string(),
        agent: Some("codex".to_string()),
        skip_permissions: true,
        env: BTreeMap::from([
            ("LAUNCHES".into(), launches.display().to_string()),
            ("LIMITED".into(), limited.to_string()),
            ("RESET_IN".into(), reset_in.to_string()),
        ]),
        ..Default::default()
    };
    let step = ProcessConfig {
        wait_for_capacity: true,
        ..base_process()
    };
    let result = run_agent(&launch, &step).expect("limited step");
    (result, std::fs::read_to_string(launches).unwrap())
}

/// The November 2 shape: a limit event on the first account, an eligible second.
#[test]
fn a_limit_moves_the_step_and_its_conversation_to_the_next_account() {
    let home = TempDir::new().expect("lf home");
    let codex = support::codex_socket_script(LIMITED_CODEX);
    let _env = EnvGuard::with_lf_home(&[("codex", &codex)], home.path());
    let _lf = branch_lf();
    let (runtime, store, primary) = two_codex_accounts(home.path(), "primary", "backup");

    let (result, launches) = run_limited_step(home.path(), "primary", 3_600);

    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.stdout.contains("finished on backup"));
    assert_eq!(launches, "start primary\nresume backup\n");
    let cooled = runtime
        .block_on(store.get_provider_account("codex", &primary))
        .unwrap()
        .unwrap();
    assert_eq!(
        cooled.cooldown_reason.as_deref(),
        Some("rate_limit_reached")
    );
    // The conversation moved on in the one shared home, now signed in as backup.
    let shared = std::path::PathBuf::from(std::env::var_os("CODEX_HOME").unwrap());
    assert!(shared.join("sessions/2026/11/05").is_dir());
    assert!(std::fs::read_to_string(shared.join("auth.json"))
        .unwrap()
        .contains("fixture-backup"));
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let (account, retry_at): (String, Option<i64>) = db
        .query_row(
            "SELECT (SELECT account_id FROM provider_session_accounts), retry_at FROM agent_limit_recoveries",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(account, "backup");
    assert_eq!(retry_at, None);
}

/// Both accounts run out; the step waits, then goes on where the reset frees it.
#[test]
fn with_every_account_exhausted_the_step_waits_for_a_reset_and_finishes() {
    let home = TempDir::new().expect("lf home");
    let codex = support::codex_socket_script(LIMITED_CODEX);
    let _env = EnvGuard::with_lf_home(&[("codex", &codex)], home.path());
    let _lf = branch_lf();
    let (_runtime, _store, _) = two_codex_accounts(home.path(), "primary", "backup");

    let (result, launches) = run_limited_step(home.path(), "primary backup", 1);

    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.stdout.contains("finished on"));
    let launches: Vec<_> = launches.lines().collect();
    assert_eq!(launches[..2], ["start primary", "resume backup"]);
    assert_eq!(launches.len(), 3, "{launches:?}");
    assert!(launches[2].starts_with("resume "));
}

/// A wait keeps the explicit choice its step ran under, for whatever restarts it.
#[test]
fn a_limit_wait_records_the_explicit_agent() {
    let home = TempDir::new().expect("lf home");
    let codex = support::codex_socket_script(LIMITED_CODEX);
    let _env = EnvGuard::with_lf_home(&[("codex", &codex)], home.path());
    let _lf = branch_lf();
    let (_runtime, _store, _) = two_codex_accounts(home.path(), "primary", "backup");
    let _selected = loopflow::config::settings::select_invocation(
        serde_yaml_ng::from_str("agent: codex").unwrap(),
    );

    let (result, _) = run_limited_step(home.path(), "primary backup", 1);

    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let agent: Option<String> = db
        .query_row("SELECT agent FROM agent_limit_recoveries", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(agent.as_deref(), Some("codex"));
}

#[test]
fn release_acceptance_recovers_from_a_revoked_selected_account() {
    let home = TempDir::new().expect("lf home");
    let codex = support::codex_socket_script(
        r#"#!/bin/sh
account=$(sed -n 's/.*"access_token":"fixture-\([^"]*\)".*/\1/p' "$CODEX_HOME/auth.json")
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r initialized
read -r thread_open
case "$thread_open" in
  *'"method":"thread/start"'*) thread="thread-$account";;
  *'"method":"thread/resume"'*)
    thread=$(printf '%s' "$thread_open" | sed -n 's/.*"threadId":"\([^"]*\)".*/\1/p')
    if [ "$account" = revoked ]; then echo "resumed on the revoked login" >&2; exit 10; fi;;
  *) echo "unexpected thread request" >&2; exit 10;;
esac
echo '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"'"$thread"'"}}}'
read -r turn_start
echo '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-test"}}}'
echo '{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"'"$thread"'","turn":{"id":"turn-test","status":"inProgress"}}}'
case "$account" in
  revoked)
    echo '{"jsonrpc":"2.0","method":"error","params":{"threadId":"'"$thread"'","turnId":"turn-test","error":{"message":"Your authentication token has been invalidated (token_invalidated). Please sign in again."},"willRetry":false}}'
    echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"'"$thread"'","turn":{"id":"turn-test","status":"failed"}}}';;
  fallback)
    echo '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"'"$thread"'","turnId":"turn-test","itemId":"message-test","delta":"fallback account completed"}}'
    echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"'"$thread"'","turn":{"id":"turn-test","status":"completed"}}}';;
  *) echo "unexpected login in CODEX_HOME" >&2; exit 9;;
esac
if [ -n "$LF_TEST_CODEX_STDIO" ]; then exit 0; fi
while read -r line; do :; done
"#,
    );
    let _env = EnvGuard::with_lf_home(&[("codex", &codex)], home.path());
    let _lf = branch_lf();
    let (runtime, store, revoked_id) = two_codex_accounts(home.path(), "revoked", "fallback");

    let launch = AgentConfig {
        task_prompt: "finish the operation".to_string(),
        agent: Some("codex".to_string()),
        skip_permissions: true,
        ..Default::default()
    };
    let result = run_agent(&launch, &base_process()).expect("route failover");

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("fallback account completed"));
    let revoked = runtime
        .block_on(store.get_provider_account("codex", &revoked_id))
        .expect("read revoked account")
        .expect("revoked account remains recorded");
    assert_eq!(revoked.credential_state, CredentialState::Missing);
    assert_eq!(
        revoked.cooldown_reason.as_deref(),
        Some("token_invalidated")
    );
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let agents = db.prepare(
        "SELECT id,agent_session_id,parent_lf_process_id,completed_at,spawn_state FROM processes WHERE kind='agent' ORDER BY rowid"
    ).unwrap().query_map([], |row| Ok((
        row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
        row.get::<_, Option<i64>>(3)?, row.get::<_, String>(4)?,
    ))).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(agents.len(), 2);
    assert_ne!(agents[0].0, agents[1].0);
    assert_eq!(agents[0].1, agents[1].1);
    assert_eq!(agents[0].2, agents[1].2);
    assert!(agents
        .iter()
        .all(|agent| agent.3.is_some() && agent.4 == "exited"));
    let thread: String = db
        .query_row(
            "SELECT provider_thread FROM agent_sessions WHERE id=?1",
            [&agents[0].1],
            |row| row.get(0),
        )
        .unwrap();
    // The shared home carries the conversation on to the fallback login.
    assert_eq!(thread, "thread-revoked");
    let observations = db
        .prepare("SELECT payload FROM session_events WHERE session_id=?1 ORDER BY seq")
        .unwrap()
        .query_map([&agents[0].1], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n");
    for retained in [
        "thread-revoked",
        "revoked",
        "fallback",
        &agents[0].0,
        &agents[1].0,
    ] {
        assert!(
            observations.contains(retained),
            "missing retained history: {retained}"
        );
    }
    for (thread, account) in [
        ("thread-revoked", "revoked"),
        ("thread-revoked", "fallback"),
    ] {
        let retained: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM session_events WHERE session_id=?1
             AND json_extract(payload,'$.evidence.provider_session_id')=?2
             AND json_extract(payload,'$.evidence.account_id')=?3)",
                rusqlite::params![agents[0].1, thread, account],
                |row| row.get(0),
            )
            .unwrap();
        assert!(retained, "native history lost account {account}");
    }
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

#[test]
fn direct_native_codex_launch_preserves_account_policy_and_literal_input() {
    let home = TempDir::new().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let _directory = support::InDirectory::enter(home.path());
    let bin = home.path().join("bin");
    support::install_native_codex(&bin);
    let path = std::env::var_os("PATH").unwrap();
    std::env::set_var(
        "PATH",
        std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
    );
    support::register_codex_account(home.path());
    // A bare Codex runs as plain `codex`; the app-server belongs to Work,
    // which starts beside the Machine server.
    let _server = loopflow_test_support::MachineServer::start(
        loopflow_test_support::lf_binary(),
        home.path(),
    );
    let launch = AgentConfig {
        agent: Some("codex:model".into()),
        system_prompt: "Keep these instructions".into(),
        task_prompt: "--- literal prompt".into(),
        cwd: Some(home.path().into()),
        skip_permissions: true,
        provider_account_id: Some(ProviderAccountId::parse("fixture").unwrap()),
        holder: loopflow::session::Holder::Server,
        ..Default::default()
    };
    let result = run_agent(&launch, &ProcessConfig::default()).unwrap();
    assert_eq!(result.exit_code, 0);
    let received = std::fs::read_to_string(home.path().join("native-argv.jsonl")).unwrap();
    let launches: Vec<Vec<String>> = received
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(launches
        .iter()
        .any(|args| args.contains(&"--remote".into())));
    // The shared home is signed in as the selected account.
    let shared = std::path::PathBuf::from(std::env::var_os("CODEX_HOME").unwrap());
    assert_eq!(
        std::fs::read(shared.join("auth.json")).unwrap(),
        std::fs::read(home.path().join("accounts/codex/fixture/auth.json")).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(home.path().join("native-thread")).unwrap(),
        result.agent_session.as_ref().unwrap().as_str()
    );
    assert!(!launches
        .iter()
        .flatten()
        .any(|arg| arg.contains("--- literal prompt")));
    let params: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.path().join("native-config.json")).unwrap())
            .unwrap();
    assert_eq!(params["model"], "model");
    assert_eq!(params["sandbox"], "danger-full-access");
    assert_eq!(params["approvalPolicy"], "never");
    let turns: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.path().join("native-turns.json")).unwrap())
            .unwrap();
    assert_eq!(
        turns[0]["items"][0]["content"][0]["text"],
        "--- literal prompt"
    );
}
