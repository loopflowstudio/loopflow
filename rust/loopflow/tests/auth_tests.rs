use crate::support;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use base64::prelude::*;
use loopflow::store::{
    CredentialState, ProviderAccount, ProviderAccountId, RoutingState, StorageConfig,
};
use support::EnvGuard;

#[test]
fn account_status_preserves_local_evidence_without_provider_executables() {
    let home = tempfile::TempDir::new().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    let seeded = account("local-account", home.path().join("absent-credential"));
    runtime
        .block_on(store.upsert_provider_account(&seeded))
        .unwrap();
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let parent = loopflow_test_support::RunningParent::insert(&home.path().join("loopflow.db"));
    let child = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        parent.adopt(&mut command);
        command
    };
    let inspect = |json: bool, verify: bool| {
        let mut command = child();
        command
            .args(["account", "codex"])
            .env("PATH", "/nonexistent");
        if json {
            command.arg("--json");
        }
        if !verify {
            command.arg("--cached");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "status should retain the local report"
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let cached_json = inspect(true, false);
    let cached_text = inspect(false, false);
    let report: serde_json::Value = serde_json::from_str(&cached_json).unwrap();
    let rows = report["accounts"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["account_id"], "local-account");
    assert_eq!(rows[0]["cached_credential_state"], "missing");
    assert_eq!(rows[1]["scope"], "local");
    assert!(cached_text.contains("local-account · local-account@example.com"));
    assert_eq!(
        runtime
            .block_on(store.list_provider_accounts(None))
            .unwrap(),
        vec![seeded]
    );
    let verified: serde_json::Value = serde_json::from_str(&inspect(true, true)).unwrap();
    assert_eq!(verified["accounts"].as_array().unwrap().len(), 2);

    let (commands, completed): (i64, i64) = db
        .query_row(
            "SELECT count(*),sum(outcome='succeeded' AND completed_at IS NOT NULL) FROM processes
             WHERE parent_lf_process_id=?1",
            [&parent.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (commands, completed),
        (3, 3),
        "each actual auth process remains a process"
    );
    let fixture = loopflow_test_support::TestRepo::new();
    let repository = child()
        .current_dir(fixture.path())
        .args(["sync", "--plan"])
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    assert!(!repository.status.success());
    assert!(
        String::from_utf8_lossy(&repository.stderr).contains("discover the current Git repository")
    );
    let failed: i64 = db
        .query_row(
            "SELECT count(*) FROM processes WHERE outcome='failed' AND completed_at IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        failed, 1,
        "repository discovery failure is the command's observed outcome"
    );
}

fn account(account_id: &str, home: std::path::PathBuf) -> ProviderAccount {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    ProviderAccount {
        provider: "codex".to_string(),
        account_id: ProviderAccountId::parse(account_id).expect("account id"),
        home: Some(home),
        login_email: Some(
            loopflow::profile::EmailAddress::parse(&format!("{account_id}@example.com")).unwrap(),
        ),
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
}

#[test]
fn headless_connect_without_a_saved_profile_names_recovery_without_registering() {
    let home = tempfile::TempDir::new().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();

    for target in [vec!["codex", "fresh@example.com"], vec!["linear"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(["account", "connect"])
            .args(&target)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(!output.status.success());
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains(&format!(
                "No saved Chrome profile. Run lf account connect {} --chrome-profile <profile>",
                target.join(" ")
            )),
            "{diagnostic}"
        );
    }
    runtime.block_on(async {
        assert!(store.list_provider_accounts(None).await.unwrap().is_empty());
        assert!(store.list_access_profiles().await.unwrap().is_empty());
        assert!(store
            .list_auth_browser_profiles(None, None)
            .await
            .unwrap()
            .is_empty());
    });
}

#[test]
fn status_refreshes_by_default_and_cached_preserves_evidence() {
    let home = tempfile::TempDir::new().expect("lf home");
    let active_home = home.path().join("accounts/codex/active");
    let revoked_home = home.path().join("accounts/codex/revoked");
    std::fs::create_dir_all(&active_home).expect("active home");
    std::fs::create_dir_all(&revoked_home).expect("revoked home");
    for (path, email) in [
        (&active_home, "active@example.com"),
        (&revoked_home, "revoked@example.com"),
    ] {
        write_identity(path, email, email);
    }
    let codex = r#"#!/bin/sh
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r initialized
read -r account
case "$CODEX_HOME" in
  */active) echo '{"id":2,"result":{"account":{"email":"active@example.com","planType":"team"}}}';;
  */revoked) echo '{"id":2,"result":{"account":{"email":"revoked@example.com"}}}';;
esac
read -r limits
case "$CODEX_HOME" in
  */active)
    if [ "$LF_TEST_EMPTY_USAGE" = 1 ]; then
      echo '{"jsonrpc":"2.0","id":3,"result":{"rateLimits":{"planType":"team"}}}'
    else
      echo '{"jsonrpc":"2.0","id":3,"result":{"rateLimits":{"planType":"team","primary":{"usedPercent":12,"windowDurationMins":300,"resetsAt":1900000000}}}}'
    fi;;
  */revoked)
    echo '{"jsonrpc":"2.0","id":3,"error":{"code":401,"message":"token_invalidated"}}'
    read -r refresh
    echo '{"id":4,"result":{"account":{"email":"revoked@example.com"}}}'
    read -r limits
    echo '{"id":5,"error":{"code":401,"message":"token_invalidated"}}';;
  *) exit 9;;
