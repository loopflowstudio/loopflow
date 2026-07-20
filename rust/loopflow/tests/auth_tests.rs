mod support;

use std::process::Command;

use loopflow::store::{
    open_store, CredentialState, ProviderAccount, ProviderAccountId, RoutingState, StorageConfig,
};
use support::EnvGuard;

fn account(account_id: &str, home: std::path::PathBuf) -> ProviderAccount {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    ProviderAccount {
        provider: "codex".to_string(),
        account_id: ProviderAccountId::parse(account_id).expect("account id"),
        home: Some(home),
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
    }
}

#[test]
fn accounts_verify_separates_cached_state_from_live_provider_state() {
    let home = tempfile::TempDir::new().expect("lf home");
    let active_home = home.path().join("accounts/codex/active");
    let revoked_home = home.path().join("accounts/codex/revoked");
    std::fs::create_dir_all(&active_home).expect("active home");
    std::fs::create_dir_all(&revoked_home).expect("revoked home");
    let codex = r#"#!/bin/sh
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r limits
case "$CODEX_HOME" in
  */active)
    echo '{"jsonrpc":"2.0","id":2,"result":{"rateLimits":{"planType":"team","primary":{"usedPercent":12,"windowDurationMins":300,"resetsAt":1900000000}}}}';;
  */revoked)
    echo '{"jsonrpc":"2.0","id":2,"error":{"code":401,"message":"token_invalidated"}}';;
  *) exit 9;;
esac
"#;
    let _env = EnvGuard::with_lf_home(&[("codex", codex)], home.path());
    let runtime = tokio::runtime::Runtime::new().expect("store runtime");
    let store = runtime
        .block_on(open_store(&StorageConfig::sqlite(
            home.path().join("loopflow.db"),
        )))
        .expect("account store");
    for seeded in [
        account("active", active_home),
        account("revoked", revoked_home),
    ] {
        runtime
            .block_on(store.upsert_provider_account(&seeded))
            .expect("seed account");
    }

    let cached_output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["auth", "accounts", "codex"])
        .output()
        .expect("read cached auth accounts");
    assert!(cached_output.status.success());
    assert!(String::from_utf8_lossy(&cached_output.stdout)
        .contains("auth: cached connected · live not checked (use --verify)"));

    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["auth", "accounts", "codex", "--verify"])
        .output()
        .expect("run auth verify");

    assert!(
        output.status.success(),
        "lf auth accounts --verify failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("auth: cached connected · live active"));
    assert!(stdout.contains("auth: cached connected · live needs re-login"));
    assert!(stdout.contains("recover: lf auth connect codex revoked"));
    let revoked_id = ProviderAccountId::parse("revoked").expect("revoked id");
    let revoked = runtime
        .block_on(store.get_provider_account("codex", &revoked_id))
        .expect("read revoked account")
        .expect("revoked account");
    assert_eq!(revoked.credential_state, CredentialState::Missing);
}
