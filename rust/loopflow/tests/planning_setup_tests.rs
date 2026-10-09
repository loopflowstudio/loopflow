use std::path::Path;
use std::process::{Command, Output};

use loopflow::id::WaveId;
use loopflow::store::{open_ephemeral_store, StorageConfig};
use loopflow::work::wave::Wave;
use loopflow_test_support::TestRepo;
use serde_json::Value;

fn invoke(repo: &Path, home: &Path, args: &[&str]) -> Output {
    invoke_lf(repo, home, &[&["repo", "planning"][..], args].concat())
}

fn invoke_lf(repo: &Path, home: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (name, _) in std::env::vars_os() {
        let name = name.to_string_lossy();
        if name.starts_with("LF_") || name.starts_with("LOOPFLOW_") {
            command.env_remove(name.as_ref());
        }
    }
    command
        .current_dir(repo)
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

#[test]
fn public_association_recovery_preserves_private_history_without_publication() {
    use loopflow::engine::planning_git::PlanningDestination;
    use loopflow::store::sqlite::SqliteStore;

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let source_home = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for directory in [home.path(), source_home.path()] {
        runtime
            .block_on(open_ephemeral_store(&StorageConfig::sqlite(
                directory.join("loopflow.db"),
            )))
            .unwrap();
    }
    let target = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let source = SqliteStore::new(&source_home.path().join("loopflow.db")).unwrap();
    let repo_path = repo.path().canonicalize().unwrap();
    let scope = repo_path.to_str().unwrap();
    let destination = PlanningDestination::resolve(
        repo.path(),
        "origin",
        "refs/loopflow/planning/shared/fixture",
    )
    .unwrap();
    for store in [&source, &target] {
        store.bind_peer_planning(scope, &destination).unwrap();
    }
    let peer_wave = Wave::new(WaveId::new(), "incoming".into(), scope.into());
    let private_wave = Wave::new(WaveId::new(), "private".into(), scope.into());
    source.create_wave(&peer_wave).unwrap();
    target.create_wave(&private_wave).unwrap();
    let mut snapshot: loopflow::pm::PmSnapshot = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/task_history_planning.json"
    ))
    .unwrap();
    snapshot.items.truncate(1);
    for (store, wave) in [(&source, &peer_wave), (&target, &private_wave)] {
        store
            .put_pm_snapshot(&loopflow::store::PmSnapshotRow {
                wave_id: wave.id().clone(),
                provider: "linear".into(),
                initiative: "initiative".into(),
                synced_at: 42,
                snapshot: snapshot.clone(),
            })
            .unwrap();
    }
    source
        .select_peer_waves(scope, &destination.id(), &[peer_wave.id().clone()])
        .unwrap();
    let incoming = source
        .task_by_issue(&snapshot.items[0].id)
        .unwrap()
        .unwrap();
    let local = target
        .task_by_issue(&snapshot.items[0].id)
        .unwrap()
        .unwrap();
    for title in ["Private losing title", "Private current title"] {
        let current = target.task(&local.id).unwrap().unwrap();
        target
            .edit_task(
                &local.id,
                current.plan.revision,
                &loopflow::pm::PmItemUpdate {
                    name: Some(title.into()),
                    assignee: Some(Some("Maya".into())),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let comment = loopflow::ops::pm::TaskComment {
        id: "private-comment".into(),
        body: "Private direction".into(),
        author: loopflow::ops::pm::TaskCommentAuthor::Person {
            name: Some("Maya".into()),
        },
        created_at: Some("2026-10-09T10:00:00Z".into()),
    };
    target.append_task_comment(&local.id, &comment).unwrap();
    let unrelated = Wave::new(WaveId::new(), "Unrelated private Wave".into(), scope.into());
    target.create_wave(&unrelated).unwrap();
    let journal = source
        .export_peer_planning(scope, &destination.id())
        .unwrap();
    target
        .import_peer_planning(scope, &destination.id(), "fixture", &journal)
        .unwrap();
    for (origin, local_id, provider) in [
        (
            incoming.id.as_str(),
            local.id.as_str(),
            snapshot.items[0].id.as_str(),
        ),
        (
            incoming.project_id.as_str(),
            local.project_id.as_str(),
            snapshot.projects[0].id.as_str(),
        ),
    ] {
        for _ in 0..2 {
            let reply = invoke(
                repo.path(),
                home.path(),
                &[
                    "associate",
                    origin,
                    "--with",
                    local_id,
                    "--linear",
                    provider,
                ],
            );
            assert!(
                reply.status.success(),
                "{}",
                String::from_utf8_lossy(&reply.stderr)
            );
            assert!(String::from_utf8_lossy(&reply.stdout).contains(local_id));
            assert!(String::from_utf8_lossy(&reply.stderr).contains("unverified"));
        }
    }
    let reply = invoke_lf(
        repo.path(),
        home.path(),
        &["task", "status", incoming.id.as_str(), "--json"],
    );
    assert!(
        reply.status.success(),
        "{}",
        String::from_utf8_lossy(&reply.stderr)
    );
    let status: Value = serde_json::from_slice(&reply.stdout).unwrap();
    assert_eq!(status["execution"]["task_id"], local.id.as_str());
    assert!(target.task(&incoming.id).unwrap().is_none());
    assert!(target.project(&incoming.project_id).unwrap().is_none());
    assert_eq!(
        target.task_by_issue(local.id.as_str()).unwrap().unwrap().id,
        local.id
    );
    let exported = target
        .export_peer_planning(scope, &destination.id())
        .unwrap();
    assert!(exported.changes.values().all(|change| ![
        incoming.id.as_str(),
        incoming.project_id.as_str(),
        local.id.as_str(),
        local.project_id.as_str(),
    ]
    .contains(&change.object.id.as_str())));
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let revision = || -> i64 {
        conn.query_row(
            "SELECT revision FROM store_revisions WHERE domain='planning'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    };
    let before = revision();
    let comments_before = target.task_comments(&local.id).unwrap();
    let pending_before = target.pending_task_changes(&local.id).unwrap();
    for _ in 0..2 {
        let status: Value =
            serde_json::from_str(&run(repo.path(), home.path(), &["status", "--json"])).unwrap();
        let plan = &status["destinations"][0];
        assert!(plan["recovery_error"].is_null(), "{plan}");
        let records = plan["records"].as_array().unwrap();
        assert!(!records
            .iter()
            .any(|r| r["object"]["id"] == unrelated.id().as_str()));
        for id in [
            local.id.as_str(),
            local.project_id.as_str(),
            private_wave.id().as_str(),
            &comment.id,
        ] {
            let record = records.iter().find(|r| r["object"]["id"] == id).unwrap();
            assert!(record["destination"].is_null());
        }
        let record = records
            .iter()
            .find(|r| r["object"]["id"] == local.id.as_str())
            .unwrap();
        let values = record["values"].as_array().unwrap();
        let losing = values
            .iter()
            .find(|v| v["value_json"] == "\"Private losing title\"")
            .unwrap();
        assert_eq!(losing["candidate"], false);
        assert!(values
            .iter()
            .any(|v| v["field"] == "planning_assignee" && v["value_json"] == "\"Maya\""));
        let record = records
            .iter()
            .find(|r| r["object"]["id"] == comment.id)
            .unwrap();
        let content = record["values"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["field"] == "content")
            .unwrap();
        assert_eq!(content["author"]["name"], "Maya");
        assert!(content["value_json"]
            .as_str()
            .unwrap()
            .contains("Private direction"));
        assert_eq!(revision(), before);
        assert_eq!(
            target
                .export_peer_planning(scope, &destination.id())
                .unwrap(),
            exported
        );
        assert_eq!(target.task_comments(&local.id).unwrap(), comments_before);
        assert_eq!(
            target.pending_task_changes(&local.id).unwrap(),
            pending_before
        );
    }
    let status = run(repo.path(), home.path(), &["status"]);
    assert!(status.contains("public Git/Linear composition is unverified"));
    assert!(status.contains("exchange and effects remain held"));
    assert!(status.contains("local only; not selected"));
    assert!(status.contains("retained alternative"));
    assert!(status.contains("Private losing title"));
    assert!(status.contains("author: Maya"));
    assert!(status.contains("Private direction"));
    assert!(status.contains(&format!("Retained reference: project {}", local.project_id)));
    assert_eq!(revision(), before);
    let refs = Command::new("git")
        .current_dir(repo.path())
        .args(["ls-remote", "origin", "refs/loopflow/planning/*"])
        .output()
        .unwrap();
    assert!(refs.status.success());
    assert!(
        refs.stdout.is_empty(),
        "recovery inspection must not publish"
    );
}