esac
"#;
    let _env = EnvGuard::with_lf_home(&[("codex", codex)], home.path());
    let runtime = tokio::runtime::Runtime::new().expect("store runtime");
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .expect("account store");
    for mut seeded in [
        account("active", active_home),
        account("revoked", revoked_home),
    ] {
        seeded.cooldown_until = Some(1900000000);
        seeded.cooldown_reason = Some("weekly".into());
        seeded.routing_state = RoutingState::ExplicitOnly;
        seeded.plan = Some("configured".into());
        runtime
            .block_on(store.upsert_provider_account(&seeded))
            .expect("seed account");
    }

    // Local and managed credentials are separate facts. An unreadable synthetic
    // encrypted token also proves cached inspection never decrypts credentials.
    let connection = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    connection
        .execute(
            "INSERT INTO provider_tokens
         (provider, access_token, expires_at, login, updated_at, credential_type, encrypted)
         VALUES ('codex', 'not-a-real-ciphertext', 1, 'local@example.com', 7, 'oauth', 1)",
            [],
        )
        .unwrap();
    let token_before: (String, i64) = connection
        .query_row(
            "SELECT access_token, updated_at FROM provider_tokens WHERE provider = 'codex'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    runtime
        .block_on(store.upsert_provider_account_limits(
            "codex",
            &ProviderAccountId::parse("active").unwrap(),
            &[
                loopflow::store::AccountLimitWindow {
                    window: "weekly".into(),
                    used_percent: 34,
                    resets_at: None,
                    plan: None,
                },
                loopflow::store::AccountLimitWindow {
                    window: "session".into(),
                    used_percent: 100,
                    resets_at: Some(1),
                    plan: None,
                },
            ],
            "stream",
        ))
        .unwrap();
    let original_windows = runtime
        .block_on(store.provider_account_limits(None))
        .unwrap();
    let old_weekly = original_windows
        .iter()
        .find(|window| window.window == "weekly")
        .unwrap();
    let cached_output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["account", "codex", "--cached"])
        .output()
        .expect("read cached auth accounts");
    assert!(cached_output.status.success());
    let cached_text = String::from_utf8_lossy(&cached_output.stdout);
    assert!(cached_text.contains("auth: cached connected · not checked"));
    assert!(cached_text.contains("session: usage unknown"));
    assert!(!cached_text.contains("100% used"));

    // Failed and incomplete refreshes must not turn an expired full window into
    // either current fullness or invented capacity. Preserve dated observations.
    for (key, value, diagnostic) in [
        ("PATH", "/nonexistent", "verification unavailable"),
        (
            "LF_TEST_EMPTY_USAGE",
            "1",
            "usage response has no recognized percentage windows",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(["account", "codex"])
            .env(key, value)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{key}={value}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("session: usage unknown"));
        assert!(!text.contains("100% used"));
        assert!(!text.contains("0% used"));
        assert!(text.contains(diagnostic));
        assert_eq!(
            runtime
                .block_on(store.provider_account_limits(None))
                .unwrap(),
            original_windows
        );
    }

    // The live report includes persisted credential state and fresh usage.
    let verified_json = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["account", "codex", "--json"])
        .output()
        .unwrap();
    assert!(
        verified_json.status.success(),
        "{}",
        String::from_utf8_lossy(&verified_json.stderr)
    );
    let verified: serde_json::Value = serde_json::from_slice(&verified_json.stdout).unwrap();
    let verified_rows = verified["accounts"].as_array().unwrap();
    let revoked_row = verified_rows
        .iter()
        .find(|r| r["account_id"] == "revoked")
        .unwrap();
    assert_eq!(revoked_row["cached_credential_state"], "missing");
    assert_eq!(revoked_row["verification"], "rejected");
    assert_eq!(
        revoked_row["recovery"],
        "lf account connect codex revoked@example.com"
    );
    let active_row = verified_rows
        .iter()
        .find(|r| r["account_id"] == "active")
        .unwrap();
    assert_eq!(active_row["cached_credential_state"], "connected");
    assert_eq!(active_row["verification"], "accepted");
    assert_eq!(active_row["windows"][0]["used_percent"], 12);
    assert_eq!(active_row["windows"][0]["resets_at"], 1900000000);
    assert_eq!(
        active_row["verified_windows"],
        serde_json::json!(["session"])
    );

    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["account", "codex"])
        .output()
        .expect("refresh auth status");

    assert!(
        output.status.success(),
        "lf account failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("auth: active (verified)"));
    assert!(stdout.contains("auth: needs login (identity or credential rejected)"));
    assert!(stdout.contains("recover: lf account connect codex revoked@example.com"));
    let revoked_id = ProviderAccountId::parse("revoked").expect("revoked id");
    let revoked = runtime
        .block_on(store.get_provider_account("codex", &revoked_id))
        .expect("read revoked account")
        .expect("revoked account");
    assert_eq!(revoked.credential_state, CredentialState::Missing);
    assert_eq!(revoked.cooldown_until, Some(1900000000));
    assert_eq!(revoked.cooldown_reason.as_deref(), Some("weekly"));
    assert_eq!(revoked.routing_state, RoutingState::ExplicitOnly);
    let observed = runtime
        .block_on(store.provider_account_limits(None))
        .unwrap();
    assert_eq!(observed.len(), 2);
    assert_eq!(&observed[1], old_weekly);
    assert!(stdout
        .lines()
        .find(|line| line.contains("weekly: 34%"))
        .unwrap()
        .contains("cached"));
    assert_eq!(observed[0].used_percent, 12);
    assert_eq!(observed[0].resets_at, Some(1900000000));
    assert_eq!(observed[0].plan.as_deref(), Some("team"));
    assert_eq!(observed[0].source, "poll");
    assert!(stdout.contains("session: 12% used, 88% left"));

    let before = runtime
        .block_on(store.list_provider_accounts(None))
        .unwrap();
    // A second process must read stored observations with the provider unavailable.
    let cached = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["account", "codex", "--cached"])
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    assert!(
        cached.status.success(),
        "{}",
        String::from_utf8_lossy(&cached.stderr)
    );
    let cached_text = String::from_utf8_lossy(&cached.stdout);
    assert!(cached_text.contains("session: 12%"));
    assert!(cached_text.contains("cached"));
    assert!(
        cached_text.contains("recover: lf account connect codex revoked@example.com"),
        "cached rejection must retain its recovery command: {cached_text}"
    );
    assert_eq!(
        runtime
            .block_on(store.provider_account_limits(None))
            .unwrap(),
        observed
    );
    assert_eq!(
        runtime
            .block_on(store.list_provider_accounts(None))
            .unwrap(),
        before
    );

    let credential_path = home.path().join("accounts/codex/active/auth.json");
    let credentials_before = std::fs::read(&credential_path).unwrap();
    let cached_json = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["account", "codex", "--cached", "--json"])
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    assert!(
        cached_json.status.success(),
        "{}",
        String::from_utf8_lossy(&cached_json.stderr)
    );
    assert!(!String::from_utf8_lossy(&cached_json.stdout).contains("not-a-real-ciphertext"));
    assert!(!String::from_utf8_lossy(&cached_json.stdout).contains("fixture"));
    let report: serde_json::Value = serde_json::from_slice(&cached_json.stdout).unwrap();
    let rows = report["accounts"].as_array().unwrap();
    let managed = rows.iter().find(|r| r["account_id"] == "active").unwrap();
    let local = rows.iter().find(|r| r["scope"] == "local").unwrap();
    assert_eq!(managed["login"], "active@example.com");
    assert_eq!(managed["cached_credential_state"], "connected");
    assert_eq!(managed["verification"], "not_checked");
    assert_eq!(
        managed["windows"][0]["observed_at"],
        observed[0].observed_at
    );
    assert_eq!(managed["windows"][0]["source"], "poll");
    assert_eq!(local["cached_credential_state"], "expired");
    assert_eq!(local["source"], "stored_token");
    assert!(cached_text.contains("active · active@example.com"));
    assert!(cached_text.contains("Local services"));
    assert!(cached_text.contains("auth: cached expired"));
    assert_eq!(std::fs::read(&credential_path).unwrap(), credentials_before);
    let token_after: (String, i64) = connection
        .query_row(
            "SELECT access_token, updated_at FROM provider_tokens WHERE provider = 'codex'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(token_after, token_before);
    assert_eq!(
        runtime
            .block_on(store.list_provider_accounts(None))
            .unwrap(),
        before
    );

    std::fs::remove_file(home.path().join("accounts/codex/active/auth.json")).unwrap();
    let missing = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["account", "codex", "--cached"])
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    assert!(missing.status.success());
    let missing_text = String::from_utf8_lossy(&missing.stdout);
    assert!(missing_text.contains("auth: cached missing"));
    assert!(!missing_text.contains("cached connected"));
    assert!(missing_text.contains("session: 12%"));
    assert!(missing_text.contains("recover: lf account connect codex active@example.com"));
    assert_eq!(
        runtime
            .block_on(store.provider_account_limits(None))
            .unwrap(),
        observed
    );
    assert_eq!(
        runtime
            .block_on(store.list_provider_accounts(None))
            .unwrap(),
        before
    );
    // An in-progress login is unavailable evidence, not a failure of the report.
    std::fs::write(&credential_path, &credentials_before).unwrap();
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(home.path().join("accounts/codex/.active.login.lock"))
        .unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();
    let busy = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["account", "codex"])
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    assert!(busy.status.success());
    assert!(String::from_utf8_lossy(&busy.stdout).contains("managed credential cannot be locked"));
    assert_eq!(
        runtime
            .block_on(store.provider_account_limits(None))
            .unwrap(),
        observed
    );
    assert_eq!(
        runtime
            .block_on(store.list_provider_accounts(None))
            .unwrap(),
        before
    );
    drop(lock);

    // Accepted credentials without new windows retain older usage as cached.
    runtime
        .block_on(store.update_provider_account_credential_state(
            "codex",
            &ProviderAccountId::parse("active").unwrap(),
            CredentialState::Missing,
        ))
        .unwrap();
    let unknown = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["account", "codex"])
        .env("LF_TEST_EMPTY_USAGE", "1")
        .output()
        .unwrap();
    assert!(unknown.status.success());
    let unknown_text = String::from_utf8_lossy(&unknown.stdout);
    assert!(unknown_text.contains("active (verified)"));
    assert!(!unknown_text.contains("recover: lf account connect codex active@example.com"));
    assert!(unknown_text.contains("observed plan: team"));
    assert!(unknown_text.contains("usage response has no recognized percentage windows"));
    let window_line = unknown_text
        .lines()
        .find(|line| line.contains("session: 12%"))
        .unwrap();
    assert!(window_line.contains("cached"));
    assert!(!window_line.contains("live"));
    assert_eq!(
        runtime
            .block_on(store.provider_account_limits(None))
            .unwrap(),
        observed
    );
    let before_clear = runtime
        .block_on(store.get_provider_account("codex", &ProviderAccountId::parse("active").unwrap()))
        .unwrap()
        .unwrap();
    let cleared = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["account", "set", "codex", "active@", "--clear-cooldown"])
        .output()
        .unwrap();
    assert!(
        cleared.status.success(),
        "{}",
        String::from_utf8_lossy(&cleared.stderr)
    );
    let mut after_clear = runtime
        .block_on(store.get_provider_account("codex", &before_clear.account_id))
        .unwrap()
        .unwrap();
    assert_eq!(after_clear.cooldown_until, None);
    assert_eq!(after_clear.cooldown_reason, None);
    assert_eq!(
        after_clear.login_email.as_ref().unwrap().as_str(),
        "active@example.com"
    );
    after_clear.login_email = before_clear.login_email.clone();
    after_clear.cooldown_until = before_clear.cooldown_until;
    after_clear.cooldown_reason = before_clear.cooldown_reason.clone();
    after_clear.updated_at = before_clear.updated_at;
    assert_eq!(after_clear, before_clear);
    assert_eq!(
        runtime
            .block_on(store.provider_account_limits(None))
            .unwrap(),
        observed
    );
    for args in [
        vec![
            "account",
            "set",
            "codex",
            "active@",
            "--routing",
            "automatic",
        ],
        vec!["set", "account.codex", "active@", "--global"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .current_dir(home.path())
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let before_route = runtime
        .block_on(store.list_provider_accounts(None))
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .current_dir(home.path())
        .args(["account", "route", "--default", "--json"])
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let routes: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let codex = routes
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["provider"] == "codex")
        .unwrap();
    assert_eq!(codex["scope"], "default");
    assert_eq!(codex["candidates"][0]["account_id"], "active");
    assert_eq!(codex["candidates"][0]["login"], "active@example.com");
    assert_eq!(
        runtime
            .block_on(store.list_provider_accounts(None))
            .unwrap(),
        before_route
    );
    let unscoped = Command::new(env!("CARGO_BIN_EXE_lf"))
        .current_dir(home.path())
        .args(["set", "account.codex", "active@"])
        .output()
        .unwrap();
    assert!(!unscoped.status.success());
    assert!(String::from_utf8_lossy(&unscoped.stderr).contains("--global"));
}

