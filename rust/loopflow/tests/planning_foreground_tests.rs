mod support;

use std::fs::{self, File};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::{fs::PermissionsExt, process::CommandExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use loopflow::engine::planning_git::{PlanningDestination, PlanningGit, PlanningPublication};
use loopflow::lf::commands::work_watch::{WorkContent, WorkFrame};
use loopflow::ops::pm::TaskCommentAuthor;
use loopflow::store::{
    open_ephemeral_store, sqlite::SqliteStore, PeerPlanningStatus, StorageConfig,
};
use loopflow::work::task::Task;
use loopflow_test_support::TestRepo;
use rusqlite::{params, Connection};
use serde_json::json;

fn open_store(home: &Path) -> SqliteStore {
    let path = home.join("loopflow.db");
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(path.clone())))
        .unwrap();
    SqliteStore::new(&path).unwrap()
}

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

fn status(store: &SqliteStore, repo: &str) -> PeerPlanningStatus {
    store.peer_planning_status(repo).unwrap().remove(0)
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

// Seed retained records only; the public commands/watchers must leave them alone.
fn seed_execution(conn: &Connection, task: &Task, repo: &Path, node: &str) {
    conn.execute(
        "INSERT INTO processes(lfid,trace_id,started_at)
         VALUES('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000002',1)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO agent_sessions(id,title,title_source,created_at,interactive,input_published,cwd,task_id,wave_id)
         VALUES('retained-session','Retained','human',1,1,1,?1,?2,?3)",
        params![repo.to_str().unwrap(), task.id.as_str(), task.wave_id.as_str()],
    ).unwrap();
    let graph =
        json!({"name":node,"nodes":[{"name":node,"skill":node,"description":null}],"edges":[]});
    conn.execute(
        "INSERT INTO task_workflows(task_id,graph,node,updated_at) VALUES(?1,?2,?3,1)",
        params![task.id.as_str(), graph.to_string(), node],
    )
    .unwrap();
}

fn execution(conn: &Connection, task: &str) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    [
        "agent_sessions",
        "processes",
        "task_workflows",
        "task_workflow_moves",
        "task_prs",
        "work_placements",
    ]
    .into_iter()
    .map(|table| {
        let (predicate, identity) = if table == "processes" {
            ("lfid=?1", "00000000-0000-4000-8000-000000000001")
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
    let source_store = SqliteStore::new(&left.path().join("loopflow.db")).unwrap();
    let worker = open_store(right.path());
    let binding =
        PlanningDestination::resolve(&source, "origin", "refs/loopflow/planning/shared/fixture")
            .unwrap();

    source_store
        .bind_peer_planning(source_key, &binding)
        .unwrap();
    source_store
        .select_peer_waves(
            source_key,
            &binding.id(),
            std::slice::from_ref(&fixture.task.wave_id),
        )
        .unwrap();
    worker.bind_peer_planning(target_key, &binding).unwrap();
    let conn = Connection::open(left.path().join("loopflow.db")).unwrap();
    conn.busy_timeout(Duration::from_secs(5)).unwrap();
    seed_execution(&conn, &fixture.task, &source, "review");
    let before = execution(&conn, fixture.task.id.as_str());
    let placement = fixture.task.worktree.clone();
    assert_eq!(status(&source_store, source_key).pending_local, Some(true));
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
            .query_row("SELECT count(*) FROM work_placements", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0,
        "cold planning acquisition must not allocate execution placement"
    );
    let third = create(&target, right.path(), "task-pr-tests");
    assert_ne!(first, third);
    assert_ne!(second, third);
    run(&source, left.path(), &["task", "move", &first, "end"]);
    assert!(worker.task(&first.parse().unwrap()).unwrap().is_some());
    let source_watch = Watch::start(&source, left.path());
    let worker_watch = Watch::start(&target, right.path());
    wait_for(|| worker.task(&fixture.task.id).unwrap().is_some());
    let received = worker.task(&fixture.task.id).unwrap().unwrap();
    assert!(received.worktree.is_none());
    assert!(worker.task_prs(&fixture.task.id).unwrap().is_empty());
    wait_for(|| {
        status(&source_store, source_key)
            .publication_state
            .as_deref()
            == Some("confirmed")
    });
    let received_status = status(&worker, target_key);
    assert!(received_status.fetched_revision.is_some());
    assert!(received_status.imported_revision.is_some());

    // The remote is unavailable; neither side loses its local write or starts a turn.
    let remote = Path::new(binding.endpoint());
    let disconnected = remote.with_extension("disconnected");
    fs::rename(remote, &disconnected).unwrap();
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
        &["task", "move", fixture.task.id.as_str(), "end"],
    );
    wait_for(|| status(&worker, target_key).acquisition_error.is_some());
    assert_eq!(status(&worker, target_key).pending_local, Some(true));
    assert_eq!(status(&source_store, source_key).pending_local, Some(true));
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
        source_store
            .task(&fixture.task.id)
            .unwrap()
            .unwrap()
            .plan
            .title
            == "Written offline on worker"
    });
    drop(effect_lock);
    wait_for(|| {
        source_store
            .task(&fixture.task.id)
            .unwrap()
            .unwrap()
            .plan
            .title
            == "Written offline on worker"
            && worker
                .task(&fixture.task.id)
                .unwrap()
                .unwrap()
                .plan
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
        let a = status(&source_store, source_key);
        let b = status(&worker, target_key);
        a.pending_local == Some(false)
            && b.pending_local == Some(false)
            && a.publication_state.as_deref() == Some("confirmed")
            && b.publication_state.as_deref() == Some("confirmed")
    });
    drop(source_watch);
    drop(worker_watch);
    assert_eq!(execution(&conn, fixture.task.id.as_str()), before);
    assert_eq!(
        source_store
            .task(&fixture.task.id)
            .unwrap()
            .unwrap()
            .worktree,
        placement
    );
    assert_eq!(worker.list_tasks(None).unwrap().len(), 4);
    assert_eq!(source_store.list_tasks(None).unwrap().len(), 4);
    let saved = source_store
        .task_comments(&fixture.task.id)
        .unwrap()
        .comments;
    assert_eq!(
        saved,
        worker.task_comments(&fixture.task.id).unwrap().comments
    );
    assert_eq!(
        saved.len(),
        2,
        "repeated exchange must not duplicate comments"
    );
    assert!(saved.iter().all(|comment| comment.created_at.is_some()));
    let mut authors: Vec<_> = saved
        .iter()
        .map(|comment| match &comment.author {
            TaskCommentAuthor::Person { name: Some(name) } => name.as_str(),
            author => panic!("expected a named author, got {author:?}"),
        })
        .collect();
    authors.sort();
    assert_eq!(authors, ["Lee", "Maya"]);
    let printed: serde_json::Value = serde_json::from_str(&run(
        &target,
        right.path(),
        &["planning", "status", "--json"],
    ))
    .unwrap();
    assert_eq!(printed["destinations"][0]["publication_state"], "confirmed");
    assert!(printed["destinations"][0].get("endpoint").is_none());
}

