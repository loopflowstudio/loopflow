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
    let id = uuid::Uuid::new_v4().to_string();
    let change = Change::TaskCreate {
        directory: task.repo.path().into(),
        request_id: id.clone(),
        request: Some("Create from the phone".into()),
        title: None,
        session: None,
        run: true,
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
                "SELECT EXISTS(SELECT 1 FROM creation_receipts WHERE request_id=?1)",
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
    let saved: serde_json::Value =
        loop {
            use rusqlite::OptionalExtension;
            let result: Option<String> = db.query_row(
            "SELECT result FROM creation_receipts WHERE request_id=?1 AND milestone='admitted'",
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
    assert_eq!(saved["milestone"], "admitted", "{saved}");
    task.wait_for_graphs();
    drop(service);
    let service = Service::start(&task);
    let reply = service.connect().request(operation()).unwrap();
    assert_eq!(reply.result, saved);
    let output = task.run(&[
        "task",
        "create",
        "Create from the phone",
        "--run",
        "--graph",
        "feature",
        "--request-id",
        &id,
        "--json",
    ]);
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
    let task = CreationFixture::new();
    task.repo.push();
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
    let first = task.run(&args);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    task.wait_for_graphs();
    assert_eq!(receipt["milestone"], "admitted");
    assert_eq!(receipt["graph"], "feature");
    assert!(receipt["session_id"].is_string());
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let counts = || -> (i64, i64, i64) {
        db.query_row("SELECT (SELECT count(*) FROM tasks), (SELECT count(*) FROM graph_processes), (SELECT count(*) FROM agent_sessions)", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap()
    };
    let after = counts();
    let retry = task.run(&args);
    assert!(retry.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&retry.stdout).unwrap(),
        receipt
    );
    assert_eq!(counts(), after);
    let mut changed = args;
    changed[2] = "A different request";
    assert!(!task.run(&changed).status.success());
    assert_eq!(counts(), after);
    // A lost launch acknowledgment must not advance an existing Graph.
    db.execute(
        "UPDATE creation_receipts SET milestone='run_requested',launch=NULL WHERE request_id=?1",
        [&id],
    )
    .unwrap();
    let retry = task.run(&args);
    assert!(!retry.status.success());
    let saved: serde_json::Value = serde_json::from_slice(&retry.stdout).unwrap();
    assert_eq!(saved["task"]["id"], receipt["task"]["id"]);
    assert_eq!(counts(), after);
    let lookup = Service::start(&task)
        .connect()
        .request(loopflow::server::wire::Operation::Read(
            loopflow::lf::commands::read::Read::CreationGet { request_id: id },
        ))
        .unwrap()
        .result;
    assert_eq!(lookup["disposition"], "inspect");
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
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let outcome: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(outcome["milestone"], "started");
    assert_eq!(outcome["task"]["title"], "Fails once");
    let before = support::recorded_graphs(task.home.path());
    assert_eq!(
        before.len(),
        3,
        "Batch creation waits for the original run's retry limit"
    );
    assert!(before
        .iter()
        .all(|(outcome, _)| outcome.as_deref() == Some("failed")));
    let retry = task.run(&args);
    assert!(!retry.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&retry.stdout).unwrap(),
        outcome
    );
    assert_eq!(support::recorded_graphs(task.home.path()), before);
}

#[test]
fn batch_creation_returns_the_original_runs_success_after_a_failed_attempt() {
    let task = CreationFixture::new();
    task.repo.push();
    let shim = task.home.path().join("bin/lf");
    fs::write(
        &shim,
        format!(
            r#"#!/bin/sh
if [ "$*" = 'graph show absent-fixture-graph' ]; then
  if [ -f "$LF_HOME/first-attempt" ]; then
    printf -- '- cmd: help task\n' > .lf/graphs/absent-fixture-graph.yaml
  else
    touch "$LF_HOME/first-attempt"
  fi
fi
exec {} "$@"
"#,
            shlex::try_quote(env!("CARGO_BIN_EXE_lf")).unwrap()
        ),
    )
    .unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let args = [
        "-b",
        "task",
        "create",
        "Recovers",
        "--run",
        "--graph",
        "gate",
        "--json",
        "--request-id",
        &id,
    ];
    let run = || {
        task.command()
            .env("LF_BIN", &shim)
            .args(args)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let before = support::recorded_graphs(task.home.path());
    assert_eq!(
        before
            .iter()
            .map(|(outcome, _)| outcome.as_deref())
            .collect::<Vec<_>>(),
        [Some("failed"), Some("succeeded")]
    );
    let retry = run();
    assert!(
        retry.status.success(),
        "{}",
        String::from_utf8_lossy(&retry.stderr)
    );
    assert_eq!(retry.stdout, output.stdout);
    assert_eq!(support::recorded_graphs(task.home.path()), before);
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
    let receipt: loopflow::engine::creation::TaskCreateReceipt =
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
    let receipt: loopflow::engine::creation::TaskCreateReceipt =
        serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(receipt.task.title, "Ordinary failure");
    assert_eq!(receipt.graph.as_deref(), Some("gate"));
}
