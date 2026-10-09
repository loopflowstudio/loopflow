use std::path::Path;
use std::process::{Command, Output};

use loopflow::id::WaveId;
use loopflow::store::{open_ephemeral_store, StorageConfig};
use loopflow::work::wave::Wave;
use loopflow_test_support::TestRepo;
use serde_json::Value;

fn invoke(repo: &Path, home: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (name, _) in std::env::vars_os() {
        let name = name.to_string_lossy();
        if name.starts_with("LF_") || name.starts_with("LOOPFLOW_") {
            command.env_remove(name.as_ref());
        }
    }
    command
        .current_dir(repo)
        .args(["repo", "planning"])
        .args(args)
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .output()
        .unwrap()
}

fn run(repo: &Path, home: &Path, args: &[&str]) -> String {
    let output = invoke(repo, home, args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

#[test]
fn public_setup_recovers_user_identity_and_selects_without_publishing() {
    let repo = TestRepo::new();
    let other = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let other_home = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            home.path().join("loopflow.db"),
        )))
        .unwrap();
    let _other_store = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            other_home.path().join("loopflow.db"),
        )))
        .unwrap();
    let scope = repo.path().canonicalize().unwrap();
    let scope = scope.to_str().unwrap();
    let private = Wave::new(WaveId::new(), "private".into(), scope.into());
    runtime.block_on(store.create_wave(&private)).unwrap();
    // Connect cannot silently generate a different person on each machine.
    assert!(
        !invoke(repo.path(), home.path(), &["connect", "--remote", "origin"])
            .status
            .success()
    );
    let key = run(repo.path(), home.path(), &["key", "--new"]);
    assert!(uuid::Uuid::parse_str(&key).is_ok());
    assert_eq!(run(repo.path(), home.path(), &["key", "--new"]), key);
    assert_eq!(
        run(other.path(), other_home.path(), &["key", "--recover", &key]),
        key
    );
    let destination = run(repo.path(), home.path(), &["connect", "--remote", "origin"]);
    let status: Value =
        serde_json::from_str(&run(repo.path(), home.path(), &["status", "--json"])).unwrap();
    assert_eq!(status["destinations"][0]["selected_records"], 0);
    assert_eq!(status["destinations"][0]["active"], false);
    assert_eq!(
        status["destinations"][0]["reference"],
        format!("refs/loopflow/planning/users/{key}")
    );
    assert!(status["destinations"][0].get("endpoint").is_none());
    run(repo.path(), home.path(), &["use", &destination]);
    let selected = Wave::new(WaveId::new(), "selected".into(), scope.into());
    runtime.block_on(store.create_wave(&selected)).unwrap();
    run(repo.path(), home.path(), &["use", "local"]);
    let local = Wave::new(WaveId::new(), "local".into(), scope.into());
    runtime.block_on(store.create_wave(&local)).unwrap();
    // A multi-Wave error rolls back the entire explicit selection.
    assert!(!invoke(
        repo.path(),
        home.path(),
        &[
            "select",
            &destination,
            "--wave",
            private.id().as_str(),
            WaveId::new().as_str()
        ]
    )
    .status
    .success());
    let status: Value =
        serde_json::from_str(&run(repo.path(), home.path(), &["status", "--json"])).unwrap();
    assert_eq!(status["destinations"][0]["selected_records"], 1);
    runtime
        .block_on(store.update_wave(&selected.clone().with_parent(private.id().clone())))
        .unwrap();
    let held: Value =
        serde_json::from_str(&run(repo.path(), home.path(), &["status", "--json"])).unwrap();
    assert!(held["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|conflict| conflict["object"]["id"] == selected.id().as_str()));
    assert!(run(repo.path(), home.path(), &["status"]).contains("Held wave"));
    run(
        repo.path(),
        home.path(),
        &["select", &destination, "--wave", private.id().as_str()],
    );
    let status: Value =
        serde_json::from_str(&run(repo.path(), home.path(), &["status", "--json"])).unwrap();
    assert_eq!(status["destinations"][0]["selected_records"], 2);
    assert!(status["conflicts"].as_array().unwrap().is_empty());
    assert!(status["destinations"][0]["imported_revision"].is_null());
    let refs = Command::new("git")
        .current_dir(repo.path())
        .args(["ls-remote", "origin", "refs/loopflow/planning/*"])
        .output()
        .unwrap();
    assert!(refs.status.success());
    assert!(refs.stdout.is_empty(), "setup must not publish");
}

#[test]
fn public_shared_join_needs_no_user_key_and_does_not_enroll_existing_work() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            home.path().join("loopflow.db"),
        )))
        .unwrap();
    let wave = Wave::new(
        WaveId::new(),
        "private".into(),
        repo.path()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
    );
    runtime.block_on(store.create_wave(&wave)).unwrap();
    run(
        repo.path(),
        home.path(),
        &["connect", "--remote", "origin", "--shared", "team"],
    );
    let status: Value =
        serde_json::from_str(&run(repo.path(), home.path(), &["status", "--json"])).unwrap();
    assert_eq!(status["destinations"][0]["selected_records"], 0);
    assert_eq!(
        status["destinations"][0]["reference"],
        "refs/loopflow/planning/shared/team"
    );
    assert!(runtime
        .block_on(store.planning_user_key())
        .unwrap()
        .is_none());
}