#[test]
fn cached_auth_leaves_no_process_or_account_state() {
    let temp = tempfile::tempdir().unwrap();
    let lf_home = temp.path().join("absent");
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .current_dir(temp.path())
        .env_clear()
        .env("LF_HOME", &lf_home)
        .env("PATH", "/nonexistent")
        .args(["account", "--cached", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["scope"] == "local" && r["cached_credential_state"] == "uninspected"));
    let database = rusqlite::Connection::open(lf_home.join("loopflow.db")).unwrap();
    let counts: (i64, i64, i64, i64) = database
        .query_row(
            "SELECT (SELECT count(*) FROM processes),
                    (SELECT count(*) FROM provider_accounts),
                    (SELECT count(*) FROM provider_routes),
                    (SELECT count(*) FROM agent_sessions)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    // Nothing references the finished read, so its exit forgets it.
    assert_eq!(counts, (0, 0, 0, 0));
    assert!(!lf_home.join("accounts").exists());
}

fn write_identity(home: &std::path::Path, email: &str, subject: &str) {
    std::fs::create_dir_all(home).unwrap();
    let claims = BASE64_URL_SAFE_NO_PAD.encode(
        serde_json::json!({"email":email,"sub":subject,"https://api.openai.com/auth":{"chatgpt_account_id":"shared-team"}}).to_string());
    std::fs::write(
        home.join("auth.json"),
        serde_json::json!({"tokens":{
            "access_token":"fixture-token", "id_token":format!("h.{claims}.s")
        }})
        .to_string(),
    )
    .unwrap();
}

#[test]
fn managed_identity_public_status_connect_import_and_relabel() {
    let home = tempfile::tempdir().unwrap();
    let codex = r#"#!/bin/sh
[ "$1" = '-c' ] && [ "$2" = 'cli_auth_credentials_store="file"' ] || exit 19
read -r initialize
echo '{"id":1,"result":{}}'
read -r initialized
read -r account
printf '{"id":2,"result":{"account":{"email":"%s","planType":"pro"}}}\n' "${LF_TEST_SERVER_EMAIL:-engineering@example.com}"
read -r limits || exit 0
echo '{"id":3,"result":{"rateLimits":{}}}'
"#;
    let _env = EnvGuard::with_lf_home(&[("codex", codex)], home.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    let engineering = account(
        "engineering",
        home.path().join("accounts/codex/engineering"),
    );
    let personal = account("personal", home.path().join("accounts/codex/personal"));
    for account in [&engineering, &personal] {
        runtime
            .block_on(store.upsert_provider_account(account))
            .unwrap();
        write_identity(
            account.home.as_ref().unwrap(),
            "personal@example.com",
            "personal-user",
        );
    }
    let status = |verify: bool, server_email: &str| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lf"));
        cmd.args(["account", "codex", "--json"]);
        if !verify {
            cmd.arg("--cached");
            cmd.env("PATH", "/nonexistent");
        }
        let out = cmd
            .env("LF_TEST_SERVER_EMAIL", server_email)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        report["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["scope"] == "managed")
            .cloned()
            .collect::<Vec<_>>()
    };
    for verify in [false, true] {
        let rows = status(verify, "personal@example.com");
        assert!(rows.iter().all(|row| row["verification"] == "rejected"));
        assert_eq!(rows[0]["login"], "personal@example.com");
        assert!(rows[0]["diagnostic"]
            .as_str()
            .unwrap()
            .contains("engineering@example.com"));
        assert!(rows[1]["diagnostic"]
            .as_str()
            .unwrap()
            .contains("share login"));
    }
    // Distinct people in the same workspace are accepted. The server is definitive.
    write_identity(
        engineering.home.as_ref().unwrap(),
        "engineering@example.com",
        "engineering-user",
    );
    let rows = status(true, "wrong@example.com");
    assert_eq!(rows[0]["verification"], "rejected");
    assert!(rows[0]["diagnostic"]
        .as_str()
        .unwrap()
        .contains("wrong@example.com"));
    let rows = status(true, "engineering@example.com");
    assert_eq!(rows[0]["verification"], "accepted");
    assert_eq!(rows[0]["observed_plan"], "pro");
    assert!(rows[0]["verified_windows"].as_array().unwrap().is_empty());
    assert_eq!(status(false, "unused")[0]["observed_plan"], "pro");
    let installed = runtime
        .block_on(store.get_provider_account("codex", &engineering.account_id))
        .unwrap()
        .unwrap();
    assert_eq!(
        installed.observed_subject.as_deref(),
        Some("engineering-user")
    );

    let relabel = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args([
            "account",
            "set",
            "codex",
            "engineering@",
            "--login-email",
            "wrong@example.com",
        ])
        .output()
        .unwrap();
    assert!(!relabel.status.success());
    assert!(String::from_utf8_lossy(&relabel.stderr).contains("cannot relabel"));
    assert_eq!(
        runtime
            .block_on(store.get_provider_account("codex", &engineering.account_id))
            .unwrap()
            .unwrap(),
        installed
    );
    let before = std::fs::read(engineering.home.as_ref().unwrap().join("auth.json")).unwrap();
    let imported = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args([
            "account",
            "connect",
            "codex",
            "engineering@example.com",
            "--import",
        ])
        .output()
        .unwrap();
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    assert_eq!(
        std::fs::read(engineering.home.as_ref().unwrap().join("auth.json")).unwrap(),
        before
    );
    // A second home holding this login blocks import and names both owners.
    write_identity(
        personal.home.as_ref().unwrap(),
        "engineering@example.com",
        "engineering-user",
    );
    let duplicate = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args([
            "account",
            "connect",
            "codex",
            "engineering@example.com",
            "--import",
        ])
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    let error = String::from_utf8_lossy(&duplicate.stderr);
    assert!(
        error.contains("engineering")
            && error.contains("personal")
            && error.contains("share login")
    );
    assert_eq!(
        std::fs::read(engineering.home.as_ref().unwrap().join("auth.json")).unwrap(),
        before
    );
}

#[test]
fn account_read_failure_preserves_credentials_unless_revoked() {
    let home = tempfile::tempdir().unwrap();
    let codex = r#"#!/usr/bin/env python3
import json, os, sys
for line in sys.stdin:
    request = json.loads(line)
    if 'id' not in request:
        continue
    response = {'id': request['id'], 'result': {}}
    if request['method'] == 'account/read':
        response = {'id': request['id'], 'error': {
            'code': int(os.environ['LF_TEST_ACCOUNT_ERROR']), 'message': 'fixture failure'}}
    print(json.dumps(response), flush=True)
"#;
    let _env = EnvGuard::with_lf_home(&[("codex", codex)], home.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    let active = account("active", home.path().join("accounts/codex/active"));
    write_identity(
        active.home.as_ref().unwrap(),
        "active@example.com",
        "active",
    );
    runtime
        .block_on(store.upsert_provider_account(&active))
        .unwrap();

    for (code, verification, state) in [
        (500, "unavailable", CredentialState::Connected),
        (401, "rejected", CredentialState::Missing),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(["account", "codex", "--json"])
            .env("LF_TEST_ACCOUNT_ERROR", code.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let row = report["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["scope"] == "managed")
            .unwrap();
        assert_eq!(row["verification"], verification);
        let saved = runtime
            .block_on(store.get_provider_account("codex", &active.account_id))
            .unwrap()
            .unwrap();
        let mut expected = active.clone();
        expected.credential_state = state;
        expected.updated_at = saved.updated_at;
        assert_eq!(saved, expected);
    }
}

#[test]
fn managed_connect_requires_email_before_starting_browser() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    let mut unlabeled = account("legacy", home.path().join("accounts/codex/legacy"));
    unlabeled.login_email = None;
    runtime
        .block_on(store.upsert_provider_account(&unlabeled))
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["account", "connect", "codex", "legacy"])
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--login-email"));
    assert_eq!(
        runtime
            .block_on(store.get_provider_account("codex", &unlabeled.account_id))
            .unwrap(),
        Some(unlabeled)
    );
}

#[test]
fn banked_resets_require_explicit_redemption_and_reuse_attempt_identity() {
    let home = tempfile::TempDir::new().unwrap();
    let script = r#"#!/bin/sh
while IFS= read -r request; do
  id=$(printf '%s' "$request" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$request" in
    *'"method":"initialize"'*) printf '{"id":%s,"result":{}}\n' "$id";;
    *'"method":"account/read"'*)
      printf '{"id":%s,"result":{"account":{"type":"chatgpt","email":"active@example.com","planType":"pro"}}}\n' "$id";;
    *'"method":"account/rateLimits/read"'*)
      if [ "$consumed" = 1 ] && [ -f "$CODEX_HOME/fail-refresh" ]; then exit 0; fi
      if [ -f "$CODEX_HOME/spent" ]; then used=0; count=1; else used=100; count=2; fi
      printf '{"id":%s,"result":{"rateLimits":{"primary":{"usedPercent":%s,"windowDurationMins":300,"resetsAt":4102444800}},"rateLimitResetCredits":{"availableCount":%s,"credits":[{"id":"fixture-credit","resetType":"codexRateLimits","status":"available","grantedAt":1700000000,"expiresAt":4102444800,"title":null,"description":null}]}}}\n' "$id" "$used" "$count";;
    *'"method":"account/rateLimitResetCredit/consume"'*)
      consumed=1
      key=$(printf '%s' "$request" | sed -n 's/.*"idempotencyKey":"\([^"]*\)".*/\1/p')
      if [ -f "$CODEX_HOME/outcome" ]; then outcome=$(cat "$CODEX_HOME/outcome")
      elif [ -f "$CODEX_HOME/spent" ]; then
        if [ "$(cat "$CODEX_HOME/spent")" = "$key" ]; then outcome=alreadyRedeemed; else outcome=noCredit; fi
      else printf '%s' "$key" > "$CODEX_HOME/spent"; outcome=reset; fi
      if [ -f "$CODEX_HOME/lose-response" ]; then exit 0; fi
      printf '{"id":%s,"result":{"outcome":"%s"}}\n' "$id" "$outcome";;
  esac
done
"#;
    let _env = EnvGuard::with_lf_home(&[("codex", script)], home.path());
    let native = home.path().join("accounts/codex/active");
    std::fs::create_dir_all(&native).unwrap();
    let claims = BASE64_URL_SAFE_NO_PAD
        .encode(serde_json::json!({"email":"active@example.com","sub":"active-user"}).to_string());
    std::fs::write(
        native.join("auth.json"),
        serde_json::json!({"tokens":{"access_token":"fixture","id_token":format!("h.{claims}.s")}})
            .to_string(),
    )
    .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    runtime
        .block_on(store.upsert_provider_account(&account("active", native.clone())))
        .unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let live = run(&["account", "codex", "--json"]);
    assert_eq!(live["accounts"][0]["reset_credits"]["availableCount"], 2);
    assert_eq!(
        live["accounts"][0]["reset_credits"]["credits"][0]["status"],
        "available"
    );
    let cached = run(&["account", "codex", "--cached", "--json"]);
    assert!(cached["accounts"][0]["reset_credits"].is_null());
    assert!(
        !native.join("spent").exists(),
        "status cannot spend a reset"
    );
    let args = [
        "account",
        "redeem-reset",
        "codex",
        "active@",
        "--idempotency-key",
        "fixture-attempt",
        "--json",
    ];
    std::fs::write(native.join("fail-refresh"), "").unwrap();
    let redeemed = run(&args);
    assert_eq!(redeemed["outcome"], "reset");
    assert_eq!(redeemed["before"]["windows"][0]["used_percent"], 100);
    assert!(redeemed["after"].is_null());
    assert!(redeemed["refresh_error"].is_string());
    let text = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(&args[..args.len() - 1])
        .output()
        .unwrap();
    assert!(text.status.success());
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("alreadyRedeemed"));
    assert!(text.contains("Before:\n  session: 0% used, 100% left"));
    assert!(text.contains("After:\n  unknown;"));
    std::fs::remove_file(native.join("fail-refresh")).unwrap();
    let retried = run(&args);
    assert_eq!(retried["outcome"], "alreadyRedeemed");
    assert_eq!(retried["after"]["reset_credits"]["availableCount"], 1);
    assert_eq!(retried["after"]["windows"][0]["used_percent"], 0);
    for outcome in ["nothingToReset", "noCredit"] {
        std::fs::write(native.join("outcome"), outcome).unwrap();
        let report = run(&args);
        assert_eq!(report["outcome"], outcome);
        assert_eq!(report["after"]["reset_credits"]["availableCount"], 1);
    }
    let windows = runtime
        .block_on(store.provider_account_limits(Some("codex")))
        .unwrap();
    assert_eq!(windows[0].used_percent, 0);
    std::fs::remove_file(native.join("spent")).unwrap();
    std::fs::remove_file(native.join("outcome")).unwrap();
    std::fs::write(native.join("lose-response"), "").unwrap();
    let lost = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .output()
        .unwrap();
    assert!(!lost.status.success());
    assert!(String::from_utf8_lossy(&lost.stderr).contains("--idempotency-key fixture-attempt"));
    std::fs::remove_file(native.join("lose-response")).unwrap();
    let recovered = run(&args);
    assert_eq!(recovered["outcome"], "alreadyRedeemed");
    assert_eq!(recovered["after"]["reset_credits"]["availableCount"], 1);
    // Changed native identity refuses the spend even though the service still reports active@.
    let wrong = BASE64_URL_SAFE_NO_PAD
        .encode(serde_json::json!({"email":"wrong@example.com","sub":"wrong-user"}).to_string());
    std::fs::write(
        native.join("auth.json"),
        serde_json::json!({"tokens":{"access_token":"fixture","id_token":format!("h.{wrong}.s")}})
            .to_string(),
    )
    .unwrap();
    let refused = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert_eq!(
        std::fs::read_to_string(native.join("spent")).unwrap(),
        "fixture-attempt"
    );
}

