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
    assert!(held["destinations"][0]["conflicts"]
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
    assert!(status["destinations"][0]["conflicts"]
        .as_array()
        .unwrap()
        .is_empty());
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

#[test]
fn public_status_keeps_healthy_plans_and_receipts_visible_beside_a_damaged_journal() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let database = home.path().join("loopflow.db");
    let store = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            database.clone(),
        )))
        .unwrap();
    let root = repo.path().canonicalize().unwrap();
    let scope = root.to_str().unwrap();
    let private = Wave::new(WaveId::new(), "private".into(), scope.into());
    runtime.block_on(store.create_wave(&private)).unwrap();
    let mut plans = Vec::new();
    for name in ["damaged", "healthy"] {
        let id = run(
            repo.path(),
            home.path(),
            &["connect", "--remote", "origin", "--shared", name],
        );
        run(repo.path(), home.path(), &["use", &id]);
        let wave = Wave::new(WaveId::new(), name.into(), scope.into());
        runtime.block_on(store.create_wave(&wave)).unwrap();
        plans.push((id, wave));
    }
    // A healthy destination's private-parent hold must still be shown.
    runtime
        .block_on(store.update_wave(&plans[1].1.clone().with_parent(private.id().clone())))
        .unwrap();
    let conn = rusqlite::Connection::open(database).unwrap();
    conn.execute(
        "INSERT INTO planning_peer_conflicts(repo,kind,object_id,reason,active)
        VALUES(?1,'wave',?2,'retained projection conflict',1)",
        rusqlite::params![scope, plans[0].1.id()],
    )
    .unwrap();
    conn.execute("UPDATE planning_destinations SET fetched_revision='fetched',publication_revision='attempted',
        publication_state='unconfirmed',publication_error='reply lost' WHERE repo=?1 AND id=?2",
        rusqlite::params![scope, plans[0].0]).unwrap();
    conn.execute(
        "INSERT INTO planning_peer_imports(repo,destination,revision) VALUES(?1,?2,'imported')",
        rusqlite::params![scope, plans[0].0],
    )
    .unwrap();
    let heads: Vec<String> = conn
        .prepare("SELECT id FROM planning_peer_heads WHERE object_id=?1")
        .unwrap()
        .query_map([plans[0].1.id()], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    conn.execute(
        "DELETE FROM planning_peer_heads WHERE object_id=?1",
        [plans[0].1.id()],
    )
    .unwrap();
    let planning_revision = || -> i64 {
        conn.query_row(
            "SELECT revision FROM store_revisions WHERE domain='planning'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    };
    let before = planning_revision();
    let read = || -> Value {
        serde_json::from_str(&run(repo.path(), home.path(), &["status", "--json"])).unwrap()
    };
    let status = read();
    let destinations = status["destinations"].as_array().unwrap();
    assert_eq!(destinations.len(), 2);
    let damaged = destinations.iter().find(|d| d["id"] == plans[0].0).unwrap();
    assert!(damaged["pending_local"].is_null());
    assert!(damaged["local_error"].as_str().unwrap().contains("unknown"));
    assert_eq!(damaged["publication_state"], "unconfirmed");
    assert_eq!(damaged["publication_revision"], "attempted");
    assert_eq!(damaged["publication_error"], "reply lost");
    assert_eq!(damaged["fetched_revision"], "fetched");
    assert_eq!(damaged["imported_revision"], "imported");
    assert_eq!(
        damaged["conflicts"][0]["reason"],
        "retained projection conflict"
    );
    let healthy = destinations.iter().find(|d| d["id"] == plans[1].0).unwrap();
    assert!(healthy["local_error"].is_null());
    assert_eq!(healthy["pending_local"], false); // All its history is held, not lost.
    assert_eq!(
        healthy["conflicts"][0]["object"]["id"],
        plans[1].1.id().as_str()
    );
    let text = run(repo.path(), home.path(), &["status"]);
    assert!(text.contains("local changes unknown"));
    assert!(text.contains("retained projection conflict"));
    assert!(text.contains(&format!("Held wave {}", plans[1].1.id())));
    assert_eq!(read(), status);
    assert_eq!(planning_revision(), before);
    // Status does not repair a damaged journal. Restoring this fixture's exact
    // index makes its current local state readable without changing its receipts.
    for id in heads {
        conn.execute(
            "INSERT INTO planning_peer_heads(id,kind,object_id,field)
            SELECT id,kind,object_id,field FROM planning_peer_changes WHERE id=?1",
            [&id],
        )
        .unwrap();
    }
    let recovered = read();
    let recovered = recovered["destinations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == plans[0].0)
        .unwrap();
    assert_eq!(recovered["pending_local"], true);
    assert!(recovered["local_error"].is_null());
    assert_eq!(recovered["publication_state"], "unconfirmed");
}
