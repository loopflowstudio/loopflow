mod support;

use std::process::Command;

use base64::Engine;
use loopflow::store::{
    CredentialState, ProviderAccount, ProviderAccountId, RoutingState, StorageConfig,
};
use support::EnvGuard;

#[test]
fn cached_status_keeps_local_evidence_without_contacting_the_inherited_broker() {
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
    // A listening socket records any attempted broker request without serving
    // metadata or credentials. Cached inspection must not even connect.
    let socket = home.path().join("broker.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let handle = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({
            "socket": socket,
            "secret": "disposable-broker-secret"
        }))
        .unwrap(),
    );
    let inspect = |json: bool, verify: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        command
            .args(["auth", "status", "codex"])
            .env("LF_ACCOUNT_LEASE", &handle)
            .env("PATH", "/nonexistent");
        if json {
            command.arg("--json");
        }
        if verify {
            command.arg("--verify");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "status should retain the local report"
        );
        for bytes in [&output.stdout, &output.stderr] {
            let text = String::from_utf8_lossy(bytes);
            assert!(!text.contains(&handle));
            assert!(!text.contains("disposable-broker-secret"));
            assert!(!text.contains(socket.to_str().unwrap()));
        }
        String::from_utf8(output.stdout).unwrap()
    };
    let connected_json = inspect(true, false);
    let connected_text = inspect(false, false);
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    drop(listener);
    std::fs::remove_file(&socket).unwrap();
    assert_eq!(inspect(true, false), connected_json);
    assert_eq!(inspect(false, false), connected_text);
    let report: serde_json::Value = serde_json::from_str(&connected_json).unwrap();
    assert!(report["forwarded_accounts_diagnostic"]
        .as_str()
        .unwrap()
        .contains("uninspected"));
    let rows = report["accounts"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["account_id"], "local-account");
    assert_eq!(rows[0]["cached_credential_state"], "missing");
    assert_eq!(rows[1]["scope"], "local");
    assert!(connected_text.contains("local-account · local-account@example.com"));
    assert!(connected_text.contains("forwarded account identities uninspected"));
    assert_eq!(
        runtime
            .block_on(store.list_provider_accounts(None))
            .unwrap(),
        vec![seeded]
    );
    // Explicit verification can inspect metadata, but a disconnected origin
    // still cannot suppress the local report or expose the handle in an error.
    let verified: serde_json::Value = serde_json::from_str(&inspect(true, true)).unwrap();
    assert!(verified["forwarded_accounts_diagnostic"]
        .as_str()
        .unwrap()
        .contains("unavailable"));
    assert_eq!(verified["accounts"].as_array().unwrap().len(), 2);
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
            .args(["auth", "connect"])
            .args(&target)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(!output.status.success());
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains(&format!(
                "No saved Chrome profile. Run lf auth connect {} --chrome-profile <profile>",
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
fn status_verify_separates_cached_state_from_live_provider_state() {
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
    echo '{"jsonrpc":"2.0","id":3,"error":{"code":401,"message":"token_invalidated"}}';;
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
            &[loopflow::store::AccountLimitWindow {
                window: "weekly".into(),
                used_percent: 34,
                resets_at: None,
                plan: None,
            }],
            "stream",
        ))
        .unwrap();
    let old_weekly = runtime
        .block_on(store.provider_account_limits(None))
        .unwrap()[0]
        .clone();
    let cached_output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["auth", "status", "codex"])
        .output()
        .expect("read cached auth accounts");
    assert!(cached_output.status.success());
    assert!(String::from_utf8_lossy(&cached_output.stdout)
        .contains("auth: cached connected · not checked"));

    // The first verification's own JSON reports the persisted credential state,
    // not the seeded pre-verification cache.
    let verified_json = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["auth", "status", "codex", "--verify", "--json"])
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
        "lf auth connect codex revoked@example.com"
    );
    let active_row = verified_rows
        .iter()
        .find(|r| r["account_id"] == "active")
        .unwrap();
    assert_eq!(active_row["cached_credential_state"], "connected");
    assert_eq!(active_row["verification"], "accepted");
    assert_eq!(
        active_row["verified_windows"],
        serde_json::json!(["session"])
    );

    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["auth", "status", "codex", "--verify"])
        .output()
        .expect("run auth verify");

    assert!(
        output.status.success(),
        "lf auth status --verify failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("auth: active (verified)"));
    assert!(stdout.contains("auth: needs login (identity or credential rejected)"));
    assert!(stdout.contains("recover: lf auth connect codex revoked@example.com"));
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
    assert_eq!(observed[1], old_weekly);
    assert!(stdout
        .lines()
        .find(|line| line.contains("weekly: 34%"))
        .unwrap()
        .contains("cached"));
    assert_eq!(observed[0].used_percent, 12);
    assert_eq!(observed[0].resets_at, Some(1900000000));
    assert_eq!(observed[0].plan.as_deref(), Some("team"));
    assert_eq!(observed[0].source, "poll");
    assert!(stdout.contains("session: 12%"));

    let before = runtime
        .block_on(store.list_provider_accounts(None))
        .unwrap();
    // A second process must read stored observations with the provider unavailable.
    let cached = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["auth", "status", "codex"])
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
        cached_text.contains("recover: lf auth connect codex revoked@example.com"),
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
        .args(["auth", "status", "codex", "--json"])
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
        .args(["auth", "status", "codex"])
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    assert!(missing.status.success());
    let missing_text = String::from_utf8_lossy(&missing.stdout);
    assert!(missing_text.contains("auth: cached missing"));
    assert!(!missing_text.contains("cached connected"));
    assert!(missing_text.contains("session: 12%"));
    assert!(missing_text.contains("recover: lf auth connect codex active@example.com"));
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
        .args(["auth", "status", "codex", "--verify"])
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
        .args(["auth", "status", "codex", "--verify"])
        .env("LF_TEST_EMPTY_USAGE", "1")
        .output()
        .unwrap();
    assert!(unknown.status.success());
    let unknown_text = String::from_utf8_lossy(&unknown.stdout);
    assert!(unknown_text.contains("active (verified)"));
    assert!(!unknown_text.contains("recover: lf auth connect codex active@example.com"));
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
        .args(["auth", "set", "codex", "active@", "--clear-cooldown"])
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
        vec!["auth", "set", "codex", "active@", "--routing", "automatic"],
        vec!["auth", "route", "set", "codex", "active@", "--default"],
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
        .args(["auth", "route", "show", "--default", "--json"])
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
        .args(["auth", "route", "set", "codex", "active@"])
        .output()
        .unwrap();
    assert!(!unscoped.status.success());
    assert!(String::from_utf8_lossy(&unscoped.stderr).contains("--default"));
}

