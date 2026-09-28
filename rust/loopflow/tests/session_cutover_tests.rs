//! Every Session is a `sessions` row and its `runs` rows from launch to completion.
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

#[test]
fn binding_an_orphan_session_starts_its_task_once() {
    let fixture = Fixture::new(false);
    let task = support::register_unrun_task(
        fixture.home.path(),
        fixture.repo.path(),
        "task-binding",
        &fixture.repo.head_sha(),
    );
    let sibling_worktree = fixture.repo.create_named_worktree("task-sibling");
    let sibling =
        support::register_sibling_task(&task, "INF-124", "task-sibling", &sibling_worktree);
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
    let started = |id: &str| -> Option<i64> {
        fixture
            .db()
            .query_row("SELECT started_at FROM tasks WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .unwrap()
    };
    let now = || -> i64 {
        fixture
            .db()
            .query_row("SELECT CAST(strftime('%s','now') AS INTEGER)", [], |row| {
                row.get(0)
            })
            .unwrap()
    };
    let task_runs = |issue: &str| -> Vec<String> {
        fixture
            .json(&["runs", "--task", issue, "--json"])
            .as_array()
            .unwrap()
            .iter()
            .map(|run| run["id"].as_str().unwrap().to_string())
            .collect()
    };

    let orphan = launch(&LAUNCH);
    let (session, ..) = fixture.session_row(&orphan);
    assert_eq!(fixture.run_parents(&orphan), (None, None, None));
    assert_eq!(started(task.task.id.as_str()), None);
    assert!(task_runs("INF-123").is_empty());

    // The bind happens measurably after the Run was created.
    std::thread::sleep(Duration::from_secs(2));
    let before = now();
    let bound = fixture.json(&["session", "bind", &session, "--task", "INF-123", "--json"]);
    let after = now();
    assert_eq!(bound["id"], session.as_str());
    assert_eq!(bound["run_id"], orphan.as_str());
    assert_eq!(
        bound["work"],
        serde_json::json!({"kind": "task", "id": task.task.id})
    );
    assert_eq!(
        fixture.run_parents(&orphan),
        (
            Some(task.task.id.to_string()),
            Some(task.task.wave_id.to_string()),
            Some("bound".to_string()),
        )
    );
    let bound_at = started(task.task.id.as_str()).expect("the bind starts the Task");
    assert!((before..=after).contains(&bound_at), "{bound_at}");
    let created: i64 = fixture
        .db()
        .query_row(
            "SELECT created_at FROM runs WHERE id=?1",
            [orphan.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert!(bound_at > created, "Started is the bind, not the launch");
    assert_eq!(task_runs("INF-123"), vec![orphan.clone()]);
    let listed = fixture.sessions();
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0]["work"],
        serde_json::json!({"kind": "task", "id": task.task.id})
    );
    assert_eq!(listed[0]["work_path"], "task-pr-tests / INF-123");

    // A Run's Task never moves; binding it again to the same Task is a no-op.
    let refused = fixture.run(&["session", "bind", &session, "--task", "INF-124", "--json"]);
    assert!(!refused.status.success());
    let reason = String::from_utf8_lossy(&refused.stderr);
    assert!(reason.contains("already has Task INF-123"), "{reason}");
    let again = fixture.run(&["session", "bind", &session, "--task", "INF-123", "--json"]);
    assert!(
        again.status.success(),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&again.stdout).unwrap(),
        bound
    );
    assert!(
        String::from_utf8_lossy(&again.stderr).contains(&format!("Binding {session} to INF-123 (")),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert_eq!(
        fixture.run_parents(&orphan).0,
        Some(task.task.id.to_string())
    );
    assert_eq!(started(sibling.id.as_str()), None);
    assert!(task_runs("INF-124").is_empty());

    std::thread::sleep(Duration::from_secs(2));
    let later = launch(&[
        "--task",
        "INF-123",
        "--tui",
        "--model",
        "opencode",
        ":",
        "Review the parser",
    ]);
    assert_eq!(started(task.task.id.as_str()), Some(bound_at));
    assert_eq!(task_runs("INF-123"), vec![later, orphan]);

    // Started is stored once and only beside a Run that names the Task.
    let db = fixture.db();
    let disagreeing: i64 = db
        .query_row(
            "SELECT count(*) FROM tasks t WHERE (t.started_at IS NOT NULL)
                != EXISTS(SELECT 1 FROM runs r WHERE r.task_id=t.id)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(disagreeing, 0);
    for (id, value) in [
        (task.task.id.as_str(), Some(bound_at + 1)),
        (task.task.id.as_str(), None),
        (sibling.id.as_str(), Some(bound_at)),
    ] {
        assert!(db
            .execute(
                "UPDATE tasks SET started_at=?2 WHERE id=?1",
                rusqlite::params![id, value]
            )
            .is_err());
    }
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
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "bookkeeping refused the launch: {stderr}"
    );
    let warnings: Vec<&str> = stderr
        .lines()
        .filter(|line| line.contains("Session is not recorded"))
        .collect();
    assert_eq!(warnings.len(), 1, "one warning line: {stderr}");
    assert!(
        warnings[0].contains("unable to open database file"),
        "the warning names the store error: {stderr}"
    );
    let run_id = fixture.launches()[0].clone();

    // A headless Run launches the same way, and settles, without its row.
    let mut headless = fixture.command(&["-b", "--model", "opencode", ":", "Tidy the parser"]);
    store.select(&mut headless);
    let output = headless.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "bookkeeping refused the launch: {stderr}"
    );
    let warnings: Vec<&str> = stderr
        .lines()
        .filter(|line| line.contains("warning:"))
        .collect();
    assert_eq!(warnings.len(), 1, "one warning line: {stderr}");
    assert!(
        warnings[0].contains("Run is not recorded")
            && warnings[0].contains("unable to open database file"),
        "the warning names the store error: {stderr}"
    );
    let unrecorded = fixture.launches()[1].clone();
    assert!(fixture.run_dir(&unrecorded).join("terminal.json").is_file());

    // Nothing waits beside the Run and nothing records the Session later.
    let rows = store.unlock();
    let mut list = fixture.command(&["session", "list", "--all", "--json"]);
    store.select(&mut list);
    let listed: Vec<Value> = serde_json::from_slice(&list.output().unwrap().stdout).unwrap();
    assert_eq!(listed, Vec::<Value>::new());
    let mut rename = fixture.command(&["session", "rename", &run_id, "Parser review", "--json"]);
    store.select(&mut rename);
    let renamed = rename.output().unwrap();
    assert!(!renamed.status.success(), "{renamed:?}");
    assert!(
        String::from_utf8_lossy(&renamed.stderr).contains("does not belong to a Session"),
        "{renamed:?}"
    );
    let stored: (i64, i64) = rows
        .query_row(
            "SELECT (SELECT count(*) FROM sessions), (SELECT count(*) FROM runs)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(stored, (0, 0));
    assert!(!fixture
        .run_dir(&run_id)
        .join("unrecorded-session.json")
        .exists());
}

/// An Ask's answer returns through its row, so an Ask that cannot be stored
/// opens no conversation.
#[test]
fn ask_needs_its_row_to_return_an_answer() {
    let fixture = Fixture::new(true);
    let (caller, caller_run) = fixture.attach(&LAUNCH);
    let store = LockedStore::new(&fixture);
    let mut ask = fixture.ask(&caller_run, "Which release target?");
    store.select(&mut ask);
    let mut asking = ask.spawn().unwrap();
    wait_for("the Ask to be refused", || asking.try_wait().unwrap());
    let refused = asking.wait_with_output().unwrap();
    assert!(!refused.status.success(), "{refused:?}");
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("unable to open database file"),
        "{refused:?}"
    );
    assert!(!fixture.launcher_requests().contains("serve-ask"));
    fixture.release(caller);
}

