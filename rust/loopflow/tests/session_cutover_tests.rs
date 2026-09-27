//! Interactive Sessions are `sessions` and `runs` rows from launch to completion.
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
        let deadline = Instant::now() + Duration::from_secs(20);
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

    fn retired_files(&self) -> Vec<PathBuf> {
        fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, found);
                } else if ["session-name.json", "session-resolution.json"]
                    .contains(&entry.file_name().to_string_lossy().as_ref())
                {
                    found.push(path);
                }
            }
        }
        let mut found = Vec::new();
        walk(&self.home.path().join("runs"), &mut found);
        found
    }
}

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
