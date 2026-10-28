use crate::support;

use std::fs::{self, File};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::{fs::PermissionsExt, process::CommandExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use loopflow::planning_git::{PlanningDestination, PlanningGit, PlanningPublication};
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
        .env("CLAUDE_CONFIG_DIR", home.join("claude"))
        .env("CODEX_HOME", home.join("codex"))
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    command
}

fn run(repo: &Path, home: &Path, args: &[&str]) -> String {
    let output = command(repo, home)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn create(repo: &Path, home: &Path, wave: &str) -> String {
    let mut args = vec!["task", "create", "--title", "Same title", "--json"];
    if !wave.is_empty() {
        args.extend(["--wave", wave]);
    }
    let output = run(repo, home, &args);
    serde_json::from_str::<serde_json::Value>(&output).unwrap()["id"]
        .as_str()
        .unwrap()
        .into()
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

#[track_caller]
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

// Execution fixtures seed SQL directly. Save through the ordinary writer before
// exercising Git exchange, which publishes only the journal of accepted edits.
fn save_fixture_task(home: &Path, task: &loopflow::work::task::Task) {
    let store = loopflow::store::sqlite::SqliteStore::new(&home.join("loopflow.db")).unwrap();
    let current = store.task(&task.id).unwrap().unwrap();
    store
        .edit_task(
            &current.id,
            current.revision,
            &loopflow::work::task::TaskUpdate {
                title: Some(current.title.clone()),
                ..Default::default()
            },
        )
        .unwrap();
}

fn status(
    runtime: &tokio::runtime::Runtime,
    store: &Store,
    repo: &str,
    binding: &PlanningDestination,
) -> PeerPlanningStatus {
    runtime
        .block_on(store.peer_planning_status(repo, Some(&binding.id())))
        .unwrap()
        .remove(0)
}

fn rows(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Vec<Vec<rusqlite::types::Value>> {
    let mut query = conn.prepare(sql).unwrap();
    let columns = query.column_count();
    query
        .query_map(params, |row| (0..columns).map(|i| row.get(i)).collect())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn execution(conn: &Connection, task: &str) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    [
        "agent_sessions",
        "processes",
        "task_graphs",
        "task_prs",
        "task_ownership",
    ]
    .into_iter()
    .map(|table| {
        let (predicate, identity) = if table == "processes" {
            ("id=?1", "00000000-0000-4000-8000-000000000001")
        } else {
            ("task_id=?1", task)
        };
        rows(
            conn,
            &format!("SELECT * FROM {table} WHERE {predicate} ORDER BY rowid"),
            [identity],
        )
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
    fs::write(left.path().join("config.yaml"), "user:\n  name: Maya\n").unwrap();
    fs::write(right.path().join("config.yaml"), "user:\n  name: Lee\n").unwrap();
    let fixture = support::register_task_with_pr(left.path(), &source, "main", &repo.head_sha());
    save_fixture_task(left.path(), &fixture.task);
    loopflow::store::sqlite::SqliteStore::new(&left.path().join("loopflow.db"))
        .unwrap()
        .ensure_wave_project(source.to_str().unwrap(), "task-pr-tests")
        .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let worker = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            right.path().join("loopflow.db"),
        )))
        .unwrap();
    let binding =
        PlanningDestination::resolve(&source, "origin", "refs/loopflow/planning/shared/fixture")
            .unwrap();
    configure(&source, &binding);
    configure(&target, &binding);
    runtime.block_on(async {
        fixture
            .store
            .bind_peer_planning(source_key, &binding)
            .await
            .unwrap();
        worker
            .bind_peer_planning(target_key, &binding)
            .await
            .unwrap();
    });
    let conn = Connection::open(left.path().join("loopflow.db")).unwrap();
    conn.busy_timeout(Duration::from_secs(5)).unwrap();
    conn.execute("INSERT INTO processes(id,trace_id,started_at) VALUES('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000002',1)", []).unwrap();
    conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,interactive,input_published,cwd,task_id,wave_id)
        VALUES('retained-session','Retained','human',1,1,1,?1,?2,?3)",
        params![source_key,fixture.task.id.as_str(),fixture.task.wave_id.as_ref().unwrap().as_str()]).unwrap();
    let graph = json!({"name":"review","steps":[{"Skill":{"skill":{"name":"review"},"id":null,"interactive":true,"returns":null,"sources":[]}}]});
    conn.execute(
        "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,'review',1)",
        params![fixture.task.id.as_str(), graph.to_string()],
    )
    .unwrap();
    let before = execution(&conn, fixture.task.id.as_str());
    let placement = fixture.task.worktree.clone();
    assert!(status(&runtime, &fixture.store, source_key, &binding).pending_local);
    // Short commands exchange without a watcher, and identical create requests
    // remain different intentions, unlike repeated delivery of the same journal.
    let first = create(&source, left.path(), "task-pr-tests");
    let second = create(&source, left.path(), "task-pr-tests");
    assert_ne!(first, second);
    let cold = run(
        &target,
        right.path(),
        &["task", "comment", &first, "--json"],
    );
    assert!(serde_json::from_str::<serde_json::Value>(&cold).is_ok());
    assert_eq!(
        Connection::open(right.path().join("loopflow.db"))
            .unwrap()
            .query_row(
                "SELECT count(*) FROM task_ownership WHERE written_here=1",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0,
        "cold planning acquisition must not claim the Task"
    );
    let third = create(&target, right.path(), "task-pr-tests");
    assert_ne!(first, third);
    assert_ne!(second, third);
    run(&source, left.path(), &["task", "complete", &first]);
    assert!(runtime
        .block_on(worker.get_task(&first.parse().unwrap()))
        .unwrap()
        .is_some());
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
        status(&runtime, &fixture.store, source_key, &binding)
            .publication_state
            .as_deref()
            == Some("confirmed")
    });
    let received_status = status(&runtime, &worker, target_key, &binding);
    assert!(received_status.fetched_revision.is_some());
    assert!(received_status.imported_revision.is_some());

    // The remote is unavailable; neither side loses its local write or starts a turn.
    let remote = Path::new(binding.endpoint());
    let disconnected = remote.with_extension("disconnected");
    fs::rename(remote, &disconnected).unwrap();
    let worker_conn = Connection::open(right.path().join("loopflow.db")).unwrap();
    worker_conn.busy_timeout(Duration::from_secs(5)).unwrap();
    run(
        &source,
        left.path(),
        &[
            "task",
            "edit",
            fixture.task.id.as_str(),
            "--notes",
            "Written offline on source",
        ],
    );
    run(
        &target,
        right.path(),
        &[
            "task",
            "edit",
            fixture.task.id.as_str(),
            "--title",
            "Written offline on worker",
        ],
    );
    run(
        &target,
        right.path(),
        &[
            "task",
            "comment",
            fixture.task.id.as_str(),
            "Keep the running conversation",
            "--json",
        ],
    );
    run(
        &source,
        left.path(),
        &[
            "task",
            "comment",
            fixture.task.id.as_str(),
            "Saved on source too",
            "--json",
        ],
    );
    run(
        &target,
        right.path(),
        &["task", "complete", fixture.task.id.as_str()],
    );
    wait_for(|| {
        status(&runtime, &worker, target_key, &binding)
            .acquisition_error
            .is_some()
    });
    assert!(status(&runtime, &worker, target_key, &binding).pending_local);
    assert!(status(&runtime, &fixture.store, source_key, &binding).pending_local);
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
            .title
            == "Written offline on worker"
    });
    drop(effect_lock);
    wait_for(|| {
        runtime
            .block_on(fixture.store.get_task(&fixture.task.id))
            .unwrap()
            .unwrap()
            .title
            == "Written offline on worker"
            && runtime
                .block_on(worker.get_task(&fixture.task.id))
                .unwrap()
                .unwrap()
                .description
                == "Written offline on source"
    });
    wait_for(|| {
        conn.query_row(
            "SELECT count(*) FROM task_comments WHERE task_id=?1 AND body LIKE '%Keep the running conversation%'",
            [fixture.task.id.as_str()],
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
        let a = status(&runtime, &fixture.store, source_key, &binding);
        let b = status(&runtime, &worker, target_key, &binding);
        !a.pending_local
            && !b.pending_local
            && a.publication_state.as_deref() == Some("confirmed")
            && b.publication_state.as_deref() == Some("confirmed")
    });
    drop(source_watch);
    drop(worker_watch);
    assert_eq!(execution(&conn, fixture.task.id.as_str()), before);
    assert_eq!(
        runtime
            .block_on(fixture.store.get_task(&fixture.task.id))
            .unwrap()
            .unwrap()
            .worktree,
        placement
    );
    assert_eq!(runtime.block_on(worker.list_tasks(None)).unwrap().len(), 4);
    assert_eq!(
        runtime
            .block_on(fixture.store.list_tasks(None))
            .unwrap()
            .len(),
        4
    );
    for connection in [&conn, &worker_conn] {
        let comments: i64 = connection
            .query_row(
                "SELECT count(*) FROM task_comments WHERE task_id=?1
                 AND body NOT LIKE '%loopflow-ownership:%'",
                [fixture.task.id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            comments, 2,
            "repeated exchange must not duplicate either comment"
        );
    }
    let comments = |connection: &Connection| {
        connection
            .prepare(
                "SELECT id,body,author,created_at FROM task_comments WHERE task_id=?1
                 AND body NOT LIKE '%loopflow-ownership:%' ORDER BY id",
            )
            .unwrap()
            .query_map([fixture.task.id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    let saved = comments(&conn);
    assert_eq!(saved, comments(&worker_conn));
    let mut authors: Vec<_> = saved
        .iter()
        .map(|(_, _, author, _)| {
            serde_json::from_str::<serde_json::Value>(author).unwrap()["name"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    authors.sort();
    assert_eq!(authors, ["Lee", "Maya"]);
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
    configure(repo.path(), &binding);
    let git = PlanningGit::new(repo.path(), &binding).unwrap();
    let invalid = git.save(b"not planning JSON", None, None).unwrap();
    assert_eq!(
        git.publish(&invalid.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    let watch = Watch::start(&repo_path, home.path());
    wait_for(|| {
        status(&runtime, &store, repo_key, &binding)
            .acquisition_error
            .is_some()
    });
    let status = status(&runtime, &store, repo_key, &binding);
    assert_eq!(
        status.fetched_revision.as_deref(),
        Some(invalid.revision.as_str())
    );
    assert!(status.imported_revision.is_none());
    assert!(status.publication_revision.is_none());
    drop(watch);
}

#[cfg(unix)]
struct Agent {
    child: Child,
    home: std::path::PathBuf,
}

#[cfg(unix)]
impl Agent {
    fn start(repo: &Path, home: &Path, interactive: bool) -> Self {
        let bin = home.join("bin");
        fs::create_dir(&bin).unwrap();
        let provider = bin.join("claude");
        fs::write(&provider, include_str!("support/holding_provider.py")).unwrap();
        fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
        let child = command(repo, home)
            .process_group(0)
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("FIXTURE_STARTED", home.join("ready"))
            .env("FIXTURE_STOP", home.join("stop"))
            .env("FIXTURE_LAUNCHES", home.join("launches"))
            .args([if interactive { "-i" } else { "-b" }, "-a", "claude:sonnet"])
            .stdin(Stdio::null())
            .stdout(File::create(home.join("agent.stdout")).unwrap())
            .stderr(File::create(home.join("agent.stderr")).unwrap())
            .spawn()
            .unwrap();
        let mut agent = Self {
            child,
            home: home.to_path_buf(),
        };
        wait_for(|| {
            assert!(
                agent.child.try_wait().unwrap().is_none(),
                "{}",
                fs::read_to_string(home.join("agent.stderr")).unwrap()
            );
            home.join("ready").exists()
        });
        agent
    }

    fn finish(&mut self) {
        fs::write(self.home.join("stop"), "stop").unwrap();
        wait_for(|| self.child.try_wait().unwrap().is_some());
        assert!(
            self.child.wait().unwrap().success(),
            "{}",
            fs::read_to_string(self.home.join("agent.stderr")).unwrap()
        );
        assert_eq!(
            fs::read_to_string(self.home.join("launches")).unwrap(),
            "launch\n"
        );
    }
}

#[cfg(unix)]
impl Drop for Agent {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            // SAFETY: this fixture created and still owns this live process group.
            unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) };
            let _ = self.child.wait();
        }
    }
}

#[test]
#[cfg(unix)]
fn taskless_terminal_and_headless_sessions_keep_planning_live() {
    let source = TestRepo::new();
    let target = TestRepo::new();
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    let source_path = source.path().canonicalize().unwrap();
    let target_path = target.path().canonicalize().unwrap();
    let source_key = source_path.to_str().unwrap();
    let target_key = target_path.to_str().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let a = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            left.path().join("loopflow.db"),
        )))
        .unwrap();
    let b = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            right.path().join("loopflow.db"),
        )))
        .unwrap();
    let binding = PlanningDestination::resolve(
        &source_path,
        "origin",
        "refs/loopflow/planning/shared/taskless",
    )
    .unwrap();
    configure(&source_path, &binding);
    configure(&target_path, &binding);
    // Session worktrees receive the same checked-in repository configuration.
    source.stage_all();
    source.commit("configure shared planning");
    source.push();
    target.stage_all();
    target.commit("configure shared planning");
    target.push();
    // Ordinary foreground Sessions exchange the configured repository plan.
    let mut source_agent = Agent::start(&source_path, left.path(), true);
    let mut target_agent = Agent::start(&target_path, right.path(), false);
    runtime.block_on(async {
        a.bind_peer_planning(source_key, &binding).await.unwrap();
        b.bind_peer_planning(target_key, &binding).await.unwrap();
    });
    let id = create(&source_path, left.path(), "");
    let task_id = id.parse().unwrap();
    // No work-watch, Task placement, explicit Work binding or further turn.
    wait_for(|| runtime.block_on(b.get_task(&task_id)).unwrap().is_some());
    let remote = Path::new(binding.endpoint());
    let disconnected = remote.with_extension("disconnected");
    fs::rename(remote, &disconnected).unwrap();
    run(
        &source_path,
        left.path(),
        &["task", "edit", &id, "--title", "Source offline"],
    );
    run(
        &target_path,
        right.path(),
        &["task", "edit", &id, "--notes", "Target offline"],
    );
    assert!(status(&runtime, &a, source_key, &binding).pending_local);
    assert!(status(&runtime, &b, target_key, &binding).pending_local);
    fs::rename(&disconnected, remote).unwrap();
    wait_for(|| {
        let source = runtime.block_on(a.get_task(&task_id)).unwrap().unwrap();
        let target = runtime.block_on(b.get_task(&task_id)).unwrap().unwrap();
        source.description == "Target offline"
            && target.title == "Source offline"
            && !status(&runtime, &a, source_key, &binding).pending_local
            && !status(&runtime, &b, target_key, &binding).pending_local
    });
    source_agent.finish();
    target_agent.finish();
    for (store, home) in [(&a, left.path()), (&b, right.path())] {
        assert!(runtime
            .block_on(store.get_task(&task_id))
            .unwrap()
            .unwrap()
            .worktree
            .is_none());
        let connection = Connection::open(home.join("loopflow.db")).unwrap();
        let sessions: (i64, i64) = connection
            .query_row(
                "SELECT count(*),count(task_id) FROM agent_sessions",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(sessions, (1, 0));
    }
}