struct CodexClient {
    child: Child,
    messages: Receiver<serde_json::Value>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl CodexClient {
    fn start(home: &std::path::Path, thread: &str) -> Self {
        let mut child = Command::new("codex")
            .arg("app-server")
            .env("CODEX_HOME", home)
            .env("CODEX_FIXTURE_THREAD", thread)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, messages) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let message = serde_json::from_str(&line.unwrap()).unwrap();
                if send.send(message).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            messages,
            reader: Some(reader),
        }
    }

    fn request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let input = self.child.stdin.as_mut().unwrap();
        writeln!(
            input,
            "{}",
            serde_json::json!({"id":1,"method":method,"params":params})
        )
        .unwrap();
        input.flush().unwrap();
        let response = self.messages.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(response["id"], 1);
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }

    fn turn(&mut self, thread: &str) -> serde_json::Value {
        let result = self.request(
            "turn/start",
            serde_json::json!({"threadId":thread,"input":[]}),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let message = self
                .messages
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            if message["method"] == "turn/completed" {
                assert_eq!(message["params"]["threadId"], thread);
                assert_eq!(message["params"]["turn"], result["turn"]);
                return result["turn"].clone();
            }
        }
    }

    fn close(&mut self) {
        drop(self.child.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "{status}");
                break;
            }
            assert!(
                Instant::now() < deadline,
                "fixture did not exit on stdin closure"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        self.reader.take().unwrap().join().unwrap();
    }
}