#[test]
fn cached_auth_records_its_exec_without_creating_account_state() {
    let temp = tempfile::tempdir().unwrap();
    let lf_home = temp.path().join("absent");
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .current_dir(temp.path())
        .env_clear()
        .env("LF_HOME", &lf_home)
        .env("LF_DB_PATH", lf_home.join("loopflow.db"))
        .env("PATH", "/nonexistent")
        .args(["auth", "status", "--json"])
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
            "SELECT (SELECT count(*) FROM execs WHERE outcome='succeeded'),
                    (SELECT count(*) FROM provider_accounts),
                    (SELECT count(*) FROM provider_routes),
                    (SELECT count(*) FROM agent_sessions)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(counts, (1, 0, 0, 0));
    assert!(!lf_home.join("accounts").exists());
}

fn write_identity(home: &std::path::Path, email: &str, subject: &str) {
    std::fs::create_dir_all(home).unwrap();
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
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
        cmd.args(["auth", "status", "codex", "--json"]);
        if verify {
            cmd.arg("--verify");
        } else {
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
            "auth",
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
            "auth",
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
            "auth",
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
    let codex = r#"#!/bin/sh
read -r initialize
echo '{"id":1,"result":{}}'
read -r initialized
read -r account
printf '{"id":2,"error":{"code":%s,"message":"fixture failure"}}\n' "$LF_TEST_ACCOUNT_ERROR"
read -r limits
echo '{"id":3,"result":{"rateLimits":{}}}'
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
            .args(["auth", "status", "codex", "--verify", "--json"])
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
        .args(["auth", "connect", "codex", "legacy"])
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
