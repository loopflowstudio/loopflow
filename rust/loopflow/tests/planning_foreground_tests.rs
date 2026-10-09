mod support;

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use loopflow::engine::planning_git::{PlanningDestination, PlanningGit, PlanningPublication};
use loopflow::store::{open_ephemeral_store, PeerPlanningStatus, StorageConfig, Store};
use loopflow_test_support::TestRepo;
use rusqlite::{params, Connection};
use serde_json::json;

fn command(repo: &Path, home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .env_clear()
        .current_dir(repo)
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    command
}

struct Watch(Child);
impl Watch {
    fn start(repo: &Path, home: &Path) -> Self {
        let output = File::create(home.join("watch.jsonl")).unwrap();
        let errors = File::create(home.join("watch.stderr")).unwrap();
        let mut child = command(repo, home)
            .args(["monitor", "work", "--watch", "--json"])
            .stdin(Stdio::piped())
            .stdout(output)
            .stderr(errors)
            .spawn()
            .unwrap();
        writeln!(
            child.stdin.as_mut().unwrap(),
            "{}",
            json!({
                "action":"scope", "id":1, "repo":repo, "wave":null, "task":null,
                "headless":true, "activity":null,
            })
        )
        .unwrap();
        Self(child)
    }
}
impl Drop for Watch {
    fn drop(&mut self) {
        self.0.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.0.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                self.0.kill().unwrap();
                self.0.wait().unwrap();
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "foreground exchange did not settle"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn status(runtime: &tokio::runtime::Runtime, store: &Store, repo: &str) -> PeerPlanningStatus {
    runtime
        .block_on(store.peer_planning_status(repo))
        .unwrap()
        .remove(0)
}

fn execution(conn: &Connection) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    [
        "agent_sessions",
        "processes",
        "task_workflows",
        "task_workflow_moves",
        "task_prs",
    ]
    .into_iter()
    .map(|table| {
        let predicate = if table == "processes" {
            " WHERE lfid='00000000-0000-4000-8000-000000000001'"
        } else {
            ""
        };
        let mut query = conn
            .prepare(&format!("SELECT * FROM {table}{predicate} ORDER BY rowid"))
            .unwrap();
        let columns = query.column_count();
        query
            .query_map([], |row| (0..columns).map(|i| row.get(i)).collect())
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    })
    .collect()
}

#[test]
fn public_work_connections_exchange_offline_edits_without_replaying_execution() {
    let repo = TestRepo::new();
    repo.push();
    let other = TestRepo::new();
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    let source = repo.path().canonicalize().unwrap();
    let target = other.path().canonicalize().unwrap();
    let source_key = source.to_str().unwrap();
    let target_key = target.to_str().unwrap();
    let fixture = support::register_unrun_task(left.path(), &source, "main", &repo.head_sha());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let worker = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            right.path().join("loopflow.db"),
        )))
        .unwrap();
    let binding =
        PlanningDestination::resolve(&source, "origin", "refs/loopflow/planning/shared/fixture")
            .unwrap();
    runtime.block_on(async {
        fixture
            .store
            .bind_peer_planning(source_key, &binding)
            .await
            .unwrap();
        fixture
            .store
            .select_peer_waves(
                source_key,
                &binding.id(),
                std::slice::from_ref(&fixture.task.wave_id),
            )
            .await
            .unwrap();
        worker
            .bind_peer_planning(target_key, &binding)
            .await
            .unwrap();
    });
    let conn = Connection::open(left.path().join("loopflow.db")).unwrap();
    conn.busy_timeout(Duration::from_secs(5)).unwrap();
    conn.execute("INSERT INTO processes(lfid,trace_id,started_at) VALUES('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000002',1)", []).unwrap();
    conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,interactive,input_published,cwd,task_id,wave_id)
        VALUES('retained-session','Retained','human',1,1,1,?1,?2,?3)",
        params![source_key,fixture.task.id.as_str(),fixture.task.wave_id.as_str()]).unwrap();
    let graph = json!({"name":"review","nodes":[{"name":"review","skill":"review","description":null}],"edges":[]});
    conn.execute(
        "INSERT INTO task_workflows(task_id,graph,node,updated_at) VALUES(?1,?2,'review',1)",
        params![fixture.task.id.as_str(), graph.to_string()],
    )
    .unwrap();
    let before = execution(&conn);
    let placement = fixture.task.worktree.clone();
    assert!(status(&runtime, &fixture.store, source_key).pending_local);
    let source_watch = Watch::start(&source, left.path());
    let worker_watch = Watch::start(&target, right.path());
    wait_for(|| {
        runtime
            .block_on(worker.get_task(&fixture.task.id))
            .unwrap()
            .is_some()
    });
    let received = runtime
        .block_on(worker.get_task(&fixture.task.id))
        .unwrap()
        .unwrap();
    assert!(received.worktree.is_none());
    assert!(runtime
        .block_on(worker.task_prs(&fixture.task.id))
        .unwrap()
        .is_empty());
    wait_for(|| {
        status(&runtime, &fixture.store, source_key)
            .publication_state
            .as_deref()
            == Some("confirmed")
    });
    let received_status = status(&runtime, &worker, target_key);
    assert!(received_status.fetched_revision.is_some());
    assert!(received_status.imported_revision.is_some());

    // The remote is unavailable; neither side loses its local write or starts a turn.
    let remote = Path::new(binding.endpoint());
    let disconnected = remote.with_extension("disconnected");
    fs::rename(remote, &disconnected).unwrap();
    let worker_conn = Connection::open(right.path().join("loopflow.db")).unwrap();
    worker_conn.busy_timeout(Duration::from_secs(5)).unwrap();
    conn.execute(
        "UPDATE tasks SET issue_description='Written offline on source' WHERE id=?1",
        [fixture.task.id.as_str()],
    )
    .unwrap();
    worker_conn.execute("UPDATE tasks SET issue_title='Written offline on worker',planning_completed=1,planning_state='completed' WHERE id=?1", [fixture.task.id.as_str()]).unwrap();
    worker_conn.execute("INSERT INTO task_comments(id,task_id,body,author,created_at) VALUES('peer-comment',?1,'Keep the running conversation',?2,'2026-10-08T12:00:00Z')",
        params![fixture.task.id.as_str(),json!({"kind":"person","name":"Maya"}).to_string()]).unwrap();
    wait_for(|| {
        status(&runtime, &worker, target_key)
            .acquisition_error
            .is_some()
    });
    assert!(status(&runtime, &worker, target_key).pending_local);
    assert!(status(&runtime, &fixture.store, source_key).pending_local);
    let lock_path = left
        .path()
        .join("locks/planning-peers")
        .join(format!("{}.lock", binding.id()));
    let effect_lock = File::options()
        .read(true)
        .write(true)
        .open(lock_path)
        .unwrap();
    fs2::FileExt::lock_exclusive(&effect_lock).unwrap();
    fs::rename(&disconnected, remote).unwrap();
    // Acquisition must proceed even while this machine cannot publish.
    wait_for(|| {
        runtime
            .block_on(fixture.store.get_task(&fixture.task.id))
            .unwrap()
            .unwrap()
            .plan
            .title
            == "Written offline on worker"
    });
    drop(effect_lock);
    wait_for(|| {
        runtime
            .block_on(fixture.store.get_task(&fixture.task.id))
            .unwrap()
            .unwrap()
            .plan
            .title
            == "Written offline on worker"
            && runtime
                .block_on(worker.get_task(&fixture.task.id))
                .unwrap()
                .unwrap()
                .plan
                .description
                == "Written offline on source"
    });
    wait_for(|| {
        conn.query_row(
            "SELECT count(*) FROM task_comments WHERE id='peer-comment'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            == 1
    });
    assert!(conn
        .query_row(
            "SELECT planning_completed FROM tasks WHERE id=?1",
            [fixture.task.id.as_str()],
            |r| r.get::<_, bool>(0)
        )
        .unwrap());
    wait_for(|| {
        let a = status(&runtime, &fixture.store, source_key);
        let b = status(&runtime, &worker, target_key);
        !a.pending_local
            && !b.pending_local
            && a.publication_state.as_deref() == Some("confirmed")
            && b.publication_state.as_deref() == Some("confirmed")
    });
    drop(source_watch);
    drop(worker_watch);
    assert_eq!(execution(&conn), before);
    assert_eq!(
        runtime
            .block_on(fixture.store.get_task(&fixture.task.id))
            .unwrap()
            .unwrap()
            .worktree,
        placement
    );
    assert_eq!(runtime.block_on(worker.list_tasks(None)).unwrap().len(), 1);
    let printed = command(&target, right.path())
        .args(["planning", "status", "--json"])
        .output()
        .unwrap();
    assert!(
        printed.status.success(),
        "{}",
        String::from_utf8_lossy(&printed.stderr)
    );
    let printed: serde_json::Value = serde_json::from_slice(&printed.stdout).unwrap();
    assert_eq!(printed["destinations"][0]["publication_state"], "confirmed");
    assert!(printed["destinations"][0].get("endpoint").is_none());
}

#[test]
fn fetched_invalid_document_does_not_claim_import_or_publication() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            home.path().join("loopflow.db"),
        )))
        .unwrap();
    let repo_path = repo.path().canonicalize().unwrap();
    let repo_key = repo_path.to_str().unwrap();
    let binding = PlanningDestination::resolve(
        repo.path(),
        "origin",
        "refs/loopflow/planning/shared/invalid",
    )
    .unwrap();
    runtime
        .block_on(store.bind_peer_planning(repo_key, &binding))
        .unwrap();
    let git = PlanningGit::new(repo.path(), &binding).unwrap();
    let invalid = git.save(b"not planning JSON", None, None).unwrap();
    assert_eq!(
        git.publish(&invalid.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    let watch = Watch::start(&repo_path, home.path());
    wait_for(|| {
        status(&runtime, &store, repo_key)
            .acquisition_error
            .is_some()
    });
    let status = status(&runtime, &store, repo_key);
    assert_eq!(
        status.fetched_revision.as_deref(),
        Some(invalid.revision.as_str())
    );
    assert!(status.imported_revision.is_none());
    assert!(status.publication_revision.is_none());
    drop(watch);
}
