// Linux honors the fixture CA through SSL_CERT_FILE without changing system trust.
#![cfg(target_os = "linux")]

mod support;

use loopflow::store::{CredentialType, ProviderToken};
use loopflow::work::task::{GithubPr, PrPublication};
use loopflow_test_support::TestRepo;
use std::process::Command;
use support::{register_task_with_pr, EnvGuard};

#[test]
fn task_abandon_binary_preserves_unresolved_execution() {
    task_management_fixture();
}

fn task_management_fixture() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    let mut registered = register_task_with_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    registered.pr.publication = Some(PrPublication {
        requested_at: time::OffsetDateTime::now_utc(),
        presentation: None,
        github: Some(GithubPr {
            number: 1,
            url: "https://github.com/loopflowstudio/fixture/pull/1".into(),
            head_sha: None,
        }),
        merge: None,
    });
    runtime
        .block_on(registered.store.update_task_pr(&registered.pr))
        .unwrap();
    let key = home.path().join("provider.key");
    std::fs::write(&key, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
    let previous_key = std::env::var_os("LF_PROVIDER_TOKEN_KEY_PATH");
    std::env::set_var("LF_PROVIDER_TOKEN_KEY_PATH", &key);
    runtime
        .block_on(registered.store.upsert_provider_token(&ProviderToken {
            provider: "linear".into(),
            access_token: "synthetic-deletion-token".into(),
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
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/e2e/task_deletion.py"
        ))
        .arg(input)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
