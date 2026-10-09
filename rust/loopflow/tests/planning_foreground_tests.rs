mod support;

use std::fs::{self, File};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::{fs::PermissionsExt, process::CommandExt};
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
        let mut query = conn
            .prepare(&format!(
                "SELECT * FROM {table} WHERE {predicate} ORDER BY rowid"
            ))
            .unwrap();
        let columns = query.column_count();
        query
            .query_map([identity], |row| (0..columns).map(|i| row.get(i)).collect())
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
    fs::write(left.path().join("config.yaml"), "user:\n  name: Maya\n").unwrap();
    fs::write(right.path().join("config.yaml"), "user:\n  name: Lee\n").unwrap();
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
    let before = execution(&conn, fixture.task.id.as_str());
    let placement = fixture.task.worktree.clone();
    assert_eq!(
        status(&runtime, &fixture.store, source_key).pending_local,
        Some(true)
    );
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
    wait_for(|| {
        status(&runtime, &worker, target_key)
            .acquisition_error
            .is_some()
    });
    assert_eq!(
        status(&runtime, &worker, target_key).pending_local,
        Some(true)
    );
    assert_eq!(
        status(&runtime, &fixture.store, source_key).pending_local,
        Some(true)
    );
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
        let a = status(&runtime, &fixture.store, source_key);
        let b = status(&runtime, &worker, target_key);
        a.pending_local == Some(false)
            && b.pending_local == Some(false)
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
                "SELECT count(*) FROM task_comments WHERE task_id=?1",
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
                "SELECT id,body,author,created_at FROM task_comments WHERE task_id=?1 ORDER BY id",
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
    // The connection must also discover destinations selected after launch.
    let mut source_agent = Agent::start(&source_path, left.path(), true);
    let mut target_agent = Agent::start(&target_path, right.path(), false);
    runtime.block_on(async {
        a.bind_peer_planning(source_key, &binding).await.unwrap();
        a.use_peer_planning(source_key, Some(&binding.id()))
            .await
            .unwrap();
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
    assert_eq!(status(&runtime, &a, source_key).pending_local, Some(true));
    assert_eq!(status(&runtime, &b, target_key).pending_local, Some(true));
    fs::rename(&disconnected, remote).unwrap();
    wait_for(|| {
        let source = runtime.block_on(a.get_task(&task_id)).unwrap().unwrap();
        let target = runtime.block_on(b.get_task(&task_id)).unwrap().unwrap();
        source.plan.description == "Target offline"
            && target.plan.title == "Source offline"
            && status(&runtime, &a, source_key).pending_local == Some(false)
            && status(&runtime, &b, target_key).pending_local == Some(false)
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
