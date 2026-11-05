use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use crate::support;
use loopflow::server::client::Connection;
use loopflow_test_support::TestRepo;
use sha2::{Digest, Sha256};

struct CreationFixture {
    repo: TestRepo,
    home: tempfile::TempDir,
}

struct Service {
    child: std::process::Child,
    socket: PathBuf,
}

impl Service {
    fn start(fixture: &CreationFixture) -> Self {
        let home = fixture.home.path().canonicalize().unwrap();
        let hash = hex::encode(Sha256::digest(home.as_os_str().as_encoded_bytes()));
        // SAFETY: geteuid has no preconditions.
        let uid = unsafe { libc::geteuid() };
        let socket = PathBuf::from(format!("/tmp/lf-{uid}-{}", &hash[..16])).join("machine.sock");
        let child = fixture
            .command()
            .args(["server", "run"])
            .stdout(Stdio::null())
            .stderr(fs::File::create(fixture.home.path().join("service.stderr")).unwrap())
            .spawn()
            .unwrap();
        let mut service = Self { child, socket };
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if Connection::open_at(&service.socket).is_ok() {
                return service;
            }
            assert!(
                service.child.try_wait().unwrap().is_none(),
                "Loopflow server exited"
            );
            assert!(Instant::now() < deadline, "Loopflow server did not listen");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn connect(&self) -> Connection {
        let connection = Connection::open_at(&self.socket).unwrap();
        connection
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        connection
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(self.socket.parent().unwrap());
    }
}

#[test]
fn task_create_survives_a_dropped_carrier_and_server_restart() {
    use loopflow::lf::commands::write::Write as Change;
    use loopflow::server::wire::Operation;
    let task = CreationFixture::new();
    task.repo.push();
    let id = loopflow::id::TaskCreateRequestId::new();
    let change = Change::TaskStart {
        directory: task.repo.path().into(),
        request_id: id.clone(),
        request: "Create from the phone".into(),
        terminal: false,
        wave: None,
        graph: Some("feature".into()),
    };
    let operation = || Operation::Write(change.clone());
    let service = Service::start(&task);
    let mut lost = service.connect();
    lost.send(operation()).unwrap();
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let admitted: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM task_create_requests WHERE id=?1)",
                [id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        if admitted {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Machine command was not admitted"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(lost);
    let saved: serde_json::Value = loop {
        use rusqlite::OptionalExtension;
        let result: Option<String> = db.query_row(
            "SELECT result_json FROM task_create_requests WHERE id=?1 AND result_json IS NOT NULL",
            [id.as_str()], |row| row.get(0)).optional().unwrap();
        if let Some(result) = result {
            break serde_json::from_str(&result).unwrap();
        }
        assert!(
            Instant::now() < deadline,
            "creation did not settle after the client disconnected: {}\n{}",
            fs::read_to_string(task.home.path().join("service.stderr")).unwrap_or_default(),
            fs::read_to_string(task.home.path().join("logs/lf.log")).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(saved["state"], "started", "{saved}");
    task.wait_for_graphs();
    drop(service);
    let service = Service::start(&task);
    let reply = service.connect().request(operation()).unwrap();
    assert_eq!(reply.result, saved);
    let args = change
        .delegated()
        .expect("task.start runs as its own lf process");
    let output = task.run(&args.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        saved
    );
    let counts: (i64, i64, i64) = db
        .query_row(
            "SELECT (SELECT count(*) FROM tasks),
        (SELECT count(*) FROM graph_processes),(SELECT count(*) FROM agent_sessions)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(counts, (1, 1, 1));
}

impl CreationFixture {
    fn new() -> Self {
        let fixture = Self {
            repo: TestRepo::new(),
            home: tempfile::tempdir().unwrap(),
        };
        fixture
            .repo
            .create_file(".lf/config.yaml", "agent: claude\n");
        fixture.repo.create_file(
            ".lf/graphs/feature.yaml",
            "- cmd: help task\n- interactive: demo\n",
        );
        fixture.repo.create_file(
            ".lf/graphs/gate.yaml",
            "- cmd: graph show absent-fixture-graph\n",
        );
        fixture.repo.stage_all();
        fixture.repo.commit("Fixture Graphs");
        fs::create_dir(fixture.home.path().join("bin")).unwrap();
        for (name, text) in [
            ("gh", "#!/bin/sh\nexit 1\n"),
            (
                "claude",
                "#!/bin/sh\nif [ \"$1\" = --version ]; then echo fixture; exit 0; fi\nexit 99\n",
            ),
        ] {
            let path = fixture.home.path().join("bin").join(name);
            fs::write(&path, text).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let output = fixture.run(&["config", "repository", "--json"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fixture
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        command
            .env_clear()
            .env("HOME", self.home.path())
            .env("LF_HOME", self.home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.home.path().join("bin").display()),
            )
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .current_dir(self.repo.path())
            .stdin(Stdio::null());
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    fn wait_for_graphs(&self) {
        let db = rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let active: i64 = db
                .query_row(
                    "SELECT count(*) FROM graph_processes g
                JOIN processes p ON p.id=g.lf_process_id WHERE p.completed_at IS NULL",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            if active == 0 {
                break;
            }
            assert!(Instant::now() < deadline, "fixture Graph did not stop");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[test]
fn task_create_request_recovers_lost_replies_without_repeating_start() {
    use fs2::FileExt;

    let task = CreationFixture::new();
    assert!(Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(task.repo.path())
        .output()
        .unwrap()
        .status
        .success());
    let id = uuid::Uuid::new_v4().to_string();
    let args = [
        "task",
        "create",
        "Keep this request",
        "--run",
        "--graph",
        "feature",
        "--json",
        "--request-id",
        &id,
    ];
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let counts = || -> (i64, i64, i64) {
        db.query_row(
            "SELECT (SELECT count(*) FROM tasks),
            (SELECT count(*) FROM graph_processes), (SELECT count(*) FROM agent_sessions)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    };
    // A second caller can ask again while the first command holds admission;
    // it receives the reserved identity without filing or starting anything.
    let locks = task.home.path().join("task-create-locks");
    fs::create_dir_all(&locks).unwrap();
    let lock = fs::File::create(locks.join(&id)).unwrap();
    lock.try_lock_exclusive().unwrap();
    let before = counts();
    let pending = task.run(&args);
    assert!(
        pending.status.success(),
        "{}",
        String::from_utf8_lossy(&pending.stderr)
    );
    let pending: serde_json::Value = serde_json::from_slice(&pending.stdout).unwrap();
    assert_eq!(pending["state"], "in_progress");
    assert_eq!(counts(), before);
    // The original process filed its reserved Task, then exited before it
    // could mark/start the Graph. Recovery must keep that Task too.
    let repository: String = db
        .query_row("SELECT id FROM repository_plans", [], |row| row.get(0))
        .unwrap();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&task.home.path().join("loopflow.db")).unwrap();
    store
        .create_task(&loopflow::work::task::NewTask {
            id: loopflow::durable::TaskId::parse(pending["task_id"].as_str().unwrap()).unwrap(),
            repository_id: loopflow::durable::RepositoryId::parse(&repository).unwrap(),
            project_id: None,
            title: "Keep this request".into(),
            description: "Keep this request".into(),
            due_date: None,
        })
        .unwrap();
    drop(lock);

    let first = task.run(&args);
    let outcome: serde_json::Value = serde_json::from_slice(&first.stdout)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&first.stderr)));
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    task.wait_for_graphs();
    assert_eq!(outcome["state"], "started");
    assert_eq!(outcome["task_id"], pending["task_id"]);
    assert_eq!(outcome["receipt"]["graph"], "feature");
    assert!(outcome["receipt"]["session_id"].is_string());
    let after = counts();
    assert_eq!(after.0, before.0 + 1);

    // The first reply could have been lost. Repeating the exact command after
    // its process exited returns the original result, even at an interactive
    // Graph step where another task run would advance real work.
    let retry = task.run(&args);
    assert!(retry.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&retry.stdout).unwrap(),
        outcome
    );
    assert_eq!(counts(), after);
    let mut changed = args;
    changed[2] = "A different request";
    assert!(!task.run(&changed).status.success());
    assert_eq!(counts(), after);

    // Model a process exit after issuing start but before saving its result.
    // Absence of a reply is never permission to advance the Graph again.
    db.execute(
        "UPDATE task_create_requests SET result_json=NULL WHERE id=?1",
        [&id],
    )
    .unwrap();
    let retry = task.run(&args);
    assert!(retry.status.success());
    let unknown: serde_json::Value = serde_json::from_slice(&retry.stdout).unwrap();
    assert_eq!(unknown["state"], "unknown");
    assert_eq!(unknown["task_id"], outcome["task_id"]);
    assert_eq!(unknown["receipt"]["task"]["id"], outcome["task_id"]);
    assert_eq!(counts(), after);
}

#[test]
fn task_create_request_preserves_a_filed_task_after_start_failure() {
    let task = CreationFixture::new();
    assert!(Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(task.repo.path())
        .output()
        .unwrap()
        .status
        .success());
    let id = uuid::Uuid::new_v4().to_string();
    let args = [
        "-b",
        "task",
        "create",
        "Fails once",
        "--run",
        "--graph",
        "gate",
        "--json",
        "--request-id",
        &id,
    ];
    let output = task.run(&args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let outcome: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(outcome["state"], "failed");
    assert!(outcome["message"].is_string());
    assert_eq!(outcome["receipt"]["task"]["title"], "Fails once");
    let before = support::recorded_graphs(task.home.path()).len();
    let retry = task.run(&args);
    assert!(retry.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&retry.stdout).unwrap(),
        outcome
    );
    assert_eq!(support::recorded_graphs(task.home.path()).len(), before);
}

#[test]
fn task_create_without_recovery_keeps_the_existing_receipt_and_exit_status() {
    let task = CreationFixture::new();
    task.repo.push();
    let refused = task.run(&[
        "task",
        "create",
        "Unknown graph",
        "--run",
        "--graph",
        "absent",
        "--json",
    ]);
    assert!(!refused.status.success());
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let count: i64 = db
        .query_row("SELECT count(*) FROM tasks", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);

    let created = task.run(&[
        "task",
        "create",
        "Ordinary creation",
        "--run",
        "--graph",
        "feature",
        "--json",
    ]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let receipt: loopflow::lf::commands::task_create::TaskCreateReceipt =
        serde_json::from_slice(&created.stdout).unwrap();
    assert_eq!(receipt.graph.as_deref(), Some("feature"));
    assert!(receipt.session_id.is_some());
    task.wait_for_graphs();

    let failed = task.run(&[
        "-b",
        "task",
        "create",
        "Ordinary failure",
        "--run",
        "--graph",
        "gate",
        "--json",
    ]);
    assert!(!failed.status.success());
    let receipt: loopflow::lf::commands::task_create::TaskCreateReceipt =
        serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(receipt.task.title, "Ordinary failure");
    assert_eq!(receipt.graph, None);
}
