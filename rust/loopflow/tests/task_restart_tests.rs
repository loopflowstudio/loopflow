mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use loopflow::durable::FlowSession;
use loopflow::engine::invocation::QueuedInvocation;
use loopflow::store::Store;
use loopflow::work::task::Task;
use loopflow_test_support::TestRepo;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

fn flow(store: &Store, task: &Task, review: bool) -> String {
    fs::create_dir_all(task.worktree.join(".lf/flows")).unwrap();
    let steps = if review {
        "- step:\n    name: demo\n    id: review\n    human: true\n"
    } else {
        "- cmd: task sync --plan\n- step:\n    name: demo\n    id: review\n    human: true\n"
    };
    fs::write(task.worktree.join(".lf/flows/proof.yaml"), steps).unwrap();
    let flow = FlowSession {
        invocation: QueuedInvocation::load(&task.worktree, "proof").unwrap(),
        cursor: Default::default(),
        version: 0,
        task_id: Some(task.id.clone()),
        wave_id: Some(task.wave_id.clone()),
        cwd: task.worktree.clone(),
        message: None,
        model: None,
        current_attempt: None,
        pending_session_id: None,
        ready_summary: None,
        worker_generation: 0,
        claim: None,
        failure: None,
        finished: false,
        updated_at: OffsetDateTime::now_utc(),
    };
    let id = flow.id().to_string();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(store.start_task_flow(&task.id, flow))
        .unwrap();
    id
}

fn command(repo: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (name, _) in
        std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
    {
        command.env_remove(name);
    }
    command
        .args(args)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("LF_TEST_WORKER_LOG", home.join("worker.log"));
    command
}

