use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use crate::support;
use loopflow_execution::machine::client::Connection;
use loopflow_test_support::TestRepo;
use sha2::{Digest, Sha256};

struct CreationFixture {
    repo: TestRepo,
    home: tempfile::TempDir,
}

struct Service {
    child: std::process::Child,
    socket: PathBuf,
    home: PathBuf,
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
        let mut service = Self {
            child,
            socket,
            home,
        };
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
        // A process still running would install its own server for this home
        // once this one stops. The terminal holder outlives its terminals.
        let deadline = Instant::now() + Duration::from_secs(30);
        while let Ok(db) = rusqlite::Connection::open(self.home.join("loopflow.db")) {
            let running: i64 = db
                .query_row(
                    "SELECT (SELECT count(*) FROM graph_processes WHERE ended_at IS NULL)
                        + (SELECT count(*) FROM task_graphs WHERE run_pid IS NOT NULL)",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            if running == 0 || Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(self.socket.parent().unwrap());
    }
}

#[test]
fn task_create_answers_and_its_conversation_starts_the_graph_across_a_server_restart() {
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
        parent: None,
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
    // The held conversation makes the checkout and launches the Graph; the
    // receipt stays as it was answered.
    task.wait_for_graph_start();
    let saved: serde_json::Value = serde_json::from_str(
        &db.query_row(
            "SELECT result FROM creation_receipts WHERE request_id=?1",
            [id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(saved["milestone"], "admitted", "{saved}");
    assert!(saved["session_id"].is_string(), "{saved}");
    task.wait_for_graphs();
    drop(service);
    let service = Service::start(&task);
    let reply = service.connect().request(operation()).unwrap();
    let replay = receipt(reply.result);
    let mut expected = saved.clone();
    let fields = expected["task"].as_object_mut().unwrap();
    for (released, current) in [
        ("linear_id", "tracker_id"),
        ("pm_snapshot_synced_at", "tracker_synced_at"),
        ("pm_writeback", "tracker_writeback"),
    ] {
        let value = fields.remove(released).expect("released receipt format");
        assert!(fields.insert(current.into(), value).is_none());
    }
    assert_eq!(replay, expected);
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
        receipt(serde_json::from_slice(&output.stdout).unwrap()),
        replay
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

#[test]
fn task_create_answers_and_its_conversation_checks_out_without_the_network() {
    use loopflow::lf::commands::write::Write as Change;
    use loopflow::server::wire::Operation;
    let task = CreationFixture::new();
    task.repo.push();
    // Every fetch waits for the test; neither the answer nor the checkout does.
    let release = task.home.path().join("release-fetch");
    let git = task.home.path().join("bin/git");
    fs::write(
        &git,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do [ \"$arg\" = fetch ] && while [ ! -e '{}' ]; do sleep 0.05; done; done\nexec /usr/bin/git \"$@\"\n",
            release.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let service = Service::start(&task);
    let id = uuid::Uuid::new_v4().to_string();
    let answer = service
        .connect()
        .request(Operation::Write(Change::TaskCreate {
            directory: task.repo.path().into(),
            request_id: id.clone(),
            request: Some("Answer before the checkout".into()),
            title: None,
            session: None,
            run: true,
            wave: None,
            parent: None,
            graph: Some("feature".into()),
        }))
        .unwrap()
        .result;
    assert_eq!(answer["milestone"], "admitted", "{answer}");
    assert_eq!(answer["graph"], "feature");
    let session = answer["session_id"].as_str().unwrap().to_owned();
    // The answer opens the conversation without another read.
    assert_eq!(answer["session"]["id"], session.as_str());
    assert!(!answer["session"]["open_argv"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(answer["row"]["graph_name"], "feature");
    assert_eq!(answer["row"]["section"], "working");
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let graphs = || -> i64 {
        db.query_row("SELECT count(*) FROM graph_processes", [], |row| row.get(0))
            .unwrap()
    };
    // The admission took up the Graph.
    assert!(db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM task_graphs WHERE task_id=?1)",
            [answer["task"]["id"].as_str().unwrap()],
            |row| row.get::<_, bool>(0),
        )
        .unwrap());

    // A repeat during setup answers the same Task and conversation.
    let repeat = task.run(&[
        "task",
        "create",
        "Answer before the checkout",
        "--run",
        "--graph",
        "feature",
        "--request-id",
        &id,
        "--json",
    ]);
    assert!(
        repeat.status.success(),
        "{}",
        String::from_utf8_lossy(&repeat.stderr)
    );
    let repeated: serde_json::Value = serde_json::from_slice(&repeat.stdout).unwrap();
    assert_eq!(repeated["task"]["id"], answer["task"]["id"]);
    assert_eq!(repeated["session_id"], session.as_str());
    assert_eq!(repeated["session"]["id"], session.as_str());

    // The conversation checks out from local refs and starts the Graph while
    // every fetch still waits.
    task.wait_for_graph_start();
    fs::write(&release, "").unwrap();
    task.wait_for_graphs();
    assert_eq!(graphs(), 1);
    let milestone: String = db
        .query_row(
            "SELECT milestone FROM creation_receipts WHERE request_id=?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    // Setup is owed by the Task; the receipt only answers repeats.
    assert_eq!(milestone, "admitted");
    // A Start's report reads the server's and the conversation's steps by
    // request ID.
    let log = fs::read_to_string(task.home.path().join("logs/lf.log")).unwrap();
    let timed = |span: &str, step: &str| {
        log.lines().any(|line| {
            line.contains(&format!("request_id=\"{id}\""))
                && line.contains(&format!("{span}{{"))
                && line.contains(&format!("step=\"{step}\""))
                && line.contains("elapsed_ms=")
        })
    };
    for step in ["file_task", "hold_conversation", "read_session", "row"] {
        assert!(timed("creation_step", step), "no {step} timing:\n{log}");
    }
    for step in ["checkout", "graph"] {
        assert!(
            timed("conversation_setup", step),
            "no {step} timing:\n{log}"
        );
    }
}

#[test]
fn task_create_takes_the_ready_checkout_and_leaves_another() {
    use std::os::unix::fs::MetadataExt;
    let task = CreationFixture::new();
    task.repo.push();
    let _service = Service::start(&task);
    let ready = loopflow_execution::git::worktrees::ready_checkout(task.repo.path());
    // The first Start in a repository without one makes it the slower way.
    let first = task.start("First Task");
    task.wait_for_graphs();
    task.wait_for_ready(&ready);
    let made = fs::metadata(&ready).unwrap().ino();
    // Git as Loopflow runs it, from now on.
    let log = task.home.path().join("git.log");
    let git = task.home.path().join("bin/git");
    fs::write(
        &git,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\nexec /usr/bin/git \"$@\"\n",
            log.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let second = task.start("Second Task");
    task.wait_for_graphs();
    let checkout = task.checkout(&second);
    assert_ne!(checkout, task.checkout(&first));
    assert_eq!(
        fs::metadata(&checkout).unwrap().ino(),
        made,
        "the second Task took the ready checkout"
    );
    let calls = fs::read_to_string(&log).unwrap();
    assert!(
        calls
            .lines()
            .any(|line| line.starts_with("-C") && line.contains("worktree move")),
        "{calls}"
    );
    // Only the replacement adds a worktree, detached at the ready path.
    assert!(
        calls
            .lines()
            .filter(|line| line.contains("worktree add"))
            .all(|line| line.contains("--detach")),
        "{calls}"
    );
    task.wait_for_ready(&ready);
    assert_ne!(fs::metadata(&ready).unwrap().ino(), made);
    // Cleanup sees no Task's checkout there and leaves it alone.
    // Build cleanup reads open files with lsof.
    let pruned = task
        .command()
        .args(["wt", "prune", "--all"])
        .env(
            "PATH",
            format!(
                "{}:/usr/bin:/bin:/usr/sbin",
                task.home.path().join("bin").display()
            ),
        )
        .output()
        .unwrap();
    assert!(
        pruned.status.success(),
        "{}",
        String::from_utf8_lossy(&pruned.stderr)
    );
    assert!(ready.join(".git").is_file());
    let listed = task.run(&["wt", "list", "--json"]);
    assert!(!String::from_utf8_lossy(&listed.stdout).contains(".ready"));
    // Deleting it by hand leaves Git's registration; the next Start replaces it.
    fs::remove_dir_all(&ready).unwrap();
    task.start("Third Task");
    task.wait_for_graphs();
    task.wait_for_ready(&ready);
}

#[test]
fn a_stopped_setup_shows_on_the_task_and_a_reopened_conversation_retries_it() {
    let task = CreationFixture::new();
    task.repo.push();
    let _service = Service::start(&task);
    // Git refuses every new checkout while the marker exists, once `released`
    // lets it answer.
    let marker = task.home.path().join("refuse-checkouts");
    let released = task.home.path().join("checkouts-answer");
    fs::write(&marker, "").unwrap();
    let git = task.home.path().join("bin/git");
    fs::write(
        &git,
        format!(
            "#!/bin/sh\ncase \"$*\" in *\"worktree add\"*|*\"worktree move\"*) while [ ! -e '{}' ]; do sleep 0.05; done; [ -e '{}' ] && exit 1;; esac\nexec /usr/bin/git \"$@\"\n",
            released.display(),
            marker.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let output = task.run(&[
        "task",
        "create",
        "Stops at checkout",
        "--run",
        "--graph",
        "feature",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answer: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let issue = answer["task"]["identifier"].as_str().unwrap().to_owned();
    let session = answer["session_id"].as_str().unwrap().to_owned();
    let reason = |status: &std::process::Output| {
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        status["task"]["reason"].as_str().unwrap().to_owned()
    };
    // Until setup answers, the Task is starting the Graph it took up.
    let before = task.run(&["task", "status", &issue, "--json"]);
    assert_eq!(reason(&before), "Starting feature");
    fs::write(&released, "").unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        let status = task.run(&["task", "status", &issue, "--json"]);
        let text = String::from_utf8_lossy(&status.stdout).into_owned();
        if text.contains("Setup stopped: checkout failed") {
            let reason = reason(&status);
            assert!(reason.starts_with("Setup stopped"), "{text}");
            assert!(
                reason.ends_with(&format!(
                    "Reopen the conversation or run `lf task run {issue}` to retry."
                )),
                "{text}"
            );
            break text;
        }
        assert!(
            Instant::now() < deadline,
            "setup's stop never reached the Task: {text}\n{}",
            fs::read_to_string(task.home.path().join("logs/lf.log")).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(!status.contains("Starting"), "{status}");
    // With the cause gone, opening the conversation again sets the Task up.
    fs::remove_file(&marker).unwrap();
    let reopened = task.run(&["session", "resume", &session]);
    task.wait_for_graph_start();
    task.wait_for_graphs();
    let status = task.run(&["task", "status", &issue, "--json"]);
    assert!(
        !String::from_utf8_lossy(&status.stdout).contains("Setup stopped"),
        "{}\n{}",
        String::from_utf8_lossy(&status.stdout),
        String::from_utf8_lossy(&reopened.stdout)
    );
}

#[test]
fn task_create_with_an_agent_runs_its_conversation_and_graph_on_it() {
    let task = CreationFixture::new();
    task.repo.push();
    let _service = Service::start(&task);
    let codex = task.home.path().join("bin/codex");
    fs::write(&codex, PARENTED_PROVIDER).unwrap();
    fs::set_permissions(&codex, fs::Permissions::from_mode(0o755)).unwrap();
    // Every `lf` the Start causes, with the agent its environment names.
    let starts = task.home.path().join("lf-starts");
    let lf = task.home.path().join("bin/lf-recorder");
    fs::write(
        &lf,
        format!(
            "#!/bin/sh\necho \"$*|${{LF_AGENT_OVERRIDE-}}\" >> '{}'\nexec '{}' \"$@\"\n",
            starts.display(),
            env!("CARGO_BIN_EXE_lf")
        ),
    )
    .unwrap();
    fs::set_permissions(&lf, fs::Permissions::from_mode(0o755)).unwrap();
    let output = task
        .command()
        .env("LF_BIN", &lf)
        .args([
            "-a",
            "codex",
            "task",
            "create",
            "Run on codex",
            "--run",
            "--graph",
            "feature",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answer: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let session = answer["session_id"].as_str().unwrap().to_owned();
    task.wait_for_graph_start();
    task.wait_for_graphs();
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let provider: String = db
        .query_row(
            "SELECT provider FROM agent_sessions WHERE id=?1",
            [session.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(provider, "codex");
    let parent = task.provider_parent();
    assert!(parent.contains("--agent codex session resume"), "{parent}");
    let calls = fs::read_to_string(&starts).unwrap();
    assert!(
        !calls.lines().any(|line| line.starts_with("-i ")),
        "the conversation exec'd a second lf: {calls}"
    );
    let graph = calls
        .lines()
        .find(|line| {
            line.contains(&format!(
                "task run {}",
                answer["task"]["identifier"].as_str().unwrap()
            ))
        })
        .unwrap_or_else(|| panic!("no Task run: {calls}"));
    assert!(graph.contains("--agent codex"), "{calls}");
}

/// A provider stand-in that records its PID and the command line of the
/// process that started it, then fails its launch.
const PARENTED_PROVIDER: &str = "#!/bin/sh
if [ \"$1\" = --version ]; then echo fixture; exit 0; fi
echo \"$$ $(ps -o command= -p \"$PPID\")\" >> \"$HOME/provider-parents\"
exit 99
";

#[test]
fn task_create_runs_its_provider_in_the_held_session_resume() {
    let task = CreationFixture::new();
    task.repo.push();
    let _service = Service::start(&task);
    let output = task.run(&[
        "task",
        "create",
        "Run in the held conversation",
        "--run",
        "--graph",
        "feature",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answer: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(answer["session_id"].is_string(), "{answer}");
    // A held provider replaces the process that launched it.
    let line = task.provider_parent();
    let (_, parent) = line.split_once(' ').unwrap();
    assert!(parent.contains("holder run"), "{line}");
    task.wait_for_graph_start();
    task.wait_for_graphs();
}

impl CreationFixture {
    /// `<pid> <parent's command line>` of the conversation's provider.
    fn provider_parent(&self) -> String {
        let parents = self.home.path().join("provider-parents");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(line) = fs::read_to_string(&parents)
                .unwrap_or_default()
                .lines()
                .next()
            {
                return line.to_owned();
            }
            assert!(
                Instant::now() < deadline,
                "the conversation started no provider:\n{}",
                fs::read_to_string(self.home.path().join("logs/lf.log")).unwrap_or_default()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

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
        for (name, text) in [("gh", "#!/bin/sh\nexit 1\n"), ("claude", PARENTED_PROVIDER)] {
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

    /// Start a new Task as Desktop does and wait for its conversation's Graph.
    fn start(&self, request: &str) -> String {
        let before: i64 = rusqlite::Connection::open(self.home.path().join("loopflow.db"))
            .unwrap()
            .query_row("SELECT count(*) FROM graph_processes", [], |row| row.get(0))
            .unwrap();
        let output = self.run(&[
            "task", "create", request, "--run", "--graph", "feature", "--json",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let answer: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let db = rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        while db
            .query_row("SELECT count(*) FROM graph_processes", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap()
            == before
        {
            assert!(
                Instant::now() < deadline,
                "the conversation started no Graph: {}",
                fs::read_to_string(self.home.path().join("logs/lf.log")).unwrap_or_default()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        answer["task"]["id"].as_str().unwrap().to_owned()
    }

    fn checkout(&self, task: &str) -> PathBuf {
        let db = rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap();
        let path: String = db
            .query_row(
                "SELECT worktree FROM task_checkouts WHERE task_id=?1",
                [task],
                |row| row.get(0),
            )
            .unwrap();
        PathBuf::from(path)
    }

    /// The background `git worktree add` has made the ready checkout: Git
    /// lists it, no longer holds its initializing lock, and it is on disk.
    fn wait_for_ready(&self, ready: &std::path::Path) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let listed = Command::new("/usr/bin/git")
                .arg("-C")
                .arg(self.repo.path())
                .args(["worktree", "list", "--porcelain"])
                .output()
                .unwrap();
            let listed = String::from_utf8_lossy(&listed.stdout).into_owned();
            let entry = listed.split("\n\n").find(|entry| {
                entry
                    .lines()
                    .next()
                    .and_then(|line| line.strip_prefix("worktree "))
                    == Some(ready.to_str().unwrap())
            });
            if entry.is_some_and(|entry| !entry.contains("locked")) && ready.join(".git").is_file()
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "no ready checkout at {}: {listed}",
                ready.display()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The conversation's detached Task run registers its Graph.
    fn wait_for_graph_start(&self) {
        let db = rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        while db
            .query_row("SELECT count(*) FROM graph_processes", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap()
            == 0
        {
            assert!(
                Instant::now() < deadline,
                "the conversation started no Graph"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn wait_for_graphs(&self) {
        let db = rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let active: i64 = db
                .query_row(
                    "SELECT count(*) FROM graph_processes WHERE ended_at IS NULL",
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
    let service = Service::start(&task);
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
    assert_eq!(receipt["milestone"], "admitted");
    assert_eq!(receipt["graph"], "feature");
    assert!(receipt["session_id"].is_string());
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    task.wait_for_graph_start();
    task.wait_for_graphs();
    let counts = || -> (i64, i64, i64) {
        db.query_row("SELECT (SELECT count(*) FROM tasks), (SELECT count(*) FROM graph_processes), (SELECT count(*) FROM agent_sessions)", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap()
    };
    let after = counts();
    let retry = task.run(&args);
    assert!(retry.status.success());
    assert_eq!(
        self::receipt(serde_json::from_slice(&retry.stdout).unwrap()),
        self::receipt(receipt.clone())
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
    let lookup = service
        .connect()
        .request(loopflow::server::wire::Operation::Read(
            loopflow::lf::commands::read::Read::CreationGet {
                request_id: id,
                task: None,
            },
        ))
        .unwrap()
        .result;
    assert_eq!(lookup["disposition"], "inspect");
}

#[test]
fn task_create_request_preserves_a_filed_task_after_start_failure() {
    let task = CreationFixture::new();
    let _service = Service::start(&task);
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
        receipt(serde_json::from_slice(&retry.stdout).unwrap()),
        receipt(outcome)
    );
    assert_eq!(support::recorded_graphs(task.home.path()), before);
}

#[test]
fn batch_creation_returns_the_original_runs_success_after_a_failed_attempt() {
    let task = CreationFixture::new();
    let _service = Service::start(&task);
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
    assert_eq!(
        receipt(serde_json::from_slice(&retry.stdout).unwrap()),
        receipt(serde_json::from_slice(&output.stdout).unwrap())
    );
    assert_eq!(support::recorded_graphs(task.home.path()), before);
}

#[test]
fn task_create_without_recovery_keeps_the_existing_receipt_and_exit_status() {
    let task = CreationFixture::new();
    let _service = Service::start(&task);
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
    let answer: loopflow::lf::commands::task_create::TaskCreated =
        serde_json::from_slice(&created.stdout).unwrap();
    let row = &answer.row;
    assert_eq!(answer.receipt.graph.as_deref(), Some("feature"));
    // The answer's row is the plan's: started, never "Not started".
    assert_eq!(row.task.id, answer.receipt.task.id.as_str());
    assert_eq!(
        row.section,
        loopflow_execution::durable::TaskSection::Working
    );
    assert_eq!(
        answer.session.map(|session| session.id),
        answer.receipt.session_id
    );
    assert_eq!(row.graph_name.as_deref(), Some("feature"));
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
    let receipt: loopflow_execution::engine::creation::TaskCreateReceipt =
        serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(receipt.task.title, "Ordinary failure");
    assert_eq!(receipt.graph.as_deref(), Some("gate"));
}

/// An answer's stored receipt: a repeat reads the conversation record and the
/// Task's row again, so only the receipt repeats exactly.
fn receipt(mut answer: serde_json::Value) -> serde_json::Value {
    let fields = answer.as_object_mut().unwrap();
    for field in ["session", "row", "parent"] {
        assert!(fields.remove(field).is_some(), "missing {field}");
    }
    answer
}