const REVIEW_FLOW: [&str; 6] = [
    "--model",
    "opencode",
    "flow",
    "review-first",
    "-b",
    "--no-loopflow",
];

impl Fixture {
    /// A saved Flow whose only step is a human review, run until it waits.
    /// Returns (Session id, invocation id, first Run id).
    fn waiting_review(&self) -> (String, String, String) {
        let lf = self.repo.path().join(".lf");
        std::fs::create_dir_all(lf.join("skills")).unwrap();
        std::fs::create_dir_all(lf.join("flows")).unwrap();
        std::fs::write(lf.join("skills/review-proof.md"), "Review the fixture.").unwrap();
        std::fs::write(
            lf.join("flows/review-first.yaml"),
            "- step:\n    id: review\n    name: review-proof\n    human: true\n",
        )
        .unwrap();
        let waiting = self.run(&REVIEW_FLOW);
        assert!(
            String::from_utf8_lossy(&waiting.stderr).contains("waiting for human input"),
            "{waiting:?}"
        );
        self.db()
            .query_row(
                "SELECT s.id, r.invocation_id, r.id FROM sessions s
                 JOIN runs r ON r.id=s.current_run_id AND r.session_id=s.id
                 WHERE s.kind='flow_review'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or_else(|error| panic!("the waiting review has no rows: {error}"))
    }

    /// (title, title_source, ready_summary, current Run, completed) of a Session.
    fn feedback(&self, id: &str) -> (String, String, Option<String>, String, bool) {
        self.db()
            .query_row(
                "SELECT title, title_source, ready_summary, current_run_id,
                    completed_at IS NOT NULL FROM sessions WHERE id=?1",
                [id],
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
            .unwrap()
    }
}

#[test]
fn taskless_flow_review_is_rows_from_request_to_completion() {
    let fixture = Fixture::new(true);
    let (id, invocation, first_run) = fixture.waiting_review();
    assert_eq!((fixture.count("sessions"), fixture.count("runs")), (1, 1));
    let parents: (Option<String>, Option<String>, String, String) = fixture
        .db()
        .query_row(
            "SELECT r.task_id, f.task_id, f.pending_session_id, f.state
             FROM runs r JOIN flow_invocations f ON f.id=r.invocation_id WHERE r.id=?1",
            [&first_run],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        parents,
        (None, None, id.clone(), "current".to_string()),
        "the Run names its invocation and neither has a Task"
    );
    assert_eq!(
        fixture.feedback(&id),
        (
            "review-proof".to_string(),
            "generated".to_string(),
            None,
            first_run.clone(),
            false
        )
    );

    let inside = |run_id: &str, args: &[&str]| -> Output {
        fixture
            .command(args)
            .env("LF_RUN_ID", run_id)
            .env(
                "LF_HUMAN_SESSION",
                serde_json::json!({"kind": "standalone_flow", "id": id}).to_string(),
            )
            .output()
            .unwrap()
    };
    let early = fixture.run(&["session", "complete", &id]);
    assert!(
        String::from_utf8_lossy(&early.stderr).contains("not marked this ready"),
        "{early:?}"
    );
    // Driving the Flow again waits at the same review.
    let waiting = fixture.run(&["flow", "resume", &invocation]);
    assert!(
        String::from_utf8_lossy(&waiting.stderr).contains("waiting for human input"),
        "{waiting:?}"
    );
    assert_eq!((fixture.count("sessions"), fixture.count("runs")), (1, 1));
    let ready = inside(&first_run, &["session", "ready", "Ship the parser"]);
    assert!(ready.status.success(), "{ready:?}");
    let named = fixture.json(&["session", "rename", &id, "Parser review", "--json"]);
    assert_eq!(named["title"], "Parser review");
    assert_eq!(named["title_source"], "human");

    // A consumed launch without provider history is replaced on the next open.
    // Title and feedback belong to the Session, so both survive.
    std::fs::remove_file(fixture.run_dir(&first_run).join("prepared")).unwrap();
    let (opened, second_run) = fixture.attach(&["session", "open", &id]);
    assert_ne!(second_run, first_run);
    assert_eq!(
        fixture.feedback(&id),
        (
            "Parser review".to_string(),
            "human".to_string(),
            Some("Ship the parser".to_string()),
            second_run.clone(),
            false
        )
    );
    let history: Vec<(String, Option<String>, Option<String>)> = fixture
        .db()
        .prepare(
            "SELECT id, invocation_id, task_id FROM runs WHERE session_id=?1
             ORDER BY created_at, attempt",
        )
        .unwrap()
        .query_map([&id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        history,
        vec![
            (first_run.clone(), Some(invocation.clone()), None),
            (second_run.clone(), Some(invocation.clone()), None)
        ],
        "the replaced Run stays in the Session's history"
    );
    let current: String = fixture
        .db()
        .query_row(
            "SELECT current_run_id FROM flow_invocations WHERE id=?1",
            [&invocation],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        current, second_run,
        "the invocation follows the replacement"
    );
    let stale = inside(&first_run, &["session", "ready", "Late feedback"]);
    assert!(!stale.status.success(), "{stale:?}");
    fixture.release(opened);

    let completed = fixture.run(&["session", "complete", &id]);
    assert!(completed.status.success(), "{completed:?}");
    assert!(fixture.feedback(&id).4, "completion is a row update");
    assert_eq!(fixture.sessions(), Vec::<Value>::new());
    let again = fixture.run(&["session", "complete", &id]);
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("already complete"),
        "{again:?}"
    );

    // The Flow reads the review's outcome from the row and finishes.
    let resumed = fixture.run(&["flow", "resume", &invocation]);
    assert!(resumed.status.success(), "{resumed:?}");
    let state: String = fixture
        .db()
        .query_row(
            "SELECT state FROM flow_invocations WHERE id=?1",
            [&invocation],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "completed");
    assert_eq!((fixture.count("sessions"), fixture.count("runs")), (1, 2));
    assert_eq!(fixture.retired_files(), Vec::<PathBuf>::new());
}

#[test]
fn session_list_reads_a_taskless_review_from_sql() {
    let fixture = Fixture::new(false);
    let (id, invocation, run_id) = fixture.waiting_review();
    // The driver's saved position is not a Session source.
    let flows = fixture.home.path().join("flows");
    std::fs::rename(&flows, fixture.home.path().join("flows.away")).unwrap();

    let listed = fixture.sessions();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["id"], id.as_str());
    assert_eq!(listed[0]["kind"], "flow");
    assert_eq!(listed[0]["run_id"], run_id.as_str());
    assert_eq!(listed[0]["title"], "review-proof");
    assert_eq!(listed[0]["title_source"], "generated");
    assert_eq!(listed[0]["detail"], "review-proof");
    assert_eq!(listed[0]["state"], "waiting");
    assert_eq!(listed[0]["provider"], "opencode");
    assert_eq!(listed[0]["work"], Value::Null);
    assert_eq!(
        listed[0]["flow_membership"],
        serde_json::json!({
            "kind": "step", "flow": "review-first", "invocation_id": invocation,
            "step": "review-proof", "node": "0", "iterations": [[]],
            "occurrence": "current"
        })
    );

    // The review's agent names its Session by `$LF_RUN_ID`.
    let suggested = fixture.json(&[
        "session",
        "rename",
        &run_id,
        "Parser",
        "--suggest",
        "--json",
    ]);
    assert_eq!(suggested["id"], id.as_str());
    assert_eq!(suggested["title"], "Parser");
    assert_eq!(fixture.sessions()[0]["title"], "Parser");
    assert!(!flows.exists(), "reading a Session recreates no Flow file");
    assert_eq!(fixture.retired_files(), Vec::<PathBuf>::new());
}

/// One Session of each origin, as an old Home kept them in files.
#[test]
fn import_stores_each_old_session_once_with_its_name() {
    use loopflow::durable::{FlowInvocation, RunId};
    use loopflow::engine::invocation::QueuedInvocation;
    use loopflow::engine::{ConcreteSkill, ConcreteStep, ExecutionCursor, OccurrencePolicy, Skill};
    use serde_json::json;

    let fixture = Fixture::new(false);
    let home = fixture.home.path();
    let repo = fixture.repo.path();
    let task = support::register_unrun_task(home, repo, "session-import", &fixture.repo.head_sha());
    // The Task review starts its own Task; the interactive Session names another.
    let sibling_worktree = fixture.repo.create_named_worktree("import-sibling");
    let sibling =
        support::register_sibling_task(&task, "INF-124", "import-sibling", &sibling_worktree);
    let write = |path: PathBuf, value: Value| {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        path
    };
    let sources = std::cell::RefCell::new(Vec::new());
    // A conversation's Run record: manifest, provider history and its name.
    let record = |title: &str, source: &str, flow: Value, subjects: Value| -> String {
        let id = RunId::new().to_string();
        let dir = fixture.run_dir(&id);
        sources.borrow_mut().push(write(
            dir.join("manifest.json"),
            json!({
                "schema_version": 1, "run_id": id, "parent_run_id": null,
                "created_at": "2026-09-20T20:00:00Z", "harness": "opencode", "model": null,
                "surface": "tui", "cwd": repo, "repo": repo, "worktree": repo,
                "skill": null, "subjects": subjects, "flow": flow, "launch": null,
                "context": null, "runtime_path": null, "runtime_digest": null,
                "host": "fixture", "boot_id": null
            }),
        ));
        sources.borrow_mut().push(write(
            dir.join("session-name.json"),
            json!({"schema_version": 1, "title": title, "source": source}),
        ));
        write(
            dir.join("provider-session.json"),
            json!({"schema_version": 1, "provider_session_id": format!("ses_{id}"), "account_id": null}),
        );
        id
    };
    let independent = json!({"kind": "independent"});

    let interactive = record(
        "Parser review",
        "human",
        independent.clone(),
        json!([{"selector": "task:INF-124", "source": "declared"}]),
    );
    let asked = record(
        "Release target",
        "generated",
        independent.clone(),
        json!([]),
    );
    sources.borrow_mut().push(write(
        home.join("human-sessions/ask_release.json"),
        json!({
            "id": "ask_release", "parent_run_id": interactive,
            "parent_run_dir": fixture.run_dir(&interactive), "work": null,
            "work_selector": null, "title": "Which release?", "detail": "opencode",
            "prompt": "Which release carries the parser?", "skill": null, "cwd": repo,
            "model": "opencode", "session_run_id": asked, "ready_summary": null,
            "status": "waiting"
        }),
    ));
    let broken = home.join("human-sessions/ask_broken.json");
    std::fs::write(&broken, "{").unwrap();

    let review = ConcreteStep::Skill(ConcreteSkill {
        skill: Skill {
            content: Some("Review the design with the person.".into()),
            ..Skill::named("review-design")
        },
        policy: OccurrencePolicy {
            id: Some("review".into()),
            human: true,
            ..Default::default()
        },
        flow_parents: vec![],
    });
    let invocation = uuid::Uuid::new_v4().to_string();
    let boundary = uuid::Uuid::new_v4().to_string();
    let reviewed = record("Design review", "human", Value::Null, json!([]));
    sources.borrow_mut().push(write(
        home.join("flows").join(&invocation).join("position.json"),
        json!({
            "id": invocation, "flow": "design", "cwd": repo, "steps": [review.clone()],
            "cursor": ExecutionCursor::default(), "message": null, "model": "opencode",
            "wave": null, "task": null, "as_work": null,
            "active": {
                "id": boundary, "run_id": reviewed, "run_dir": fixture.run_dir(&reviewed),
                "completed": false, "ready_summary": "Keep the old parser"
            },
            "failure": null, "finished": false
        }),
    ));

    let task_review = record("Task review", "human", Value::Null, json!([]));
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let parked = task
            .store
            .start_task_flow(
                &task.task.id,
                FlowInvocation {
                    invocation: QueuedInvocation::new("captured", vec![review]).unwrap(),
                    cursor: ExecutionCursor::default(),
                    version: 0,
                    task_id: Some(task.task.id.clone()),
                    wave_id: Some(task.task.wave_id.clone()),
                    cwd: task.task.worktree.clone(),
                    message: None,
                    model: None,
                    current_attempt: None,
                    pending_session_id: None,
                    ready_summary: None,
                    worker_generation: 0,
                    claim: None,
                    failure: None,
                    finished: false,
                    updated_at: time::OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();
        // The review Session the old Home kept, with the Run it launched.
        let review_id = format!("{}:{}:captured:review:0", task.task.id, parked.id());
        let run = loopflow::session::Run {
            id: RunId::parse(&task_review).unwrap(),
            session_id: Some(review_id.clone()),
            invocation_id: Some(parked.id().to_owned()),
            node: None,
            iterations: None,
            attempt: None,
            task_id: Some(task.task.id.clone()),
            wave_id: Some(task.task.wave_id.clone()),
            work_source: Some(loopflow::session::WorkSource::Inherited),
            created_at: 1,
            published: true,
            cwd: task.task.worktree.clone(),
            skill: Some("review-design".into()),
            provider: Some("opencode".into()),
            model: None,
            caller_run_id: None,
            ended: None,
        };
        let session = loopflow::session::Session {
            id: review_id,
            current_run_id: run.id.clone(),
            kind: loopflow::session::SessionKind::FlowReview,
            title: task.task.plan.title.clone(),
            title_source: loopflow::session::TitleSource::Generated,
            request: None,
            ready_summary: None,
            completed_at: None,
            created_at: 1,
        };
        task.store
            .create_session(session, run, Some(parked))
            .await
            .unwrap();
    });
    // Old Runs outside any Session had no rows: a settled headless Run a
    // conversation launched, and an earlier step of a Flow.
    let settled = |flow: Value, subjects: Value, parent: Value| -> String {
        let id = RunId::new().to_string();
        let dir = fixture.run_dir(&id);
        sources.borrow_mut().push(write(
            dir.join("manifest.json"),
            json!({
                "schema_version": 1, "run_id": id, "parent_run_id": parent,
                "created_at": "2026-09-20T20:00:00Z", "harness": "opencode", "model": "gpt",
                "surface": "headless", "cwd": repo, "repo": repo, "worktree": repo,
                "skill": "implement", "subjects": subjects, "flow": flow, "launch": null,
                "context": null, "runtime_path": null, "runtime_digest": null,
                "host": "fixture", "boot_id": null
            }),
        ));
        sources.borrow_mut().push(write(
            dir.join("terminal.json"),
            json!({
                "schema_version": 1, "outcome": "failed",
                "ended_at": "2026-09-20T20:05:00Z", "result_ref": null
            }),
        ));
        id
    };
    let headless = settled(
        independent.clone(),
        json!([{"selector": "wave:task-pr-tests", "source": "declared"}]),
        json!(interactive),
    );
    let step = settled(
        json!({
            "kind": "step", "task_id": null, "boundary_key": "0", "invocation_id": invocation,
            "flow": "design", "step": "implement", "node": "0", "iterations": [[]]
        }),
        json!([{"selector": "task:INF-124", "source": "inherited"}]),
        Value::Null,
    );
    let evidence: Vec<(PathBuf, Vec<u8>)> = sources
        .into_inner()
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();

    let names = || -> Vec<(String, String, String)> {
        let mut names: Vec<_> = fixture
            .sessions()
            .iter()
            .map(|session| {
                (
                    session["title"].as_str().unwrap().to_string(),
                    session["kind"].as_str().unwrap().to_string(),
                    session["run_id"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        names().len(),
        1,
        "before the import only the stored Task review lists"
    );

    let planned = fixture.json(&["session", "import", "--dry-run", "--json"]);
    assert_eq!(fixture.count("sessions"), 1, "a dry run stores nothing");
    let first = fixture.json(&["session", "import", "--json"]);
    for report in [&planned, &first] {
        for (kind, count) in [
            ("interactive", 1),
            ("ask", 1),
            ("flow_review", 1),
            ("task_review", 1),
            ("run", 2),
            ("unchanged", 0),
        ] {
            assert_eq!(report[kind], count, "{kind}: {report}");
        }
        assert_eq!(report["tasks_started"], json!([sibling.id]), "{report}");
        let failed = report["failed"].as_array().unwrap();
        assert_eq!(failed.len(), 1, "{report}");
        assert_eq!(failed[0]["path"], json!(broken));
        assert!(!failed[0]["reason"].as_str().unwrap().is_empty());
    }
    let again = fixture.json(&["session", "import", "--json"]);
    for (kind, count) in [
        ("interactive", 0),
        ("ask", 0),
        ("flow_review", 0),
        ("task_review", 0),
        ("run", 0),
        ("unchanged", 4),
    ] {
        assert_eq!(again[kind], count, "{kind}: {again}");
    }
    assert_eq!(again["tasks_started"], json!([]));

    let expected = {
        let mut expected = vec![
            ("Design review".to_string(), "flow".to_string(), reviewed),
            (
                "Parser review".to_string(),
                "interactive".to_string(),
                interactive.clone(),
            ),
            ("Release target".to_string(), "ask".to_string(), asked),
            ("Task review".to_string(), "flow".to_string(), task_review),
        ];
        expected.sort();
        expected
    };
    assert_eq!(names(), expected);
    assert_eq!(fixture.count("sessions"), 4);
    assert_eq!(fixture.count("runs"), 6);
    // (session, invocation, caller, provider, outcome, ended) of an imported Run.
    type Imported = (
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
        i64,
    );
    let imported = |run: &str| -> Imported {
        fixture
            .db()
            .query_row(
                "SELECT session_id, invocation_id, caller_run_id, provider, outcome, ended_at
                 FROM runs WHERE id=?1",
                [run],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap_or_else(|error| panic!("Run {run} has no row: {error}"))
    };
    let ended = 1_789_934_700;
    assert_eq!(
        imported(&headless),
        (
            None,
            None,
            Some(interactive.clone()),
            "opencode".to_string(),
            "failed".to_string(),
            ended
        )
    );
    assert_eq!(
        fixture.run_parents(&headless),
        (
            None,
            Some(task.task.wave_id.to_string()),
            Some("declared".to_string())
        )
    );
    assert_eq!(imported(&step).1, None, "its invocation has moved on");
    assert_eq!(
        fixture.run_parents(&step),
        (
            Some(sibling.id.to_string()),
            Some(sibling.wave_id.to_string()),
            Some("inherited".to_string()),
        )
    );
    let listed = fixture.json(&["runs", "--task", "INF-124", "--json"]);
    let mut listed: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|run| run["id"].as_str().unwrap())
        .collect();
    listed.sort();
    let mut expected_runs = vec![interactive.as_str(), step.as_str()];
    expected_runs.sort();
    assert_eq!(listed, expected_runs);
    assert_eq!(
        fixture.run_parents(&interactive),
        (
            Some(sibling.id.to_string()),
            Some(sibling.wave_id.to_string()),
            Some("declared".to_string()),
        )
    );
    let feedback: String = fixture
        .db()
        .query_row(
            "SELECT ready_summary FROM sessions WHERE id=?1",
            [format!("flow:{invocation}:{boundary}")],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(feedback, "Keep the old parser");

    // The files stay as evidence and nothing reads them again.
    for (path, bytes) in &evidence {
        assert_eq!(&std::fs::read(path).unwrap(), bytes, "{}", path.display());
    }
    for (path, _) in &evidence {
        if !path.ends_with("manifest.json") && !path.ends_with("position.json") {
            std::fs::remove_file(path).unwrap();
        }
    }
    assert_eq!(names(), expected);
}

#[test]
fn every_launch_is_one_row_and_every_reader_lists_it_once() {
    let fixture = Fixture::new(false);
    let task = support::register_unrun_task(
        fixture.home.path(),
        fixture.repo.path(),
        "task-binding",
        &fixture.repo.head_sha(),
    );
    let (task_id, wave_id) = (task.task.id.to_string(), task.task.wave_id.to_string());
    let lf = fixture.repo.path().join(".lf");
    std::fs::create_dir_all(lf.join("skills")).unwrap();
    std::fs::create_dir_all(lf.join("flows")).unwrap();
    std::fs::write(lf.join("skills/work-proof.md"), "Do the fixture work.").unwrap();
    std::fs::write(lf.join("skills/review-proof.md"), "Review the fixture.").unwrap();
    std::fs::write(
        lf.join("flows/work-then-review.yaml"),
        "- work-proof\n- step:\n    id: review\n    name: review-proof\n    human: true\n",
    )
    .unwrap();
    let launch = |command: &mut Command| -> String {
        let before = fixture.launches().len();
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fixture.launches()[before].clone()
    };
    const HEADLESS: [&str; 5] = ["-b", "--model", "opencode", ":", "Tidy the parser"];
    let headless = |work: &[&str]| fixture.command(&[work, &HEADLESS[..]].concat());
    // (invocation, session, caller) of one Run.
    let membership = |run: &str| -> (Option<String>, Option<String>, Option<String>) {
        fixture
            .db()
            .query_row(
                "SELECT invocation_id, session_id, caller_run_id FROM runs WHERE id=?1",
                [run],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or_else(|error| panic!("Run {run} has no row: {error}"))
    };
    let declared = Some("declared".to_string());

    // A headless Run that names only a Wave.
    let of_wave = launch(&mut headless(&["--wave", "task-pr-tests"]));
    assert_eq!(
        fixture.run_parents(&of_wave),
        (None, Some(wave_id.clone()), declared.clone())
    );
    assert_eq!(membership(&of_wave), (None, None, None));

    // A headless Run that names a Task, and a child agent it launches. The
    // child names no Work and takes its caller's.
    let of_task = launch(&mut headless(&["--task", "INF-123"]));
    assert_eq!(
        fixture.run_parents(&of_task),
        (
            Some(task_id.clone()),
            Some(wave_id.clone()),
            declared.clone()
        )
    );
    let child = launch(
        headless(&[])
            .env("LF_RUN_ID", &of_task)
            .env("LF_RUN_DIR", fixture.run_dir(&of_task)),
    );
    assert_eq!(
        fixture.run_parents(&child),
        (
            Some(task_id.clone()),
            Some(wave_id.clone()),
            Some("inherited".to_string())
        )
    );
    assert_eq!(membership(&child), (None, None, Some(of_task.clone())));

    // A saved Flow: its headless step, then the review it waits at. Both
    // Runs name the Flow's invocation.
    let before = fixture.launches().len();
    let waiting = fixture.run(&[
        "--wave",
        "task-pr-tests",
        "--model",
        "opencode",
        "flow",
        "work-then-review",
        "-b",
        "--no-loopflow",
    ]);
    assert!(
        String::from_utf8_lossy(&waiting.stderr).contains("waiting for human input"),
        "{waiting:?}"
    );
    let step = fixture.launches()[before].clone();
    let (review, invocation): (String, String) = fixture
        .db()
        .query_row(
            "SELECT r.id, r.invocation_id FROM sessions s
             JOIN runs r ON r.id=s.current_run_id WHERE s.kind='flow_review'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(membership(&step), (Some(invocation.clone()), None, None));
    assert_eq!(
        fixture.run_parents(&step),
        (None, Some(wave_id.clone()), declared.clone())
    );
    assert_eq!(fixture.run_parents(&review), fixture.run_parents(&step));
    let positions: Vec<(String, i64, i64)> = fixture
        .db()
        .prepare("SELECT id, node, attempt FROM runs WHERE invocation_id=?1 ORDER BY node")
        .unwrap()
        .query_map([&invocation], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(positions.len(), 2, "{positions:?}");
    assert_eq!((&positions[0].0, positions[0].2), (&step, 1));
    assert_eq!((&positions[1].0, positions[1].2), (&review, 1));
    assert_ne!(positions[0].1, positions[1].1, "each step is its own node");

    // Every settled Run's state is its row.
    for run in [&of_wave, &of_task, &child, &step] {
        let (outcome, ended): (Option<String>, Option<i64>) = fixture
            .db()
            .query_row(
                "SELECT outcome, ended_at FROM runs WHERE id=?1",
                [run],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(outcome.as_deref(), Some("completed"), "{run}");
        assert!(ended.is_some());
    }

    // One row per launch, and every reader lists each Run once.
    assert_eq!(fixture.count("runs"), 5);
    let listed =
        |args: &[&str]| -> Vec<Value> { serde_json::from_value(fixture.json(args)).unwrap() };
    let ids = |runs: &[Value]| -> Vec<String> {
        let mut ids: Vec<String> = runs
            .iter()
            .map(|run| run["id"].as_str().unwrap().to_string())
            .collect();
        ids.sort();
        ids
    };
    let sorted = |mut runs: Vec<String>| {
        runs.sort();
        runs
    };
    let all = sorted(vec![
        of_wave.clone(),
        of_task.clone(),
        child.clone(),
        step.clone(),
        review.clone(),
    ]);
    let of_wave_runs = listed(&["runs", "--wave", "task-pr-tests", "--json"]);
    assert_eq!(ids(&of_wave_runs), all);
    for run in &of_wave_runs {
        let subjects: Vec<&str> = run["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|subject| subject["selector"].as_str().unwrap())
            .collect();
        assert!(subjects.contains(&"wave:task-pr-tests"), "{run}");
        assert_eq!(
            subjects.contains(&"task:INF-123"),
            [&of_task, &child].contains(&&run["id"].as_str().unwrap().to_string()),
            "{run}"
        );
    }
    assert_eq!(ids(&listed(&["runs", "--json"])), all);
    assert_eq!(
        ids(&listed(&["usage", "--wave", "task-pr-tests", "--json"])),
        all
    );
    let of_task_runs = sorted(vec![of_task.clone(), child.clone()]);
    for args in [
        &["runs", "--task", "INF-123", "--json"][..],
        &["runs", "--project", "task-pr-tests", "--json"][..],
        &["usage", "--task", "INF-123", "--json"][..],
    ] {
        assert_eq!(ids(&listed(args)), of_task_runs, "{args:?}");
    }
    let children = listed(&["runs", "--parent", &of_task, "--json"]);
    assert_eq!(ids(&children), vec![child.clone()]);
    assert_eq!(children[0]["parent_run_id"], of_task.as_str());

    let activity = fixture.json(&["activity", "--wave", "task-pr-tests", "--json"]);
    let started = sorted(
        activity["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["fact"]["kind"] == "run_started")
            .map(|item| item["fact"]["run_id"].as_str().unwrap().to_string())
            .collect(),
    );
    assert_eq!(started, all);

    let sessions = fixture.sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["run_id"], review.as_str());
}

/// A provider stand-in that fails its first launch when `fail-once` exists
/// and records its decision inside a Flow step. A development `lf` resolves
/// its own Home, so the stand-in names the fixture's Home for the nested call.
fn saved_flow_stand_in(fixture: &Fixture) {
    let provider = fixture.home.path().join("bin/opencode");
    std::fs::write(
        &provider,
        format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\n\
             printf '%s\\n' \"message=created id=ses_$LF_RUN_ID\" >&2\n\
             echo \"$LF_RUN_ID\" >> '{}'\n\
             if [ -f \"$LF_CONTROL_HOME/fail-once\" ]; then rm -f \"$LF_CONTROL_HOME/fail-once\"; exit 7; fi\n\
             if [ -n \"$LF_FLOW_STEP\" ]; then LF_HOME=\"$LF_CONTROL_HOME\" LF_DB_PATH=\"$LF_CONTROL_DB_PATH\" \
             '{}' flow decide advance 'Proof observed' \
             >> \"$LF_CONTROL_HOME/decide.log\" 2>&1 || true; fi\n",
            fixture.launched.display(),
            env!("CARGO_BIN_EXE_lf")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let lf = fixture.repo.path().join(".lf");
    std::fs::create_dir_all(lf.join("skills")).unwrap();
    std::fs::create_dir_all(lf.join("flows")).unwrap();
    std::fs::write(lf.join("skills/work-proof.md"), "Do the fixture work.").unwrap();
    std::fs::write(lf.join("skills/review-proof.md"), "Review the fixture.").unwrap();
    std::fs::write(lf.join("skills/decide-proof.md"), "Decide the fixture.").unwrap();
    std::fs::write(
        lf.join("flows/work-then-review.yaml"),
        "- work-proof\n- step:\n    id: review\n    name: review-proof\n    human: true\n",
    )
    .unwrap();
    std::fs::write(
        lf.join("flows/work-then-decide.yaml"),
        "- step:\n    id: work\n    name: work-proof\n- step:\n    id: decide\n    name: decide-proof\n    repeat:\n      from: work\n",
    )
    .unwrap();
}

/// (id, node, attempt, outcome, task) of one Run.
type AttemptRow = (String, i64, i64, Option<String>, Option<String>);

/// Every Run of one invocation, oldest attempt first.
fn invocation_runs(fixture: &Fixture, invocation: &str) -> Vec<AttemptRow> {
    fixture
        .db()
        .prepare(
            "SELECT id, node, attempt, outcome, task_id FROM runs WHERE invocation_id=?1
             ORDER BY created_at, attempt",
        )
        .unwrap()
        .query_map([invocation], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// The invocation row is the only cursor owner of a saved Flow: a step whose
/// provider dies is a failed attempt, `--retry` is the next attempt, the
/// review lists under the Task, and no position file ever exists.
#[test]
fn a_task_flow_runs_on_its_row_through_failure_retry_and_review() {
    let fixture = Fixture::new(false);
    saved_flow_stand_in(&fixture);
    let task = support::register_unrun_task(
        fixture.home.path(),
        fixture.repo.path(),
        "task-row-flow",
        &fixture.repo.head_sha(),
    );
    let task_id = task.task.id.to_string();
    std::fs::write(fixture.home.path().join("fail-once"), "").unwrap();
    let blocked = fixture.run(&[
        "--task",
        "INF-123",
        "--model",
        "opencode",
        "flow",
        "work-then-review",
        "-b",
        "--no-loopflow",
    ]);
    let stderr = String::from_utf8_lossy(&blocked.stderr);
    assert!(!blocked.status.success(), "{blocked:?}");
    let (invocation, failure, pointer): (String, Option<String>, Option<String>) = fixture
        .db()
        .query_row(
            "SELECT f.id, f.failure_json, t.current_invocation_id FROM flow_invocations f
             JOIN tasks t ON t.id=f.task_id WHERE f.task_id=?1",
            [&task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    let flow_dir = fixture.home.path().join("flows").join(&invocation);
    let no_position_file = || {
        assert!(
            !flow_dir.join("position.json").exists(),
            "the row is the only cursor owner"
        );
        assert!(!flow_dir.join("position.lock").exists());
    };
    no_position_file();
    assert!(stderr.contains("blocked"), "{stderr}");
    let failure = failure.unwrap();
    assert!(failure.contains("work-proof Run failed"), "{failure}");
    assert_eq!(pointer, None, "a Flow about the Task is not its Flow");
    let failed = invocation_runs(&fixture, &invocation);
    assert_eq!(failed.len(), 1, "{failed:?}");
    assert_eq!(
        (failed[0].2, failed[0].3.as_deref(), failed[0].4.as_deref()),
        (1, Some("failed"), Some(task_id.as_str()))
    );

    let waiting = fixture.run(&["flow", "resume", &invocation, "--retry"]);
    assert!(
        String::from_utf8_lossy(&waiting.stderr).contains("waiting for human input"),
        "{waiting:?}"
    );
    no_position_file();
    let runs = invocation_runs(&fixture, &invocation);
    assert_eq!(runs.len(), 3, "{runs:?}");
    assert_eq!(runs[0].0, failed[0].0);
    assert_eq!(
        (runs[1].1, runs[1].2, runs[1].3.as_deref()),
        (runs[0].1, 2, Some("completed"))
    );
    assert_eq!((runs[2].2, runs[2].3.as_deref()), (1, None));
    assert_ne!(runs[2].1, runs[1].1, "the review is its own node");
    for run in &runs {
        assert_eq!(run.4.as_deref(), Some(task_id.as_str()), "{run:?}");
    }
    let review = runs[2].0.clone();
    let listed: Vec<Value> =
        serde_json::from_value(fixture.json(&["runs", "--task", "INF-123", "--json"])).unwrap();
    let mut listed: Vec<&str> = listed
        .iter()
        .map(|run| run["id"].as_str().unwrap())
        .collect();
    listed.sort();
    let mut expected: Vec<&str> = runs.iter().map(|run| run.0.as_str()).collect();
    expected.sort();
    assert_eq!(listed, expected);

    let sessions = fixture.sessions();
    assert_eq!(sessions.len(), 1, "{sessions:?}");
    let session = &sessions[0];
    let id = session["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("session_"), "{id}");
    assert_eq!(session["kind"], "flow");
    assert_eq!(session["run_id"], review.as_str());
    assert_eq!(
        session["work"],
        serde_json::json!({"kind": "task", "id": task_id})
    );
    assert_eq!(session["flow_membership"]["flow"], "work-then-review");
    assert_eq!(session["flow_membership"]["node"], "1");

    let ready = fixture
        .command(&["session", "ready", "Ship the parser"])
        .env("LF_RUN_ID", &review)
        .env(
            "LF_HUMAN_SESSION",
            serde_json::json!({"kind": "standalone_flow", "id": id}).to_string(),
        )
        .output()
        .unwrap();
    assert!(ready.status.success(), "{ready:?}");
    let completed = fixture.run(&["session", "complete", &id]);
    assert!(completed.status.success(), "{completed:?}");
    let resumed = fixture.run(&["flow", "resume", &invocation]);
    assert!(resumed.status.success(), "{resumed:?}");
    let state: String = fixture
        .db()
        .query_row(
            "SELECT state FROM flow_invocations WHERE id=?1",
            [&invocation],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "completed");
    no_position_file();
    assert_eq!(invocation_runs(&fixture, &invocation).len(), 3);
}

/// `lf flow decide` inside a taskless step writes the invocation row, and the
/// Flow takes the recorded edge.
#[test]
fn a_taskless_step_records_its_decision_on_the_invocation() {
    let fixture = Fixture::new(false);
    saved_flow_stand_in(&fixture);
    let output = fixture.run(&[
        "--model",
        "opencode",
        "flow",
        "work-then-decide",
        "-b",
        "--no-loopflow",
    ]);
    let log = std::fs::read_to_string(fixture.home.path().join("decide.log")).unwrap_or_default();
    assert!(
        output.status.success(),
        "{}\n--- decide.log ---\n{log}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        log.lines()
            .filter(|line| line.contains("Decision recorded"))
            .count(),
        1,
        "{log}"
    );
    assert!(
        log.contains("does not own a decision"),
        "the work step refuses a decision: {log}"
    );
    let (state, task, cursor): (String, Option<String>, String) = fixture
        .db()
        .query_row(
            "SELECT state, task_id, review_json FROM flow_invocations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((state.as_str(), task), ("completed", None));
    let cursor: Value = serde_json::from_str(&cursor).unwrap();
    assert_eq!(
        cursor["index"], 2,
        "the recorded Advance left the loop: {cursor}"
    );
    assert_eq!(fixture.launches().len(), 2, "work once, decide once");
    assert!(std::fs::read_dir(fixture.home.path().join("flows"))
        .map(|entries| entries.flatten().all(|entry| {
            std::fs::read_dir(entry.path())
                .unwrap()
                .flatten()
                .all(|file| file.file_name() == "driver.lock")
        }))
        .unwrap_or(true));
}

/// A Flow whose invocation row cannot be written does not start: the store
/// error is the refusal and no provider runs.
#[test]
fn a_flow_refuses_to_start_without_its_row() {
    let fixture = Fixture::new(false);
    saved_flow_stand_in(&fixture);
    let store = LockedStore::new(&fixture);
    let mut launch = fixture.command(&[
        "--model",
        "opencode",
        "flow",
        "work-then-decide",
        "-b",
        "--no-loopflow",
    ]);
    store.select(&mut launch);
    let output = launch.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("unable to open database file"),
        "the refusal names the store error: {stderr}"
    );
    assert_eq!(fixture.launches(), Vec::<String>::new());
    assert!(!fixture.home.path().join("flows").exists());
}
