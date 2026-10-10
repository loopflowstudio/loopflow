// Linux honors the fixture CA through SSL_CERT_FILE without changing system trust.
#![cfg(target_os = "linux")]

mod support;

use loopflow::store::{CredentialType, ProviderToken};
use loopflow_test_support::TestRepo;
use std::process::Command;
use support::{register_task_without_pr, EnvGuard};

#[test]
fn work_watch_reconnects_repository_planning() {
    planning_reconnect_fixture("watch");
}

#[test]
fn public_flow_reconnects_planning_without_another_turn() {
    planning_reconnect_fixture("flow");
}

fn planning_reconnect_fixture(mode: &str) {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let key = home.path().join("provider.key");
    std::fs::write(&key, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
    let previous_key = std::env::var_os("LF_PROVIDER_TOKEN_KEY_PATH");
    std::env::set_var("LF_PROVIDER_TOKEN_KEY_PATH", &key);
    runtime
        .block_on(registered.store.upsert_provider_token(&ProviderToken {
            provider: "linear".into(),
            access_token: "synthetic-planning-token".into(),
            refresh_token: None,
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: 1,
            credential_type: CredentialType::OAuth,
        }))
        .unwrap();
    match previous_key {
        Some(value) => std::env::set_var("LF_PROVIDER_TOKEN_KEY_PATH", value),
        None => std::env::remove_var("LF_PROVIDER_TOKEN_KEY_PATH"),
    }
    let project = runtime
        .block_on(registered.store.get_project(&registered.task.project_id))
        .unwrap()
        .unwrap();
    let fixture = serde_json::json!({
        "lf": env!("CARGO_BIN_EXE_lf"), "repo": repo.path(), "home": home.path(),
        "issue": registered.task.plan.linear_id.as_ref().unwrap().as_str(), "task": registered.task.id.as_str(),
        "project": project.plan.linear_id.as_ref().unwrap().as_str(), "wave": registered.task.wave_id.as_str(),
    });
    let input = home.path().join("fixture.json");
    std::fs::write(&input, serde_json::to_vec(&fixture).unwrap()).unwrap();
    let output = Command::new("uv")
        .args(["run", "python"])
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/e2e/planning_reconnect.py"),
        )
        .arg(input)
        .arg(mode)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
