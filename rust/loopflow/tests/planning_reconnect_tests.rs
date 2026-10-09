// Linux honors the fixture CA through SSL_CERT_FILE without changing system trust.
#![cfg(target_os = "linux")]

mod support;

use loopflow::store::{CredentialType, ProviderToken};
use loopflow_test_support::TestRepo;
use std::process::Command;
use support::{register_unrun_task, EnvGuard};

#[test]
fn public_watch_exports_peer_born_plans_and_recovers_mapped_receipts() {
    planning_reconnect_fixture("exports");
}

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
    let registered = register_unrun_task(home.path(), repo.path(), "main", &repo.head_sha());
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
    let mut fixture = serde_json::json!({
        "lf": env!("CARGO_BIN_EXE_lf"), "repo": repo.path(), "home": home.path(),
        "issue": registered.task.plan.linear_id.as_ref().unwrap().as_str(), "task": registered.task.id.as_str(),
        "project": project.plan.linear_id.as_ref().unwrap().as_str(), "wave": registered.task.wave_id.as_str(),
    });
    if mode == "exports" {
        fixture["peer"] = import_unprepared_plans(repo.path(), home.path());
    }
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

// Import portable plans before connecting Linear. Mixed-provider Git exchange
// remains disabled; this exercises the common foreground owner after import.
fn import_unprepared_plans(repo: &std::path::Path, home: &std::path::Path) -> serde_json::Value {
    use loopflow::engine::planning_git::PlanningDestination;
    use loopflow::store::sqlite::SqliteStore;

    let source_repo = TestRepo::new();
    let source_home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", source_home.path())
        .env("LF_HOME", source_home.path())
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .current_dir(source_repo.path())
        .args(["task", "create", "--title", "Peer-born task", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let task: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task = task["id"].as_str().unwrap();
    let source = SqliteStore::new(&source_home.path().join("loopflow.db")).unwrap();
    let target = SqliteStore::new(&home.join("loopflow.db")).unwrap();
    let saved = source.task(&task.parse().unwrap()).unwrap().unwrap();
    let source_path = source_repo.path().to_str().unwrap();
    let target_path = repo.to_str().unwrap();
    let binding = PlanningDestination::new(
        source_repo.bare_path().to_str().unwrap(),
        "refs/loopflow/planning/shared/fixture",
    )
    .unwrap();
    source.bind_peer_planning(source_path, &binding).unwrap();
    source
        .select_peer_waves(
            source_path,
            &binding.id(),
            std::slice::from_ref(&saved.wave_id),
        )
        .unwrap();
    target.bind_peer_planning(target_path, &binding).unwrap();
    let snapshot = source
        .export_peer_planning(source_path, &binding.id())
        .unwrap();
    target
        .import_peer_planning(target_path, &binding.id(), "unprepared", &snapshot)
        .unwrap();
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let name: String = db
        .query_row(
            "SELECT name FROM waves WHERE id=?1",
            [saved.wave_id.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    let wave_dir = repo.join("wave").join(name);
    std::fs::create_dir_all(&wave_dir).unwrap();
    std::fs::write(
        wave_dir.join("GOAL.md"),
        format!(
            "---\nid: {}\npm:\n  linear_initiative: initiative-peer\n---\nPeer export fixture\n",
            saved.wave_id
        ),
    )
    .unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM task_creation_intents WHERE task_id=?1",
            [task],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM project_transitions WHERE successor_id=?1",
            [saved.project_id.as_str()],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    serde_json::json!({"task":task,"project":saved.project_id.as_str(),"wave":saved.wave_id.as_str()})
}