#[test]
#[cfg(unix)]
fn public_completion_exchange_supersedes_only_old_completion_requests() {
    let repo = TestRepo::new();
    let other = TestRepo::new();
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    let source = repo.path().canonicalize().unwrap();
    let target = other.path().canonicalize().unwrap();
    let fixture = support::register_task_with_pr(left.path(), &source, "main", &repo.head_sha());
    let task = &fixture.task.id;
    let source_store = SqliteStore::new(&left.path().join("loopflow.db")).unwrap();
    let worker = open_store(right.path());
    let conn = Connection::open(left.path().join("loopflow.db")).unwrap();
    conn.busy_timeout(Duration::from_secs(5)).unwrap();
    seed_execution(&conn, &fixture.task, &source, "review");
    let mut agent = Agent::start(&source, left.path(), false);
    let marker = source.join("retained-draft.txt");
    fs::write(&marker, "unfinished draft\n").unwrap();
    let binding = PlanningDestination::resolve(
        &source,
        "origin",
        "refs/loopflow/planning/shared/completion-fixture",
    )
    .unwrap();
    source_store
        .bind_peer_planning(source.to_str().unwrap(), &binding)
        .unwrap();
    source_store
        .select_peer_waves(
            source.to_str().unwrap(),
            &binding.id(),
            std::slice::from_ref(&fixture.task.wave_id),
        )
        .unwrap();
    worker
        .bind_peer_planning(target.to_str().unwrap(), &binding)
        .unwrap();
    let source_watch = Watch::start(&source, left.path());
    let worker_watch = Watch::start(&target, right.path());
    let settled = || {
        [&source_store, &worker]
            .into_iter()
            .zip([&source, &target])
            .all(|(store, repo)| {
                let status = status(store, repo.to_str().unwrap());
                status.pending_local == Some(false)
                    && status.publication_state.as_deref() == Some("confirmed")
                    && status.conflicts.is_empty()
                    && status.acquisition_error.is_none()
            })
    };
    wait_for(|| worker.task(task).unwrap().is_some() && settled());
    let request = || {
        conn.query_row(
            "SELECT completion_request,completion_error FROM tasks WHERE id=?1",
            [task.as_str()],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .unwrap()
    };
    let completed = |store: &SqliteStore| {
        store
            .planning_task(task)
            .unwrap()
            .record
            .unwrap()
            .item
            .completed
    };
    let fail = |args: &[&str]| {
        let output = command(&source, left.path()).args(args).output().unwrap();
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{args:?} unexpectedly succeeded");
        assert!(error.to_lowercase().contains("pull request"), "{error}");
        let (id, error) = request();
        assert!(error.is_some());
        id.unwrap()
    };
    let old = fail(&["task", "move", task.as_str(), "end"]);
    let before = execution(&conn, task.as_str());

    // The other Machine has planning, not this checkout or its unsettled PR.
    // Public completion imports no request and cannot settle local execution.
    run(&target, right.path(), &["task", "complete", task.as_str()]);
    wait_for(|| completed(&source_store) && settled());
    assert_eq!(request(), (None, None));
    assert_eq!(execution(&conn, task.as_str()), before);
    assert!(agent.child.try_wait().unwrap().is_none());
    assert_eq!(fs::read_to_string(&marker).unwrap(), "unfinished draft\n");

    // Reopen locally while the remote still contains completion, then retain a
    // newer failed intention. Acquiring that stale remote must not clear it.
    let remote = Path::new(binding.endpoint());
    let disconnected = remote.with_extension("disconnected");
    fs::rename(remote, &disconnected).unwrap();
    run(&source, left.path(), &["task", "reopen", task.as_str()]);
    let newer = fail(&["task", "complete", task.as_str()]);
    assert_ne!(old, newer);
    let pending = request();
    fs::rename(&disconnected, remote).unwrap();
    wait_for(|| !completed(&worker) && settled());
    assert_eq!(
        request(),
        pending,
        "stale completion cannot supersede a newer request"
    );

    // Repeated acquisition includes the accepted reopening. Change another
    // field to prove a fresh exchange, rather than accepting cached status.
    run(
        &target,
        right.path(),
        &["task", "edit", task.as_str(), "--title", "After reopening"],
    );
    wait_for(|| {
        source_store.task(task).unwrap().unwrap().plan.title == "After reopening" && settled()
    });
    assert_eq!(
        request(),
        pending,
        "replayed reopening cannot clear newer intent"
    );

    // Both commands happen offline: the receiver sees open -> open, but the
    // accepted causal reopening is new and must supersede the retained request.
    fs::rename(remote, &disconnected).unwrap();
    run(&target, right.path(), &["task", "complete", task.as_str()]);
    run(&target, right.path(), &["task", "reopen", task.as_str()]);
    run(
        &target,
        right.path(),
        &[
            "task",
            "edit",
            task.as_str(),
            "--title",
            "Continue after retry",
        ],
    );
    fs::rename(&disconnected, remote).unwrap();
    wait_for(|| {
        source_store.task(task).unwrap().unwrap().plan.title == "Continue after retry" && settled()
    });
    assert!(!completed(&source_store));
    assert_eq!(
        request(),
        (None, None),
        "new reopening supersedes failed end intent even when the value is unchanged"
    );

    // Ordinary retry at the retained end cannot resurrect the superseded
    // request. Explicit `task complete` would instead author a new intention.
    run(
        &source,
        left.path(),
        &["task", "move", task.as_str(), "end"],
    );
    assert!(!completed(&source_store));
    assert_eq!(request(), (None, None));
    assert_eq!(execution(&conn, task.as_str()), before);
    assert!(agent.child.try_wait().unwrap().is_none());
    assert_eq!(fs::read_to_string(&marker).unwrap(), "unfinished draft\n");
    assert!(worker.task(task).unwrap().unwrap().worktree.is_none());
    let remote_conn = Connection::open(right.path().join("loopflow.db")).unwrap();
    assert!(rows(
        &remote_conn,
        "SELECT completion_request FROM tasks WHERE id=?1 AND completion_request IS NOT NULL",
        [task.as_str()]
    )
    .is_empty());
    drop(source_watch);
    drop(worker_watch);
    agent.finish();
}

#[test]
fn fetched_invalid_document_does_not_claim_import_or_publication() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let store = open_store(home.path());
    let repo_path = repo.path().canonicalize().unwrap();
    let repo_key = repo_path.to_str().unwrap();
    let binding = PlanningDestination::resolve(
        repo.path(),
        "origin",
        "refs/loopflow/planning/shared/invalid",
    )
    .unwrap();
    store.bind_peer_planning(repo_key, &binding).unwrap();
    let git = PlanningGit::new(repo.path(), &binding).unwrap();
    let invalid = git.save(b"not planning JSON", None, None).unwrap();
    assert_eq!(
        git.publish(&invalid.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    let watch = Watch::start(&repo_path, home.path());
    wait_for(|| status(&store, repo_key).acquisition_error.is_some());
    let status = status(&store, repo_key);
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
        fs::write(
            &provider,
            r#"#!/bin/sh
printf 'launch\n' >> "$HOME/launches"
for argument in "$@"; do
  if [ "$argument" = --print ]; then cat > /dev/null; fi
done
printf 'ready\n' > "$HOME/ready.tmp"
mv "$HOME/ready.tmp" "$HOME/ready"
while [ ! -f "$HOME/stop" ]; do sleep 0.1; done
printf '%s\n' '{"type":"result","subtype":"success","result":"done"}'
"#,
        )
        .unwrap();
        fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
        let child = command(repo, home)
            .process_group(0)
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
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
    let a = open_store(left.path());
    let b = open_store(right.path());
    let binding = PlanningDestination::resolve(
        &source_path,
        "origin",
        "refs/loopflow/planning/shared/taskless",
    )
    .unwrap();
    // The connection must also discover destinations selected after launch.
    let mut source_agent = Agent::start(&source_path, left.path(), true);
    let mut target_agent = Agent::start(&target_path, right.path(), false);

    a.bind_peer_planning(source_key, &binding).unwrap();
    a.use_peer_planning(source_key, Some(&binding.id()))
        .unwrap();
    b.bind_peer_planning(target_key, &binding).unwrap();
    let id = create(&source_path, left.path(), "");
    let task_id = id.parse().unwrap();
    // No work-watch, Task placement, explicit Work binding or further turn.
    wait_for(|| b.task(&task_id).unwrap().is_some());
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
    assert_eq!(status(&a, source_key).pending_local, Some(true));
    assert_eq!(status(&b, target_key).pending_local, Some(true));
    fs::rename(&disconnected, remote).unwrap();
    wait_for(|| {
        let source = a.task(&task_id).unwrap().unwrap();
        let target = b.task(&task_id).unwrap().unwrap();
        source.plan.description == "Target offline"
            && target.plan.title == "Source offline"
            && status(&a, source_key).pending_local == Some(false)
            && status(&b, target_key).pending_local == Some(false)
    });
    source_agent.finish();
    target_agent.finish();
    for (store, home) in [(&a, left.path()), (&b, right.path())] {
        assert!(store.task(&task_id).unwrap().unwrap().worktree.is_none());
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
    let source_store =
        loopflow::store::sqlite::SqliteStore::new(&left.path().join("loopflow.db")).unwrap();
    let retained = create(&target, right.path(), "");
    let worker =
        loopflow::store::sqlite::SqliteStore::new(&right.path().join("loopflow.db")).unwrap();
    let binding =
        PlanningDestination::resolve(&source, "origin", "refs/loopflow/planning/shared/fixture")
            .unwrap();
    let source_key = source.to_str().unwrap();
    let target_key = target.to_str().unwrap();
    source_store
        .bind_peer_planning(source_key, &binding)
        .unwrap();
    source_store
        .select_peer_waves(
            source_key,
            &binding.id(),
            std::slice::from_ref(&fixture.task.wave_id),
        )
        .unwrap();
    worker.bind_peer_planning(target_key, &binding).unwrap();
    let snapshot = source_store
        .export_peer_planning(source_key, &binding.id())
        .unwrap();
    worker
        .import_peer_planning(target_key, &binding.id(), "fixture", &snapshot)
        .unwrap();
    assert!(worker.peer_planning_status(target_key).unwrap()[0]
        .conflicts
        .is_empty());
    let wave_id = fixture.task.wave_id.as_str();
    let conn = Connection::open(right.path().join("loopflow.db")).unwrap();
    let retained_task = worker.task(&retained.parse().unwrap()).unwrap().unwrap();
    seed_execution(&conn, &retained_task, &target, "review");
    let before = execution(&conn, &retained);
    let imported_before = execution(&conn, fixture.task.id.as_str());
    let placements = || rows(&conn, "SELECT * FROM work_placements ORDER BY rowid", []);
    let placements_before = placements();
    let imported_work = loopflow::durable::WorkRef::Wave(fixture.task.wave_id.clone());
    assert!(worker.find_placement(&imported_work).unwrap().is_none());
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
    assert!(worker.find_placement(&imported_work).unwrap().is_none());
    assert!(worker
        .find_placement(&loopflow::durable::WorkRef::Task(fixture.task.id.clone()))
        .unwrap()
        .is_none());
    assert_eq!(placements(), placements_before);
    assert_eq!(execution(&conn, fixture.task.id.as_str()), imported_before);
    assert_eq!(execution(&conn, &retained), before);
}

// Provider observations seed independently retained origins, but both repositories
// are disconnected from Linear. All exchange goes through the public foreground
// owner, not direct PlanningGit publication or Store import calls.
#[test]
fn associated_origins_reconnect_through_foreground_exchange_and_work_stream() {
    use loopflow::id::WaveId;
    use loopflow::store::PmSnapshotRow;
    use loopflow::work::wave::Wave;

    let left_repo = TestRepo::new();
    let right_repo = TestRepo::new();
    let left_home = tempfile::tempdir().unwrap();
    let right_home = tempfile::tempdir().unwrap();
    fs::write(
        left_home.path().join("config.yaml"),
        "user:\n  name: Maya\n",
    )
    .unwrap();
    fs::write(
        right_home.path().join("config.yaml"),
        "user:\n  name: Lee\n",
    )
    .unwrap();
    let left_path = left_repo.path().canonicalize().unwrap();
    let right_path = right_repo.path().canonicalize().unwrap();
    let binding = PlanningDestination::resolve(
        &left_path,
        "origin",
        "refs/loopflow/planning/shared/associated-fixture",
    )
    .unwrap();
    let destination = binding.id();
    let wave = WaveId::new();
    let private = WaveId::new();
    let mut snapshot: loopflow::pm::PmSnapshot = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/task_history_planning.json"
    ))
    .unwrap();
    snapshot.items.truncate(1);
    snapshot.items[0].state = Some("unstarted".into());
    snapshot.items[0].revision = Some("2026-10-08T10:00:00Z".into());
    let mut observation = PmSnapshotRow {
        wave_id: wave.clone(),
        provider: "linear".into(),
        initiative: "initiative".into(),
        synced_at: 42,
        snapshot,
    };
    let stores = [
        (&left_path, left_home.path()),
        (&right_path, right_home.path()),
    ]
    .map(|(repo, home)| {
        fs::create_dir_all(repo.join(".lf")).unwrap();
        fs::write(repo.join(".lf/config.yaml"), "pm: null\n").unwrap();
        let store = open_store(home);
        let scope = repo.to_str().unwrap();
        store.bind_peer_planning(scope, &binding).unwrap();
        store
            .create_wave(&Wave::new(wave.clone(), "Shared".into(), scope.into()))
            .unwrap();
        store.put_pm_snapshot(&observation).unwrap();
        store
            .select_peer_waves(scope, &destination, std::slice::from_ref(&wave))
            .unwrap();
        store
            .create_wave(&Wave::new(private.clone(), "Private".into(), scope.into()))
            .unwrap();
        store
    });
    let left = &stores[0];
    let right = &stores[1];
    let a = left
        .task_by_issue(&observation.snapshot.items[0].id)
        .unwrap()
        .unwrap();
    let b = right
        .task_by_issue(&observation.snapshot.items[0].id)
        .unwrap()
        .unwrap();
    assert_ne!(a.id, b.id);
    assert_ne!(a.project_id, b.project_id);
    let connections = [left_home.path(), right_home.path()].map(|home| {
        let conn = Connection::open(home.join("loopflow.db")).unwrap();
        conn.busy_timeout(Duration::from_secs(5)).unwrap();
        conn
    });
    // Each origin keeps a different captured Workflow and its own Session.
    for (conn, task, repo, node) in [
        (&connections[0], &a, &left_path, "review"),
        (&connections[1], &b, &right_path, "design"),
    ] {
        seed_execution(conn, task, repo, node);
    }
    let before = [
        execution(&connections[0], a.id.as_str()),
        execution(&connections[1], b.id.as_str()),
    ];
    let left_watch = Watch::start(&left_path, left_home.path());
    let right_watch = Watch::start(&right_path, right_home.path());
    wait_for(|| {
        !status(left, left_path.to_str().unwrap())
            .conflicts
            .is_empty()
            && !status(right, right_path.to_str().unwrap())
                .conflicts
                .is_empty()
    });
    // Matching provider IDs do not silently associate either origin.
    assert!(left.task(&b.id).unwrap().is_none());
    assert!(right.task(&a.id).unwrap().is_none());
    for (repo, home, incoming, local) in [
        (&left_path, left_home.path(), &b, &a),
        (&right_path, right_home.path(), &a, &b),
    ] {
        for (origin, owner, provider) in [
            (
                incoming.id.as_str(),
                local.id.as_str(),
                observation.snapshot.items[0].id.as_str(),
            ),
            (
                incoming.project_id.as_str(),
                local.project_id.as_str(),
                observation.snapshot.projects[0].id.as_str(),
            ),
        ] {
            run(
                repo,
                home,
                &[
                    "planning",
                    "associate",
                    origin,
                    "--with",
                    owner,
                    "--linear",
                    provider,
                ],
            );
        }
    }
    let settled = || {
        [&left_path, &right_path]
            .into_iter()
            .zip(&stores)
            .all(|(repo, store)| {
                let status = status(store, repo.to_str().unwrap());
                status.conflicts.is_empty()
                    && status.pending_local == Some(false)
                    && status.publication_state.as_deref() == Some("confirmed")
                    && status.acquisition_error.is_none()
                    && status.publication_error.is_none()
            })
    };
    wait_for(settled);
    let initial = left
        .export_peer_planning(left_path.to_str().unwrap(), &destination)
        .unwrap();

    let remote = Path::new(binding.endpoint());
    let disconnected = remote.with_extension("disconnected");
    fs::rename(remote, &disconnected).unwrap();
    run(
        &left_path,
        left_home.path(),
        &["task", "edit", a.id.as_str(), "--title", "Peer next focus"],
    );
    run(
        &right_path,
        right_home.path(),
        &[
            "task",
            "edit",
            b.id.as_str(),
            "--notes",
            "Saved while disconnected",
        ],
    );
    run(
        &right_path,
        right_home.path(),
        &[
            "task",
            "comment",
            b.id.as_str(),
            "Retain this direction",
            "--json",
        ],
    );
    let comment = right.task_comments(&b.id).unwrap().comments.remove(0);
    // A previous provider attempt lost its reply. Git confirmation must never
    // acknowledge that distinct provider effect or change its captured input.
    right
        .record_comment_delivery(&comment.id, Some("lost provider reply"))
        .unwrap();
    let receipt = |conn: &Connection| {
        rows(conn,
        "SELECT comment_json,acknowledged,conflicting_comment_json FROM task_comment_deliveries WHERE comment_id=?1",
        [&comment.id])
    };
    let uncertain = receipt(&connections[1]);
    assert_eq!(uncertain[0][1], rusqlite::types::Value::Integer(0));
    assert_eq!(
        left.task(&a.id).unwrap().unwrap().plan.title,
        "Peer next focus"
    );
    assert_eq!(
        right.task(&b.id).unwrap().unwrap().plan.description,
        "Saved while disconnected"
    );
    for (repo, home) in [
        (&left_path, left_home.path()),
        (&right_path, right_home.path()),
    ] {
        wait_for(|| {
            peer_frame(home, repo).is_some_and(|s| {
                s.pending_local == Some(true)
                    && s.acquisition_error.is_some()
                    && s.imported_revision.is_some()
            })
        });
    }
    fs::rename(&disconnected, remote).unwrap();
    wait_for(|| {
        right.task(&b.id).unwrap().unwrap().plan.title == "Peer next focus"
            && left.task(&a.id).unwrap().unwrap().plan.description == "Saved while disconnected"
            && settled()
    });
    run(
        &right_path,
        right_home.path(),
        &[
            "task",
            "edit",
            b.id.as_str(),
            "--title",
            "Local continuation",
        ],
    );
    wait_for(|| left.task(&a.id).unwrap().unwrap().plan.title == "Local continuation" && settled());
    let continued = left
        .export_peer_planning(left_path.to_str().unwrap(), &destination)
        .unwrap();
    let (peer_id, _) = continued
        .changes
        .iter()
        .find(|(_, c)| c.value == "Peer next focus")
        .unwrap();
    let local = continued
        .changes
        .values()
        .find(|c| c.value == "Local continuation")
        .unwrap();
    assert!(local.parents.contains(peer_id));
    for (id, change) in initial.changes {
        assert_eq!(continued.changes[&id], change);
    }
    assert!(!continued
        .changes
        .values()
        .any(|c| c.object.id == private.as_str()));

    // An observed completion and its later reopening cross associated origins.
    // They change planning only; neither captured Workflow may move.
    observation.snapshot.items[0].completed = true;
    observation.snapshot.items[0].state = Some("completed".into());
    observation.snapshot.items[0].revision = Some("2026-10-08T11:00:00Z".into());
    left.put_pm_snapshot(&observation).unwrap();
    wait_for(|| {
        right
            .planning_task(&b.id)
            .unwrap()
            .record
            .unwrap()
            .item
            .completed
            && settled()
    });
    let completed = left
        .export_peer_planning(left_path.to_str().unwrap(), &destination)
        .unwrap();
    fs::rename(remote, &disconnected).unwrap();
    observation.snapshot.items[0].completed = false;
    observation.snapshot.items[0].state = Some("unstarted".into());
    observation.snapshot.items[0].revision = Some("2026-10-08T12:00:00Z".into());
    right.put_pm_snapshot(&observation).unwrap();
    fs::rename(&disconnected, remote).unwrap();
    // Publication acquires the still-completed remote first. It must not roll
    // back the reopening saved against the accepted completion frontier.
    wait_for(|| {
        !left
            .planning_task(&a.id)
            .unwrap()
            .record
            .unwrap()
            .item
            .completed
            && settled()
    });
    let reopened = left
        .export_peer_planning(left_path.to_str().unwrap(), &destination)
        .unwrap();
    for (id, change) in completed.changes {
        assert_eq!(reopened.changes[&id], change);
    }
    for (repo, home) in [
        (&left_path, left_home.path()),
        (&right_path, right_home.path()),
    ] {
        wait_for(|| {
            peer_frame(home, repo).is_some_and(|s| {
                s.pending_local == Some(false)
                    && s.publication_state.as_deref() == Some("confirmed")
                    && s.fetched_revision == s.imported_revision
                    && s.imported_revision == s.publication_revision
                    && s.acquisition_error.is_none()
                    && s.publication_error.is_none()
                    && s.conflicts.is_empty()
            })
        });
    }
    drop(left_watch);
    drop(right_watch);
    let journals: Vec<_> = connections
        .iter()
        .map(|conn| rows(conn, "SELECT * FROM planning_peer_changes ORDER BY id", []))
        .collect();
    let revision = status(left, left_path.to_str().unwrap()).publication_revision;
    // A fresh public connection repeats both acquisition and publication.
    for (conn, repo, home) in [
        (&connections[0], &left_path, left_home.path()),
        (&connections[1], &right_path, right_home.path()),
    ] {
        // Simulate a lost durable receipt for an effect already on the remote.
        conn.execute("UPDATE planning_destinations SET publication_state='unconfirmed' WHERE repo=?1 AND id=?2",
            params![repo.to_str().unwrap(), destination]).unwrap();
        fs::remove_file(home.join("watch.jsonl")).unwrap();
        let watch = Watch::start(repo, home);
        wait_for(|| {
            peer_frame(home, repo).is_some_and(|s| {
                s.pending_local == Some(false)
                    && s.publication_revision == revision
                    && s.publication_state.as_deref() == Some("confirmed")
            })
        });
        drop(watch);
    }
    assert_eq!(
        connections[1]
            .query_row(
                "SELECT error FROM task_comment_deliveries WHERE comment_id=?1",
                [&comment.id],
                |row| row.get::<_, Option<String>>(0)
            )
            .unwrap()
            .as_deref(),
        Some("lost provider reply")
    );
    for (index, task) in [&a, &b].into_iter().enumerate() {
        assert_eq!(
            rows(
                &connections[index],
                "SELECT * FROM planning_peer_changes ORDER BY id",
                []
            ),
            journals[index]
        );
        assert_eq!(
            execution(&connections[index], task.id.as_str()),
            before[index]
        );
        assert_eq!(receipt(&connections[index]), uncertain);
        assert_eq!(
            rows(
                &connections[index],
                "SELECT body FROM task_comments WHERE task_id=?1",
                [task.id.as_str()]
            )
            .len(),
            1
        );
        assert!(
            !stores[index]
                .planning_task(&task.id)
                .unwrap()
                .record
                .unwrap()
                .item
                .completed
        );
    }
}

fn peer_frame(home: &Path, repo: &Path) -> Option<PeerPlanningStatus> {
    fs::read_to_string(home.join("watch.jsonl"))
        .unwrap()
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<WorkFrame>(line).ok())
        .filter(|frame| frame.unavailable.is_none())
        .find_map(|frame| match frame.content {
            WorkContent::PeerPlanning(Some(part)) if Path::new(&part.repo) == repo => {
                part.destinations.into_iter().next()
            }
            _ => None,
        })
}

