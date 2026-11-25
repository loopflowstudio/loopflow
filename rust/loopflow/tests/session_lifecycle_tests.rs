//! Conversations own admission, immutable inputs, and history through completion.
//! The real `lf` binary runs in a private Machine; a script stands in for the provider.
#![cfg(unix)]

use loopflow_execution::process::attachment;
mod support;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use loopflow_test_support::{MachineServer, TestRepo};
use rusqlite::OptionalExtension;
use serde_json::Value;

struct Fixture {
    /// Task and Wave launches start their agent only once it answers.
    _server: MachineServer,
    home: tempfile::TempDir,
    repo: TestRepo,
    launched: PathBuf,
}

impl Fixture {
    /// `waits` keeps the stand-in attached until its stdin receives a line.
    fn new(waits: bool) -> Self {
        Self::with_server_provider(waits, true)
    }

    /// Manual resumes pass their own PATH to the holder. Keep the stand-in
    /// off the server's PATH so the recovery watcher cannot launch it first.
    fn manual_conversations() -> Self {
        Self::with_server_provider(false, false)
    }

    fn with_server_provider(waits: bool, server_has_provider: bool) -> Self {
        let home = tempfile::tempdir().unwrap();
        support::seed_store(home.path());
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_lf"), bin.join("lf")).unwrap();
        let launched = home.path().join("launched");
        let provider = bin.join("opencode");
        std::fs::write(
            &provider,
            include_str!("../../loopflow-execution/tests/support/opencode_cli.py")
                .replace("__WAIT__", if waits { "True" } else { "False" }),
        )
        .unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
        let server_path = if server_has_provider {
            format!("{}:/usr/bin:/bin", bin.display())
        } else {
            "/usr/bin:/bin".to_owned()
        };
        Self {
            _server: MachineServer::start_with_path(
                env!("CARGO_BIN_EXE_lf"),
                home.path(),
                &server_path,
            ),
            home,
            repo: TestRepo::new(),
            launched,
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        for (key, _) in std::env::vars_os() {
            let key = key.to_string_lossy().into_owned();
            // An inherited cmux workspace would receive the fixture Session's name.
            if ["LF_", "LOOPFLOW_", "LINEAR_", "CMUX_"]
                .iter()
                .any(|prefix| key.starts_with(prefix))
            {
                command.env_remove(key);
            }
        }
        let home = self.home.path();
        // Another installed harness would leave the stand-in one of several, and
        // lf selects only when exactly one is installed.
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        let inherited = std::env::split_paths(&inherited).filter(|dir| {
            !["claude", "codex", "pi"]
                .iter()
                .any(|h| dir.join(h).exists())
        });
        let path = std::env::join_paths(std::iter::once(home.join("bin")).chain(inherited));
        command
            .args(args)
            .current_dir(self.repo.path())
            .env("HOME", home)
            .env("LF_HOME", home)
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("PATH", path.unwrap())
            // A routed launch signs the provider's shared home in; keep it in the fixture.
            .env("CODEX_HOME", home.join(".codex"))
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
        serde_json::from_value(self.json(&["desktop", "sessions", "--all", "--json"])).unwrap()
    }

    fn recorded_sessions(&self) -> Vec<Value> {
        let db = self.db();
        let mut query = db
            .prepare("SELECT id FROM agent_sessions ORDER BY title,id")
            .unwrap();
        let ids = query
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        ids.iter()
            .map(|id| self.json(&["session", "card", id, "--json"])["session"].clone())
            .collect()
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
        // Its own process group, as a terminal's foreground job has.
        let mut child =
            std::os::unix::process::CommandExt::process_group(&mut self.command(args), 0)
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

    /// Release every live client even if a continuity assertion fails.
    fn with_attached_clients<T>(
        &self,
        clients: impl IntoIterator<Item = Child>,
        check: impl FnOnce() -> T,
    ) -> T {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(check));
        for client in clients {
            self.release(client);
        }
        result.unwrap_or_else(|error| std::panic::resume_unwind(error))
    }

    fn finish_on_resume(&self) {
        std::fs::write(
            self.home.path().join("bin/opencode"),
            include_str!("../../loopflow-execution/tests/support/opencode_cli.py")
                .replace("__WAIT__", "False"),
        )
        .unwrap();
    }

    fn wait_for_graphs(&self) {
        let deadline = Instant::now() + PATIENCE;
        while support::recorded_graphs(self.home.path())
            .iter()
            .any(|(outcome, _)| outcome.is_none())
        {
            assert!(
                Instant::now() < deadline,
                "Graph did not stop at its interactive step"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The store, with Session history attached as `history` and turns as `usage`.
    fn db(&self) -> rusqlite::Connection {
        let store = self.home.path().join("loopflow.db");
        let db = rusqlite::Connection::open(&store).unwrap();
        db.execute(
            "ATTACH ?1 AS history",
            [loopflow_storage::history::history_path(&store)
                .to_string_lossy()
                .as_ref()],
        )
        .unwrap();
        db.execute(
            "ATTACH ?1 AS usage",
            [loopflow_storage::history::history_path(&store)
                .to_string_lossy()
                .as_ref()],
        )
        .unwrap();
        db
    }

    fn count(&self, table: &str) -> i64 {
        self.db()
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn captures(&self) -> i64 {
        self.db()
            .query_row(
                "SELECT count(*) FROM session_events WHERE kind='captured'",
                [],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn input_attachment(&self, session: &str) -> String {
        self.db().query_row(
            "SELECT json_array(s.current_capture,s.input_published,p.agent_session_id,p.attached_lf_process_id,p.attachment_token)
             FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?1",
            [session], |row| row.get(0),
        ).unwrap()
    }

    /// (session id, title, title_source, completed) of the captured input’s Session.
    fn session_row(&self, run_id: &str) -> (String, String, String, bool) {
        self.db()
            .query_row(
                "SELECT s.id, s.title, s.title_source, s.completed_at IS NOT NULL
                 FROM agent_sessions s JOIN session_events i ON i.session_id=s.id AND i.kind='captured'
                 WHERE i.receipt_key=?1",
                [run_id],
                |row| {
Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                },
            )
            .unwrap_or_else(|error| panic!("Capture {run_id} has no Session: {error}"))
    }

    /// Original input attribution, independent of a later conversation binding.
    fn run_parents(&self, run_id: &str) -> (Option<String>, Option<String>, Option<String>) {
        self.db()
            .query_row(
                "SELECT CASE WHEN m.seq IS NOT NULL THEN m.task_id ELSE s.task_id END,
                    CASE WHEN m.seq IS NOT NULL THEN m.wave_id ELSE s.wave_id END,
                    CASE WHEN m.seq IS NULL OR (s.bound_at IS NULL
                              AND m.task_id IS s.task_id AND m.wave_id IS s.wave_id)
                         THEN s.work_source
                         ELSE coalesce(json_extract(m.payload,'$.evidence.subjects[0].source'),
                            CASE WHEN m.task_id IS s.task_id AND m.wave_id IS s.wave_id THEN s.work_source END)
                    END
                 FROM agent_sessions s JOIN session_events i ON i.session_id=s.id AND i.kind='captured'
                 LEFT JOIN history.session_history m ON m.session_id=s.id
                    AND m.receipt_key=i.receipt_key||':manifest.json'
                 WHERE i.receipt_key=?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or_else(|error| panic!("Capture {run_id} has no attribution: {error}"))
    }

    fn run_dir(&self, run_id: &str) -> PathBuf {
        self.home
            .path()
            .join("runs")
            .join(&run_id.strip_prefix("run_").unwrap_or(run_id)[..2])
            .join(run_id)
    }
}

/// An upper bound only: a debug `lf` on a busy machine takes tens of seconds to launch.
const PATIENCE: Duration = Duration::from_secs(180);

const LAUNCH: [&str; 5] = ["-i", "--agent", "opencode", ":", "Review the parser"];

#[test]
fn task_checkout_prepares_the_conversation_and_manual_launch_reuses_it() {
    for headless in [false, true] {
        let fixture = Fixture::manual_conversations();
        std::fs::create_dir_all(fixture.repo.path().join(".lf")).unwrap();
        std::fs::write(
            fixture.repo.path().join(".lf/config.yaml"),
            if headless {
                "agent: codex\n"
            } else {
                "agent: opencode\n"
            },
        )
        .unwrap();
        if headless {
            let binary = fixture.home.path().join("bin/codex");
            std::fs::write(
                &binary,
                support::codex_app_server_script(
                    "Started the Task conversation",
                    r#"printf '%s\n' "$LF_CAPTURE_KEY" >> "$HOME/launched""#,
                ),
            )
            .unwrap();
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let output = fixture
            .command(&[
                "task",
                "create",
                "--wave",
                "inbox",
                "--title",
                "Preserve my conversation",
                "--json",
            ])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let receipt: Value = serde_json::from_slice(&output.stdout).unwrap();
        let task = &receipt["task"];
        let id = task["id"].as_str().unwrap();
        let db = fixture.db();
        let claims = || -> Vec<String> {
            let mut statement = db
                .prepare("SELECT kind FROM task_ownership WHERE task_id=?1 ORDER BY at")
                .unwrap();
            statement
                .query_map([id], |row| row.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        let stored: (Option<String>, Option<String>) = db
            .query_row(
                "SELECT p.session_id,c.worktree FROM tasks
                 LEFT JOIN task_checkouts c ON c.task_id=tasks.id
                 LEFT JOIN task_primary_sessions p ON p.task_id=tasks.id WHERE tasks.id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(stored, (None, None));
        assert!(claims().is_empty(), "creating a Task claims nothing");
        assert_eq!(
            fixture.count("agent_sessions"),
            0,
            "creating a Task makes no conversation"
        );
        assert!(fixture.launches().is_empty());
        let ensured = std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                fixture.json(&["session", "resume", "--prepare", "--task", id, "--json"])
            });
            let second = scope.spawn(|| {
                fixture.json(&["session", "resume", "--prepare", "--task", id, "--json"])
            });
            let first = first.join().unwrap();
            assert_eq!(first["id"], second.join().unwrap()["id"]);
            first
        });
        let session = ensured["id"].as_str().unwrap().to_owned();
        let primary: String = db
            .query_row(
                "SELECT session_id FROM task_primary_sessions WHERE task_id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(primary, session);
        assert_eq!(ensured["workspace"]["task_id"], id);
        assert_ne!(
            ensured["cwd"],
            fixture.repo.path().to_string_lossy().as_ref()
        );
        assert!(fixture.launches().is_empty());
        let prepared = fixture.count("agent_sessions");
        assert_eq!(fixture.captures(), 0);
        // Allocating the checkout is the work reservation that claims the Task.
        assert_eq!(claims(), ["claim"]);
        let opened = if headless {
            fixture.run(&["-b", "steer", &session, "Start the Task conversation"])
        } else {
            fixture.run(&["session", "resume", &session])
        };
        assert!(opened.status.success(), "{opened:?}");
        assert_eq!(
            fixture.count("agent_sessions"),
            prepared,
            "first start reused the prepared Session"
        );
        assert_eq!(fixture.captures(), 1);
        let native: String = db.query_row(
            "SELECT provider_thread FROM session_events WHERE session_id=?1 AND kind='captured' AND provider_thread IS NOT NULL ORDER BY seq DESC LIMIT 1",
            [&session], |row| row.get(0)).unwrap();
        assert!(
            !native.is_empty(),
            "the prepared Session owns native history"
        );
        if !headless {
            assert_eq!(
                fixture.session_row(fixture.launches().last().unwrap()).0,
                session
            );
        }
        assert_eq!(claims(), ["claim"], "launching keeps the one claim");
    }
}

#[test]
fn primary_readiness_is_metadata_only_for_repository_wave_and_task() {
    let fixture = Fixture::manual_conversations();
    support::register_task_with_pr(
        fixture.home.path(),
        fixture.repo.path(),
        "main",
        &fixture.repo.head_sha(),
    );
    for scope in [
        vec![],
        vec!["--wave", "task-pr-tests"],
        vec!["--task", "INF-123"],
    ] {
        let args = [&["session", "resume", "--prepare", "--json"][..], &scope].concat();
        let first = fixture.json(&args);
        let id = first["id"].as_str().unwrap();
        assert_eq!(fixture.json(&args)["id"], id);
        assert!(fixture.launches().is_empty());
        let before: i64 = fixture
            .db()
            .query_row(
                "SELECT (SELECT count(*) FROM session_events WHERE session_id=?1)
                    + (SELECT count(*) FROM history.session_history WHERE session_id=?1)",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        // Nothing runs: Desktop's one prepared shape reports no live client to reattach.
        let prepared = fixture.json(&["session", "resume", id, "--prepare", "--json"]);
        assert!(
            !matches!(prepared["state"].as_str(), Some("active" | "detached")),
            "{prepared}"
        );
        assert!(fixture.launches().is_empty());
        let after: i64 = fixture
            .db()
            .query_row(
                "SELECT (SELECT count(*) FROM session_events WHERE session_id=?1)
                    + (SELECT count(*) FROM history.session_history WHERE session_id=?1)",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(before, after, "attachment admitted or replayed an input");
        assert_eq!(fixture.json(&args)["id"], id);
    }
}

#[test]
fn work_first_repository_entry_preserves_primary_and_mixed_inventory() {
    let fixture = Fixture::new(false);
    fixture
        .repo
        .create_file(".lf/config.yaml", "agent: opencode:opencode/test-model\n");
    fixture.repo.stage_all();
    fixture.repo.commit("Configure repository conversation");
    let preview = fixture.json(&["skill", "repo/session", "--context", "--json"]);
    let prompt = preview["task_prompt"].as_str().unwrap();
    assert!(prompt.contains("What do you want to work on?"));
    assert!(prompt.contains("no setup checklist first"));
    assert!(prompt.contains("Opening asks for nothing"));
    let empty = fixture.json(&["desktop", "plan", "--all", "--json"]);
    let primary = fixture.json(&["session", "resume", "--prepare", "--json"]);
    let again = fixture.json(&["session", "resume", "--prepare", "--json"]);
    assert_eq!(primary["id"], again["id"]);
    assert_eq!(fixture.count("agent_sessions"), 1);
    let configured: (String, String) = fixture
        .db()
        .query_row(
            "SELECT provider, model FROM agent_sessions WHERE id=?1",
            [primary["id"].as_str().unwrap()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        configured,
        ("opencode".into(), "opencode/test-model".into())
    );

    // Ordinary unscoped history and Task conversations survive the navigation cut.
    let unscoped = fixture.repo.create_named_worktree("unscoped");
    assert!(fixture
        .command(&LAUNCH)
        .current_dir(unscoped)
        .output()
        .unwrap()
        .status
        .success());
    let task = support::register_task_without_pr(
        fixture.home.path(),
        fixture.repo.path(),
        "work-first",
        &fixture.repo.head_sha(),
    );
    let task_primary = fixture.json(&[
        "session",
        "resume",
        "--prepare",
        "--task",
        "INF-123",
        "--json",
    ]);
    let population = serde_json::json!({
        "empty": empty,
        "primary": primary,
        "reopened": again,
        "task_primary": task_primary,
        "sessions": fixture.recorded_sessions(),
        "roadmap": fixture.json(&["desktop", "plan", "--all", "--json"]),
    });
    assert_eq!(fixture.count("agent_sessions"), 3);
    assert_eq!(fixture.count("graph_processes"), 0);
    assert!(task.task.worktree.is_some());
    assert_work_first_capture(&population);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/dto/work_first_entry.json");
    if std::env::var_os("LOOPFLOW_UPDATE_WORK_FIRST_FIXTURE").is_some() {
        std::fs::write(
            path,
            serde_json::to_string_pretty(&population).unwrap() + "\n",
        )
        .unwrap();
    } else {
        let saved = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_work_first_capture(&saved);
    }
}

fn assert_work_first_capture(capture: &Value) {
    use loopflow::lf::commands::waves::RoadmapSnapshot;

    assert_eq!(capture["primary"]["id"], capture["reopened"]["id"]);
    assert_eq!(capture["primary"]["primary_scope"], "repository");
    assert_eq!(capture["primary"]["provider"], "opencode");
    assert_eq!(capture["empty"]["waves"], serde_json::json!([]));
    let _: RoadmapSnapshot = serde_json::from_value(capture["roadmap"].clone()).unwrap();
    let sessions = capture["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 3);
    assert_eq!(
        sessions
            .iter()
            .filter(|row| row["id"] == capture["primary"]["id"])
            .count(),
        1
    );
    assert_eq!(
        sessions
            .iter()
            .filter(|row| row["id"] == capture["task_primary"]["id"])
            .count(),
        1
    );
    assert_eq!(
        sessions
            .iter()
            .filter(|row| row["primary_scope"].is_null()
                && row["task_ids"].as_array().unwrap().is_empty())
            .count(),
        1
    );
}

#[test]
fn personal_task_launch_resume_and_skill_graph_keep_native_identity() {
    let fixture = Fixture::new(false);
    support::install_native_codex(&fixture.home.path().join("bin"));
    fixture
        .repo
        .create_file(".lf/skills/inspect-parser.md", "Inspect the parser.");
    fixture.repo.create_file(
        ".lf/graphs/proof.yaml",
        "- inspect-parser\n- break: review\n",
    );
    fixture.repo.stage_all();
    fixture.repo.commit("Native lifecycle fixture");
    assert!(Command::new("git")
        .current_dir(fixture.repo.path())
        .args(["remote", "remove", "origin"])
        .status()
        .unwrap()
        .success());
    let task =
        fixture.json(&["task", "create", "--title", "Native local work", "--json"])["task"].clone();
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(fixture.count("agent_sessions"), 0);
    let placed = fixture.json(&["task", "checkout", task_id, "--json"]);
    let worktree = Path::new(placed["worktree"].as_str().unwrap());
    let launch = fixture
        .command(&[
            "-i",
            "--task",
            task_id,
            "--agent",
            "codex",
            ":",
            "Inspect parser",
        ])
        .current_dir(worktree)
        .output()
        .unwrap();
    assert!(launch.status.success(), "{launch:?}");
    // A chat is a bare Codex terminal: Codex keeps its own configured roots
    // and argv adds the main checkout the worktree's Git metadata lives in.
    let argv: Vec<String> = serde_json::from_str(
        std::fs::read_to_string(fixture.home.path().join("native-argv.jsonl"))
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let main_checkout = fixture.repo.path().canonicalize().unwrap();
    assert!(
        argv.windows(2)
            .any(|pair| pair[0] == "--add-dir" && Path::new(&pair[1]) == main_checkout),
        "worktree Git access needs the main checkout: {argv:?}"
    );
    let capture = fixture.launches()[0].clone();
    let (session, ..) = fixture.session_row(&capture);
    let native: String = fixture
        .db()
        .query_row(
            "SELECT provider_thread FROM session_events WHERE session_id=?1 AND kind='captured' AND provider_thread IS NOT NULL ORDER BY seq DESC LIMIT 1",
            [&session],
            |row| row.get(0),
        )
        .unwrap();
    let resumed = fixture
        .command(&["session", "resume", &session])
        .current_dir(worktree)
        .output()
        .unwrap();
    assert!(resumed.status.success(), "{resumed:?}");
    assert_eq!(fixture.count("agent_sessions"), 1);
    assert_eq!(
        fixture
            .db()
            .query_row(
                "SELECT provider_thread FROM session_events WHERE session_id=?1 AND kind='captured' AND provider_thread IS NOT NULL ORDER BY seq DESC LIMIT 1",
                [&session],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        native
    );
    support::seed_store(fixture.home.path());
    // A bare run stops before the interactive review step; it may return as
    // its Graph registers, so wait for it to stop there. The Task reads as
    // moving until the run's own Process has ended.
    let run = fixture
        .command(&["--agent", "codex", "task", "run", task_id, "proof"])
        .output()
        .unwrap();
    assert!(run.status.success(), "{run:?}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let status = fixture.json(&["desktop", "task", task_id, "--json"]);
        let finished = support::recorded_graphs(fixture.home.path())
            .first()
            .is_some_and(|graph| graph.0.is_some());
        if finished
            && status["execution"]["work"]["graph"]["position"]["kind"] == "breakpoint"
            && status["execution"]["work"]["graph"]["position"]["step"] == "review"
            && status["execution"]["status"] == "active"
        {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{status}");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(fixture.count("agent_sessions"), 2);
    let graphs = support::recorded_graphs(fixture.home.path());
    assert_eq!(graphs.len(), 1);
    assert_eq!(graphs[0].0.as_deref(), Some("succeeded"));
    assert_eq!(graphs[0].1.len(), 1);
    // Provider deletion remains a refusal at native resume; it grants no provider launch.
    // Change only planning ownership in this disposable fixture, retaining the Session.
    fixture
        .db()
        .execute(
            "UPDATE tasks SET external_issue_id='fixture-deleted' WHERE id=?1",
            [task_id],
        )
        .unwrap();
    loopflow_execution::store::open_sqlite(&fixture.home.path().join("loopflow.db"))
        .unwrap()
        .record_tracker_event("fixture-deleted", None, true)
        .unwrap();
    let refused = fixture
        .command(&["session", "resume", &session])
        .current_dir(worktree)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(
        [&refused.stdout, &refused.stderr]
            .iter()
            .any(|output| String::from_utf8_lossy(output).contains("deleted")),
        "{refused:?}"
    );
    assert_eq!(fixture.count("agent_sessions"), 2);
    fixture.json(&["task", "abandon", task_id, "--json"]);
    let terminal = fixture.run(&["-b", "--agent", "codex", "task", "run", task_id]);
    assert!(!terminal.status.success());
    assert_eq!(fixture.count("agent_sessions"), 2);
    let removed = fixture.run(&["task", "delete", task_id]);
    assert!(removed.status.success(), "{removed:?}");
    let history = fixture.json(&["desktop", "task", task_id, "--json"]);
    assert_eq!(history["tracker_state"]["state"], "removed");
    assert_eq!(
        history["execution"]["actions"]["reason"],
        "Task was deleted; retained history is read-only"
    );
    let refused = fixture
        .command(&["session", "resume", &session])
        .current_dir(fixture.repo.path())
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert_eq!(fixture.count("agent_sessions"), 2);
    if worktree.exists() {
        std::fs::remove_dir_all(worktree).unwrap();
    }
}

#[test]
fn task_conversation_reopens_after_terminal_startup_failure() {
    let fixture = Fixture::new(false);
    let task = support::register_task_with_pr(
        fixture.home.path(),
        &fixture.repo.path().canonicalize().unwrap(),
        "reopen",
        &fixture.repo.head_sha(),
    );
    let provider = fixture.home.path().join("bin/opencode");
    let original = std::fs::read(&provider).unwrap();
    std::fs::write(&provider, "#!/bin/sh\nexit 3\n").unwrap();
    let failed = fixture.run(&LAUNCH);
    assert!(!failed.status.success());
    let sessions = fixture.sessions();
    assert_eq!(sessions.len(), 1);
    let id = sessions[0]["id"].as_str().unwrap();
    std::fs::write(&provider, original).unwrap();
    for args in [vec!["session", "resume"], vec!["session", "resume", id]] {
        let result = fixture.run(&args);
        assert!(result.status.success(), "{result:?}");
    }
    assert_eq!(fixture.count("agent_sessions"), 1);
    assert_eq!(fixture.sessions()[0]["id"], id);
    assert!(fixture.sessions()[0]["task_ids"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == task.task.id.as_str()));
}

#[test]
fn conversation_keeps_its_name_and_identity_until_completed() {
    let fixture = Fixture::new(true);

    let (first, first_run) = fixture.attach(&LAUNCH);
    assert_eq!(fixture.sessions().len(), 1);
    let (id, title, source, completed) = fixture.session_row(&first_run);
    assert_ne!(
        id, first_run,
        "conversation identity differs from its captured input"
    );
    assert_eq!((source.as_str(), completed), ("generated", false));
    assert_eq!(title, "Review parser");
    assert_eq!(fixture.run_parents(&first_run), (None, None, None));

    let listed = fixture.sessions();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["id"], id.as_str());
    assert_eq!(listed[0]["title"], title.as_str());
    assert_eq!(listed[0]["title_source"], "generated");
    assert_eq!(listed[0]["state"], "active");
    assert_eq!(listed[0]["provider"], "opencode");
    assert_eq!(listed[0]["work"], Value::Null);

    // The agent and person name the same durable Session.
    let suggested = fixture.json(&["session", "rename", &id, "Parser", "--suggest", "--json"]);
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
    let (_, title, source, _) = fixture.session_row(&first_run);
    assert_eq!(
        (title.as_str(), source.as_str()),
        ("Parser review", "human")
    );
    let blank = fixture.run(&["session", "rename", &id, "  "]);
    assert!(!blank.status.success());

    fixture.release(first);
    assert!(
        fixture.sessions().is_empty(),
        "exited orphans leave the working set"
    );
    let (_, saved_title, saved_source, completed) = fixture.session_row(&first_run);
    assert!(completed);
    assert_eq!(
        (saved_title.as_str(), saved_source.as_str()),
        ("Parser review", "human")
    );
    let card = fixture.json(&["session", "card", &id, "--json"]);
    assert_eq!(card["session"]["id"], id);
    assert_eq!(card["session"]["state"], "closed");

    let (second, second_run) = fixture.attach(&LAUNCH);
    let (second_id, ..) = fixture.session_row(&second_run);
    assert_ne!(second_id, id);
    assert_eq!(fixture.sessions().len(), 1);
    fixture.release(second);
    assert!(fixture.sessions().is_empty());
}

#[test]
fn sigint_records_session_interruption_and_retires_the_orphan() {
    let fixture = Fixture::new(true);
    let (mut child, input) = fixture.attach(&LAUNCH);
    let (id, ..) = fixture.session_row(&input);
    // SAFETY: the launch leads its own process group. Terminal Ctrl+C reaches
    // that whole group: the lf typed there and its provider.
    assert_eq!(unsafe { libc::kill(-(child.id() as i32), libc::SIGINT) }, 0);
    let deadline = Instant::now() + PATIENCE;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.code(), Some(130));
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("interrupt did not finish");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(fixture.sessions().is_empty());
    assert!(fixture.session_row(&input).3);
    let history = fixture.json(&["session", "history", &id, "--json"]);
    assert!(history["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["payload"]["source"] == "terminal.json"
            && event["payload"]["evidence"]["outcome"] == "interrupted"));
}

#[test]
fn inventory_scopes_before_paging_and_keeps_worktree_repository_identity() {
    let fixture = Fixture::new(false);
    let task = support::register_task_with_pr(
        fixture.home.path(),
        fixture.repo.path(),
        "inventory",
        &fixture.repo.head_sha(),
    );
    let foreign = TestRepo::new();
    let worktree = fixture.repo.create_named_worktree("inventory-sibling");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for index in 0..113 {
        let id = format!("inventory-{index:03}");
        let foreign_row = index < 110;
        let session = loopflow_execution::session::LfSession {
            captured: None,
            caller_artifact_key: None,
            task_id: (index >= 110).then(|| task.task.id.clone()),
            wave_id: None,
            graph_lf_process_id: None,
            work_source: (index >= 110)
                .then_some(loopflow_execution::session::WorkSource::Declared),
            bound_at: None,
            id: id.clone().into(),
            artifact_key: Some(uuid::Uuid::new_v4().simple().to_string()),
            input_published: true,
            cwd: if foreign_row {
                foreign.path().to_path_buf()
            } else {
                worktree.clone()
            },
            skill: None,
            provider: Some("opencode".into()),
            model: None,
            node: None,
            iterations: None,
            interactive: true,
            repo: None,
            title: format!(
                "{}-{index:03}",
                if foreign_row {
                    "AAA foreign"
                } else {
                    "ZZZ local"
                }
            ),
            title_source: loopflow_execution::session::TitleSource::Human,
            request: None,
            completed_at: None,
            created_at: 1,
        };
        if foreign_row {
            let dir = fixture.run_dir(session.artifact_key.as_deref().unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("provider-session.json"),
                "broken payload outside selected repo",
            )
            .unwrap();
        }
        runtime
            .block_on(task.store.create_session(session))
            .unwrap();
    }
    let list = ["session", "list", "--json", "--limit", "2"];
    let page = fixture.json(&list);
    assert_eq!(
        page.as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["inventory-110", "inventory-111"]
    );
    // `--select` keeps only the named paths of each listed Session.
    assert_eq!(
        fixture.json(&["session", "list", "--json", "--limit", "2", "--select", "id"]),
        serde_json::json!([{"id": "inventory-110"}, {"id": "inventory-111"}])
    );
    let output = fixture
        .command(&list)
        .current_dir(&worktree)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        page
    );
    let filtered = fixture.json(&[
        "session",
        "list",
        "--json",
        "--limit",
        "1",
        "--search",
        "local-112",
        "--task",
        &task.task.identifier,
    ]);
    assert_eq!(filtered.as_array().unwrap().len(), 1);
    assert_eq!(filtered[0]["id"], "inventory-112");
    let end = fixture.json(&[
        "session",
        "list",
        "--json",
        "--page",
        "--limit",
        "2",
        "--after",
        "inventory-111",
    ]);
    assert_eq!(end["entries"].as_array().unwrap().len(), 1);
    assert_eq!(end["entries"][0]["id"], "inventory-112");
    fixture
        .db()
        .execute(
            "UPDATE agent_sessions SET interactive=0 WHERE id IN ('inventory-000','inventory-112')",
            [],
        )
        .unwrap();
    let default = fixture.json(&["session", "list", "--json", "--limit", "0"]);
    assert_eq!(default.as_array().unwrap().len(), 2);
    let both = fixture.json(&["session", "list", "--headless", "--json", "--limit", "0"]);
    assert_eq!(both.as_array().unwrap().len(), 3);
    assert!(both
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["id"].as_str().unwrap() >= "inventory-110"));
    let everywhere = fixture.json(&[
        "session",
        "list",
        "--headless",
        "--all",
        "--json",
        "--limit",
        "0",
    ]);
    assert_eq!(everywhere.as_array().unwrap().len(), 113);

    // All three local conversations belong to this Task, but only unfinished
    // interactive participation belongs in its ordinary Session list.
    fixture
        .db()
        .execute(
            "UPDATE agent_sessions SET completed_at=2 WHERE id='inventory-110'",
            [],
        )
        .unwrap();
    let list = ["session", "list", "--task", &task.task.identifier, "--json"];
    for _ in 0..2 {
        let current = fixture.json(&list);
        assert_eq!(current.as_array().unwrap().len(), 1);
        assert_eq!(current[0]["id"], "inventory-111");
    }
    let current = fixture.json(&[
        "session",
        "list",
        "--task",
        &task.task.identifier,
        "--headless",
        "--json",
    ]);
    assert_eq!(current.as_array().unwrap().len(), 2);
    let history = fixture.json(&["session", "card", "inventory-110", "--json"]);
    // A historical Session completion without an AgentProcess proves no death.
    assert_eq!(history["session"]["state"], "unknown");
    assert_eq!(
        history["session"]["task_ids"],
        serde_json::json!([task.task.id])
    );
    assert_eq!(fixture.count("agent_sessions"), 113);
}

#[test]
fn public_history_discovers_unlinked_native_receipts_without_borrowing_a_later_bind() {
    let fixture = Fixture::new(false);
    let launched = fixture.run(&LAUNCH);
    assert!(launched.status.success(), "{launched:?}");
    let (session, ..) = fixture.session_row(&fixture.launches()[0]);
    let task = support::register_task_with_pr(
        fixture.home.path(),
        fixture.repo.path(),
        "native-history",
        &fixture.repo.head_sha(),
    );
    let store =
        loopflow_execution::store::open_sqlite(&fixture.home.path().join("loopflow.db")).unwrap();
    let closed = store.session(&session).unwrap().unwrap();
    loopflow_execution::store::sessions::bind_session(
        &store,
        &session,
        closed.captured,
        &task.task.id,
    )
    .unwrap();
    store
        .execute_history(
            "INSERT INTO turns(session_id,provider_thread,provider_turn,ended_at,status,
                 usage_at,usage_final,usage_partial,input_tokens,cached_input_tokens,output_tokens,
                 reasoning_tokens)
             VALUES(?1,'retained-thread','retained-turn',strftime('%s','now'),
                 'completed',strftime('%s','now'),0,0,12,0,3,0)",
            [&session],
        )
        .unwrap();
    let captures = fixture.count("agent_sessions");
    {
        let command = vec!["history", "usage", "--days", "0", "--json"];
        let rows = fixture.json(&command);
        let recovered = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["captured"].is_null())
            .unwrap();
        assert_eq!(recovered["session_id"], session);
        assert!(recovered["artifact_key"].is_null());
        assert!(recovered["task_id"].is_null());
        assert!(recovered["wave_id"].is_null());
        assert!(recovered["providers"][0]["lf_process_id"].is_null());
        assert!(recovered["providers"][0]["reference"]["start_seq"].is_null());
        assert_eq!(recovered["providers"][0]["outcome"], "completed");
        assert_eq!(recovered["usage"]["input_tokens"], 12);
        assert_eq!(recovered["usage"]["final_streams"], 0);
        let mut filtered = command;
        filtered.extend(["--task", task.task.id.as_str()]);
        assert!(fixture.json(&filtered).as_array().unwrap().is_empty());
    }
    assert_eq!(fixture.count("agent_sessions"), captures);
}

#[test]
fn ensured_task_conversation_opens_and_recovers_unrecorded_native_exits() {
    let fixture = Fixture::manual_conversations();
    support::register_task_with_pr(
        fixture.home.path(),
        fixture.repo.path(),
        "main",
        &fixture.repo.head_sha(),
    );
    let first = fixture.json(&[
        "-a",
        "opencode",
        "session",
        "resume",
        "--prepare",
        "--task",
        "INF-123",
        "--json",
    ]);
    assert!(fixture.launches().is_empty());
    assert!(first["agent_process_id"].is_null());

    {
        let id = first["id"].as_str().unwrap();
        let opened = fixture.run(&["session", "resume", id]);
        assert!(opened.status.success(), "{opened:?}");
        assert_eq!(
            fixture.session_row(fixture.launches().last().unwrap()).0,
            id
        );

        // Model the installed failure: the native provider exited, but its
        // launcher never recorded that fact. Its identity and history survive.
        for exact in [true, false] {
            let agent: String = fixture
                .db()
                .query_row(
                    "SELECT agent_process_id FROM agent_sessions WHERE id=?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap();
            fixture.db().execute(
                "UPDATE processes SET completed_at=NULL,outcome=NULL,spawn_state='spawn_requested'
                 WHERE id=?1 AND agent_interactive=1",
                [&agent],
            ).unwrap();
            let before = fixture.launches().len();
            let reopened = if exact {
                fixture.run(&["session", "resume", id])
            } else {
                fixture.run(&["session", "resume"])
            };
            assert!(reopened.status.success(), "{reopened:?}");
            assert_eq!(fixture.launches().len(), before + 1);
            assert_eq!(
                fixture.session_row(fixture.launches().last().unwrap()).0,
                id
            );
            let (completed, outcome): (bool, Option<String>) = fixture
                .db()
                .query_row(
                    "SELECT completed_at IS NOT NULL,outcome FROM processes WHERE id=?1",
                    [&agent],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert!(completed);
            assert!(
                outcome.is_none(),
                "observed death is not a successful outcome"
            );
        }
    }
}

#[test]
fn ensured_conversation_recovers_native_exit_before_history_is_published() {
    {
        let fixture = Fixture::manual_conversations();
        support::register_task_with_pr(
            fixture.home.path(),
            fixture.repo.path(),
            "main",
            &fixture.repo.head_sha(),
        );
        let session = fixture.json(&[
            "-a",
            "opencode",
            "session",
            "resume",
            "--prepare",
            "--task",
            "INF-123",
            "--json",
        ]);
        let id = session["id"].as_str().unwrap();
        let provider = fixture.home.path().join("bin/opencode");
        let script = std::fs::read_to_string(&provider).unwrap();
        std::fs::write(
            &provider,
            script.replace(
                "if \"run\" not in sys.argv:\n    _record_launch()",
                "if \"run\" not in sys.argv:\n    _record_launch()\n    raise SystemExit(3)",
            ),
        )
        .unwrap();
        let failed = fixture.run(&["session", "resume", id]);
        assert!(!failed.status.success());
        // The reopening runs in a held terminal, so its error reaches the
        // viewer's screen.
        let screen = format!(
            "{}{}",
            String::from_utf8_lossy(&failed.stdout),
            String::from_utf8_lossy(&failed.stderr)
        );
        assert!(screen.contains("opencode exited"), "{screen}");
        let previous = fixture.launches()[0].clone();
        fixture
            .db()
            .execute(
                "UPDATE processes SET completed_at=NULL,outcome=NULL,spawn_state='spawn_requested'
             WHERE id=(SELECT agent_process_id FROM agent_sessions WHERE id=?1)",
                [id],
            )
            .unwrap();
        std::fs::write(provider, script).unwrap();

        let opened = fixture.run(&["session", "resume", id]);
        assert!(opened.status.success(), "{opened:?}");
        let launches = fixture.launches();
        // The reopening continues the conversation lf named at the first launch.
        assert_eq!(launches.len(), 2);
        assert_eq!(fixture.session_row(&previous).0, id);
        assert_eq!(fixture.session_row(&launches[1]).0, id);
        let primary = fixture.json(&[
            "session",
            "resume",
            "--prepare",
            "--task",
            "INF-123",
            "--json",
        ]);
        assert_eq!(primary["id"], session["id"]);
        assert!(!fixture.session_row(&launches[1]).3);
    }
}

#[test]
fn a_task_primary_is_one_of_its_own_conversations() {
    let fixture = Fixture::manual_conversations();
    let sole_path = fixture.repo.create_named_worktree("task-sole");
    let task = support::register_task_with_pr(
        fixture.home.path(),
        &sole_path,
        "task-sole",
        &fixture.repo.head_sha(),
    );
    let several_path = fixture.repo.create_named_worktree("task-several");
    support::register_sibling_task(&task, "INF-124", "task-several", &several_path);
    let empty_path = fixture.repo.create_named_worktree("task-empty");
    support::register_sibling_task(&task, "INF-125", "task-empty", &empty_path);
    let converse = |checkout: &Path, opened_at: i64| -> String {
        let before = fixture.launches().len();
        let output = fixture
            .command(&LAUNCH)
            .current_dir(checkout)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let launches = fixture.launches();
        let [launch] = &launches[before..] else {
            panic!(
                "expected one launch for {}, got {:?}",
                checkout.display(),
                &launches[before..]
            );
        };
        let session = fixture.session_row(launch).0;
        fixture
            .db()
            .execute(
                "UPDATE agent_sessions SET opened_at=?2 WHERE id=?1",
                rusqlite::params![session, opened_at],
            )
            .unwrap();
        session
    };
    let primary = |args: &[&str]| -> Value {
        fixture.json(
            &[
                &["session", "resume", "--prepare", "--json", "--task"],
                args,
            ]
            .concat(),
        )
    };
    let members = |issue: &str| -> Vec<String> {
        fixture
            .json(&["session", "list", "--all", "--task", issue, "--json"])
            .as_array()
            .unwrap()
            .iter()
            .map(|session| session["id"].as_str().unwrap().to_string())
            .collect()
    };

    // No Session is different from an existing Session without native history.
    // Reading the Start offer must not admit either a Session or a turn.
    let absent = fixture.json(&["resume", "--task", "INF-125", "--explain", "--json"]);
    assert_eq!(absent["state"], Value::Null);
    assert_eq!(absent["action"]["intent"], "start");
    assert_eq!(absent["opening"]["label"], "Start conversation");
    assert!(members("INF-125").is_empty());

    // The Task's only unfinished interactive conversation is its primary.
    let sole = converse(&sole_path, 1_000);
    let chosen = primary(&["inf-123"]);
    assert_eq!(chosen["id"], sole.as_str());
    // Its row is marked for its Task, and it stays a member of that Task.
    assert_eq!(chosen["primary_scope"], "task");
    assert_eq!(members("Inf-123"), std::slice::from_ref(&sole));
    for args in [
        vec!["resume", "--task", "inf-123", "--prepare", "--json"],
        vec!["--task", "Inf-123", "resume", "--prepare", "--json"],
    ] {
        assert_eq!(fixture.json(&args)["id"], sole.as_str());
    }

    // Among several, the most recently used one; the others stay open.
    let earlier = converse(&several_path, 1_000);
    let later = converse(&several_path, 2_000);
    assert_eq!(primary(&["INF-124"])["id"], later.as_str());
    assert_eq!(members("INF-124").len(), 2);

    // Opening an exact conversation leaves the Task's primary unchanged.
    assert_eq!(
        fixture.json(&["resume", &earlier, "--prepare", "--json"])["id"],
        earlier.as_str()
    );
    let explained = fixture.json(&["resume", "--task", "inf-124", "--explain", "--json"]);
    assert_eq!(explained["resolution"]["session"]["value"], later.as_str());
    let newest = converse(&several_path, 3_000);
    assert_eq!(primary(&["INF-124"])["id"], later.as_str());
    assert_eq!(members("INF-124").len(), 3);
    // The listing marks it, so a reader finds it without choosing one.
    let marked: Vec<String> = fixture
        .json(&[
            "desktop", "sessions", "--all", "--task", "INF-124", "--json",
        ])
        .as_array()
        .unwrap()
        .iter()
        .filter(|session| session["primary_scope"] == "task")
        .map(|session| session["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(marked, std::slice::from_ref(&later));
    assert!(members("INF-124").contains(&newest));

    // A finished primary gives way to the Task's most recent conversation.
    fixture
        .db()
        .execute(
            "UPDATE agent_sessions SET completed_at=1 WHERE id=?1",
            [&later],
        )
        .unwrap();
    assert_eq!(primary(&["INF-124"])["id"], newest.as_str());

    // Reading inventory creates nothing; asking for the primary of a Task
    // with no conversation prepares one in its checkout.
    let before = fixture.count("agent_sessions");
    assert!(members("INF-125").is_empty());
    assert_eq!(fixture.count("agent_sessions"), before);
    // A retained Task whose checkout disappeared restores its recorded branch
    // before creating the conversation; it neither resets Git nor starts a Graph.
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&empty_path)
        .output()
        .unwrap()
        .stdout;
    std::fs::remove_dir_all(&empty_path).unwrap();
    let launches_before = fixture.launches().len();
    let created = primary(&["INF-125"]);
    assert_eq!(fixture.launches().len(), launches_before);
    assert!(created["open_argv"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg == "resume"));
    assert!(empty_path.join(".git").exists());
    assert_eq!(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&empty_path)
            .output()
            .unwrap()
            .stdout,
        head
    );
    assert_eq!(created["title"], "INF-125 Sibling");
    assert_eq!(fixture.count("agent_sessions"), before + 1);
    let (skill, cwd): (String, String) = fixture
        .db()
        .query_row(
            "SELECT skill,cwd FROM agent_sessions WHERE id=?1",
            [created["id"].as_str().unwrap()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(skill, "task-session");
    assert_eq!(
        Path::new(&cwd).canonicalize().unwrap(),
        empty_path.canonicalize().unwrap()
    );
    // Reopening a retained primary also restores its missing checkout.
    std::fs::remove_dir_all(&empty_path).unwrap();
    assert_eq!(primary(&["INF-125"])["id"], created["id"]);
    assert!(empty_path.join(".git").exists());
    assert_eq!(
        members("INF-125"),
        [created["id"].as_str().unwrap().to_string()]
    );

    let successor = created;
    let nested = empty_path.join("nested");
    std::fs::create_dir(&nested).unwrap();
    {
        let verb = "resume";
        let opened = fixture
            .command(&[verb, "--prepare", "--json"])
            .current_dir(&nested)
            .output()
            .unwrap();
        assert!(opened.status.success(), "{opened:?}");
        assert_eq!(
            serde_json::from_slice::<Value>(&opened.stdout).unwrap()["id"],
            successor["id"]
        );
        let repository = fixture
            .command(&[
                "--repo",
                fixture.repo.path().to_str().unwrap(),
                verb,
                "--prepare",
                "--json",
            ])
            .current_dir(&nested)
            .output()
            .unwrap();
        assert!(repository.status.success(), "{repository:?}");
        let repository: Value = serde_json::from_slice(&repository.stdout).unwrap();
        assert_eq!(repository["primary_scope"], "repository");
        assert_ne!(repository["id"], successor["id"]);
    }
}

#[test]
fn provider_parentage_does_not_assign_work_outside_its_checkout() {
    let fixture = Fixture::new(false);
    let task_path = fixture.repo.create_named_worktree("task-binding");
    let task = support::register_task_with_pr(
        fixture.home.path(),
        &task_path,
        "task-binding",
        &fixture.repo.head_sha(),
    );
    let sibling_path = fixture.repo.create_named_worktree("sibling-task");
    let sibling = support::register_sibling_task(&task, "INF-124", "sibling-task", &sibling_path);
    assert!(fixture.run(&LAUNCH).status.success());
    let original = fixture.launches()[0].clone();
    let (session, ..) = fixture.session_row(&original);
    let store =
        loopflow_execution::store::open_sqlite(&fixture.home.path().join("loopflow.db")).unwrap();
    let origin: String = fixture
        .db()
        .query_row(
            "SELECT lf_process_id FROM session_events WHERE kind='captured' ORDER BY rowid LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let origin = loopflow_model::id::LfProcessId::parse(&origin).unwrap();
    let attachment = store
        .session_attachment(&session)
        .unwrap()
        .unwrap_or_else(|| {
            attachment::claim_session_attachment(&store, &session, None, &origin).unwrap()
        });
    let caller = serde_json::to_string(&attachment.caller(session.clone().into(), None)).unwrap();
    let closed = store.session(&session).unwrap().unwrap();
    loopflow_execution::store::sessions::bind_session(
        &store,
        &session,
        closed.captured,
        &task.task.id,
    )
    .unwrap();
    // Synthetic provider provenance crosses real public CLI admission. The
    // same causal parent may issue taskless, checkout-bound or explicitly selected work.
    for (cwd, args, expected, source, declaration) in [
        (fixture.repo.path(), LAUNCH.to_vec(), None, None, false),
        (
            fixture.repo.path(),
            LAUNCH.to_vec(),
            Some(task.task.id.to_string()),
            Some("declared"),
            true,
        ),
        (
            task_path.as_path(),
            LAUNCH.to_vec(),
            Some(task.task.id.to_string()),
            Some("checkout"),
            true,
        ),
        (
            sibling_path.as_path(),
            LAUNCH.to_vec(),
            Some(sibling.id.to_string()),
            Some("checkout"),
            true,
        ),
        (
            fixture.repo.path(),
            BOUND_LAUNCH.to_vec(),
            Some(task.task.id.to_string()),
            Some("declared"),
            true,
        ),
    ] {
        let mut command = fixture.command(&args);
        if declaration {
            command.env("LF_AS", format!("task:{}", task.task.id));
        }
        let output = command
            .current_dir(cwd)
            .env("LF_AGENT_CALLER", &caller)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let capture = fixture.launches().last().unwrap().clone();
        let work = fixture.run_parents(&capture);
        assert_eq!(work.0, expected);
        assert_eq!(work.2.as_deref(), source);
    }
    // A stale provider keeps its original causal parent and grants no Work.
    attachment::claim_session_attachment(&store, &session, Some(&attachment), &origin).unwrap();
    let before = fixture.launches().len();
    let stale = fixture
        .command(&LAUNCH)
        .env("LF_AGENT_CALLER", &caller)
        .output()
        .unwrap();
    assert!(stale.status.success(), "{stale:?}");
    assert_eq!(fixture.launches().len(), before + 1);
    assert_eq!(
        fixture.run_parents(fixture.launches().last().unwrap()),
        (None, None, None)
    );
}

#[test]
fn declared_agent_tools_use_their_checkout() {
    let fixture = Fixture::new(false);
    let x = fixture.repo.create_named_worktree("task-x");
    let task =
        support::register_task_with_pr(fixture.home.path(), &x, "task-x", &fixture.repo.head_sha());
    let y = fixture.repo.create_named_worktree("task-y");
    let sibling = support::register_sibling_task(&task, "INF-124", "task-y", &y);
    std::fs::write(fixture.home.path().join("tool-command.json"), serde_json::to_vec(&serde_json::json!({
        "argv": [env!("CARGO_BIN_EXE_lf"), "-i", "--agent", "opencode", ":", "Work in Y"], "cwd": y,
    })).unwrap()).unwrap();
    let output = fixture.run(&[
        "--task",
        "INF-123",
        "-i",
        "--agent",
        "opencode",
        ":",
        "Call a tool in Y",
    ]);
    assert!(output.status.success(), "{output:?}");
    let result: Value = serde_json::from_slice(
        &std::fs::read(fixture.home.path().join("tool-result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(result["code"], 0, "{result}");
    let captures = fixture.launches();
    assert_eq!(captures.len(), 2);
    assert_eq!(
        fixture.run_parents(&captures[0]).0,
        Some(task.task.id.to_string())
    );
    assert_eq!(
        fixture.run_parents(&captures[1]).0,
        Some(sibling.id.to_string())
    );
    for line in std::fs::read_to_string(fixture.home.path().join("declarations"))
        .unwrap()
        .lines()
    {
        let env: Value = serde_json::from_str(line).unwrap();
        assert_eq!(env["as"], format!("task:{}", task.task.id));
    }
}

#[test]
#[ignore = "requires disposable Linux account with no installed Loopflow"]
fn declared_agent_can_start_another_tasks_graph() {
    assert!(Path::new("/.dockerenv").is_file());
    assert!(!loopflow_execution::installation::root().unwrap().exists());
    let fixture = Fixture::new(false);
    support::bind_task_planning(&fixture.repo);
    let y = fixture.repo.create_named_worktree("task-y");
    let target =
        support::register_task_with_pr(fixture.home.path(), &y, "task-y", &fixture.repo.head_sha());
    let x = fixture.repo.create_named_worktree("task-x");
    let caller = support::register_sibling_task(&target, "INF-124", "task-x", &x);
    std::fs::create_dir_all(y.join(".lf/graphs")).unwrap();
    std::fs::write(
        y.join(".lf/graphs/switch-proof.yaml"),
        "- cmd: sync --plan\n- break: demo\n",
    )
    .unwrap();
    // The Wave default reads the repository's committed graphs.
    std::fs::create_dir_all(fixture.repo.path().join(".lf/graphs")).unwrap();
    std::fs::write(
        fixture.repo.path().join(".lf/graphs/switch-proof.yaml"),
        "- cmd: sync --plan\n- break: demo\n",
    )
    .unwrap();
    let store =
        loopflow_execution::store::open_sqlite(&fixture.home.path().join("loopflow.db")).unwrap();
    // X's explicit Session selection must not become Y's Graph policy.
    support::select_wave_graph(
        fixture.repo.path(),
        &store,
        target.task.wave_id.as_ref().unwrap(),
        "switch-proof",
    );
    let bin = fixture.home.path().join("bin");
    std::fs::remove_file(bin.join("lf")).unwrap();
    // Record only the Graph command; background conversation hooks also use lf.
    std::fs::write(
        bin.join("lf"),
        format!(
            "#!/bin/sh\nif [ \"$#\" -eq 2 ] && [ \"$1\" = sync ] && [ \"$2\" = --plan ]; then\n  printf '%s' \"$LF_AS\" >'{}'\nfi\nexec '{}' \"$@\"\n",
            fixture.home.path().join("step-declaration").display(),
            env!("CARGO_BIN_EXE_lf")
        ),
    )
    .unwrap();
    std::fs::set_permissions(bin.join("lf"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(
        fixture.home.path().join("tool-command.json"),
        serde_json::to_vec(&serde_json::json!({
            "argv": ["env", format!("LF_BIN={}", bin.join("lf").display()), env!("CARGO_BIN_EXE_lf"), "task", "run", "INF-123", "switch-proof"], "cwd": x,
        }))
        .unwrap(),
    )
    .unwrap();
    let output = fixture
        .command(&["--task", "INF-124", "-a", "opencode", "-i", ":", "Start Y"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let result: Value = serde_json::from_slice(
        &std::fs::read(fixture.home.path().join("tool-result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(result["code"], 0, "{result}");
    // A bare run returns after registration; wait for Y to stop at demo.
    // Its background conversation opens separately from the Graph's demo step.
    let deadline = Instant::now() + PATIENCE;
    let graphs = loop {
        let graphs = support::recorded_graphs(fixture.home.path());
        if graphs.first().is_some_and(|graph| graph.0.is_some()) {
            break graphs;
        }
        assert!(
            Instant::now() < deadline,
            "Y's Graph never stopped: {graphs:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(graphs.len(), 1);
    assert_eq!(graphs[0].0.as_deref(), Some("succeeded"));
    let status = fixture.json(&["desktop", "task", "INF-123", "--json"]);
    assert_eq!(
        status["execution"]["work"]["graph"]["position"]["step"],
        "demo"
    );
    let deadline = Instant::now() + PATIENCE;
    let captures = loop {
        let captures = fixture.launches();
        if captures.len() >= 2 {
            break captures;
        }
        assert!(
            Instant::now() < deadline,
            "Y's background conversation never opened: {captures:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(captures.len(), 2);
    let mut launched_tasks: Vec<_> = captures
        .iter()
        .map(|capture| fixture.run_parents(capture).0.unwrap())
        .collect();
    launched_tasks.sort();
    let mut expected = vec![caller.id.to_string(), target.task.id.to_string()];
    expected.sort();
    assert_eq!(launched_tasks, expected);
    assert_eq!(
        std::fs::read_to_string(fixture.home.path().join("step-declaration")).unwrap(),
        format!("task:{}", target.task.id)
    );
    // The Graph ran in Y's checkout, started by X's conversation.
    let observed: (String, String) = fixture
        .db()
        .query_row(
            "SELECT g.cwd,s.task_id FROM graph_processes g
             JOIN agent_sessions s ON s.id=g.caller_session_id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(observed, (y.display().to_string(), caller.id.to_string()));
}

const BOUND_LAUNCH: [&str; 7] = [
    "--task",
    "INF-123",
    "-i",
    "--agent",
    "opencode",
    ":",
    "Review the parser",
];

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
        command.env("LF_HOME", &self.directory);
    }

    fn unlock(&self) -> rusqlite::Connection {
        std::fs::set_permissions(&self.directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        rusqlite::Connection::open(self.directory.join("loopflow.db")).unwrap()
    }
}

#[test]
fn failed_process_observation_cannot_admit_a_provider() {
    let fixture = Fixture::new(false);
    let initialized = fixture.run(&["session", "list", "--all", "--json"]);
    assert!(initialized.status.success(), "{initialized:?}");
    // Fail just the process observation. The conversation store remains writable.
    fixture
        .db()
        .execute_batch(
            "CREATE TRIGGER refuse_fixture_process BEFORE INSERT ON processes
         BEGIN SELECT RAISE(ABORT, 'fixture refuses Process observation'); END;",
        )
        .unwrap();
    for args in [
        LAUNCH.as_slice(),
        &["--batch", "--agent", "opencode", ":", "Tidy the parser"],
    ] {
        let output = fixture.run(args);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(
            fixture.launches().is_empty(),
            "a writable Session store cannot replace Process admission"
        );
    }
    // The earlier inspection finished unreferenced and was forgotten at exit.
    assert_eq!(
        fixture.count("processes"),
        0,
        "no refused observation reaches the store"
    );
}

#[test]
fn malformed_caller_cannot_use_library_agent_admission() {
    let fixture = Fixture::new(false);
    for args in [
        LAUNCH.as_slice(),
        &["--batch", "--agent", "opencode", ":", "Tidy the parser"],
    ] {
        let output = fixture
            .command(args)
            .env("LF_AGENT_CALLER", "not-json")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("requires an admitted invocation"),
            "{output:?}"
        );
        assert!(
            fixture.launches().is_empty(),
            "no provider starts without process admission"
        );
    }
    // A diagnostic command is still usable. Invalid provenance never becomes
    // a fabricated root/direct Process merely to make logging succeed.
    let output = fixture
        .command(&["session", "list", "--all", "--json"])
        .env("LF_AGENT_CALLER", "not-json")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(fixture.count("processes"), 0);
}

#[test]
fn unavailable_store_starts_no_provider() {
    let fixture = Fixture::new(false);
    saved_graph_stand_in(&fixture);
    let store = LockedStore::new(&fixture);
    for args in [
        LAUNCH.as_slice(),
        &["--batch", "--agent", "opencode", ":", "Tidy the parser"],
        &[
            "--agent",
            "opencode",
            "graph",
            "work-then-decide",
            "--batch",
        ],
    ] {
        let mut launch = fixture.command(args);
        store.select(&mut launch);
        let output = launch.output().unwrap();
        assert!(!output.status.success(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("unable to open database file"),
            "{output:?}"
        );
        assert!(
            fixture.launches().is_empty(),
            "no provider starts without its conversation"
        );
    }
    store.unlock();
    let mut inventory = fixture.command(&["session", "list", "--all", "--json"]);
    store.select(&mut inventory);
    let output = inventory.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        serde_json::json!([])
    );
}

#[test]
fn finished_headless_runs_leave_the_open_list_and_keep_their_history() {
    let fixture = Fixture::new(false);
    support::seed_store(fixture.home.path());
    let provider = fixture.home.path().join("bin/codex");
    std::fs::write(
        &provider,
        support::codex_app_server_script("Parser tidied.", ""),
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    support::register_task_with_pr(
        fixture.home.path(),
        fixture.repo.path(),
        "history",
        &fixture.repo.head_sha(),
    );
    let output = fixture.run(&[
        "--wave",
        "task-pr-tests",
        "--batch",
        "--agent",
        "codex",
        ":",
        "Tidy the parser",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert!(fixture.sessions().is_empty());
    assert_eq!(
        fixture.json(&["session", "list", "--headless", "--json"]),
        serde_json::json!([])
    );
    let sessions = fixture.recorded_sessions();
    assert_eq!(sessions.len(), 1);
    let session = &sessions[0]["id"];
    {
        let rows = fixture.json(&["history", "usage", "--wave", "task-pr-tests", "--json"]);
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert_eq!(&rows[0]["session_id"], session);
        assert_eq!(rows[0]["recorded_outcome"], "completed");
    }
    let activity = fixture.json(&["history", "--wave", "task-pr-tests", "--json"]);
    let captures = activity["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["fact"]["kind"] == "input_captured")
        .collect::<Vec<_>>();
    assert_eq!(captures.len(), 1);
    assert_eq!(&captures[0]["fact"]["session_id"], session);
}

/// A native provider fixture that returns a schema-constrained final result.
fn saved_graph_stand_in(fixture: &Fixture) {
    std::fs::write(fixture.home.path().join("decide-enabled"), "").unwrap();
    let lf = fixture.repo.path().join(".lf");
    std::fs::create_dir_all(lf.join("skills")).unwrap();
    std::fs::create_dir_all(lf.join("graphs")).unwrap();
    std::fs::write(lf.join("skills/work-proof.md"), "Do the fixture work.").unwrap();
    std::fs::write(lf.join("skills/review-proof.md"), "Review the fixture.").unwrap();
    std::fs::write(lf.join("skills/decide-proof.md"), "Decide the fixture.").unwrap();
    std::fs::write(
        lf.join("graphs/work-then-decide.yaml"),
        "- loop:\n    - work-proof\n    - decide-proof\n",
    )
    .unwrap();
    // Committed, as a repository's definitions are: a step leaves only its own edits.
    fixture.repo.stage_all();
    fixture.repo.commit("Define the fixture skills and Graph");
}

/// (input, node, ordinal at that node, outcome, Task) from conversation history.

#[test]
fn taskless_structured_output_correction_is_bounded_and_preserves_the_conversation() {
    for exhausted in [false, true] {
        let fixture = Fixture::new(false);
        saved_graph_stand_in(&fixture);
        std::fs::write(
            fixture.home.path().join(if exhausted {
                "invalid-output-always"
            } else {
                "invalid-output-once"
            }),
            "",
        )
        .unwrap();
        let output = fixture.run(&[
            "--agent",
            "opencode",
            "graph",
            "work-then-decide",
            "--batch",
        ]);
        assert_eq!(
            output.status.success(),
            !exhausted,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fixture.count("agent_sessions"), 2);
        assert_eq!(
            fixture.launches().len(),
            if exhausted { 4 } else { 3 },
            "exhausted={exhausted}; launches={:?}; results={}; stderr={}",
            fixture.launches(),
            std::fs::read_to_string(fixture.home.path().join("decide.log")).unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr)
        );
        let starts: i64 = fixture
            .db()
            .query_row(
                "SELECT count(*) FROM history.turns WHERE started_at IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(starts, if exhausted { 4 } else { 3 });
        // One Graph: its work step, then the decision and each correction of it.
        let graphs = support::recorded_graphs(fixture.home.path());
        assert_eq!(graphs.len(), 1);
        let (outcome, steps) = &graphs[0];
        assert_eq!(
            outcome.as_deref(),
            Some(if exhausted { "failed" } else { "succeeded" })
        );
        assert_eq!(steps.len(), if exhausted { 4 } else { 3 });
        // Every correction repeats the decision's node in its one conversation.
        let decision = &steps[1];
        for correction in &steps[2..] {
            assert_eq!(correction["key"], decision["key"]);
            assert_eq!(correction["iterations"], decision["iterations"]);
        }
        if exhausted {
            assert!(String::from_utf8_lossy(&output.stderr).contains("exhausted after 3"));
        }
    }
}

#[test]
fn failed_taskless_decision_stops_and_keeps_its_history() {
    let fixture = Fixture::new(false);
    saved_graph_stand_in(&fixture);
    std::fs::write(fixture.home.path().join("blocked-once"), "").unwrap();
    let mut command = fixture.command(&[
        "--agent",
        "opencode",
        "graph",
        "work-then-decide",
        "--batch",
    ]);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + PATIENCE;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("blocked decision did not stop before the deadline");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(!status.success());
    // The Graph stopped at its decision; its Graph process says why.
    let graphs = support::recorded_graphs(fixture.home.path());
    assert_eq!(graphs.len(), 1);
    let (outcome, steps) = &graphs[0];
    assert_eq!(outcome.as_deref(), Some("failed"));
    assert_eq!(steps.len(), 2);
    let stopped = &steps[1];
    assert_eq!(stopped["label"], "decide-proof");
    assert_eq!(stopped["key"], 1);
    assert_eq!(stopped["iterations"], serde_json::json!([[0]]));
    let error: String = fixture
        .db()
        .query_row("SELECT error FROM graph_processes", [], |row| row.get(0))
        .unwrap();
    assert!(error.contains("Release target is missing"), "{error}");
    assert!(fixture.sessions().is_empty());
    assert_eq!(fixture.launches().len(), 2);
    let session: String = fixture
        .db()
        .query_row(
            "SELECT id FROM agent_sessions WHERE skill='decide-proof'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    // The decision that stopped it is the answer its conversation kept.
    let answer = fixture.run(&["history", "show", &session, "--final"]);
    assert!(answer.status.success(), "{answer:?}");
    assert!(
        String::from_utf8_lossy(&answer.stdout).contains("Release target is missing"),
        "{answer:?}"
    );
}

#[test]
fn custom_router_returns_a_captured_path_without_an_in_turn_command() {
    let fixture = Fixture::new(false);
    saved_graph_stand_in(&fixture);
    std::fs::write(fixture.repo.path().join(".lf/graphs/choose.yaml"),
        "- xor:\n    router: decide-proof\n    paths:\n      alpha:\n        description: Do the selected work\n        skill: work-proof\n      zeta:\n        description: Leave this path untouched\n        skill: review-proof\n").unwrap();
    fixture.repo.stage_all();
    fixture.repo.commit("Define the routed Graph");
    let output = fixture.run(&["--agent", "opencode", "graph", "choose", "--batch"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let log = std::fs::read_to_string(fixture.home.path().join("decide.log")).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(log.trim()).unwrap(),
        serde_json::json!({"path":"alpha"})
    );
    assert_eq!(
        fixture.launches().len(),
        2,
        "router and selected path once each"
    );
    let graphs = support::recorded_graphs(fixture.home.path());
    assert_eq!(graphs.len(), 1);
    let (outcome, steps) = &graphs[0];
    assert_eq!(outcome.as_deref(), Some("succeeded"));
    assert_eq!(steps.len(), 2);
    // The router is the XOR's own node; the selected path's step lies inside it.
    assert_eq!(steps[0]["label"], "decide-proof");
    assert_eq!(steps[0]["key"], 0);
    assert_eq!(steps[1]["label"], "work-proof");
    assert_eq!(steps[1]["key"], 1);
}

#[test]
fn standalone_graph_launches_in_a_completed_tasks_checkout() {
    let fixture = Fixture::new(false);
    support::install_native_codex(&fixture.home.path().join("bin"));
    support::seed_store(fixture.home.path());
    let checkout = fixture.repo.create_named_worktree("finished-work");
    std::fs::create_dir_all(checkout.join(".lf/graphs")).unwrap();
    std::fs::create_dir_all(checkout.join(".lf/skills")).unwrap();
    std::fs::write(
        checkout.join(".lf/graphs/incident.yaml"),
        "- recovery-proof\n",
    )
    .unwrap();
    std::fs::write(
        checkout.join(".lf/skills/recovery-proof.md"),
        "Inspect the failure.\n",
    )
    .unwrap();
    let created = fixture.json(&[
        "task",
        "create",
        "--title",
        "Finished work",
        "--worktree",
        checkout.to_str().unwrap(),
        "--json",
    ])["task"]
        .clone();
    let task = created["id"].as_str().unwrap();
    fixture
        .db()
        .execute(
            "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,'end',1)",
            rusqlite::params![
                task,
                serde_json::json!({"name": "code", "steps": []}).to_string()
            ],
        )
        .unwrap();
    let completed = fixture.run(&["task", "complete", task]);
    assert!(completed.status.success(), "{completed:?}");
    let before = fixture.json(&["task", "status", task, "--json"])["task"].clone();
    assert_eq!(before["state"], "done");
    assert_eq!(
        before["graph"],
        serde_json::json!({"name": "code", "step": "end"})
    );
    let output = fixture
        .command(&["-b", "--agent", "codex", "incident"])
        .current_dir(&checkout)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fixture.count("agent_sessions"), 1);
    let session_task: String = fixture
        .db()
        .query_row(
            "SELECT task_id FROM agent_sessions WHERE skill='recovery-proof'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(session_task, task);
    assert!(fixture
        .home
        .path()
        .join("native-server-config.json")
        .exists());
    assert_eq!(
        fixture.json(&["task", "status", task, "--json"])["task"],
        before
    );
    assert_eq!(
        support::recorded_graphs(fixture.home.path())[0]
            .0
            .as_deref(),
        Some("succeeded")
    );
    let restart = fixture.run(&["-b", "task", "run", task]);
    assert!(!restart.status.success(), "{restart:?}");
    assert!(
        String::from_utf8_lossy(&restart.stderr)
            .contains("reopens planning without moving its Graph"),
        "{restart:?}"
    );
    assert_eq!(fixture.count("agent_sessions"), 1);
    assert_eq!(
        fixture.json(&["task", "status", task, "--json"])["task"],
        before
    );
}

#[test]
fn public_taskless_graph_records_distinct_completed_loop_passes() {
    let fixture = Fixture::new(false);
    saved_graph_stand_in(&fixture);
    std::fs::write(fixture.home.path().join("remaining-passes"), "2").unwrap();
    let output = fixture.run(&[
        "--agent",
        "opencode",
        "graph",
        "work-then-decide",
        "--batch",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // One Graph process ran all three passes; each decision names its own pass.
    let graphs = support::recorded_graphs(fixture.home.path());
    assert_eq!(graphs.len(), 1);
    let (outcome, steps) = &graphs[0];
    assert_eq!(outcome.as_deref(), Some("succeeded"));
    let passes: Vec<(String, serde_json::Value)> = steps
        .iter()
        .map(|step| {
            (
                step["label"].as_str().unwrap().to_owned(),
                step["iterations"].clone(),
            )
        })
        .collect();
    let pass = |label: &str, returns: u32| (label.to_owned(), serde_json::json!([[returns]]));
    assert_eq!(
        passes,
        [
            pass("work-proof", 0),
            pass("decide-proof", 0),
            pass("work-proof", 1),
            pass("decide-proof", 1),
            pass("work-proof", 2),
            pass("decide-proof", 2),
        ]
    );
    assert_eq!(fixture.launches().len(), 6);
    let sessions: std::collections::BTreeMap<String, Vec<Value>> = serde_json::from_slice(
        &std::fs::read(fixture.home.path().join("native-sessions.json")).unwrap(),
    )
    .unwrap();
    let mut decisions: Vec<_> = sessions
        .values()
        .filter_map(|messages| {
            messages[0]["text"]
                .as_str()?
                .lines()
                .find(|line| line.starts_with("Decision pass "))
        })
        .collect();
    decisions.sort_unstable();
    assert_eq!(
        decisions,
        [
            "Decision pass 1. Iterating repeats 2 steps, back to work-proof.",
            "Decision pass 2. Iterating repeats 2 steps, back to work-proof.",
            "Decision pass 3. Iterating repeats 2 steps, back to work-proof.",
        ]
    );
}

#[test]
fn an_opencode_run_that_dies_after_a_tool_fails_its_turn_and_is_not_retried() {
    let fixture = Fixture::new(false);
    saved_graph_stand_in(&fixture);
    std::fs::write(fixture.home.path().join("disconnect-after-tool"), "").unwrap();
    let output = fixture.run(&[
        "--agent",
        "opencode",
        "graph",
        "work-then-decide",
        "--batch",
    ]);
    assert!(!output.status.success(), "{output:?}");
    assert!(fixture.home.path().join("tool-effect").exists());
    let starts: i64 = fixture
        .db()
        .query_row(
            "SELECT count(*) FROM history.turns WHERE started_at IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(starts, 1);
    // The run left without an answer: its turn failed, and never reads as
    // one that finished.
    let endings: Vec<String> = fixture
        .db()
        .prepare("SELECT status FROM history.turns WHERE ended_at IS NOT NULL")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(endings, ["failed"]);
    // The Graph stopped at its first step and launched nothing after it.
    let graphs = support::recorded_graphs(fixture.home.path());
    assert_eq!(graphs.len(), 1);
    assert_eq!(graphs[0].1.len(), 1);
    assert_eq!(
        fixture.launches().len(),
        1,
        "uncertain effect is not silently retried"
    );
    assert_eq!(graphs[0].0.as_deref(), Some("failed"));
}

#[test]
fn opencode_transient_failure_does_not_invent_a_continuation_or_accept_failed_output() {
    let fixture = Fixture::new(false);
    saved_graph_stand_in(&fixture);
    std::fs::write(fixture.home.path().join("transient-once"), "").unwrap();
    let output = fixture.run(&[
        "--agent",
        "opencode",
        "graph",
        "work-then-decide",
        "--batch",
    ]);
    assert!(!output.status.success());
    let graphs = support::recorded_graphs(fixture.home.path());
    assert_eq!(graphs[0].0.as_deref(), Some("failed"));
    assert_eq!(
        fixture.launches().len(),
        2,
        "each step admits exactly one input"
    );
    assert_eq!(fixture.captures(), 2);
    assert!(
        !fixture.home.path().join("decide.log").exists(),
        "failed answer cannot advance the Graph"
    );
}

#[test]
fn repository_upgrade_and_move_resume_the_same_native_conversation() {
    let fixture = Fixture::new(false);
    support::install_native_codex(&fixture.home.path().join("bin"));
    let identity = fixture.json(&["config", "repository", "--json"]);
    let task =
        fixture.json(&["task", "create", "--title", "Retained checkout", "--json"])["task"].clone();
    let task_id = task["id"].as_str().unwrap();
    fixture.json(&["task", "checkout", task_id, "--json"]);
    let launch = fixture.run(&[
        "--task",
        task_id,
        "-i",
        "--agent",
        "codex",
        ":",
        "Keep this conversation",
    ]);
    assert!(launch.status.success(), "{launch:?}");
    let session: String = fixture
        .db()
        .query_row("SELECT id FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    let native = || {
        fixture.db().query_row(
        "SELECT provider_thread FROM session_events WHERE session_id=?1 AND kind='captured' AND provider_thread IS NOT NULL ORDER BY seq DESC LIMIT 1",
        [&session], |row| row.get::<_, String>(0)).unwrap()
    };
    let before = native();
    let machine: String = fixture
        .db()
        .query_row("SELECT id FROM machines WHERE is_local=1", [], |row| {
            row.get(0)
        })
        .unwrap();
    let locator = fixture
        .repo
        .path()
        .join(".git/loopflow")
        .join(machine)
        .join("loopflow-repository");
    std::fs::remove_file(&locator).unwrap();
    let resumed = fixture.run(&["session", "resume", &session]);
    assert!(resumed.status.success(), "{resumed:?}");
    assert_eq!(fixture.count("agent_sessions"), 1);
    assert_eq!(native(), before);
    assert_eq!(
        std::fs::read_to_string(locator).unwrap().trim(),
        identity["id"].as_str().unwrap()
    );
    fixture
        .repo
        .create_file("unfinished.txt", "Retain unpublished edits");
    let root = tempfile::tempdir().unwrap();
    let moved = root.path().join("renamed");
    std::fs::rename(fixture.repo.path(), &moved).unwrap();
    let resumed = fixture
        .command(&["session", "resume", &session])
        .current_dir(&moved)
        .output()
        .unwrap();
    assert!(resumed.status.success(), "{resumed:?}");
    assert_eq!(fixture.count("agent_sessions"), 1);
    assert_eq!(native(), before);
    let output = fixture
        .command(&["config", "repository", "--json"])
        .current_dir(&moved)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        identity
    );
    assert_eq!(
        std::fs::read_to_string(moved.join("unfinished.txt")).unwrap(),
        "Retain unpublished edits"
    );
}

#[test]
fn pi_managed_graph_routes_one_branch_and_nested_lf_retains_task_and_checkout() {
    let fixture = Fixture::new(false);
    let task = support::register_task_without_pr(
        fixture.home.path(),
        &fixture.repo.path().canonicalize().unwrap(),
        "main",
        &fixture.repo.head_sha(),
    );
    let provider = fixture.home.path().join("bin/pi");
    std::fs::write(
        &provider,
        format!(
            "#!/usr/bin/python3 -I\n{}",
            include_str!("../../loopflow-execution/src/harness/testdata/pi_rpc.py")
        ),
    )
    .unwrap();
    std::fs::set_permissions(provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    fixture.repo.create_file(
        ".lf/config.yaml",
        "agent: pi:fixture/model\ndiff: false\ndiff_files: false\npaste: false\n",
    );
    fixture
        .repo
        .create_file(".lf/skills/route-proof.md", "PI_ROUTE_SENTINEL");
    fixture
        .repo
        .create_file(".lf/skills/work-proof.md", "SELECTED_START 雪 SELECTED_END");
    fixture
        .repo
        .create_file(".lf/skills/review-proof.md", "UNSELECTED_PATH");
    fixture.repo.create_file(".lf/graphs/choose.yaml", "- xor:\n    router: route-proof\n    paths:\n      alpha:\n        description: Select work\n        skill: work-proof\n      zeta:\n        description: Skip work\n        skill: review-proof\n");
    let received = fixture.home.path().join("pi-received.json");
    let output = fixture
        .command(&[
            "--task",
            task.task.id.as_str(),
            "--agent",
            "pi:fixture/model",
            "graph",
            "choose",
            "--batch",
        ])
        .env("PI_FIXTURE_RECORD", &received)
        .env("PI_FIXTURE_NESTED", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let received: Value = serde_json::from_slice(&std::fs::read(received).unwrap()).unwrap();
    assert!(received["input"]
        .as_str()
        .unwrap()
        .contains("SELECTED_START 雪 SELECTED_END"));
    assert_eq!(
        received["cwd"],
        fixture
            .repo
            .path()
            .canonicalize()
            .unwrap()
            .display()
            .to_string()
    );
    assert!(received["nested_context"]
        .as_str()
        .unwrap()
        .contains(task.task.id.as_str()));
    let graphs = support::recorded_graphs(fixture.home.path());
    assert_eq!(graphs.len(), 1);
    assert_eq!(graphs[0].0.as_deref(), Some("succeeded"));
    let labels: Vec<_> = graphs[0]
        .1
        .iter()
        .map(|step| step["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["route-proof", "work-proof"]);
    let wrong_owners: i64 = fixture
        .db()
        .query_row(
            "SELECT count(*) FROM agent_sessions WHERE task_id IS NULL OR task_id!=?1",
            [task.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(wrong_owners, 0);
}

#[test]
fn promotion_explicit_sources_with_run_keep_the_selected_work() {
    for select_session in [false, true] {
        let fixture = Fixture::new(true);
        fixture
            .repo
            .create_file(".lf/graphs/pause.yaml", "- break: demo\n");
        fixture.repo.stage_all();
        fixture
            .repo
            .commit("Add a graph that stops before launching work");
        fixture.repo.push();
        let root = fixture
            .repo
            .create_named_worktree("prototype")
            .canonicalize()
            .unwrap();
        let (child, capture) = fixture.attach(&[
            "--repo",
            root.to_str().unwrap(),
            "-i",
            "--agent",
            "opencode",
            ":",
            "Research",
        ]);
        fixture.with_attached_clients([child], || {
            let id = fixture.session_row(&capture).0;
            let store =
                loopflow_execution::store::open_sqlite(&fixture.home.path().join("loopflow.db"))
                    .unwrap();
            let attachment = store.session_attachment(&id).unwrap().unwrap();
            let caller =
                serde_json::to_string(&attachment.caller(id.clone().into(), None)).unwrap();
            let source = if select_session {
                ["--session", id.as_str()]
            } else {
                ["--worktree", root.to_str().unwrap()]
            };
            let args = [
                vec![
                    "task",
                    "create",
                    "--title",
                    "Keep research",
                    "--run",
                    "--graph",
                    "pause",
                    "--json",
                ],
                source.to_vec(),
            ]
            .concat();
            let output = fixture
                .command(&args)
                .current_dir(&root)
                .env("LF_AGENT_CALLER", caller)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            let receipt: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(receipt["task"]["worktree"], root.to_str().unwrap());
            assert_eq!(receipt["task"]["branch"], "prototype");
            // --run opens the Task's conversation through ordinary admission;
            // the adopted checkout now associates this conversation with it.
            assert_eq!(receipt["session_id"], id);
            assert_eq!(store.session_attachment(&id).unwrap().unwrap(), attachment);
            assert_eq!(store.session(&id).unwrap().unwrap().cwd, root);
            let summary = fixture
                .sessions()
                .into_iter()
                .find(|s| s["id"] == id)
                .unwrap();
            assert_eq!(summary["cwd"], root.to_str().unwrap());
            assert_eq!(fixture.launches().len(), 1);
            fixture.wait_for_graphs();
        });
    }
}

#[test]
fn promotion_sets_directory_immediately_and_resumes_same_native_conversation() {
    let fixture = Fixture::new(true);
    let origin = fixture
        .repo
        .create_named_worktree("research")
        .canonicalize()
        .unwrap();
    let destination = fixture
        .repo
        .create_named_worktree("delivery")
        .canonicalize()
        .unwrap();
    std::fs::write(origin.join("draft.txt"), "keep unfinished work").unwrap();
    let task = fixture.json(&[
        "task",
        "create",
        "--worktree",
        destination.to_str().unwrap(),
        "--title",
        "Delivery",
        "--json",
    ])["task"]
        .clone();
    let task_id = task["id"].as_str().unwrap();
    let (child, capture) = fixture.attach(&[
        "--repo",
        origin.to_str().unwrap(),
        "-i",
        "--agent",
        "opencode",
        ":",
        "Research",
    ]);
    let id = fixture.with_attached_clients([child], || {
        let id = fixture.session_row(&capture).0;
        let before = fixture
            .sessions()
            .into_iter()
            .find(|s| s["id"] == id)
            .unwrap();
        let captures = fixture.captures();
        let attachment = fixture.input_attachment(&id);
        let bound = fixture.json(&["session", "bind", &id, "--task", task_id, "--json"]);
        assert_eq!(bound["id"], id);
        assert_eq!(fixture.input_attachment(&id), attachment);
        let after = fixture
            .sessions()
            .into_iter()
            .find(|s| s["id"] == id)
            .unwrap();
        assert_eq!(after["cwd"], destination.to_str().unwrap());
        assert_eq!(after["agent_process_id"], before["agent_process_id"]);
        // Binding selects the directory without rewriting the running child's launch.
        let launch_cwd: String = fixture
            .db()
            .query_row(
                "SELECT cwd FROM processes WHERE id=?1",
                [before["agent_process_id"].as_str().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(launch_cwd, origin.to_str().unwrap());
        let opened = fixture.json(&["resume", "--task", task_id, "--prepare", "--json"]);
        assert_eq!(opened["id"], id);
        assert_eq!(opened["cwd"], after["cwd"]);
        let status = fixture.json(&["task", "status", task_id, "--json"]);
        assert_eq!(status["sessions"][0]["cwd"], after["cwd"]);
        assert_eq!(fixture.captures(), captures);
        assert_eq!(fixture.launches().len(), 1);
        assert_eq!(fixture.count("task_graphs"), 0);
        id
    });
    let native_before: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.home.path().join("native-sessions.json")).unwrap(),
    )
    .unwrap();
    fixture.finish_on_resume();
    let continued = fixture.run(&["resume", &id]);
    assert!(continued.status.success(), "{continued:?}");
    let received: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.home.path().join("received.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(received["cwd"], destination.to_str().unwrap());
    let native_after: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.home.path().join("native-sessions.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        native_after, native_before,
        "resume preserves native identity and messages without a turn"
    );
    let locations: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.home.path().join("native-locations.json")).unwrap(),
    )
    .unwrap();
    assert!(locations
        .as_object()
        .unwrap()
        .values()
        .all(|cwd| cwd == destination.to_str().unwrap()));
    let after = fixture
        .sessions()
        .into_iter()
        .find(|s| s["id"] == id)
        .unwrap();
    assert_eq!(after["cwd"], destination.to_str().unwrap());
    assert_eq!(
        std::fs::read_to_string(origin.join("draft.txt")).unwrap(),
        "keep unfinished work"
    );
    assert_eq!(fixture.count("tasks"), 1);
}

#[test]
fn promotion_selects_session_in_owned_and_shared_checkouts() {
    // One owned case: `task create --session` from inside the conversation.
    // Shared checkouts and binding are decision rows (`adoption::tests`).
    let fixture = Fixture::new(true);
    let root = fixture
        .repo
        .create_named_worktree("prototype")
        .canonicalize()
        .unwrap();
    let args = [
        "--repo",
        root.to_str().unwrap(),
        "-i",
        "--agent",
        "opencode",
        ":",
        "Research",
    ];
    let (first, first_capture) = fixture.attach(&args);
    let (second, second_capture) = fixture.attach(&args);
    let (task, session, cwd) = fixture.with_attached_clients([first, second], || {
        let ids = [
            fixture.session_row(&first_capture).0,
            fixture.session_row(&second_capture).0,
        ];
        let store =
            loopflow_execution::store::open_sqlite(&fixture.home.path().join("loopflow.db"))
                .unwrap();
        let attachment = store.session_attachment(&ids[1]).unwrap().unwrap();
        let caller =
            serde_json::to_string(&attachment.caller(ids[1].clone().into(), None)).unwrap();
        let caller_cwd = store.session(&ids[1]).unwrap().unwrap().cwd;
        let created = fixture.json(&[
            "task",
            "create",
            "--title",
            "Prototype",
            "--session",
            &ids[0],
            "--json",
        ])["task"]
            .clone();
        let task = created["id"].as_str().unwrap();
        let output = fixture
            .command(&[
                "task",
                "create",
                "--session",
                &ids[1],
                "--title",
                "Same work",
                "--json",
            ])
            .current_dir(&caller_cwd)
            .env("LF_AGENT_CALLER", caller)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["task"]["id"], task);
        let primary = fixture.json(&["resume", "--task", task, "--prepare", "--json"]);
        assert_eq!(primary["id"], ids[1]);
        assert_eq!(primary["interactive"], true);
        assert_eq!(primary["cwd"], caller_cwd.to_str().unwrap());
        assert_eq!(fixture.launches().len(), 2);
        (task.to_owned(), ids[1].clone(), caller_cwd)
    });
    fixture.finish_on_resume();
    let continued = fixture.run(&["resume", "--task", &task]);
    assert!(continued.status.success(), "{continued:?}");
    assert_eq!(
        fixture.session_row(fixture.launches().last().unwrap()).0,
        session
    );
    let resumed: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.home.path().join("received.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(resumed["cwd"], cwd.to_str().unwrap());
}

#[test]
fn promotion_background_run_does_not_claim_primary() {
    let fixture = Fixture::new(false);
    let root = fixture
        .repo
        .create_named_worktree("prototype")
        .canonicalize()
        .unwrap();
    let created = fixture.json(&[
        "task",
        "create",
        "--worktree",
        root.to_str().unwrap(),
        "--title",
        "Prototype",
        "--json",
    ])["task"]
        .clone();
    let task = created["id"].as_str().unwrap();
    fixture.db().execute(
        "INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,repo,interactive,input_published) VALUES('background','Research','human',1,?1,?2,0,0)",
        rusqlite::params![root.to_str().unwrap(), fixture.repo.path().to_str().unwrap()],
    ).unwrap();
    let caller = serde_json::json!({"session_id":"background", "origin_exec_id": uuid::Uuid::new_v4().to_string()});
    let output = fixture
        .command(&["task", "create", "--title", "Same work", "--json"])
        .current_dir(&root)
        .env("LF_AGENT_CALLER", caller.to_string())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let returned = serde_json::from_slice::<Value>(&output.stdout).unwrap();
    assert_ne!(returned["task"]["id"], task);
    assert!(returned["task"]["worktree"].is_null());
    let primary: Option<String> = fixture
        .db()
        .query_row(
            "SELECT session_id FROM task_primary_sessions WHERE task_id=?1",
            [task],
            |row| row.get(0),
        )
        .optional()
        .unwrap();
    assert_eq!(primary, None);
    let bound: Option<String> = fixture
        .db()
        .query_row(
            "SELECT task_id FROM agent_sessions WHERE id='background'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(bound, None);
    assert_eq!(fixture.count("tasks"), 2);
    assert!(fixture.launches().is_empty());
}
