mod support;

use base64::Engine;
use std::collections::BTreeMap;
use std::time::Duration;

use loopflow::engine::agent::{run_agent, AgentCapabilities, AgentConfig, ProcessConfig};
use loopflow::engine::error::CoreError;
use loopflow::profile::{ProviderRoute, RouteScope};
use loopflow::provider_auth::Provider;
use loopflow::store::{
    CredentialState, ProviderAccount, ProviderAccountId, RoutingState, StorageConfig,
};
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
fn claude_batch_reads_large_context_without_argv_limits() {
    let _env = EnvGuard::new(&[(
        "claude",
        "#!/bin/sh\ncat > \"$LF_TEST_INPUT\"\nprintf 'received\\n'\n",
    )]);
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
        let result = run_agent(&launch, &process, &AgentCapabilities::default())
            .expect("large context launch");
        assert_eq!(result.exit_code, 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), prompt);
    }
}

#[test]
fn library_launch_records_each_provider_under_its_invocation() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nprintf '%s' \"$$\"\n")]);
    let directory = TempDir::new().unwrap();
    let launch = AgentConfig {
        cwd: Some(directory.path().to_path_buf()),
        ..base_launch()
    };
    for _ in 0..2 {
        let result = run_agent(&launch, &base_process(), &AgentCapabilities::default()).unwrap();
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
        let agent = rows.iter().find(|row| row.pid == Some(pid)).unwrap();
        assert_eq!(agent.kind, loopflow::process::ProcessKind::Agent);
        assert!(agent.os_started_at.is_some());
        assert!(agent.completed_at.is_some());
        let parent = store
            .process(agent.parent_lf_process_id.as_ref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(parent.kind, loopflow::process::ProcessKind::Lf);
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
            .filter(|row| row.kind == loopflow::process::ProcessKind::Agent)
            .count(),
        2
    );
}

#[test]
fn library_launch_reuses_the_enclosing_invocation() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nexit 0\n")]);
    let directory = TempDir::new().unwrap();
    let launch = AgentConfig {
        cwd: Some(directory.path().to_path_buf()),
        ..base_launch()
    };
    loopflow::journal::with_runtime(directory.path(), &["fixture".into()], || {
        for _ in 0..2 {
            let result = run_agent(&launch, &base_process(), &AgentCapabilities::default())?;
            assert_eq!(result.exit_code, 0);
        }
        let store =
            loopflow::store::sqlite::SqliteStore::new(&loopflow::store::database_path_from_env()?)?;
        let rows = store.processes_since(0)?;
        let parents = rows
            .iter()
            .filter(|row| row.kind == loopflow::process::ProcessKind::Lf)
            .collect::<Vec<_>>();
        assert_eq!(parents.len(), 1);
        assert!(parents[0].completed_at.is_none());
        let agents = rows
            .iter()
            .filter(|row| row.kind == loopflow::process::ProcessKind::Agent)
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
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nexit 0\n")]);
    let result = run_agent(
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
    let result = run_agent(
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
    let result = run_agent(
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
    let launch = AgentConfig {
        env: BTreeMap::from([("LF_TEST_SCOPED_ENV".to_string(), "owned".to_string())]),
        ..base_launch()
    };

    let result = run_agent(&launch, &base_process(), &AgentCapabilities::default())
        .expect("launch with scoped environment");

    assert_eq!(result.stdout, "owned");
}

#[test]
fn launch_nonzero_exit() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nexit 7\n")]);
    let result = run_agent(
        &base_launch(),
        &base_process(),
        &AgentCapabilities::default(),
    )
    .expect("launch");
    assert_eq!(result.exit_code, 7);
}

#[test]
fn release_acceptance_recovers_from_a_revoked_selected_account() {
    let home = TempDir::new().expect("lf home");
    let codex = support::codex_socket_script(
        r#"#!/bin/sh
thread="thread-${CODEX_HOME##*/}"
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r initialized
read -r thread_start
case "$thread_start" in
  *'"method":"thread/start"'*) ;;
  *) echo "failover attempted to resume the old account" >&2; exit 10;;
esac
echo '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"'"$thread"'"}}}'
read -r turn_start
echo '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-test"}}}'
echo '{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"'"$thread"'","turn":{"id":"turn-test","status":"inProgress"}}}'
case "$CODEX_HOME" in
  */revoked)
    echo '{"jsonrpc":"2.0","method":"error","params":{"threadId":"'"$thread"'","turnId":"turn-test","error":{"message":"Your authentication token has been invalidated (token_invalidated). Please sign in again."},"willRetry":false}}'
    echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"'"$thread"'","turn":{"id":"turn-test","status":"failed"}}}';;
  */fallback)
    echo '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"'"$thread"'","turnId":"turn-test","itemId":"message-test","delta":"fallback account completed"}}'
    echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"'"$thread"'","turn":{"id":"turn-test","status":"completed"}}}';;
  *) echo "unexpected CODEX_HOME" >&2; exit 9;;