#[test]
fn public_wave_reads_imported_planning_without_placing_or_changing_execution() {
    let repo = TestRepo::new();
    let other = TestRepo::new();
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    let source = repo.path().canonicalize().unwrap();
    let target = other.path().canonicalize().unwrap();
    let fixture = support::register_task_with_pr(left.path(), &source, "main", &repo.head_sha());
    save_fixture_task(left.path(), &fixture.task);
    loopflow::store::sqlite::SqliteStore::new(&left.path().join("loopflow.db"))
        .unwrap()
        .ensure_wave_project(source.to_str().unwrap(), "task-pr-tests")
        .unwrap();
    let source_store =
        loopflow::store::sqlite::SqliteStore::new(&left.path().join("loopflow.db")).unwrap();
    let retained = create(&target, right.path(), "");
    let worker =
        loopflow::store::sqlite::SqliteStore::new(&right.path().join("loopflow.db")).unwrap();
    let binding =
        PlanningDestination::resolve(&source, "origin", "refs/loopflow/planning/shared/fixture")
            .unwrap();
    configure(&source, &binding);
    configure(&target, &binding);
    let source_key = source.to_str().unwrap();
    let target_key = target.to_str().unwrap();
    source_store
        .bind_peer_planning(source_key, &binding)
        .unwrap();
    worker.bind_peer_planning(target_key, &binding).unwrap();
    let snapshot = source_store
        .export_peer_planning(source_key, &binding.id())
        .unwrap();
    worker
        .import_peer_planning(target_key, &binding.id(), "fixture", &snapshot)
        .unwrap();
    assert!(worker.peer_planning_status(target_key, None).unwrap()[0]
        .conflicts
        .is_empty());
    let wave_id = fixture.task.wave_id.as_ref().unwrap().as_str();
    let conn = Connection::open(right.path().join("loopflow.db")).unwrap();
    conn.execute("INSERT INTO processes(id,trace_id,started_at) VALUES('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000002',1)", []).unwrap();
    let retained_task = worker.task(&retained.parse().unwrap()).unwrap().unwrap();
    conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,interactive,input_published,cwd,task_id,wave_id)
        VALUES('retained-session','Retained','human',1,1,1,?1,?2,?3)", params![target_key, retained, retained_task.wave_id.as_ref().map(|wave| wave.as_str())]).unwrap();
    let graph = json!({"name":"review","steps":[{"Skill":{"skill":{"name":"review"},"id":null,"interactive":true,"returns":null,"sources":[]}}]});
    conn.execute(
        "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,'review',1)",
        params![retained, graph.to_string()],
    )
    .unwrap();
    let before = execution(&conn, &retained);
    let imported_before = execution(&conn, fixture.task.id.as_str());
    let ownership = || rows(&conn, "SELECT * FROM task_ownership ORDER BY rowid", []);
    let ownership_before = ownership();
    for _ in 0..2 {
        let detail: serde_json::Value = serde_json::from_str(&run(
            &target,
            right.path(),
            &["wave", "status", wave_id, "--json"],
        ))
        .unwrap();
        assert_eq!(detail["wave"]["id"], wave_id);
        assert!(detail["wave"]["machine"].is_null());
        assert_eq!(detail["wave"]["active_tasks"], 1);
        assert!(detail["tasks"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["runtime"]["work_id"] == fixture.task.id.as_str()));
        let list: serde_json::Value =
            serde_json::from_str(&run(&target, right.path(), &["wave", "list", "--json"])).unwrap();
        let wave = list
            .as_array()
            .unwrap()
            .iter()
            .find(|wave| wave["id"] == wave_id)
            .unwrap();
        assert!(wave["machine"].is_null());
        let roadmap: serde_json::Value = serde_json::from_str(&run(
            &target,
            right.path(),
            &["wave", "show", wave_id, "--json"],
        ))
        .unwrap();
        assert_eq!(roadmap["waves"][0]["wave"]["id"], wave_id);
        assert!(roadmap["waves"][0]["wave"]["machine"].is_null());
        assert!(run(&target, right.path(), &["wave", "status", wave_id]).contains("unplaced"));
        assert!(run(&target, right.path(), &["wave", "list"]).contains("unplaced"));
    }
    assert_eq!(ownership(), ownership_before);
    assert_eq!(execution(&conn, fixture.task.id.as_str()), imported_before);
    assert_eq!(execution(&conn, &retained), before);
}