#[test]
fn public_delegation_exchange_keeps_execution_local() {
    use loopflow::durable::{PlacementProvenance, WorkRef};
    use loopflow::id::WaveId;
    use loopflow::work::wave::Wave;

    let repo = TestRepo::new();
    let other = TestRepo::new();
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    let source = repo.path().canonicalize().unwrap();
    let target = other.path().canonicalize().unwrap();
    let fixture = support::register_task_with_pr(left.path(), &source, "main", &repo.head_sha());
    let a = SqliteStore::new(&left.path().join("loopflow.db")).unwrap();
    let b = open_store(right.path());
    let machine_a = a.local_machine().unwrap().id;
    let machine_b = b.local_machine().unwrap().id;
    let root = &fixture.task.wave_id;
    let child = Wave::new(
        WaveId::new(),
        "child".into(),
        source.to_str().unwrap().into(),
    )
    .with_parent(root.clone());
    a.create_wave(&child).unwrap();
    run(
        &source,
        left.path(),
        &["wave", "ensure", "task-pr-tests/child", "--json"],
    );
    let inherited = create(&source, left.path(), "task-pr-tests");
    let narrower = create(&source, left.path(), child.id().as_str());
    let conn = Connection::open(left.path().join("loopflow.db")).unwrap();
    conn.busy_timeout(Duration::from_secs(5)).unwrap();
    seed_execution(&conn, &fixture.task, &source, "review");
    let before = execution(&conn, fixture.task.id.as_str());
    let route = a.task_execution_route(&fixture.task.id).unwrap();
    let binding = PlanningDestination::resolve(
        &source,
        "origin",
        "refs/loopflow/planning/shared/delegation",
    )
    .unwrap();
    a.bind_peer_planning(source.to_str().unwrap(), &binding)
        .unwrap();
    a.select_peer_waves(
        source.to_str().unwrap(),
        &binding.id(),
        std::slice::from_ref(root),
    )
    .unwrap();
    b.bind_peer_planning(target.to_str().unwrap(), &binding)
        .unwrap();
    let source_watch = Watch::start(&source, left.path());
    let worker_watch = Watch::start(&target, right.path());
    let settled = || {
        [&a, &b]
            .into_iter()
            .zip([&source, &target])
            .all(|(store, repo)| {
                let status = status(store, repo.to_str().unwrap());
                status.pending_local == Some(false)
                    && status.publication_state.as_deref() == Some("confirmed")
                    && status.acquisition_error.is_none()
            })
    };
    wait_for(|| b.task(&narrower.parse().unwrap()).unwrap().is_some() && settled());

    let remote = Path::new(binding.endpoint());
    let disconnected = remote.with_extension("disconnected");
    fs::rename(remote, &disconnected).unwrap();
    run(
        &source,
        left.path(),
        &["wave", "place", root.as_str(), machine_a.as_str(), "--json"],
    );
    run(
        &target,
        right.path(),
        &["wave", "place", root.as_str(), machine_b.as_str(), "--json"],
    );
    run(
        &target,
        right.path(),
        &[
            "wave",
            "place",
            child.id().as_str(),
            machine_b.as_str(),
            "--json",
        ],
    );
    assert_eq!(
        status(&a, source.to_str().unwrap()).pending_local,
        Some(true)
    );
    assert_eq!(
        status(&b, target.to_str().unwrap()).pending_local,
        Some(true)
    );
    let saved_a = a
        .export_peer_planning(source.to_str().unwrap(), &binding.id())
        .unwrap();
    let saved_b = b
        .export_peer_planning(target.to_str().unwrap(), &binding.id())
        .unwrap();
    fs::rename(disconnected, remote).unwrap();
    let work = WorkRef::Task(inherited.parse().unwrap());
    wait_for(|| a.placement(&work).unwrap() == b.placement(&work).unwrap() && settled());
    let assignment = a.placement(&work).unwrap();
    assert_eq!(assignment.source, WorkRef::Wave(root.clone()));
    assert_eq!(assignment.provenance, PlacementProvenance::Explicit);
    let narrow_work = WorkRef::Task(narrower.parse().unwrap());
    for store in [&a, &b] {
        let selected = store.placement(&narrow_work).unwrap();
        assert_eq!(selected.source, WorkRef::Wave(child.id().clone()));
        assert_eq!(selected.machine_id, machine_b);
    }
    assert!(a.machine_by_id(&machine_b).unwrap().is_none());
    assert!(b.machine_by_id(&machine_a).unwrap().is_none());
    assert_eq!(a.task_execution_route(&fixture.task.id).unwrap(), route);
    assert_eq!(execution(&conn, fixture.task.id.as_str()), before);
    assert!(b
        .task(&fixture.task.id)
        .unwrap()
        .unwrap()
        .worktree
        .is_none());

    let retained = a
        .export_peer_planning(source.to_str().unwrap(), &binding.id())
        .unwrap();
    for saved in [&saved_a, &saved_b] {
        for (id, change) in &saved.changes {
            assert_eq!(retained.changes.get(id), Some(change));
        }
    }
    let competing: Vec<_> = retained
        .heads()
        .filter(|(_, change)| change.object.id == root.as_str() && change.field == "delegation")
        .collect();
    assert_eq!(
        competing.len(),
        2,
        "concurrent losing intent remains recoverable"
    );
    let future = create(&target, right.path(), child.id().as_str());
    wait_for(|| a.task(&future.parse().unwrap()).unwrap().is_some());
    assert_eq!(
        a.placement(&WorkRef::Task(future.parse().unwrap()))
            .unwrap()
            .machine_id,
        machine_b
    );
    assert!(!b.task_started(&future.parse().unwrap()).unwrap());
    // Assignment supplies no exclusive admission, even on its selected Machine.
    let refused = command(&target, right.path())
        .args(["task", "checkout", &future])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("admission"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    assert!(b
        .task(&future.parse().unwrap())
        .unwrap()
        .unwrap()
        .worktree
        .is_none());
    drop(worker_watch);
    drop(source_watch);
}
