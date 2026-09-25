mod support;

use std::collections::BTreeMap;
use std::time::Duration;

use loopflow::engine::agent::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};
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
    let launch = AgentConfig {
        env: BTreeMap::from([("LF_TEST_SCOPED_ENV".to_string(), "owned".to_string())]),
        ..base_launch()
    };

    let result = launch_agent(&launch, &base_process(), &AgentCapabilities::default())
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
fn release_acceptance_recovers_from_a_revoked_selected_account() {
    let home = TempDir::new().expect("lf home");
    let codex = r#"#!/bin/sh
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r initialized
read -r thread_start
echo '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"thread-test"}}}'
read -r turn_start
echo '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-test"}}}'
echo '{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"thread-test","turn":{"id":"turn-test","status":"inProgress"}}}'
case "$CODEX_HOME" in
  */revoked)
    echo '{"jsonrpc":"2.0","method":"error","params":{"threadId":"thread-test","turnId":"turn-test","error":{"message":"Your authentication token has been invalidated (token_invalidated). Please sign in again."},"willRetry":false}}'
    echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread-test","turn":{"id":"turn-test","status":"failed"}}}';;
  */fallback)
    echo '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"thread-test","turnId":"turn-test","itemId":"message-test","delta":"fallback account completed"}}'
    echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread-test","turn":{"id":"turn-test","status":"completed"}}}';;
  *) echo "unexpected CODEX_HOME" >&2; exit 9;;
esac
while read -r line; do :; done
"#;
    let _env = EnvGuard::with_lf_home(&[("codex", codex)], home.path());
    let revoked_home = home.path().join("accounts/codex/revoked");
    let fallback_home = home.path().join("accounts/codex/fallback");
    std::fs::create_dir_all(&revoked_home).expect("revoked home");
    std::fs::create_dir_all(&fallback_home).expect("fallback home");
    let revoked_id = ProviderAccountId::parse("revoked").expect("revoked id");
    let fallback_id = ProviderAccountId::parse("fallback").expect("fallback id");
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let account = |account_id: ProviderAccountId, path: std::path::PathBuf| ProviderAccount {
        provider: "codex".to_string(),
        account_id,
        home: Some(path),
        login_email: None,
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
    let result = launch_agent(&launch, &base_process(), &AgentCapabilities::default())
        .expect("route failover");

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