fn configure(repo: &Path, binding: &PlanningDestination) {
    fs::create_dir_all(repo.join(".lf")).unwrap();
    let shared = binding
        .reference()
        .strip_prefix("refs/loopflow/planning/shared/")
        .unwrap();
    assert!(Command::new("git")
        .current_dir(repo)
        .args(["config", "remote.plans.url", binding.endpoint()])
        .status()
        .unwrap()
        .success());
    fs::write(
        repo.join(".lf/config.yaml"),
        format!("planning:\n  provider: git\n  remote: plans\n  shared: {shared}\n"),
    )
    .unwrap();
}

#[test]
fn planning_status_reads_retained_journal_without_inventing_recovery_records() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let source = repo.path().canonicalize().unwrap();
    let task = create(&source, home.path(), "");
    run(
        &source,
        home.path(),
        &["task", "edit", &task, "--title", "Later title"],
    );
    let status: serde_json::Value = serde_json::from_str(&run(
        &source,
        home.path(),
        &["planning", "status", "--json"],
    ))
    .unwrap();
    let destinations: Vec<PeerPlanningStatus> =
        serde_json::from_value(status["destinations"].clone()).unwrap();
    let active = destinations
        .iter()
        .find(|destination| destination.active)
        .unwrap();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let journal = store
        .export_peer_planning(source.to_str().unwrap(), &active.id)
        .unwrap();
    assert_eq!(active.changes, journal.changes);
    let (old_id, old) = active
        .changes
        .iter()
        .find(|(_, change)| {
            change.edit.target == task
                && change.edit.change.field() == "task_title"
                && change.edit.change.value().unwrap() == "Same title"
        })
        .unwrap();
    assert!(!active.candidates.contains(old_id));
    assert_eq!(old.edit.change.value().unwrap(), "Same title");
    let (current_id, current) = active
        .changes
        .iter()
        .find(|(_, change)| {
            change.edit.target == task
                && change.edit.change.field() == "task_title"
                && change.edit.change.value().unwrap() == "Later title"
        })
        .unwrap();
    assert!(active.candidates.contains(current_id));
    assert!(current.parents.contains(old_id));
    let text = run(&source, home.path(), &["planning", "status"]);
    assert!(text.contains("retained alternative]: \"Same title\""));
    assert!(text.contains("candidate, not confirmed]: \"Later title\""));
    assert_eq!(
        store
            .export_peer_planning(source.to_str().unwrap(), &active.id)
            .unwrap(),
        journal
    );

    // Switching config selects a different ref without replacing the old route.
    let config_path = source.join(".lf/config.yaml");
    let config = config_path
        .exists()
        .then(|| fs::read(&config_path).unwrap());
    let other =
        PlanningDestination::resolve(&source, "origin", "refs/loopflow/planning/shared/other")
            .unwrap();
    configure(&source, &other);
    let switched: serde_json::Value = serde_json::from_str(&run(
        &source,
        home.path(),
        &["planning", "status", "--json"],
    ))
    .unwrap();
    let switched: Vec<PeerPlanningStatus> =
        serde_json::from_value(switched["destinations"].clone()).unwrap();
    assert_eq!(switched.iter().find(|d| d.active).unwrap().id, other.id());
    let retained = switched.iter().find(|d| d.id == active.id).unwrap();
    assert!(!retained.active);
    assert_eq!(retained.changes, active.changes);
    assert_eq!(retained.publication_revision, active.publication_revision);
    match config {
        Some(bytes) => fs::write(&config_path, bytes).unwrap(),
        None => fs::remove_file(&config_path).unwrap(),
    }

    // An unreadable journal fails the read; it must not fabricate an empty plan
    // or overwrite the bytes needed for repair. Work frames carry that read error.
    let conn = Connection::open(home.path().join("loopflow.db")).unwrap();
    conn.execute(
        "UPDATE planning_peer_changes SET mutation=json_set(mutation,'$.parents',json('[\"missing\"]')) WHERE id=?1",
        [current_id],
    )
    .unwrap();
    let failed = command(&source, home.path())
        .args(["planning", "status", "--json"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("missing planning predecessor"));
    let retained: String = conn
        .query_row(
            "SELECT mutation -> '$.parents' FROM planning_peer_changes WHERE id=?1",
            [current_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(retained, "[\"missing\"]");
    assert_eq!(
        store.task(&task.parse().unwrap()).unwrap().unwrap().title,
        "Later title"
    );
}

#[test]
fn personal_plan_moves_to_shared_default_with_identity() {
    let repo = TestRepo::new();
    repo.push();
    let other = TestRepo::new();
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    let source = repo.path().canonicalize().unwrap();
    let target = other.path().canonicalize().unwrap();
    fs::write(left.path().join("config.yaml"), "user:\n  name: Maya\n").unwrap();
    fs::write(right.path().join("config.yaml"), "user:\n  name: Lee\n").unwrap();
    let origin = repo.bare_path().to_str().unwrap();
    for checkout in [&source, &target] {
        assert!(Command::new("git")
            .current_dir(checkout)
            .args(["config", "remote.origin.url", origin])
            .status()
            .unwrap()
            .success());
    }
    let refs = |pattern: &str| {
        let output = Command::new("git")
            .current_dir(&source)
            .args(["ls-remote", "origin", pattern])
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap()
    };

    // An earlier build published this plan to Maya's personal ref.
    fs::create_dir_all(source.join(".lf")).unwrap();
    fs::write(
        source.join(".lf/config.yaml"),
        "planning:\n  provider: git\n  personal: true\n",
    )
    .unwrap();
    let task = create(&source, left.path(), "");
    run(&source, left.path(), &["task", "claim", &task, "--json"]);
    run(
        &source,
        left.path(),
        &[
            "task",
            "comment",
            &task,
            "Start with the CPU path",
            "--json",
        ],
    );
    assert!(!refs("refs/loopflow/planning/users/*").is_empty());
    assert!(refs("refs/loopflow/planning/shared/*").is_empty());
    let conn = Connection::open(left.path().join("loopflow.db")).unwrap();
    let before = execution(&conn, &task);

    // The upgrade drops the personal default; the next save publishes everything.
    fs::remove_file(source.join(".lf/config.yaml")).unwrap();
    run(
        &source,
        left.path(),
        &[
            "task",
            "comment",
            &task,
            "Moved to the shared plan",
            "--json",
        ],
    );
    assert!(!refs("refs/loopflow/planning/shared/main").is_empty());
    assert_eq!(execution(&conn, &task), before);

    // Lee's fresh Home, with no planning config, reads the same plan.
    let comments = |repo: &Path, home: &Path| -> Vec<String> {
        let listed: serde_json::Value =
            serde_json::from_str(&run(repo, home, &["task", "comment", &task, "--json"])).unwrap();
        listed["comments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|comment| comment["body"].as_str().unwrap().to_owned())
            .collect()
    };
    let theirs = comments(&target, right.path());
    assert_eq!(theirs, comments(&source, left.path()));
    assert!(
        theirs
            .iter()
            .any(|body| body.starts_with("Start with the CPU path")),
        "{theirs:?}"
    );
    let open =
        |home: &Path| loopflow::store::sqlite::SqliteStore::new(&home.join("loopflow.db")).unwrap();
    let id: loopflow::work::task::TaskId = task.parse().unwrap();
    let (mine, theirs) = (open(left.path()), open(right.path()));
    let (left_task, right_task) = (
        mine.task(&id).unwrap().unwrap(),
        theirs.task(&id).unwrap().unwrap(),
    );
    assert_eq!(right_task.identifier, left_task.identifier);
    assert_eq!(right_task.title, left_task.title);
    let people = |store: &loopflow::store::sqlite::SqliteStore| {
        store
            .task_ownership(&id)
            .unwrap()
            .log
            .into_iter()
            .map(|entry| (entry.kind, entry.person))
            .collect::<Vec<_>>()
    };
    assert!(!people(&mine).is_empty());
    assert_eq!(people(&theirs), people(&mine));

    let status: serde_json::Value = serde_json::from_str(&run(
        &source,
        left.path(),
        &["planning", "status", "--json"],
    ))
    .unwrap();
    let destinations: Vec<PeerPlanningStatus> =
        serde_json::from_value(status["destinations"].clone()).unwrap();
    let active = destinations.iter().find(|d| d.active).unwrap();
    assert_eq!(active.reference, "refs/loopflow/planning/shared/main");
    let personal = destinations.iter().find(|d| !d.active).unwrap();
    assert!(personal
        .reference
        .starts_with("refs/loopflow/planning/users/"));
    assert!(!personal.changes.is_empty());
}