#[test]
fn restart_uses_old_valid_planning_and_preserves_invalid_work() {
    for condition in [
        "valid", "delayed", "invalid", "removed", "terminal", "moved", "advice",
    ] {
        let repo = TestRepo::new();
        support::bind_task_planning(&repo);
        repo.create_branch("restart-proof");
        let home = tempfile::tempdir().unwrap();
        let _env = support::EnvGuard::new(&[
            ("tmux", "#!/bin/sh\nif [ \"$1\" = new-session ]; then\nfor arg do command=$arg; done\n/bin/sh -c \"$command\" </dev/null >\"$LF_TEST_WORKER_LOG\" 2>&1 &\nfi\nexit 0\n"),
            ("open", "#!/bin/sh\nexit 0\n"),
            ("gh", "#!/bin/sh\nexit 1\n"),
        ]);
        let registered = support::register_task(
            home.path(),
            &repo.path().canonicalize().unwrap(),
            "restart-proof",
            &repo.head_sha(),
        );
        let old_id = flow(&registered.store, &registered.task, true);
        // The replacement executes a real mechanical step before stopping for review.
        fs::write(
            repo.path().join(".lf/flows/proof.yaml"),
            "- cmd: task sync --plan\n- step:\n    name: demo\n    id: review\n    human: true\n",
        )
        .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let old = runtime
            .block_on(registered.store.flow(&old_id))
            .unwrap()
            .unwrap();
        let scope = repo.path().canonicalize().unwrap().display().to_string();
        let mut record = runtime
            .block_on(
                registered
                    .store
                    .pm_task_observation(&scope, "linear", "INF-123"),
            )
            .unwrap()
            .record
            .unwrap();
        record.item.revision = Some("2026-10-04T12:00:00Z".into());
        if condition == "terminal" {
            record.item.state = Some("canceled".into());
        }
        if condition == "moved" {
            record.project.as_mut().unwrap().id = "another-project".into();
            record.item.project_id = Some("another-project".into());
        }
        runtime
            .block_on(
                registered
                    .store
                    .put_pm_task(&scope, "linear", record, None, None),
            )
            .unwrap();
        // Age the acquired facts without pretending a stale provider response won.
        rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .execute_batch(
                "UPDATE pm_items SET observed_at=1; UPDATE pm_projects SET observed_at=1;",
            )
            .unwrap();
        if condition == "invalid" || condition == "removed" {
            runtime
                .block_on(registered.store.observe_pm_issue_change(
                    registered.task.plan.id.as_str(),
                    None,
                    condition == "removed",
                ))
                .unwrap();
        }
        let mut accepted_plan = runtime
            .block_on(registered.store.get_task(&registered.task.id))
            .unwrap()
            .unwrap()
            .plan;
        let mut args = vec!["task", "restart", "INF-123", "--flow", "proof", "--json"];
        if condition == "advice" {
            args.push("new direction requires publication");
        }
        if condition == "delayed" {
            let hook = repo.path().join(".git/hooks/pre-commit");
            fs::write(&hook,
                "#!/bin/sh\ntouch \"$LF_HOME/checkpoint-waiting\"\nfor attempt in $(seq 1 300); do\n  [ -f \"$LF_HOME/planning-accepted\" ] && exit 0\n  sleep 0.1\ndone\nexit 1\n",
            ).unwrap();
            fs::set_permissions(hook, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut child = command(repo.path(), home.path(), &args)
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if condition == "delayed" {
            let deadline = Instant::now() + Duration::from_secs(20);
            while !home.path().join("checkpoint-waiting").exists() {
                assert!(
                    child.try_wait().unwrap().is_none(),
                    "restart exited before checkpoint"
                );
                assert!(Instant::now() < deadline, "restart never checkpointed");
                std::thread::sleep(Duration::from_millis(20));
            }
            let mut record = runtime
                .block_on(
                    registered
                        .store
                        .pm_task_observation(&scope, "linear", "INF-123"),
                )
                .unwrap()
                .record
                .unwrap();
            record.item.name = "New direction during restart".into();
            record.item.description = "Preserve the accepted provider change".into();
            record.item.revision = Some("2026-10-05T12:00:00Z".into());
            record.observed_at = OffsetDateTime::now_utc().unix_timestamp() + 1;
            runtime
                .block_on(
                    registered
                        .store
                        .put_pm_task(&scope, "linear", record, None, None),
                )
                .unwrap();
            accepted_plan = runtime
                .block_on(registered.store.get_task(&registered.task.id))
                .unwrap()
                .unwrap()
                .plan;
            fs::write(home.path().join("planning-accepted"), "accepted").unwrap();
        }
        let output = child.wait_with_output().unwrap();
        let current = runtime
            .block_on(registered.store.task_flow(&registered.task.id))
            .unwrap()
            .unwrap();
        if matches!(condition, "valid" | "delayed") {
            assert!(
                output.status.success(),
                "{} worker: {}",
                String::from_utf8_lossy(&output.stderr),
                fs::read_to_string(home.path().join("worker.log")).unwrap_or_default()
            );
            let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(
                response["pm_snapshot_synced_at"],
                accepted_plan.pm_snapshot_synced_at
            );
            assert_ne!(current.id(), old.id());
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let current = runtime
                    .block_on(registered.store.task_flow(&registered.task.id))
                    .unwrap()
                    .unwrap();
                if current.is_human() {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "replacement never reached review: {current:?}"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(runtime
                .block_on(registered.store.flow(&old_id))
                .unwrap()
                .is_some());
            let task = runtime
                .block_on(registered.store.get_task(&registered.task.id))
                .unwrap()
                .unwrap();
            assert_eq!(task.id, registered.task.id);
            assert_eq!(task.project_id, registered.task.project_id);
            assert_eq!(task.wave_id, registered.task.wave_id);
            assert_eq!(task.worktree, registered.task.worktree);
            assert_eq!(task.plan, accepted_plan);
        } else {
            assert!(!output.status.success(), "{condition} admitted restart");
            let error = String::from_utf8_lossy(&output.stderr);
            let expected = match condition {
                "terminal" => "terminal",
                "moved" => "no longer matches",
                "advice" => "Restart stopped before replacing",
                _ => "planning",
            };
            assert!(error.contains(expected), "{condition}: {error}");
            assert_eq!(current, old, "{condition} replaced existing execution");
        }
        assert_eq!(
            runtime
                .block_on(registered.store.active_task_pr(&registered.task.id))
                .unwrap()
                .unwrap()
                .id,
            registered.pr.id
        );
    }
}

struct ReviewProcess {
    pid: u32,
    exited: std::sync::mpsc::Receiver<std::process::ExitStatus>,
    stop: std::sync::mpsc::Sender<()>,
    waiter: Option<std::thread::JoinHandle<()>>,
}

impl ReviewProcess {
    fn start() -> Self {
        let mut child = Command::new("sleep").arg("60").spawn().unwrap();
        let pid = child.id();
        let (stop, stopped) = std::sync::mpsc::channel();
        let (done, exited) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || loop {
            if let Some(status) = child.try_wait().unwrap() {
                let _ = done.send(status);
                break;
            }
            if stopped.try_recv().is_ok() {
                let _ = child.kill();
                let _ = done.send(child.wait().unwrap());
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        });
        Self {
            pid,
            exited,
            stop,
            waiter: Some(waiter),
        }
    }

    fn record_exec(&self, home: &Path, conn: &rusqlite::Connection) -> loopflow::id::ExecId {
        let id = loopflow::id::ExecId::new();
        let now = OffsetDateTime::now_utc().unix_timestamp();
        conn.execute(
            "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,?1,?2)",
            rusqlite::params![id, now],
        )
        .unwrap();
        let root = home.join("runtime/exec-processes");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(format!("{}.json", self.pid)),
            serde_json::to_vec(&serde_json::json!({
                "schema_version":1, "trace_id":id, "exec_id":id, "pid":self.pid,"started_at":now
            }))
            .unwrap(),
        )
        .unwrap();
        id
    }
}

impl Drop for ReviewProcess {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        self.waiter.take().unwrap().join().unwrap();
    }
}

#[test]
fn waiting_review_restart_retires_exact_execution_and_retries_interrupted_stop() {
    for boundary in ["complete", "interrupted", "unknown", "independent"] {
        let _env = support::EnvGuard::new(&[
            ("tmux", "#!/bin/sh\nif [ \"$1\" = new-session ]; then\n[ -z \"$LF_TEST_LAUNCH_FAIL\" ] || exit 1\nfor arg do command=$arg; done\n/bin/sh -c \"$command\" </dev/null >\"$LF_TEST_WORKER_LOG\" 2>&1 &\nfi\nexit 0\n"),
            ("open", "#!/bin/sh\nexit 0\n"),
            ("gh", "#!/bin/sh\nexit 1\n"),
            ("kill", "#!/bin/sh\nif [ -n \"$LF_TEST_RESTART_PAUSE\" ]; then touch \"$LF_TEST_RESTART_PAUSE\"; sleep 2; exit 1; fi\nexec /bin/kill \"$@\"\n"),
        ]);
        let repo = TestRepo::new();
        support::bind_task_planning(&repo);
        repo.create_branch("restart-review");
        let home = tempfile::tempdir().unwrap();
        let registered = support::register_task(
            home.path(),
            &repo.path().canonicalize().unwrap(),
            "restart-review",
            &repo.head_sha(),
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let old_id = flow(&registered.store, &registered.task, true);
        let old = runtime
            .block_on(registered.store.task_flow(&registered.task.id))
            .unwrap()
            .unwrap();
        let waiting = runtime
            .block_on(registered.store.reserve_task_review(old.id(), old.version))
            .unwrap();
        let id = waiting.pending_session_id.as_ref().unwrap();
        let sqlite =
            loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
        let session = sqlite.session(id).unwrap().unwrap();
        let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let service = ReviewProcess::start();
        let driver = ReviewProcess::start();
        let provider = ReviewProcess::start();
        let unrelated = ReviewProcess::start();
        let service_exec = service.record_exec(home.path(), &conn);
        let driver_exec = driver.record_exec(home.path(), &conn);
        let owner = sqlite
            .claim_session_driver(id, None, &driver_exec, true)
            .unwrap();
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,exec_id,observed_at,payload,captured_event)
            VALUES(?1,'observed',?2,?3,1,'{\"type\":\"review_service\"}',?4)",
            rusqlite::params![id, format!("review_service:{service_exec}"),service_exec,session.captured]).unwrap();
        // The service has its own exact receipt; neither capture nor driver names it.
        conn.execute("UPDATE agent_sessions SET input_published=1,provider='sleep',provider_thread='retained-native-thread' WHERE id=?1", [id]).unwrap();
        let prefix = &session
            .artifact_key
            .strip_prefix("run_")
            .unwrap_or(&session.artifact_key)[..2];
        let dir = home
            .path()
            .join("runs")
            .join(prefix)
            .join(&session.artifact_key);
        fs::create_dir_all(dir.join("provider-clients")).unwrap();
        fs::write(dir.join("manifest.json"), serde_json::to_vec(&serde_json::json!({
            "schema_version":1, "artifact_key":session.artifact_key,
            "created_at":OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap(),
            "harness":"sleep", "surface":"tui", "cwd":repo.path(), "subjects":[],"host":"fixture"
        })).unwrap()).unwrap();
        fs::write(dir.join("provider-clients").join(format!("{}.json",provider.pid)), serde_json::to_vec(&serde_json::json!({
            "schema_version":1,"pid":provider.pid,"terminal_id":null,
            "started_at":OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap()
        })).unwrap()).unwrap();
        fs::write(dir.join("native-history"), "retained provider history").unwrap();
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,provider_thread,provider_turn,observed_at,payload,captured_event)
            VALUES(?1,'started','original-turn','retained-native-thread','turn',1,'{}',?2)",rusqlite::params![id,session.captured]).unwrap();
        fs::write(
            repo.path().join(".lf/flows/proof.yaml"),
            "- cmd: task sync --plan\n- step:\n    name: demo\n    id: review\n    human: true\n",
        )
        .unwrap();
        if boundary == "unknown" {
            fs::remove_file(
                home.path()
                    .join("runtime/exec-processes")
                    .join(format!("{}.json", service.pid)),
            )
            .unwrap();
        }
        if boundary == "independent" {
            let mut independent = session.clone();
            independent.id = "independent-review".into();
            independent.captured = None;
            independent.artifact_key = "run_00000000000000000000000000001234".into();
            independent.flow_session_id = None;
            independent.input_published = true;
            runtime
                .block_on(registered.store.create_session(independent, None))
                .unwrap();
        }
        let restart = || {
            let mut command = command(
                repo.path(),
                home.path(),
                &["task", "restart", "INF-123", "--flow", "proof", "--json"],
            );
            command
                .env("HTTPS_PROXY", "http://127.0.0.1:1")
                .env("HTTP_PROXY", "http://127.0.0.1:1");
            command
        };
        if boundary == "interrupted" {
            let paused = home.path().join("stopping-driver");
            let log = fs::File::create(home.path().join("interrupted.log")).unwrap();
            let mut child = restart()
                .env("LF_TEST_RESTART_PAUSE", &paused)
                .stdout(log.try_clone().unwrap())
                .stderr(log)
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(15);
            while !paused.exists() {
                assert!(
                    child.try_wait().unwrap().is_none(),
                    "{}",
                    fs::read_to_string(home.path().join("interrupted.log")).unwrap()
                );
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
            child.kill().unwrap();
            child.wait().unwrap();
            assert_eq!(
                runtime
                    .block_on(registered.store.task_flow(&registered.task.id))
                    .unwrap()
                    .unwrap()
                    .id(),
                old_id
            );
            assert!(sqlite.session(id).unwrap().unwrap().completed_at.is_some());
            assert!(sqlite
                .record_session_connection(id, &owner, "stale", "stale")
                .is_err());
            assert!(sqlite
                .claim_session_driver(
                    id,
                    sqlite.session_driver(id).unwrap().as_ref(),
                    &driver_exec,
                    false
                )
                .is_err());
            assert!(fs::read_dir(dir.join("provider-clients"))
                .unwrap()
                .next()
                .is_none());
            assert_eq!(conn.query_row("SELECT count(*) FROM session_events WHERE session_id=?1 AND receipt_key='task_restart:stopped'",[id],|row| row.get::<_, i64>(0)).unwrap(),0);
        }
        let output = if boundary == "launching" {
            let root = home.path().join("human-sessions");
            fs::create_dir_all(&root).unwrap();
            let name = hex::encode(&Sha256::digest(id.as_bytes())[..16]);
            let lock = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(root.join(format!(".{name}.launch.lock")))
                .unwrap();
            fs2::FileExt::lock_exclusive(&lock).unwrap();
            let child = restart()
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            std::thread::sleep(Duration::from_millis(250));
            assert!(sqlite.session(id).unwrap().unwrap().completed_at.is_none());
            // Finish an in-flight launch while holding its real lock. The new
            // exact child must be included after restart acquires that lock.
            let late = ReviewProcess::start();
            let exec = late.record_exec(home.path(), &conn);
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,exec_id,observed_at,payload,captured_event)
                VALUES(?1,'observed',?2,?3,1,'{\"type\":\"review_service\"}',?4)",
                rusqlite::params![id,format!("review_service:{exec}"),exec,session.captured]).unwrap();
            conn.execute(
                "UPDATE flow_sessions SET position_version=position_version+1 WHERE id=?1",
                [&old_id],
            )
            .unwrap();
            drop(lock);
            let output = child.wait_with_output().unwrap();
            assert!(!late
                .exited
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .success());
            output
        } else if boundary == "launch" {
            let failed = restart().env("LF_TEST_LAUNCH_FAIL", "1").output().unwrap();
            assert!(!failed.status.success());
            let saved = runtime
                .block_on(registered.store.task_flow(&registered.task.id))
                .unwrap()
                .unwrap();
            assert_ne!(saved.id(), old_id);
            assert!(saved.claim.is_none());
            let resumed = command(
                repo.path(),
                home.path(),
                &["--task", "INF-123", "flow", "start", "--json"],
            )
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .output()
            .unwrap();
            assert_eq!(
                runtime
                    .block_on(registered.store.task_flow(&registered.task.id))
                    .unwrap()
                    .unwrap()
                    .id(),
                saved.id()
            );
            resumed
        } else {
            restart().output().unwrap()
        };
        if boundary == "unknown" {
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("unresolved process identity"));
            assert_eq!(
                runtime
                    .block_on(registered.store.task_flow(&registered.task.id))
                    .unwrap()
                    .unwrap()
                    .id(),
                old_id
            );
            assert!(service.exited.try_recv().is_err());
            assert!(driver.exited.try_recv().is_err());
            assert!(provider.exited.try_recv().is_err());
            continue;
        }
        if boundary == "independent" {
            assert!(!output.status.success());
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("independent-review"),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(sqlite
                .session("independent-review")
                .unwrap()
                .unwrap()
                .completed_at
                .is_none());
        } else {
            assert!(
                output.status.success(),
                "{boundary}: {} worker: {}",
                String::from_utf8_lossy(&output.stderr),
                fs::read_to_string(home.path().join("worker.log")).unwrap_or_default()
            );
        }
        for process in [&service, &driver, &provider] {
            assert!(!process
                .exited
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .success());
        }
        assert!(unrelated.exited.try_recv().is_err());
        let retired = sqlite.session(id).unwrap().unwrap();
        assert_eq!(retired.captured, session.captured);
        assert_eq!(retired.artifact_key, session.artifact_key);
        assert!(retired.completed_at.is_some());
        assert!(retired.ready_summary.is_none());
        assert_eq!(
            fs::read_to_string(dir.join("native-history")).unwrap(),
            "retained provider history"
        );
        let completed: i64 = conn
            .query_row(
                "SELECT count(*) FROM session_events WHERE session_id=?1 AND kind='completed'",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(completed, 0, "retirement never manufactures review success");
        assert!(sqlite
            .record_session_connection(id, &owner, "stale", "stale")
            .is_err());
        assert_eq!(
            runtime
                .block_on(registered.store.active_task_pr(&registered.task.id))
                .unwrap()
                .unwrap()
                .id,
            registered.pr.id
        );
        let task = runtime
            .block_on(registered.store.get_task(&registered.task.id))
            .unwrap()
            .unwrap();
        assert_eq!(task.worktree, registered.task.worktree);
        assert_eq!(task.project_id, registered.task.project_id);
        let replacement = runtime
            .block_on(registered.store.task_flow(&task.id))
            .unwrap()
            .unwrap();
        if boundary == "independent" {
            assert_eq!(replacement.id(), old_id);
            continue;
        }
        assert_ne!(replacement.id(), old_id);
        {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let current = runtime
                    .block_on(registered.store.task_flow(&task.id))
                    .unwrap()
                    .unwrap();
                assert_eq!(current.id(), replacement.id());
                if current.is_human() {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "replacement failed to advance: {current:?}"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}