impl Drop for CodexClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[test]
fn codex_same_login_recovery_status_launch_and_reset() {
    let home = tempfile::tempdir().unwrap();
    let script = support::codex_socket_script(&format!(
        "#!/bin/sh\nexec python3 '{}' \"$@\"\n",
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/codex_access.py")
    ));
    let _env = EnvGuard::with_lf_home(&[("codex", &script)], home.path());
    let repo = loopflow_test_support::TestRepo::new();
    repo.create_file(
        ".lf/config.yaml",
        "agent: codex\ndiff: false\ndiff_files: false\npaste: false\n",
    );
    let profile = home.path().join("accounts/codex/active");
    let native = home.path().join("native");
    for path in [&profile, &native] {
        write_identity(path, "active@example.com", "active");
    }
    let set_token = |path: &std::path::Path, token: &str| {
        let mut auth: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path.join("auth.json")).unwrap()).unwrap();
        auth["tokens"]["access_token"] = token.into();
        auth["expires_at"] = 4102444800_i64.into();
        let bytes = serde_json::to_vec(&auth).unwrap();
        std::fs::write(path.join("auth.json"), &bytes).unwrap();
        bytes
    };
    let good = set_token(&profile, "fixture");
    set_token(&native, "fixture");
    let active_thread = "0199a213-81c0-7800-8aa1-bbab2a035a54";
    let mut client = CodexClient::start(&native, active_thread);
    let active_pid = client.child.id();
    client.request("initialize", serde_json::json!({}));
    let started = client.request("thread/start", serde_json::json!({}));
    assert_eq!(started["thread"]["id"], active_thread);
    let first_turn = client.turn(active_thread);
    let bad = set_token(&native, "rejected");
    std::fs::write(native.join("history.jsonl"), "unrelated native history\n").unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    let selected = account("active", profile.clone());
    runtime
        .block_on(store.upsert_provider_account(&selected))
        .unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .current_dir(repo.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("CODEX_HOME", &native)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    let status = run(&["account", "codex", "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(report["accounts"][0]["verification"], "accepted");
    assert_eq!(std::fs::read(native.join("auth.json")).unwrap(), bad);
    let events = std::fs::read(profile.join("probe-events")).unwrap();
    run(&["account", "codex", "--cached", "--json"]);
    assert_eq!(std::fs::read(profile.join("probe-events")).unwrap(), events);
    run(&[
        "account",
        "redeem-reset",
        "codex",
        "active@",
        "--idempotency-key",
        "one-spend",
        "--json",
    ]);
    assert_eq!(
        std::fs::read_to_string(profile.join("spent")).unwrap(),
        "one-spend\n"
    );
    assert!(!native.join("spent").exists());
    let excluded = home.path().join("accounts/codex/excluded");
    write_identity(&excluded, "excluded@example.com", "excluded");
    let excluded_auth = set_token(&excluded, "fixture");
    runtime
        .block_on(store.upsert_provider_account(&account("excluded", excluded.clone())))
        .unwrap();
    run(&["set", "account.codex", "excluded@", "--global"]);
    run(&["set", "account.codex", "active@"]);
    let routes = || {
        let output = run(&["account", "route", "--json"]);
        let scoped: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let output = run(&["account", "route", "--default", "--json"]);
        let default: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        (scoped, default)
    };
    let prior_routes = routes();
    let codex_route = |report: &serde_json::Value| {
        report
            .as_array()
            .unwrap()
            .iter()
            .find(|route| route["provider"] == "codex")
            .unwrap()
            .clone()
    };
    assert_eq!(
        codex_route(&prior_routes.0)["candidates"][0]["account_id"],
        "active"
    );
    assert_eq!(
        codex_route(&prior_routes.1)["candidates"][0]["account_id"],
        "excluded"
    );
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let stored_routes = || {
        db.prepare(
            "SELECT json_array(scope,scope_id,provider,position,account_id,created_at,updated_at)
                    FROM provider_routes ORDER BY scope,scope_id,provider,position",
        )
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    };
    let prior_stored_routes = stored_routes();
    run(&["-b", ":", "Reply with RECOVERED."]);
    assert_eq!(std::fs::read(native.join("auth.json")).unwrap(), good);
    let (session, thread): (String, String) = db
        .query_row(
            "SELECT id,provider_thread FROM agent_sessions WHERE provider_thread IS NOT NULL",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_ne!(thread, active_thread);
    assert!(client.child.try_wait().unwrap().is_none());
    let second_turn = client.turn(active_thread);
    assert_eq!(client.child.id(), active_pid);
    let retained = client.request(
        "thread/turns/list",
        serde_json::json!({"threadId":active_thread}),
    );
    assert_eq!(
        retained["data"],
        serde_json::json!([first_turn, second_turn])
    );
    client.close();
    let sessions =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let prior = sessions.session(&session).unwrap().unwrap();
    assert!(prior.completed_at.is_some());
    let listing = run(&["history", "list", "--json"]);
    let listing: serde_json::Value = serde_json::from_slice(&listing.stdout).unwrap();
    assert!(!listing["entries"].as_array().unwrap().is_empty());
    let usage = run(&["usage", "--json"]);
    let usage: serde_json::Value = serde_json::from_slice(&usage.stdout).unwrap();
    let captures: Vec<_> = usage
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["session_id"] == session)
        .map(|row| row["captured"].as_i64().unwrap())
        .collect();
    assert!(captures.contains(&prior.captured.unwrap()));
    run(&["doctor", "--json"]);
    let routed: String = db
        .query_row(
            "SELECT account_id FROM provider_session_accounts WHERE provider_session_id=?1",
            [&thread],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(routed, "active");
    let turns: serde_json::Value = serde_json::from_slice(
        &std::fs::read(native.join(format!("turns-{thread}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(turns.as_array().unwrap().len(), 1);
    assert_eq!(
        std::fs::read_to_string(native.join("history.jsonl")).unwrap(),
        "unrelated native history\n"
    );
    assert_eq!(std::fs::read(profile.join("auth.json")).unwrap(), good);
    assert_eq!(routes(), prior_routes);
    assert_eq!(stored_routes(), prior_stored_routes);
    assert_eq!(
        std::fs::read(excluded.join("auth.json")).unwrap(),
        excluded_auth
    );
    assert!(!excluded.join("probe-events").exists());
    let events: Vec<serde_json::Value> = std::fs::read_to_string(native.join("probe-events"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let launches: Vec<_> = events
        .iter()
        .filter(|event| event["method"] == "turn/start")
        .collect();
    assert_eq!(launches.len(), 3);
    for event in &launches {
        assert_eq!(event["home"], native.to_str().unwrap());
        assert_eq!(event["email"], "active@example.com");
    }
    assert_eq!(
        launches
            .iter()
            .filter(|event| event["thread"] == thread)
            .count(),
        1
    );
    assert_eq!(
        launches
            .iter()
            .filter(|event| event["thread"] == active_thread && event["pid"] == active_pid)
            .count(),
        2
    );
}
