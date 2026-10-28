//! Conversations own admission, immutable inputs, and history through completion.
//! The real `lf` binary runs in a private Machine; a script stands in for the provider.
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
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_lf"), bin.join("lf")).unwrap();
        let launched = home.path().join("launched");
        let provider = bin.join("opencode");
        std::fs::write(
            &provider,
            include_str!("support/opencode_server.py")
                .replace("__WAIT__", if waits { "True" } else { "False" }),
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
            // An inherited cmux workspace would receive the fixture Session's name.
            if ["LF_", "LOOPFLOW_", "LINEAR_", "CMUX_"]
                .iter()
                .any(|prefix| key.starts_with(prefix))
            {
                command.env_remove(key);
            }
        }
        let home = self.home.path();
        command
            .args(args)
            .current_dir(self.repo.path())
            .env("HOME", home)
            .env("LF_HOME", home)
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
        serde_json::from_value(self.json(&["desktop", "sessions", "--all", "--json"])).unwrap()
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

    fn captures(&self) -> i64 {
        self.db()
            .query_row(
                "SELECT count(*) FROM session_events WHERE kind='captured'",
                [],
                |row| row.get(0),
            )
            .unwrap()
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
                 LEFT JOIN session_events m ON m.session_id=s.id AND m.kind='observed'
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
fn task_conversation_starts_at_launch_not_creation() {
    for headless in [false, true] {
        let fixture = Fixture::new(false);
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
            support::register_codex_account(fixture.home.path());
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
        let task: Value = serde_json::from_slice(&output.stdout).unwrap();
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
                "SELECT primary_session_id,worktree FROM tasks WHERE id=?1",
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
                fixture.json(&["session", "connect", "--prepare", "--task", id, "--json"])
            });
            let second = scope.spawn(|| {
                fixture.json(&["session", "connect", "--prepare", "--task", id, "--json"])
            });
            let first = first.join().unwrap();
            assert_eq!(first["id"], second.join().unwrap()["id"]);
            first
        });
        let session = ensured["id"].as_str().unwrap().to_owned();
        let primary: String = db
            .query_row(
                "SELECT primary_session_id FROM tasks WHERE id=?1",
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
            fixture.run(&[
                "-b",
                "session",
                "connect",
                &session,
                "--message",
                "Start the Task conversation",
            ])
        } else {
            fixture.run(&["session", "connect", &session])
        };
        assert!(opened.status.success(), "{opened:?}");
        assert_eq!(
            fixture.count("agent_sessions"),
            prepared,
            "first start reused the prepared Session"
        );
        assert_eq!(fixture.captures(), 1);
        let native: String = db.query_row(
            "SELECT json_extract(payload,'$.evidence.provider_session_id') FROM session_events WHERE session_id=?1 AND json_extract(payload,'$.evidence.provider_session_id') IS NOT NULL ORDER BY seq DESC LIMIT 1",
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
    let fixture = Fixture::new(false);
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
        let args = [&["session", "connect", "--prepare", "--json"][..], &scope].concat();
        let first = fixture.json(&args);
        let id = first["id"].as_str().unwrap();
        assert_eq!(fixture.json(&args)["id"], id);
        assert!(fixture.launches().is_empty());
        let before: i64 = fixture
            .db()
            .query_row(
                "SELECT count(*) FROM session_events WHERE session_id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        let attachment =
            fixture.json(&["session", "attach", id, "--if-free", "--prepare", "--json"]);
        assert!(attachment.is_null(), "nothing is running to attach to");
        let refused = fixture.run(&["session", "attach", id, "--if-free"]);
        assert!(!refused.status.success());
        assert!(fixture.launches().is_empty());
        let after: i64 = fixture
            .db()
            .query_row(
                "SELECT count(*) FROM session_events WHERE session_id=?1",
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
    assert!(prompt.contains("do not require a Wave, Task, setup checklist or operating pass first"));
    assert!(prompt.contains("When Work exists, apply it"));
    let empty = fixture.json(&["desktop", "plan", "--all", "--json"]);
    let primary = fixture.json(&["session", "connect", "--prepare", "--json"]);
    let again = fixture.json(&["session", "connect", "--prepare", "--json"]);
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
        "connect",
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
        "sessions": fixture.json(&["desktop", "sessions", "--history", "--all", "--json"]),
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
        "- inspect-parser\n- interactive: {name: demo, id: review}\n",
    );
    fixture.repo.stage_all();
    fixture.repo.commit("Native lifecycle fixture");
    assert!(Command::new("git")
        .current_dir(fixture.repo.path())
        .args(["remote", "remove", "origin"])
        .status()
        .unwrap()
        .success());
    let task = fixture.json(&["task", "create", "--title", "Native local work", "--json"]);
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
    let config: serde_json::Value = serde_json::from_slice(
        &std::fs::read(fixture.home.path().join("native-config.json")).unwrap(),
    )
    .unwrap();
    let roots = config["config"]["sandbox_workspace_write.writable_roots"]
        .as_array()
        .unwrap();
    assert!(
        roots.contains(&serde_json::json!(fixture
            .home
            .path()
            .canonicalize()
            .unwrap()
            .join("configured-root"))),
        "{roots:?}"
    );
    assert!(
        roots.len() >= 2,
        "worktree Git access must retain configured roots"
    );
    let capture = fixture.launches()[0].clone();
    let (session, ..) = fixture.session_row(&capture);
    let native: String = fixture
        .db()
        .query_row(
            "SELECT json_extract(payload,'$.evidence.provider_session_id') FROM session_events WHERE session_id=?1 AND json_extract(payload,'$.evidence.provider_session_id') IS NOT NULL ORDER BY seq DESC LIMIT 1",
            [&session],
            |row| row.get(0),
        )
        .unwrap();
    let resumed = fixture
        .command(&["session", "connect", &session])
        .current_dir(worktree)
        .output()
        .unwrap();
    assert!(resumed.status.success(), "{resumed:?}");
    assert_eq!(fixture.count("agent_sessions"), 1);
    assert_eq!(
        fixture
            .db()
            .query_row(
                "SELECT json_extract(payload,'$.evidence.provider_session_id') FROM session_events WHERE session_id=?1 AND json_extract(payload,'$.evidence.provider_session_id') IS NOT NULL ORDER BY seq DESC LIMIT 1",
                [&session],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        native
    );
    support::managed_codex_account(fixture.home.path());
    // A bare run stops before the interactive review step; it may return as
    // its Graph registers, so wait for it to stop there.
    let run = fixture
        .command(&["--agent", "codex", "task", "run", task_id, "proof"])
        .env("LF_ACCOUNT_ISOLATION", "isolated")
        .output()
        .unwrap();
    assert!(run.status.success(), "{run:?}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let final_status = loop {
        let status = fixture.json(&["desktop", "task", task_id, "--json"]);
        let finished = support::recorded_graphs(fixture.home.path())
            .first()
            .is_some_and(|graph| graph.0.is_some());
        if finished
            && status["execution"]["work"]["graph"]["position"]
                == serde_json::json!({"kind": "at", "step": "review"})
        {
            break status;
        }
        assert!(std::time::Instant::now() < deadline, "{status}");
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    assert_eq!(final_status["execution"]["status"], "active");
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
    loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("loopflow.db"))
        .unwrap()
        .observe_pm_issue_change("fixture-deleted", None, true)
        .unwrap();
    let refused = fixture
        .command(&["session", "connect", &session])
        .current_dir(worktree)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("deleted"),
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
    assert_eq!(history["planning_state"], "removed");
    assert_eq!(
        history["execution"]["actions"]["reason"],
        "Task was deleted; retained history is read-only"
    );
    let refused = fixture
        .command(&["session", "connect", &session])
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
    std::fs::write(&provider, "#!/bin/sh\nexit 0\n").unwrap();
    let failed = fixture.run(&LAUNCH);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("private server failed to start"));
    let sessions = fixture.sessions();
    assert_eq!(sessions.len(), 1);
    let id = sessions[0]["id"].as_str().unwrap();
    std::fs::write(&provider, original).unwrap();
    for args in [
        vec!["session", "resume"],
        vec!["session", "connect", id, "--takeover"],
    ] {
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
    let history: i64 = fixture
        .db()
        .query_row(
            "SELECT count(*) FROM session_events WHERE session_id=?1 AND kind='captured'",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(history, 2);
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
    let history = fixture.json(&["session", "list", "--history", "--all", "--json"]);
    assert_eq!(history[0]["id"], id);
    assert_eq!(history[0]["state"], "closed");

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
    let executor: i32 = fixture.db().query_row(
        "SELECT p.pid FROM processes p JOIN session_events e ON e.lf_process_id=p.id WHERE e.kind='captured' AND e.receipt_key=?1",
        [&input], |row| row.get(0),
    ).unwrap();
    // SAFETY: the owner starts the captured executor in its own process group.
    // Terminal Ctrl+C reaches that whole group, including the native provider.
    assert_eq!(unsafe { libc::kill(-executor, libc::SIGINT) }, 0);
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
    assert!(history
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["payload"]["type"] == "attachment_exit"
            && event["payload"]["outcome"] == "interrupted"));
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
        let session = loopflow::session::LfSession {
            captured: None,
            caller_artifact_key: None,
            task_id: (index >= 110).then(|| task.task.id.clone()),
            wave_id: None,
            graph_lf_process_id: None,
            work_source: (index >= 110).then_some(loopflow::session::WorkSource::Declared),
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
            title_source: loopflow::session::TitleSource::Human,
            request: None,
            ready_summary: None,
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
    let end = fixture.json(&["session", "list", "--json", "--limit", "2", "--offset", "2"]);
    assert_eq!(end.as_array().unwrap().len(), 1);
    assert_eq!(end[0]["id"], "inventory-112");
    fixture
        .db()
        .execute(
            "UPDATE agent_sessions SET interactive=0 WHERE id IN ('inventory-000','inventory-112')",
            [],
        )
        .unwrap();
    let default = fixture.json(&["session", "list", "--json", "--limit", "0"]);
    assert_eq!(default.as_array().unwrap().len(), 2);
    let both = fixture.json(&[
        "session",
        "list",
        "--interactive",
        "all",
        "--json",
        "--limit",
        "0",
    ]);
    assert_eq!(both.as_array().unwrap().len(), 3);
    assert!(both
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["id"].as_str().unwrap() >= "inventory-110"));
    let everywhere = fixture.json(&[
        "session",
        "list",
        "--interactive",
        "all",
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
    let history = fixture.json(&[
        "session",
        "list",
        "--task",
        &task.task.identifier,
        "--interactive",
        "all",
        "--history",
        "--json",
    ]);
    assert_eq!(history.as_array().unwrap().len(), 3);
    // A historical Session completion without an AgentProcess proves no death.
    assert_eq!(history[0]["state"], "unknown");
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
    fixture.json(&[
        "session",
        "bind",
        &session,
        "--task",
        task.task.id.as_str(),
        "--json",
    ]);
    let count = serde_json::json!({"inputTokens":12,"outputTokens":3,
        "cachedInputTokens":0,"reasoningOutputTokens":0});
    for (kind, payload) in [
        ("usage", serde_json::json!({"total":count,"last":count})),
        ("completed", serde_json::json!({"status":"completed"})),
    ] {
        fixture.db().execute(
            "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload)
             VALUES(?1,'retained-thread','retained-turn',?2,'',strftime('%s','now'),?3)",
            rusqlite::params![session,kind,payload.to_string()]).unwrap();
    }
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
fn binding_leaves_ownership_and_prior_work_unchanged() {
    let fixture = Fixture::new(false);
    let task_path = fixture.repo.create_named_worktree("task-binding");
    let task = support::register_task_with_pr(
        fixture.home.path(),
        &task_path,
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
    // Ownership entries as (kind, at): binding reserves no work, so it appends none.
    let ownership = |id: &str| -> Vec<(String, i64)> {
        let db = fixture.db();
        let mut statement = db
            .prepare("SELECT kind,at FROM task_ownership WHERE task_id=?1 ORDER BY at")
            .unwrap();
        statement
            .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };
    let claimed = ownership(task.task.id.as_str());
    assert_eq!(claimed.len(), 1, "the seeded checkout claims the Task");
    let sibling_claimed = ownership(sibling.id.as_str());
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
            .json(&["history", "usage", "--days", "0", "--task", issue, "--json"])
            .as_array()
            .unwrap()
            .iter()
            .map(|run| run["artifact_key"].as_str().unwrap().to_string())
            .collect()
    };

    let orphan = launch(&LAUNCH);
    let (session, ..) = fixture.session_row(&orphan);
    assert_eq!(fixture.run_parents(&orphan), (None, None, None));
    assert_eq!(ownership(task.task.id.as_str()), claimed);
    assert!(task_runs("INF-123").is_empty());

    let wrong_identity = fixture.run(&["session", "bind", &orphan, "--task", "INF-123", "--json"]);
    assert!(!wrong_identity.status.success(), "{wrong_identity:?}");
    assert!(String::from_utf8_lossy(&wrong_identity.stderr).contains("was not found"));

    let preview = fixture.json(&[
        "session",
        "bind",
        &session,
        "--task",
        "INF-123",
        "--dry-run",
        "--json",
    ]);
    assert_eq!(preview["session_id"], session);
    assert_eq!(preview["task_id"], task.task.id.as_str());
    assert_eq!(preview["identifier"], "INF-123");
    assert_eq!(ownership(task.task.id.as_str()), claimed);
    assert!(fixture.sessions().is_empty());
    let history = fixture.json(&["session", "list", "--history", "--all", "--json"]);
    assert_eq!(history[0]["work"], Value::Null);
    assert_eq!(fixture.launches().len(), 1);

    // The bind happens measurably after the conversation was created.
    std::thread::sleep(Duration::from_secs(2));
    let before = now();
    let bound = fixture.json(&[
        "session",
        "bind",
        &session,
        "--task",
        preview["task_id"].as_str().unwrap(),
        "--json",
    ]);
    let after = now();
    assert_eq!(bound["id"], session.as_str());
    assert_eq!(
        bound["work"],
        serde_json::json!({"kind": "task", "id": task.task.id})
    );
    assert_eq!(fixture.run_parents(&orphan), (None, None, None));
    let bound_at: i64 = fixture
        .db()
        .query_row(
            "SELECT bound_at FROM agent_sessions WHERE id=?1",
            [&session],
            |row| row.get(0),
        )
        .unwrap();
    assert!((before..=after).contains(&bound_at));
    assert_eq!(ownership(task.task.id.as_str()), claimed);
    assert!(task_runs("INF-123").is_empty());
    assert!(
        fixture.sessions().is_empty(),
        "binding preserves completion"
    );
    let listed = fixture.json(&["desktop", "sessions", "--history", "--all", "--json"]);
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(
        listed[0]["work"],
        serde_json::json!({"kind": "task", "id": task.task.id})
    );
    assert_eq!(listed[0]["work_path"], "task-pr-tests / INF-123");

    // A Session's Task never moves; binding it again to the same Task is a no-op.
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
    assert_eq!(fixture.run_parents(&orphan).0, None);
    assert_eq!(ownership(sibling.id.as_str()), sibling_claimed);
    assert!(task_runs("INF-124").is_empty());

    std::thread::sleep(Duration::from_secs(2));
    let later = launch(&[
        "--task",
        "INF-123",
        "-i",
        "--agent",
        "opencode",
        ":",
        "Review the parser",
    ]);
    assert_eq!(ownership(task.task.id.as_str()), claimed);
    assert_eq!(task_runs("INF-123"), vec![later]);
    assert_eq!(fixture.run_parents(&orphan), (None, None, None));
}

#[test]
fn preparation_rejects_messages_without_admitting_or_launching_a_conversation() {
    let fixture = Fixture::new(false);
    let primary = fixture.json(&["connect", "--prepare", "--json"]);
    let before = fixture.count("agent_sessions");
    for verb in ["connect", "resume"] {
        let output = fixture.run(&[
            "-b",
            verb,
            "--fresh",
            "--prepare",
            "--message",
            "Do not submit",
            "--json",
        ]);
        assert!(!output.status.success(), "{output:?}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("--prepare") && error.contains("--message"),
            "{error}"
        );
        assert_eq!(fixture.count("agent_sessions"), before);
        assert!(fixture.launches().is_empty());
        assert_eq!(
            fixture.json(&["connect", "--prepare", "--json"])["id"],
            primary["id"]
        );
    }
}

#[test]
fn fresh_preparation_keeps_the_previous_live_client_and_named_history() {
    let fixture = Fixture::new(true);
    let first = fixture.json(&["-a", "opencode", "connect", "--prepare", "--json"]);
    let id = first["id"].as_str().unwrap();
    fixture.json(&["session", "rename", id, "Keep this conversation", "--json"]);
    let (mut client, input) = fixture.attach(&["connect", id]);
    let previous = fixture.json(&["connect", id, "--prepare", "--json"]);
    let fresh = fixture.json(&[
        "-a",
        "opencode",
        "-b",
        "resume",
        "--fresh",
        "--prepare",
        "--takeover",
        "--json",
    ]);
    assert_ne!(fresh["id"], id);
    assert_eq!(fresh["cwd"], first["cwd"]);
    assert!(client.try_wait().unwrap().is_none());
    assert_eq!(fixture.launches(), [input]);
    let retained = fixture.json(&["connect", id, "--prepare", "--json"]);
    assert_eq!(retained["title"], "Keep this conversation");
    assert_eq!(retained["agent_process_id"], previous["agent_process_id"]);
    assert_eq!(retained["state"], "active");
    assert_eq!(
        fixture.json(&["connect", "--prepare", "--json"])["id"],
        fresh["id"]
    );
    fixture.release(client);
}

#[test]
fn ensured_and_replaced_task_conversations_open_and_recover_unrecorded_native_exits() {
    let fixture = Fixture::new(false);
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
        "connect",
        "--prepare",
        "--task",
        "INF-123",
        "--json",
    ]);
    assert!(fixture.launches().is_empty());
    assert!(first["agent_process_id"].is_null());

    let mut session = first;
    for replacement in [false, true] {
        if replacement {
            let before = fixture.launches();
            session = fixture.json(&[
                "-a",
                "opencode",
                "session",
                "connect",
                "--task",
                "INF-123",
                "--fresh",
                "--prepare",
                "--json",
            ]);
            assert_eq!(fixture.launches(), before);
            assert!(session["agent_process_id"].is_null());
        }
        let id = session["id"].as_str().unwrap();
        let opened = fixture.run(&["session", "connect", id]);
        assert!(opened.status.success(), "{opened:?}");
        assert_eq!(
            fixture.session_row(fixture.launches().last().unwrap()).0,
            id
        );

        // Model the installed failure: the native provider exited, but its
        // launcher never recorded that fact. Its identity and history survive.
        for command in ["connect", "resume"] {
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
            let reopened = if command == "resume" {
                fixture.run(&["session", command])
            } else {
                fixture.run(&["session", command, id])
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
    for retired in [false, true] {
        let fixture = Fixture::new(false);
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
            "connect",
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
                "print(json.dumps({\"url\": f\"http://127.0.0.1:{server.server_port}\"}), flush=True)",
                "_record_launch()\nraise SystemExit(0)",
            ),
        )
        .unwrap();
        let failed = fixture.run(&["session", "connect", id]);
        assert!(!failed.status.success());
        assert!(String::from_utf8_lossy(&failed.stderr).contains("before becoming resumable"));
        let previous = fixture.launches()[0].clone();
        fixture
            .db()
            .execute(
                "UPDATE processes SET completed_at=NULL,outcome=NULL,spawn_state='spawn_requested'
             WHERE id=(SELECT agent_process_id FROM agent_sessions WHERE id=?1)",
                [id],
            )
            .unwrap();
        let successor = retired.then(|| {
            fixture.json(&[
                "-a",
                "opencode",
                "session",
                "connect",
                "--task",
                "INF-123",
                "--fresh",
                "--prepare",
                "--json",
            ])
        });
        std::fs::write(provider, script).unwrap();

        let opened = fixture.run(&["session", "connect", id]);
        assert!(opened.status.success(), "{opened:?}");
        let launches = fixture.launches();
        assert_eq!(launches.len(), 2);
        assert_ne!(launches[1], previous);
        assert_eq!(fixture.session_row(&previous).0, id);
        assert_eq!(fixture.session_row(&launches[1]).0, id);
        let primary = fixture.json(&[
            "session",
            "connect",
            "--prepare",
            "--task",
            "INF-123",
            "--json",
        ]);
        assert_eq!(primary["id"], successor.as_ref().unwrap_or(&session)["id"]);
        assert_eq!(fixture.session_row(&launches[1]).3, retired);
    }
}

#[test]
fn a_task_primary_is_one_of_its_own_conversations() {
    let fixture = Fixture::new(false);
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
    let converse = |checkout: &Path| -> String {
        let before = fixture.launches().len();
        let output = fixture
            .command(&LAUNCH)
            .current_dir(checkout)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        fixture.session_row(&fixture.launches()[before]).0
    };
    let primary = |args: &[&str]| -> Value {
        fixture.json(
            &[
                &["session", "connect", "--prepare", "--json", "--task"],
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

    // The Task's only unfinished interactive conversation is its primary.
    let sole = converse(&sole_path);
    let chosen = primary(&["inf-123"]);
    assert_eq!(chosen["id"], sole.as_str());
    // It stays an ordinary member of its Task.
    assert_eq!(chosen["primary_scope"], Value::Null);
    assert_eq!(chosen["task_primary"], true);
    assert_eq!(members("Inf-123"), std::slice::from_ref(&sole));
    for args in [
        vec!["connect", "--task", "inf-123", "--prepare", "--json"],
        vec!["--task", "Inf-123", "connect", "--prepare", "--json"],
    ] {
        assert_eq!(fixture.json(&args)["id"], sole.as_str());
    }

    // Among several, the most recently used one; the others stay open.
    let earlier = converse(&several_path);
    let later = converse(&several_path);
    fixture
        .db()
        .execute(
            "UPDATE agent_sessions SET created_at=created_at-60 WHERE id=?1",
            [&earlier],
        )
        .unwrap();
    assert_eq!(primary(&["INF-124"])["id"], later.as_str());
    assert_eq!(members("INF-124").len(), 2);

    // Opening an exact conversation leaves the Task's primary unchanged.
    assert_eq!(
        fixture.json(&["connect", &earlier, "--prepare", "--json"])["id"],
        earlier.as_str()
    );
    let explained = fixture.json(&["connect", "--task", "inf-124", "--explain", "--json"]);
    assert_eq!(explained["resolution"]["session"]["value"], later.as_str());
    let newest = converse(&several_path);
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
        .filter(|session| session["task_primary"] == true)
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
        .any(|arg| arg == "connect"));
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

    // Replacing it finishes that conversation and names a fresh one.
    let successor = fixture.json(&[
        "session",
        "connect",
        "--task",
        "INF-125",
        "--fresh",
        "--prepare",
        "--json",
    ]);
    assert_ne!(successor["id"], created["id"]);
    assert_eq!(primary(&["INF-125"])["id"], successor["id"]);
    assert_eq!(
        members("INF-125"),
        [successor["id"].as_str().unwrap().to_string()]
    );
    let nested = empty_path.join("nested");
    std::fs::create_dir(&nested).unwrap();
    for verb in ["connect", "resume"] {
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
    let store = loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("loopflow.db"))
        .unwrap();
    let origin: String = fixture
        .db()
        .query_row(
            "SELECT lf_process_id FROM session_events WHERE kind='captured' ORDER BY rowid LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let origin = loopflow::id::LfProcessId::parse(&origin).unwrap();
    let attachment = store
        .session_attachment(&session)
        .unwrap()
        .unwrap_or_else(|| {
            store
                .claim_session_attachment(&session, None, &origin, true)
                .unwrap()
        });
    let caller = serde_json::to_string(&attachment.caller(session.clone().into())).unwrap();
    fixture.json(&["session", "bind", &session, "--task", "INF-123", "--json"]);
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
        let retained: bool = fixture
            .db()
            .query_row(
                "WITH RECURSIVE ancestors(id,parent_lf_process_id) AS (
                SELECT p.id,p.parent_lf_process_id FROM processes p
                JOIN session_events e ON e.lf_process_id=p.id
                WHERE e.kind='captured' AND e.receipt_key=?1
                UNION ALL
                SELECT p.id,p.parent_lf_process_id FROM processes p
                JOIN ancestors a ON p.id=a.parent_lf_process_id
             ) SELECT EXISTS(SELECT 1 FROM ancestors WHERE id=?2)",
                rusqlite::params![capture, origin],
                |row| row.get(0),
            )
            .unwrap();
        assert!(retained, "provider-issued work lost its causal parent");
    }
    // A stale provider keeps its original causal parent and grants no Work.
    store
        .claim_session_attachment(&session, Some(&attachment), &origin, true)
        .unwrap();
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
fn declared_agent_tools_use_their_checkout_and_keep_the_process_parent() {
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
    let (caller, ..) = fixture.session_row(&captures[0]);
    let parent: (String, String) = fixture.db().query_row("SELECT c.parent_lf_process_id,a.parent_lf_process_id FROM processes c JOIN agent_sessions s ON s.id=c.caller_session_id JOIN processes a ON a.id=s.agent_process_id WHERE c.caller_session_id=?1", [&caller], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
    assert_eq!(parent.0, parent.1);
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
    assert!(!loopflow::installation::root().unwrap().exists());
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
        "- cmd: sync --plan\n- interactive: demo\n",
    )
    .unwrap();
    // The Wave default reads the repository's committed graphs.
    std::fs::create_dir_all(fixture.repo.path().join(".lf/graphs")).unwrap();
    std::fs::write(
        fixture.repo.path().join(".lf/graphs/switch-proof.yaml"),
        "- cmd: sync --plan\n- interactive: demo\n",
    )
    .unwrap();
    let store = loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("loopflow.db"))
        .unwrap();
    // Saved Task defaults stay local to each Task. An explicit --agent would
    // override Y too, selecting OpenCode without a managed checkout boundary.
    // Y's mechanical Graph never starts a provider; X's OpenCode issues the command.
    store.set_task_agent(&caller.id, "opencode").unwrap();
    store.set_task_agent(&target.task.id, "claude").unwrap();
    support::select_wave_graph(
        fixture.repo.path(),
        &store,
        target.task.wave_id.as_ref().unwrap(),
        "switch-proof",
    );
    let bin = fixture.home.path().join("bin");
    std::fs::remove_file(bin.join("lf")).unwrap();
    std::fs::write(
        bin.join("lf"),
        format!(
            "#!/bin/sh\nprintf '%s' \"$LF_AS\" >'{}'\nexec '{}' \"$@\"\n",
            fixture.home.path().join("step-declaration").display(),
            env!("CARGO_BIN_EXE_lf")
        ),
    )
    .unwrap();
    std::fs::set_permissions(bin.join("lf"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(
        fixture.home.path().join("tool-command.json"),
        serde_json::to_vec(&serde_json::json!({
            "argv": [env!("CARGO_BIN_EXE_lf"), "task", "run", "INF-123", "switch-proof"], "cwd": x,
        }))
        .unwrap(),
    )
    .unwrap();
    let output = fixture
        .command(&["--task", "INF-124", "-i", ":", "Start Y"])
        .env("LF_BIN", bin.join("lf"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let result: Value = serde_json::from_slice(
        &std::fs::read(fixture.home.path().join("tool-result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(result["code"], 0, "{result}");
    // A bare run returns after registration; wait for Y to stop at demo.
    // -b would execute demo headlessly and start Y's provider.
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
        status["execution"]["work"]["graph"]["position"],
        serde_json::json!({"kind": "at", "step": "demo"})
    );
    assert_eq!(fixture.launches().len(), 1);
    assert_eq!(
        std::fs::read_to_string(fixture.home.path().join("step-declaration")).unwrap(),
        format!("task:{}", target.task.id)
    );
    // The step ran in Y's checkout under a Graph process X's conversation started.
    let observed: (String, String) = fixture
        .db()
        .query_row(
            "WITH RECURSIVE ancestors(cwd,id,parent_lf_process_id,caller_session_id) AS (
            SELECT step.cwd,step.id,step.parent_lf_process_id,step.caller_session_id
            FROM processes step
            JOIN graph_process_steps recorded ON recorded.lf_process_id=step.id
            UNION ALL
            SELECT a.cwd,p.id,p.parent_lf_process_id,p.caller_session_id
            FROM processes p JOIN ancestors a ON p.id=a.parent_lf_process_id
         ) SELECT DISTINCT a.cwd,s.task_id FROM ancestors a
         JOIN agent_sessions s ON s.id=a.caller_session_id",
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
            String::from_utf8_lossy(&output.stderr)
                .contains("Execution requires an admitted invocation"),
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
            String::from_utf8_lossy(&output.stderr).contains("Cannot open execution transport log"),
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
fn headless_history_is_discoverable_without_entering_the_interactive_list() {
    let fixture = Fixture::new(false);
    support::managed_codex_account(fixture.home.path());
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
    let sessions = fixture.json(&["session", "list", "--interactive", "false", "--json"]);
    assert_eq!(sessions.as_array().unwrap().len(), 1);
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
        "- work-proof\n- loop: work-proof\n  step: decide-proof\n",
    )
    .unwrap();
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
                "SELECT count(*) FROM session_events WHERE kind='started'",
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
        // The decision is the plain skill command; its contract is in the message.
        let decision = &steps[1];
        assert_eq!(decision["argv"][0], "--batch");
        assert!(decision["argv"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "decide-proof"));
        assert!(decision["argv"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("Return the final answer as the declared JSON value"));
        // Every correction resumes the conversation that gave the answer.
        let conversation: String = fixture
            .db()
            .query_row(
                "SELECT id FROM agent_sessions WHERE skill='decide-proof'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        for correction in &steps[2..] {
            let args = correction["argv"].as_array().unwrap();
            assert_eq!(args[0], "--batch");
            assert_eq!(args[1], "--output-schema");
            let schema: serde_json::Value =
                serde_json::from_str(args[2].as_str().unwrap()).unwrap();
            assert_eq!(
                schema["properties"]["decision"]["enum"],
                serde_json::json!(["advance", "iterate", "blocked"])
            );
            assert_eq!(args[3..6], ["session", "connect", conversation.as_str()]);
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
        .query_row(
            "SELECT d.error FROM processes d JOIN graph_processes g ON g.lf_process_id=d.id",
            [],
            |row| row.get(0),
        )
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
    let history = fixture.json(&["session", "history", &session, "--json"]);
    assert!(
        history.to_string().contains("Release target is missing"),
        "{history}"
    );
}

#[test]
fn custom_router_returns_a_captured_path_without_an_in_turn_command() {
    let fixture = Fixture::new(false);
    saved_graph_stand_in(&fixture);
    std::fs::write(fixture.repo.path().join(".lf/graphs/choose.yaml"),
        "- xor:\n    router: decide-proof\n    paths:\n      alpha:\n        description: Do the selected work\n        skill: work-proof\n      zeta:\n        description: Leave this path untouched\n        skill: review-proof\n").unwrap();
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
}

#[test]
fn opencode_disconnect_after_tool_preserves_unknown_native_completion() {
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
    let rows: (i64, i64) = fixture
        .db()
        .query_row(
            "SELECT (SELECT count(*) FROM session_events WHERE kind='started'),
                (SELECT count(*) FROM session_events WHERE kind='completed')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(rows, (1, 0));
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
    let task = fixture.json(&["task", "create", "--title", "Retained checkout", "--json"]);
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
        "SELECT json_extract(payload,'$.evidence.provider_session_id') FROM session_events WHERE session_id=?1 AND json_extract(payload,'$.evidence.provider_session_id') IS NOT NULL ORDER BY seq DESC LIMIT 1",
        [&session], |row| row.get::<_, String>(0)).unwrap()
    };
    let before = native();
    let machine: String = fixture
        .db()
        .query_row("SELECT id FROM machines WHERE route='local'", [], |row| {
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
    let resumed = fixture.run(&["session", "connect", &session]);
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
        .command(&["session", "connect", &session])
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
            include_str!("../src/harness/testdata/pi_rpc.py")
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