esac
if [ -n "$LF_TEST_CODEX_STDIO" ]; then exit 0; fi
while read -r line; do :; done
"#,
    );
    let _env = EnvGuard::with_lf_home(&[("codex", &codex)], home.path());
    // Isolated launches run in the routed account's own home.
    std::env::set_var("LF_ACCOUNT_ISOLATION", "isolated");
    struct RestoreLfBin(Option<std::ffi::OsString>);
    impl Drop for RestoreLfBin {
        fn drop(&mut self) {
            match &self.0 {
                Some(value) => std::env::set_var("LF_BIN", value),
                None => std::env::remove_var("LF_BIN"),
            }
        }
    }
    let _lf_bin = RestoreLfBin(std::env::var_os("LF_BIN"));
    std::env::set_var("LF_BIN", env!("CARGO_BIN_EXE_lf"));
    let revoked_home = home.path().join("accounts/codex/revoked");
    let fallback_home = home.path().join("accounts/codex/fallback");
    std::fs::create_dir_all(&revoked_home).expect("revoked home");
    std::fs::create_dir_all(&fallback_home).expect("fallback home");
    let revoked_id = ProviderAccountId::parse("revoked").expect("revoked id");
    let fallback_id = ProviderAccountId::parse("fallback").expect("fallback id");
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let account = |account_id: ProviderAccountId, path: std::path::PathBuf| {
        let email = format!("{account_id}@example.com");
        let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::json!({"email":email, "sub":account_id.as_str()}).to_string());
        std::fs::write(path.join("auth.json"), serde_json::json!({"tokens":{"access_token":"fixture", "id_token":format!("h.{claims}.s")}}).to_string()).unwrap();
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
            &StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .expect("account store");
    runtime
        .block_on(store.upsert_provider_account(&account(revoked_id.clone(), revoked_home)))
        .expect("revoked account");
    runtime
        .block_on(store.upsert_provider_account(&account(fallback_id.clone(), fallback_home)))
        .expect("fallback account");
    runtime
        .block_on(store.set_provider_route(&ProviderRoute {
            scope: RouteScope::Default,
            provider: Provider::Codex,
            accounts: vec![revoked_id.clone(), fallback_id],
            created_at: now,
            updated_at: now,
        }))
        .expect("codex route");

    let launch = AgentConfig {
        task_prompt: "finish the operation".to_string(),
        agent: Some("codex".to_string()),
        skip_permissions: true,
        ..Default::default()
    };
    let result =
        run_agent(&launch, &base_process(), &AgentCapabilities::default()).expect("route failover");

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
        "SELECT id,agent_session_id,parent_lf_process_id,completed_at,spawn_state FROM processes WHERE kind='agent' ORDER BY provider_generation"
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
    assert_eq!(thread, "thread-fallback");
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
        "thread-fallback",
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
        ("thread-fallback", "fallback"),
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
    let result = run_agent(
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
        run_agent(&launch, &base_process(), &AgentCapabilities::default()).expect("launch");
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
        run_agent(&base_launch(), &process, &AgentCapabilities::default()).expect("launch");
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

    let result = run_agent(&base_launch(), &process, &AgentCapabilities::default());
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

    let result = run_agent(&base_launch(), &process, &AgentCapabilities::default());
    assert!(
        matches!(result, Err(CoreError::ExecutionFailed(ref message)) if message.contains("timed out")),
        "expected timeout error, got: {result:?}"
    );
}
