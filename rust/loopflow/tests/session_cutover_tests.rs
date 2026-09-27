//! Interactive and Ask Sessions are `sessions` and `runs` rows from launch to completion.
//! The real `lf` binary runs in a private Home; a script stands in for the provider.
#![cfg(unix)]

mod support;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use loopflow_test_support::TestRepo;
use serde_json::Value;

struct Fixture {
    home: tempfile::TempDir,
    repo: TestRepo,
    launched: PathBuf,
}

impl Fixture {
    /// `waits` keeps the stand-in attached until its stdin receives a line.
    fn new(waits: bool) -> Self {
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let launched = home.path().join("launched");
        let provider = bin.join("opencode");
        std::fs::write(
            &provider,
            format!(
                "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\n\
                 printf '%s\\n' \"message=created id=ses_$LF_RUN_ID\" >&2\n\
                 echo \"$LF_RUN_ID\" >> '{}'\n{}",
                launched.display(),
                if waits { "read -r input\n" } else { "" }
            ),
        )
        .unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
        // `lf ask` hands its conversation to a background launcher. The test
        // opens the Session itself, so the launcher only records its request.
        let tmux = bin.join("tmux");
        std::fs::write(
            &tmux,
            format!(
                "#!/bin/sh\necho \"$@\" >> '{}'\nif [ \"$1\" = has-session ]; then exit 1; fi\n",
                home.path().join("tmux.log").display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            home,
            repo: TestRepo::new(),
            launched,
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        for (key, _) in std::env::vars_os() {
            let key = key.to_string_lossy().into_owned();
            if key.starts_with("LF_") || key.starts_with("LOOPFLOW_") {
                command.env_remove(key);
            }
        }
        let home = self.home.path();
        command
            .args(args)
            .current_dir(self.repo.path())
            .env("HOME", home)
            .env("LF_HOME", home)
            .env("LF_CONTROL_HOME", home)
            .env("LF_DB_PATH", home.join("loopflow.db"))
            .env("LF_CONTROL_DB_PATH", home.join("loopflow.db"))
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    home.join("bin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("NO_COLOR", "1")
            .env("RUST_LOG", "off");
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "lf {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn sessions(&self) -> Vec<Value> {
        serde_json::from_value(self.json(&["session", "list", "--all", "--json"])).unwrap()
    }

    fn launches(&self) -> Vec<String> {
        std::fs::read_to_string(&self.launched)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// Start `lf` and return once the stand-in has recorded one more launch.
    fn attach(&self, args: &[&str]) -> (Child, String) {
        let before = self.launches().len();
        let mut child = self
            .command(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + PATIENCE;
        while self.launches().len() == before
            && Instant::now() < deadline
            && child.try_wait().unwrap().is_none()
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        let Some(run_id) = self.launches().get(before).cloned() else {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "lf {args:?} never reached the provider: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        (child, run_id)
    }

    fn release(&self, mut child: Child) {
        let _ = child
            .stdin
            .take()
            .map(|mut stdin| stdin.write_all(b"done\n"));
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{output:?}");
    }

    fn db(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap()
    }

    fn count(&self, table: &str) -> i64 {
        self.db()
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    /// (session id, kind, title, title_source, completed) of the Run's Session.
    fn session_row(&self, run_id: &str) -> (String, String, String, String, bool) {
        self.db()
            .query_row(
                "SELECT s.id, s.kind, s.title, s.title_source, s.completed_at IS NOT NULL
                 FROM sessions s JOIN runs r ON r.session_id=s.id AND s.current_run_id=r.id
                 WHERE r.id=?1",
                [run_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap_or_else(|error| panic!("Run {run_id} has no Session row: {error}"))
    }

    /// (task, wave, work_source) of one Run.
    fn run_parents(&self, run_id: &str) -> (Option<String>, Option<String>, Option<String>) {
        self.db()
            .query_row(
                "SELECT task_id, wave_id, work_source FROM runs WHERE id=?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or_else(|error| panic!("Run {run_id} has no row: {error}"))
    }

    fn run_dir(&self, run_id: &str) -> PathBuf {
        self.home
            .path()
            .join("runs")
            .join(&run_id[4..6])
            .join(run_id)
    }

    /// `lf ask` as called from inside the caller's Run.
    fn ask(&self, caller_run: &str, question: &str) -> Command {
        let mut command = self.command(&["ask", question]);
        command
            .env("LF_RUN_ID", caller_run)
            .env("LF_RUN_DIR", self.run_dir(caller_run))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn launcher_requests(&self) -> String {
        std::fs::read_to_string(self.home.path().join("tmux.log")).unwrap_or_default()
    }

    fn retired_files(&self) -> Vec<PathBuf> {
        fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, found);
                } else if ["session-name.json", "session-resolution.json"]
                    .contains(&entry.file_name().to_string_lossy().as_ref())
                    || path
                        .extension()
                        .is_some_and(|extension| extension == "json")
                        && dir.ends_with("human-sessions")
                {
                    found.push(path);
                }
            }
        }
        let mut found = Vec::new();
        walk(&self.home.path().join("runs"), &mut found);
        walk(&self.home.path().join("human-sessions"), &mut found);
        found
    }
}

/// An upper bound only: a debug `lf` on a busy machine takes tens of seconds to launch.
const PATIENCE: Duration = Duration::from_secs(180);

const LAUNCH: [&str; 5] = ["--tui", "--model", "opencode", ":", "Review the parser"];

#[test]
fn interactive_session_is_rows_from_launch_to_completion() {
    let fixture = Fixture::new(true);

    let (first, first_run) = fixture.attach(&LAUNCH);
    assert_eq!(
        (fixture.count("sessions"), fixture.count("runs")),
        (1, 1),
        "launch reserves one Session and its Run"
    );
    let (id, kind, title, source, completed) = fixture.session_row(&first_run);
    assert_ne!(id, first_run, "a Session is not its Run");
    assert_eq!(
        (kind.as_str(), source.as_str(), completed),
        ("interactive", "generated", false)
    );
    let (magical, musical) = title.split_once('-').expect("magical-musical pair");
    assert!(!magical.is_empty() && !musical.is_empty() && !musical.contains('-'));
    assert_eq!(fixture.run_parents(&first_run), (None, None, None));

    let listed = fixture.sessions();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["id"], id.as_str());
    assert_eq!(listed[0]["run_id"], first_run.as_str());
    assert_eq!(listed[0]["kind"], "interactive");
    assert_eq!(listed[0]["title"], title.as_str());
    assert_eq!(listed[0]["title_source"], "generated");
    assert_eq!(listed[0]["state"], "active");
    assert_eq!(listed[0]["provider"], "opencode");
    assert_eq!(listed[0]["work"], Value::Null);

    // The agent names its own Session by `$LF_RUN_ID`; a person then renames it.
    let suggested = fixture.json(&[
        "session",
        "rename",
        &first_run,
        "Parser",
        "--suggest",
        "--json",
    ]);
    assert_eq!(suggested["id"], id.as_str());
    assert_eq!(suggested["title"], "Parser");
    assert_eq!(suggested["title_source"], "generated");
    let named = fixture.json(&["session", "rename", &id, "Parser review", "--json"]);
    assert_eq!(named["title"], "Parser review");
    assert_eq!(named["title_source"], "human");
    fixture.json(&[
        "session",
        "rename",
        &id,
        "Better guess",
        "--suggest",
        "--json",
    ]);
    let (_, _, title, source, _) = fixture.session_row(&first_run);
    assert_eq!(
        (title.as_str(), source.as_str()),
        ("Parser review", "human")
    );
    let blank = fixture.run(&["session", "rename", &id, "  "]);
    assert!(!blank.status.success());

    fixture.release(first);
    let closed = fixture.sessions();
    assert_eq!(closed.len(), 1, "provider exit does not complete a Session");
    assert_eq!(closed[0]["state"], "closed");
    assert_eq!(closed[0]["title"], "Parser review");

    // Open resumes the same Run under the same Session.
    let described = fixture.json(&["session", "open", &id, "--json"]);
    assert_eq!(described["id"], id.as_str());
    assert_eq!(described["run_id"], first_run.as_str());
    let (resumed, resumed_run) = fixture.attach(&["session", "open", &id]);
    assert_eq!(resumed_run, first_run);
    assert_eq!(fixture.sessions()[0]["state"], "active");
    fixture.release(resumed);
    assert_eq!((fixture.count("sessions"), fixture.count("runs")), (1, 1));

    // Another launch in the same checkout is another conversation.
    let (second, second_run) = fixture.attach(&LAUNCH);
    fixture.release(second);
    let (second_id, ..) = fixture.session_row(&second_run);
    assert_ne!(second_id, id);
    assert_eq!((fixture.count("sessions"), fixture.count("runs")), (2, 2));
    assert_eq!(fixture.sessions().len(), 2);

    let completed = fixture.run(&["session", "complete", &id]);
    assert!(completed.status.success(), "{completed:?}");
    assert!(
        fixture.session_row(&first_run).4,
        "completion is a row update"
    );
    let remaining = fixture.sessions();
    assert_eq!(remaining.len(), 1, "{remaining:?}");
    assert_eq!(remaining[0]["id"], second_id.as_str());
    let reopened = fixture.run(&["session", "open", &id, "--json"]);
    assert!(!reopened.status.success(), "{reopened:?}");
    assert!(
        String::from_utf8_lossy(&reopened.stderr).contains("already complete"),
        "{reopened:?}"
    );

    assert_eq!(fixture.retired_files(), Vec::<PathBuf>::new());
}

#[test]
fn interactive_run_records_checkout_and_declared_work() {
    let fixture = Fixture::new(false);
    let task = support::register_unrun_task(
        fixture.home.path(),
        fixture.repo.path(),
        "task-binding",
        &fixture.repo.head_sha(),
    );
    let launch = |args: &[&str]| -> String {
        let before = fixture.launches().len();
        let output = fixture.run(args);
        assert!(
            output.status.success(),
            "lf {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        fixture.launches()[before].clone()
    };
    let parents = |source: &str| {
        (
            Some(task.task.id.to_string()),
            Some(task.task.wave_id.to_string()),
            Some(source.to_string()),
        )
    };

    fixture.repo.create_branch("task-binding");
    let bound = launch(&LAUNCH);
    assert_eq!(fixture.run_parents(&bound), parents("checkout"));
    let started: Option<i64> = fixture
        .db()
        .query_row(
            "SELECT started_at FROM tasks WHERE id=?1",
            [task.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert!(started.is_some(), "the first Run starts its Task");
    let listed = fixture.sessions();
    assert_eq!(
        listed[0]["work"],
        serde_json::json!({"kind": "task", "id": task.task.id})
    );
    assert_eq!(listed[0]["wave_id"], serde_json::json!(task.task.wave_id));

    fixture.repo.create_branch("unregistered");
    let unbound = launch(&LAUNCH);
    assert_eq!(fixture.run_parents(&unbound), (None, None, None));

    let declared = launch(&[
        "--task",
        "INF-123",
        "--tui",
        "--model",
        "opencode",
        ":",
        "Review the parser",
    ]);
    assert_eq!(fixture.run_parents(&declared), parents("declared"));
    assert_eq!(fixture.count("sessions"), 3);
}

fn wait_for<T>(what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + PATIENCE;
    loop {
        if let Some(found) = probe() {
            return found;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

const BOUND_LAUNCH: [&str; 7] = [
    "--task",
    "INF-123",
    "--tui",
    "--model",
    "opencode",
    ":",
    "Review the parser",
];

#[test]
fn ask_session_is_rows_from_request_to_answer() {
    let fixture = Fixture::new(true);
    let task = support::register_unrun_task(
        fixture.home.path(),
        fixture.repo.path(),
        "task-binding",
        &fixture.repo.head_sha(),
    );
    let (caller, caller_run) = fixture.attach(&BOUND_LAUNCH);

    let asking = fixture
        .ask(&caller_run, "Which release target?")
        .spawn()
        .unwrap();
    let (id, first_run): (String, String) = wait_for("the Ask Session row", || {
        fixture
            .db()
            .query_row(
                "SELECT id, current_run_id FROM sessions WHERE kind='ask'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok()
    });
    wait_for("the conversation launcher", || {
        fixture
            .launcher_requests()
            .contains("serve-ask")
            .then_some(())
    });
    assert_eq!((fixture.count("sessions"), fixture.count("runs")), (2, 2));
    let inherited = (
        Some(task.task.id.to_string()),
        Some(task.task.wave_id.to_string()),
        Some("inherited".to_string()),
    );
    assert_eq!(fixture.run_parents(&first_run), inherited);
    let ask_run = |run_id: &str| -> (Option<String>, Option<String>, String) {
        fixture
            .db()
            .query_row(
                "SELECT invocation_id, caller_run_id, session_id FROM runs WHERE id=?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
    };
    assert_eq!(
        ask_run(&first_run),
        (None, Some(caller_run.clone()), id.clone()),
        "an Ask inherits work and its caller, never an invocation"
    );

    let listed = fixture.sessions();
    let listed = listed
        .iter()
        .find(|session| session["id"] == id.as_str())
        .unwrap_or_else(|| panic!("the Ask does not list: {listed:?}"));
    assert_eq!(listed["kind"], "ask");
    assert_eq!(listed["run_id"], first_run.as_str());
    assert_eq!(listed["title"], "Which release target?");
    assert_eq!(listed["title_source"], "generated");
    assert_eq!(listed["state"], "waiting");
    assert_eq!(
        listed["work"],
        serde_json::json!({"kind": "task", "id": task.task.id})
    );
    assert_eq!(listed["flow_membership"]["kind"], "independent");

    let named = fixture.json(&["session", "rename", &first_run, "Launch notes", "--json"]);
    assert_eq!(named["id"], id.as_str());
    assert_eq!(named["title"], "Launch notes");
    assert_eq!(named["title_source"], "human");

    let inside = |run_id: &str, args: &[&str]| -> Output {
        fixture
            .command(args)
            .env("LF_RUN_ID", run_id)
            .env(
                "LF_HUMAN_SESSION",
                serde_json::json!({"kind": "ask", "id": id}).to_string(),
            )
            .output()
            .unwrap()
    };
    let early = fixture.run(&["session", "complete", &id]);
    assert!(
        String::from_utf8_lossy(&early.stderr).contains("not marked this ready"),
        "{early:?}"
    );
    let ready = inside(&first_run, &["session", "ready", "Ship to staging"]);
    assert!(ready.status.success(), "{ready:?}");
    let feedback = || -> (String, Option<String>, String, bool) {
        fixture
            .db()
            .query_row(
                "SELECT title, ready_summary, current_run_id, completed_at IS NOT NULL
                 FROM sessions WHERE id=?1",
                [&id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap()
    };
    assert_eq!(
        feedback(),
        (
            "Launch notes".to_string(),
            Some("Ship to staging".to_string()),
            first_run.clone(),
            false
        )
    );

    // A consumed launch without provider history is replaced on the next open.
    // The replacement is another Run of the same conversation.
    std::fs::remove_file(fixture.run_dir(&first_run).join("prepared")).unwrap();
    let (opened, second_run) = fixture.attach(&["session", "open", &id]);
    assert_ne!(second_run, first_run);
    assert_eq!(
        feedback(),
        (
            "Launch notes".to_string(),
            Some("Ship to staging".to_string()),
            second_run.clone(),
            false
        )
    );
    assert_eq!(
        ask_run(&second_run),
        (None, Some(caller_run.clone()), id.clone())
    );
    assert_eq!(fixture.run_parents(&second_run), inherited);
    let history: i64 = fixture
        .db()
        .query_row(
            "SELECT count(*) FROM runs WHERE session_id=?1",
            [&id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        history, 2,
        "the replaced Run stays in the Session's history"
    );
    let stale = inside(&first_run, &["session", "ready", "Late answer"]);
    assert!(!stale.status.success(), "{stale:?}");
    assert_eq!(fixture.count("sessions"), 2);

    let completed = fixture.run(&["session", "complete", &id]);
    assert!(completed.status.success(), "{completed:?}");
    let answer = asking.wait_with_output().unwrap();
    assert!(answer.status.success(), "{answer:?}");
    assert_eq!(
        String::from_utf8_lossy(&answer.stdout)
            .matches("Ship to staging")
            .count(),
        1,
        "{answer:?}"
    );
    assert!(feedback().3, "completion is a row update");
    assert!(fixture
        .sessions()
        .iter()
        .all(|session| session["id"] != id.as_str()));
    let again = fixture.run(&["session", "complete", &id]);
    assert!(!again.status.success(), "{again:?}");
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("already complete"),
        "{again:?}"
    );

    fixture.release(opened);
    fixture.release(caller);
    assert_eq!(fixture.retired_files(), Vec::<PathBuf>::new());
}

/// A store under a directory that cannot be written until `unlock`.
struct LockedStore {
    directory: PathBuf,
}

impl LockedStore {
    fn new(fixture: &Fixture) -> Self {
        let directory = fixture.home.path().join("locked");
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555)).unwrap();
        Self { directory }
    }

    fn select(&self, command: &mut Command) {
        let path = self.directory.join("loopflow.db");
        command
            .env("LF_DB_PATH", &path)
            .env("LF_CONTROL_DB_PATH", &path);
    }

    fn unlock(&self) -> rusqlite::Connection {
        std::fs::set_permissions(&self.directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        rusqlite::Connection::open(self.directory.join("loopflow.db")).unwrap()
    }
}

#[test]
fn launch_proceeds_when_the_store_cannot_be_written() {
    let fixture = Fixture::new(false);
    let store = LockedStore::new(&fixture);
    let mut launch = fixture.command(&LAUNCH);
    store.select(&mut launch);
    let output = launch.output().unwrap();
    assert!(
        output.status.success(),
        "bookkeeping refused the launch: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run_id = fixture.launches()[0].clone();

    drop(store.unlock());
    let mut list = fixture.command(&["session", "list", "--all", "--json"]);
    store.select(&mut list);
    let listed: Vec<Value> = serde_json::from_slice(&list.output().unwrap().stdout).unwrap();
    assert_eq!(
        listed,
        Vec::<Value>::new(),
        "an unrecorded Session does not list"
    );

    // The next operation that touches the Run records its Session.
    let mut rename = fixture.command(&["session", "rename", &run_id, "Parser review", "--json"]);
    store.select(&mut rename);
    let renamed = rename.output().unwrap();
    assert!(renamed.status.success(), "{renamed:?}");
    let renamed: Value = serde_json::from_slice(&renamed.stdout).unwrap();
    assert_eq!(renamed["run_id"], run_id.as_str());
    assert_eq!(renamed["kind"], "interactive");
    assert_eq!(renamed["title"], "Parser review");
    let mut list = fixture.command(&["session", "list", "--all", "--json"]);
    store.select(&mut list);
    let listed: Vec<Value> = serde_json::from_slice(&list.output().unwrap().stdout).unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["id"], renamed["id"]);
    let rows: (i64, i64) = store
        .unlock()
        .query_row(
            "SELECT (SELECT count(*) FROM sessions), (SELECT count(*) FROM runs)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(rows, (1, 1));
}

#[test]
fn ask_proceeds_when_the_store_cannot_be_written() {
    let fixture = Fixture::new(true);
    let (caller, caller_run) = fixture.attach(&LAUNCH);
    let store = LockedStore::new(&fixture);
    let mut ask = fixture.ask(&caller_run, "Which release target?");
    store.select(&mut ask);
    let mut asking = ask.spawn().unwrap();
    let run_id = wait_for("the conversation launcher", || {
        if let Some(status) = asking.try_wait().unwrap() {
            panic!("bookkeeping refused the Ask: {status}");
        }
        let requests = fixture.launcher_requests();
        let served = requests.split("serve-ask").nth(1)?;
        let start = served.find("run_")?;
        Some(served[start..start + 36].to_string())
    });

    let rows = store.unlock();
    let mut rename = fixture.command(&["session", "rename", &run_id, "Launch notes", "--json"]);
    store.select(&mut rename);
    let renamed = rename.output().unwrap();
    assert!(renamed.status.success(), "{renamed:?}");
    let renamed: Value = serde_json::from_slice(&renamed.stdout).unwrap();
    assert_eq!(renamed["kind"], "ask");
    assert_eq!(renamed["run_id"], run_id.as_str());
    assert_eq!(renamed["title"], "Launch notes");
    let recorded: (String, String) = rows
        .query_row(
            "SELECT s.kind, s.request FROM sessions s WHERE s.current_run_id=?1",
            [&run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        recorded,
        ("ask".to_string(), "Which release target?".to_string())
    );

    asking.kill().unwrap();
    asking.wait().unwrap();
    fixture.release(caller);
}
