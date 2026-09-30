//! Conversations own admission, immutable inputs, and history through completion.
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

    fn retired_run_tables(&self) -> i64 {
        self.db()
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='runs'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn count(&self, table: &str) -> i64 {
        self.db()
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn artifact(&self, id: &str) -> String {
        self.db().query_row("SELECT e.receipt_key FROM agent_sessions s JOIN session_events e ON e.seq=s.current_capture WHERE s.id=?1", [id], |row| row.get(0)).unwrap()
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

    /// (session id, kind, title, title_source, completed) of the Run's Session.
    fn session_row(&self, run_id: &str) -> (String, String, String, String, bool) {
        self.db()
            .query_row(
                "SELECT s.id, s.kind, s.title, s.title_source, s.completed_at IS NOT NULL
                 FROM agent_sessions s JOIN session_events i ON i.session_id=s.id AND i.kind='captured'
                 WHERE i.receipt_key=?1",
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
            .unwrap_or_else(|error| panic!("Run {run_id} has no row: {error}"))
    }

    fn run_dir(&self, run_id: &str) -> PathBuf {
        self.home
            .path()
            .join("runs")
            .join(&run_id.strip_prefix("run_").unwrap_or(run_id)[..2])
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

    fn wait_for_ask(&self, asking: &mut Child) -> (String, String) {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let stored = self.db().query_row(
                "SELECT id,(SELECT receipt_key FROM session_events WHERE seq=agent_sessions.current_capture) FROM agent_sessions WHERE kind='ask'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            );
            match stored {
                Ok(stored) => return stored,
                Err(rusqlite::Error::QueryReturnedNoRows) => {}
                Err(rusqlite::Error::SqliteFailure(error, _))
                    if matches!(
                        error.code,
                        rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                    ) => {}
                Err(error) => {
                    let _ = asking.kill();
                    let _ = asking.wait();
                    panic!("cannot inspect stored Ask: {error}");
                }
            }
            if asking.try_wait().unwrap().is_some() || Instant::now() >= deadline {
                let _ = asking.kill();
                let status = asking.wait();
                panic!("Ask stored no Session: {status:?}");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
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
        (
            fixture.count("agent_sessions"),
            fixture.retired_run_tables()
        ),
        (1, 0),
        "launch reserves one Session and no Run"
    );
    let (id, kind, title, source, completed) = fixture.session_row(&first_run);
    assert_ne!(id, first_run, "a Session is not its Run");
    assert_eq!(
        (kind.as_str(), source.as_str(), completed),
        ("conversation", "generated", false)
    );
    let (magical, musical) = title.split_once('-').expect("magical-musical pair");
    assert!(!magical.is_empty() && !musical.is_empty() && !musical.contains('-'));
    assert_eq!(fixture.run_parents(&first_run), (None, None, None));

    let listed = fixture.sessions();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["id"], id.as_str());
    assert!(listed[0].get("run_id").is_none());
    assert_eq!(listed[0]["kind"], "conversation");
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
    let retained = fixture.sessions();
    assert_eq!(
        retained.len(),
        1,
        "provider exit does not complete a Session"
    );
    // Passive inventory no longer opens native history to infer closure.
    assert_eq!(retained[0]["state"], "unknown");
    assert_eq!(retained[0]["title"], "Parser review");

    // Open resumes the same Run under the same Session.
    let described = fixture.json(&["session", "open", &id, "--json"]);
    assert_eq!(described["id"], id.as_str());
    assert!(described.get("run_id").is_none());
    let (resumed, resumed_run) = fixture.attach(&["session", "open", &id]);
    assert_eq!(resumed_run, first_run);
    assert_eq!(fixture.sessions()[0]["state"], "active");
    fixture.release(resumed);
    assert_eq!(
        (
            fixture.count("agent_sessions"),
            fixture.retired_run_tables()
        ),
        (1, 0)
    );

    // Another launch in the same checkout is another conversation.
    let (second, second_run) = fixture.attach(&LAUNCH);
    fixture.release(second);
    let (second_id, ..) = fixture.session_row(&second_run);
    assert_ne!(second_id, id);
    assert_eq!(
        (
            fixture.count("agent_sessions"),
            fixture.retired_run_tables()
        ),
        (2, 0)
    );
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
    assert_eq!(fixture.count("agent_sessions"), 3);
}

#[test]
fn inventory_scopes_before_paging_and_keeps_worktree_repository_identity() {
    let fixture = Fixture::new(false);
    let task = support::register_unrun_task(
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
        let session = loopflow::session::AgentSession {
            captured: None,
            caller_artifact_key: None,
            task_id: (index == 112).then(|| task.task.id.clone()),
            wave_id: None,
            flow_session_id: None,
            work_source: (index == 112).then_some(loopflow::session::WorkSource::Declared),
            bound_at: None,
            id: id.clone(),
            artifact_key: uuid::Uuid::new_v4().simple().to_string(),
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
            kind: loopflow::session::SessionKind::Conversation,
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
            let dir = fixture.run_dir(session.artifact_key.as_str());
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("provider-session.json"),
                "broken payload outside selected repo",
            )
            .unwrap();
        }
        runtime
            .block_on(task.store.create_session(session, None))
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
        &task.task.plan.identifier,
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
}

#[test]
fn public_history_discovers_unlinked_native_receipts_without_borrowing_a_later_bind() {
    let fixture = Fixture::new(false);
    let launched = fixture.run(&LAUNCH);
    assert!(launched.status.success(), "{launched:?}");
    let (session, ..) = fixture.session_row(&fixture.launches()[0]);
    let task = support::register_unrun_task(
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
    for command in [
        vec!["runs", "--json"],
        vec!["usage", "--days", "0", "--json"],
    ] {
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
        assert!(recovered["providers"][0]["exec_id"].is_null());
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
fn binding_starts_the_task_once_without_reattributing_prior_work() {
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
            .map(|run| run["artifact_key"].as_str().unwrap().to_string())
            .collect()
    };

    let orphan = launch(&LAUNCH);
    let (session, ..) = fixture.session_row(&orphan);
    assert_eq!(fixture.run_parents(&orphan), (None, None, None));
    assert_eq!(started(task.task.id.as_str()), None);
    assert!(task_runs("INF-123").is_empty());

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
    assert_eq!(started(task.task.id.as_str()), None);
    assert_eq!(fixture.sessions()[0]["work"], Value::Null);
    assert_eq!(fixture.launches().len(), 1);

    // The bind happens measurably after the Run was created.
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
    assert!(bound.get("run_id").is_none());
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
    assert_eq!(started(task.task.id.as_str()), Some(bound_at));
    assert!(task_runs("INF-123").is_empty());
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
    assert_eq!(fixture.run_parents(&orphan).0, None);
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
    let started_at = started(task.task.id.as_str()).expect("future work starts the Task");
    assert_eq!(started_at, bound_at);
    assert_eq!(task_runs("INF-123"), vec![later]);
    assert_eq!(fixture.run_parents(&orphan), (None, None, None));

    // Started is stored once beside admitted work naming the Task.
    let db = fixture.db();
    let disagreeing: i64 = db
        .query_row(
            "SELECT count(*) FROM tasks t WHERE (t.started_at IS NOT NULL)
                != (EXISTS(SELECT 1 FROM import_evidence i WHERE i.historical_task_id=t.id)
                OR EXISTS(SELECT 1 FROM agent_sessions s WHERE s.task_id=t.id))",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(disagreeing, 0);
    for (id, value) in [
        (task.task.id.as_str(), Some(started_at + 1)),
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

#[test]
fn provider_parentage_does_not_assign_work_outside_its_checkout() {
    let fixture = Fixture::new(false);
    let task_path = fixture.repo.create_named_worktree("task-binding");
    let task = support::register_unrun_task(
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
            "SELECT id FROM execs ORDER BY started_at,id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let origin = loopflow::id::ExecId::parse(&origin).unwrap();
    let driver = store.session_driver(&session).unwrap().unwrap_or_else(|| {
        store
            .claim_session_driver(&session, None, &origin, true)
            .unwrap()
    });
    let caller = serde_json::to_string(&driver.caller(session.clone())).unwrap();
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
            .env("LF_TASK_ORIGIN", "1")
            .env("LF_WORK_ADVANCE_CLAIM", "obsolete identity")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let capture = fixture.launches().last().unwrap().clone();
        let work = fixture.run_parents(&capture);
        assert_eq!(work.0, expected);
        assert_eq!(work.2.as_deref(), source);
        let parent: String = fixture.db().query_row("SELECT parent_exec_id FROM execs WHERE caller_session_id=?1 ORDER BY rowid DESC LIMIT 1", [&session], |row| row.get(0)).unwrap();
        assert_eq!(parent, origin.as_str());
    }
    // Asking outside the Task retains causal input but does not inherit its Task.
    let mut asking = fixture
        .ask(&original, "Which target?")
        .env("LF_AGENT_CALLER", &caller)
        .spawn()
        .unwrap();
    let (ask, input) = fixture.wait_for_ask(&mut asking);
    assert_eq!(fixture.run_parents(&input), (None, None, None));
    store
        .ready_session(
            &ask,
            store.captured_sequence(&input).unwrap(),
            "Keep this checkout",
        )
        .unwrap();
    assert!(fixture.run(&["session", "complete", &ask]).status.success());
    assert!(asking.wait_with_output().unwrap().status.success());
    assert_eq!(fixture.run_parents(&original), (None, None, None));
    // A stale provider keeps its original causal parent and grants no Work.
    store
        .claim_session_driver(&session, Some(&driver), &origin, true)
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
fn declared_agent_tools_use_their_checkout_and_keep_the_exec_parent() {
    let fixture = Fixture::new(false);
    let x = fixture.repo.create_named_worktree("task-x");
    let task =
        support::register_unrun_task(fixture.home.path(), &x, "task-x", &fixture.repo.head_sha());
    let y = fixture.repo.create_named_worktree("task-y");
    let sibling = support::register_sibling_task(&task, "INF-124", "task-y", &y);
    std::fs::write(fixture.home.path().join("tool-command.json"), serde_json::to_vec(&serde_json::json!({
        "argv": [env!("CARGO_BIN_EXE_lf"), "--tui", "--model", "opencode", ":", "Work in Y"], "cwd": y,
    })).unwrap()).unwrap();
    let output = fixture.run(&[
        "--as",
        "task:INF-123",
        "--tui",
        "--model",
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
    let parent: (String, String) = fixture.db().query_row("SELECT c.parent_exec_id,s.provider_exec_id FROM execs c JOIN agent_sessions s ON s.id=c.caller_session_id WHERE c.caller_session_id=?1", [&caller], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
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
fn declared_agent_can_start_another_tasks_flow() {
    assert!(Path::new("/.dockerenv").is_file());
    assert!(!loopflow::machine_install::root().unwrap().exists());
    let fixture = Fixture::new(false);
    let y = fixture.repo.create_named_worktree("task-y");
    let target =
        support::register_unrun_task(fixture.home.path(), &y, "task-y", &fixture.repo.head_sha());
    let x = fixture.repo.create_named_worktree("task-x");
    let caller = support::register_sibling_task(&target, "INF-124", "task-x", &x);
    std::fs::create_dir_all(y.join(".lf/flows")).unwrap();
    std::fs::write(
        y.join(".lf/flows/switch-proof.yaml"),
        "- op: rebase --plan\n",
    )
    .unwrap();
    let store = loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("loopflow.db"))
        .unwrap();
    // Y needs a harness that supports the checkout boundary. Its mechanical
    // Flow never starts a provider; X's interactive OpenCode only issues the command.
    store.set_task_agent(&target.task.id, "claude").unwrap();
    let flow = store
        .start_task_flow(
            &target.task.id,
            &loopflow::durable::FlowSession {
                invocation: loopflow::engine::invocation::QueuedInvocation::load(
                    &y,
                    "switch-proof",
                )
                .unwrap(),
                cursor: Default::default(),
                version: 0,
                task_id: Some(target.task.id.clone()),
                wave_id: Some(target.task.wave_id.clone()),
                cwd: y.clone(),
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
        .unwrap();
    let bin = fixture.home.path().join("bin");
    // A local process stands in for tmux; the actual Task driver and op execute.
    std::fs::write(bin.join("tmux"), format!(
        "#!/bin/sh\ncase \"$1\" in new-session) cd \"$6\"; shift 8; /bin/sh -c \"$1\" >'{}' 2>&1 & ;; has-session) exit 1 ;; esac\n",
        fixture.home.path().join("worker.log").display())).unwrap();
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
            "argv": [env!("CARGO_BIN_EXE_lf"), "task", "run", "INF-123", "--json"], "cwd": x,
        }))
        .unwrap(),
    )
    .unwrap();
    let output = fixture.run(&[
        "--as",
        "task:INF-124",
        "--tui",
        "--model",
        "opencode",
        ":",
        "Start Y",
    ]);
    assert!(output.status.success(), "{output:?}");
    let result: Value = serde_json::from_slice(
        &std::fs::read(fixture.home.path().join("tool-result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(result["code"], 0, "{result}");
    wait_for("Y Flow completion", || {
        let current = store.flow(flow.id()).unwrap().unwrap();
        assert!(
            current.failure.is_none(),
            "{:?}; {}",
            current.failure,
            std::fs::read_to_string(fixture.home.path().join("worker.log")).unwrap_or_default()
        );
        current.finished.then_some(())
    });
    assert_eq!(
        std::fs::read_to_string(fixture.home.path().join("step-declaration")).unwrap(),
        format!("task:{}", target.task.id)
    );
    let observed: (String,String) = fixture.db().query_row("SELECT f.task_id,s.task_id FROM flow_events h JOIN flow_sessions f ON f.id=h.flow_id JOIN execs child ON child.id=h.exec_id JOIN execs worker ON worker.id=child.parent_exec_id JOIN execs command ON command.id=worker.parent_exec_id JOIN agent_sessions s ON s.id=command.caller_session_id WHERE f.id=?1 AND h.kind='operation_started'", [flow.id()], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
    assert_eq!(
        observed,
        (target.task.id.to_string(), caller.id.to_string())
    );
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
    fixture.repo.create_branch("task-binding");
    let (caller, caller_run) = fixture.attach(&BOUND_LAUNCH);

    let mut asking = fixture
        .ask(&caller_run, "Which release target?")
        .spawn()
        .unwrap();
    let (id, first_run) = fixture.wait_for_ask(&mut asking);
    wait_for("the conversation launcher", || {
        fixture
            .launcher_requests()
            .contains("serve-ask")
            .then_some(())
    });
    assert_eq!(
        (
            fixture.count("agent_sessions"),
            fixture.retired_run_tables()
        ),
        (2, 0)
    );
    let inherited = (
        Some(task.task.id.to_string()),
        Some(task.task.wave_id.to_string()),
        Some("checkout".to_string()),
    );
    assert_eq!(fixture.run_parents(&first_run), inherited);
    let ask_run = |run_id: &str| -> (Option<String>, Option<String>, String) {
        fixture
            .db()
            .query_row(
                "SELECT s.flow_session_id, json_extract(i.payload,'$.caller_key'), s.id FROM session_events i JOIN agent_sessions s ON s.id=i.session_id AND i.kind='captured' WHERE i.receipt_key=?1",
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
    assert!(listed.get("run_id").is_none());
    assert_eq!(listed["title"], "Which release target?");
    assert_eq!(listed["title_source"], "generated");
    assert_eq!(listed["state"], "unknown");
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
                "SELECT title, ready_summary, (SELECT receipt_key FROM session_events WHERE seq=agent_sessions.current_capture), completed_at IS NOT NULL
                 FROM agent_sessions WHERE id=?1",
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
            "SELECT count(*) FROM session_events WHERE kind='captured' AND session_id=?1",
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
    assert_eq!(fixture.count("agent_sessions"), 2);

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
fn inspection_execs_leave_an_existing_task_unstarted() {
    let fixture = Fixture::new(false);
    let task = support::register_unrun_task(
        fixture.home.path(),
        fixture.repo.path(),
        "inspection-only",
        &fixture.repo.head_sha(),
    );
    for args in [
        vec!["session", "list", "--task", "INF-123", "--json"],
        vec!["usage", "--task", "INF-123", "--json"],
    ] {
        let output = fixture.run(&args);
        assert!(output.status.success(), "{output:?}");
    }
    let started: Option<i64> = fixture
        .db()
        .query_row(
            "SELECT started_at FROM tasks WHERE id=?1",
            [task.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(started, None);
    assert_eq!(fixture.count("agent_sessions"), 0);
    let completed: i64 = fixture.db().query_row(
        "SELECT count(*) FROM execs WHERE outcome='succeeded' AND exit_code=0 AND completed_at IS NOT NULL",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(completed, 2);
}

#[test]
fn failed_exec_observation_cannot_admit_a_provider() {
    let fixture = Fixture::new(false);
    let initialized = fixture.run(&["session", "list", "--all", "--json"]);
    assert!(initialized.status.success(), "{initialized:?}");
    // Fail just the process observation. The conversation store remains writable.
    fixture
        .db()
        .execute_batch(
            "CREATE TRIGGER refuse_fixture_exec BEFORE INSERT ON execs
         BEGIN SELECT RAISE(ABORT, 'fixture refuses Exec observation'); END;",
        )
        .unwrap();
    for args in [
        LAUNCH.as_slice(),
        &["-b", "--model", "opencode", ":", "Tidy the parser"],
    ] {
        let output = fixture.run(args);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(
            fixture.launches().is_empty(),
            "a writable Session store cannot replace Exec admission"
        );
    }
    assert_eq!(
        fixture.count("execs"),
        1,
        "retain the earlier inspection only"
    );
}

#[test]
fn malformed_caller_cannot_use_library_agent_admission() {
    let fixture = Fixture::new(false);
    for args in [
        LAUNCH.as_slice(),
        &["-b", "--model", "opencode", ":", "Tidy the parser"],
    ] {
        let output = fixture
            .command(args)
            .env("LF_AGENT_CALLER", "not-json")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("agent launch requires an admitted Exec"),
            "{output:?}"
        );
        assert!(
            fixture.launches().is_empty(),
            "no provider starts without process admission"
        );
    }
    // A diagnostic command is still usable. Invalid provenance never becomes
    // a fabricated root/direct Exec merely to make logging succeed.
    let output = fixture
        .command(&["session", "list", "--all", "--json"])
        .env("LF_AGENT_CALLER", "not-json")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(fixture.count("execs"), 0);
}

#[test]
fn agent_admission_requires_the_store_before_provider_launch() {
    let fixture = Fixture::new(false);
    let store = LockedStore::new(&fixture);
    for args in [
        LAUNCH.as_slice(),
        &["-b", "--model", "opencode", ":", "Tidy the parser"],
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
    let rows = store.unlock();
    let mut inventory = fixture.command(&["session", "list", "--all", "--json"]);
    store.select(&mut inventory);
    let output = inventory.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        serde_json::json!([])
    );
    let stored: (i64, i64) = rows
        .query_row(
            "SELECT (SELECT count(*) FROM agent_sessions), (SELECT count(*) FROM sqlite_master WHERE type='table' AND name='runs')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(stored, (0, 0));
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
                "SELECT s.id, s.flow_session_id, (SELECT receipt_key FROM session_events WHERE seq=s.current_capture) FROM agent_sessions s
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
                "SELECT title, title_source, ready_summary, (SELECT receipt_key FROM session_events WHERE seq=agent_sessions.current_capture),
                    completed_at IS NOT NULL FROM agent_sessions WHERE id=?1",
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
    assert_eq!(
        (
            fixture.count("agent_sessions"),
            fixture.retired_run_tables()
        ),
        (1, 0)
    );
    let parents: (Option<String>, Option<String>, String, String) = fixture
        .db()
        .query_row(
            "SELECT r.task_id, f.task_id, f.pending_session_id, f.state
             FROM agent_sessions r JOIN flow_sessions f ON f.id=r.flow_session_id WHERE (SELECT receipt_key FROM session_events WHERE seq=r.current_capture)=?1",
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
    assert_eq!(
        (
            fixture.count("agent_sessions"),
            fixture.retired_run_tables()
        ),
        (1, 0)
    );
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
            "SELECT i.receipt_key, s.flow_session_id, s.task_id FROM session_events i JOIN agent_sessions s ON s.id=i.session_id AND i.kind='captured' WHERE i.session_id=?1
             ORDER BY i.receipt_key",
        )
        .unwrap()
        .query_map([&id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        history
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        [
            (first_run.clone(), Some(invocation.clone()), None),
            (second_run.clone(), Some(invocation.clone()), None)
        ]
        .into_iter()
        .collect(),
        "the replaced Run stays in the Session's history"
    );
    let current: String = fixture
        .db()
        .query_row(
            "SELECT (SELECT receipt_key FROM session_events WHERE seq=current_capture) FROM flow_sessions WHERE id=?1",
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
            "SELECT state FROM flow_sessions WHERE id=?1",
            [&invocation],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "completed");
    assert_eq!(
        (
            fixture.count("agent_sessions"),
            fixture.retired_run_tables()
        ),
        (1, 0)
    );
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
    assert!(listed[0].get("run_id").is_none());
    assert_eq!(listed[0]["title"], "review-proof");
    assert_eq!(listed[0]["title_source"], "generated");
    assert_eq!(listed[0]["detail"], "review-proof");
    assert_eq!(listed[0]["state"], "unknown");
    assert_eq!(listed[0]["provider"], "opencode");
    assert_eq!(listed[0]["work"], Value::Null);
    assert_eq!(
        listed[0]["flow_membership"],
        serde_json::json!({
            "kind": "step", "flow": "review-first", "invocation_id": invocation,
            "step": "review-proof", "node": 0, "iterations": [[]],
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

#[test]
fn importing_after_bind_does_not_report_the_task_started_again() {
    let fixture = Fixture::new(false);
    let task = support::register_unrun_task(
        fixture.home.path(),
        fixture.repo.path(),
        "bound-import",
        &fixture.repo.head_sha(),
    );
    assert!(fixture.run(&LAUNCH).status.success());
    let original = fixture.launches()[0].clone();
    let (session, ..) = fixture.session_row(&original);
    fixture.json(&["session", "bind", &session, "--task", "INF-123", "--json"]);
    let started = || -> i64 {
        fixture
            .db()
            .query_row(
                "SELECT started_at FROM tasks WHERE id=?1",
                [task.task.id.as_str()],
                |row| row.get(0),
            )
            .unwrap()
    };
    let bound_at = started();
    assert_eq!(fixture.run_parents(&original), (None, None, None));

    let historical = uuid::Uuid::new_v4().simple().to_string();
    let mut manifest: Value = serde_json::from_slice(
        &std::fs::read(fixture.run_dir(&original).join("manifest.json")).unwrap(),
    )
    .unwrap();
    manifest["run_id"] = serde_json::json!(historical);
    manifest["surface"] = serde_json::json!("headless");
    manifest["subjects"] = serde_json::json!([
        {"selector": "task:INF-123", "source": "declared"}
    ]);
    let dir = fixture.run_dir(historical.as_str());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    for args in [
        vec!["session", "import", "--dry-run", "--json"],
        vec!["session", "import", "--json"],
    ] {
        let report = fixture.json(&args);
        assert_eq!(report["tasks_started"], serde_json::json!([]), "{report}");
        assert_eq!(report["run"], 1, "{report}");
        assert_eq!(report["failed"], serde_json::json!([]), "{report}");
        assert_eq!(started(), bound_at);
    }
    assert_eq!(fixture.run_parents(&original), (None, None, None));
    assert_eq!(
        fixture.run_parents(historical.as_str()).0.as_deref(),
        Some(task.task.id.as_str())
    );
}

#[test]
fn import_retains_replaced_inputs_without_rebinding_their_history() {
    use loopflow::session::{AgentSession, SessionKind, TitleSource};
    use serde_json::json;

    let fixture = Fixture::new(false);
    let task = support::register_unrun_task(
        fixture.home.path(),
        fixture.repo.path(),
        "import-members",
        &fixture.repo.head_sha(),
    );
    let prior = uuid::Uuid::new_v4().simple().to_string();
    let current = uuid::Uuid::new_v4().simple().to_string();
    let caller = uuid::Uuid::new_v4().simple().to_string();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let saved = runtime.block_on(async {
        let first = AgentSession {
            captured: None,
            id: "retained-conversation".into(),
            artifact_key: prior.clone(),
            caller_artifact_key: Some(caller.clone()),
            input_published: true,
            cwd: fixture.repo.path().to_path_buf(),
            skill: Some("implement".into()),
            provider: Some("opencode".into()),
            model: None,
            node: None,
            iterations: None,
            task_id: None,
            wave_id: None,
            flow_session_id: None,
            work_source: None,
            bound_at: None,
            kind: SessionKind::Conversation,
            interactive: false,
            repo: None,
            title: "Retained human name".into(),
            title_source: TitleSource::Human,
            request: None,
            ready_summary: Some("Retained feedback".into()),
            completed_at: None,
            created_at: 1,
        };
        let first = task.store.create_session(first, None).await.unwrap();
        let mut bound = task
            .store
            .bind_session("retained-conversation", first.captured, &task.task.id)
            .await
            .unwrap();
        bound.artifact_key = current.clone();
        bound.provider = Some("claude".into());
        task.store
            .replace_session_input(bound.captured, bound)
            .await
            .unwrap()
    });
    let started = || {
        fixture
            .db()
            .query_row(
                "SELECT started_at FROM tasks WHERE id=?1",
                [task.task.id.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
    };
    let original_started = started();
    for (input, attributed, outcome, usage) in [
        (
            &prior,
            false,
            "failed",
            json!({"attempt":1,"account_id":"first","input":21,"output":null}),
        ),
        (
            &current,
            true,
            "completed",
            json!({"attempt":1,"account_id":"second","input":0,"output":0}),
        ),
    ] {
        let dir = fixture.run_dir(input.as_str());
        std::fs::create_dir_all(&dir).unwrap();
        let subjects = if attributed {
            json!([{"selector":"task:INF-123","source":"inherited"}])
        } else {
            json!([])
        };
        let at = chrono::Utc::now() - chrono::Duration::days(if attributed { 1 } else { 10 });
        let manifest = json!({
            "schema_version":1,"run_id":input,"parent_run_id":caller,
            "created_at":at.to_rfc3339(),"harness":if attributed {"claude"} else {"opencode"},"model":null,
            "surface":"headless","cwd":fixture.repo.path(),"repo":fixture.repo.path(),
            "worktree":fixture.repo.path(),"skill":"implement","subjects":subjects,
            "flow":{"kind":"independent"},"launch":null,"context":null,
            "runtime_path":null,"runtime_digest":null,"host":"fixture","boot_id":null
        });
        std::fs::write(
            dir.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        std::fs::write(dir.join("terminal.json"), serde_json::to_vec(&serde_json::json!({
            "schema_version":1,"outcome":outcome,"ended_at":(at+chrono::Duration::minutes(5)).to_rfc3339(),"result_ref":null
        })).unwrap()).unwrap();
        let measurement = json!({"schema_version":1,"seq":1,"observed_at":at.to_rfc3339(),
            "type":"usage","usage_stream_id":"repeated-local-stream","provider":manifest["harness"],
            "model":null,"attempt_key":"attempt-1","turn_key":"turn-1","observation_seq":1,
            "counter_kind":"cumulative","start_known":true,"final_receipt":attributed,
            "usage":{"input_tokens":usage["input"],"output_tokens":usage["output"]}});
        std::fs::write(
            dir.join("events.jsonl"),
            format!("{usage}\n{measurement}\n"),
        )
        .unwrap();
    }
    for dry in [true, false] {
        let args = if dry {
            vec!["session", "import", "--dry-run", "--json"]
        } else {
            vec!["session", "import", "--json"]
        };
        let report = fixture.json(&args);
        assert_eq!(report["failed"], json!([]), "{report}");
        assert_eq!(report["task_review"], 2, "{report}");
        assert_eq!(report["tasks_started"], json!([]));
        assert_eq!(fixture.count("session_events"), if dry { 2 } else { 10 });
        assert_eq!(started(), original_started);
        assert_eq!(
            runtime.block_on(task.store.session(&saved.id)).unwrap(),
            Some(saved.clone())
        );
    }
    let history = fixture.json(&["session", "history", &saved.id, "--json"]);
    let events = history.as_array().unwrap();
    for (input, task_id, outcome, tokens) in [
        (
            &prior,
            Value::Null,
            "failed",
            json!({"attempt":1,"account_id":"first","input":21,"output":null}),
        ),
        (
            &current,
            json!(task.task.id),
            "completed",
            json!({"attempt":1,"account_id":"second","input":0,"output":0}),
        ),
    ] {
        let member: Vec<_> = events
            .iter()
            .filter(|event| event["payload"]["input_id"] == json!(input))
            .collect();
        assert_eq!(member.len(), 4);
        assert!(member.iter().all(|event| event["task_id"] == task_id
            && event["exec_id"].is_null()
            && event["provider_turn"].is_null()
            && event["kind"] == "observed"));
        assert_eq!(
            member
                .iter()
                .find(|event| event["payload"]["source"] == "terminal.json")
                .unwrap()["payload"]["evidence"]["outcome"],
            outcome
        );
        assert_eq!(
            member
                .iter()
                .find(|event| event["payload"]["source"] == "events.jsonl:0")
                .unwrap()["payload"]["evidence"],
            tokens
        );
    }
    let again = fixture.json(&["session", "import", "--json"]);
    assert_eq!(again["unchanged"], 2, "{again}");
    assert_eq!(
        fixture.json(&["session", "history", &saved.id, "--json"]),
        history
    );
    for input in [&prior, &current] {
        std::fs::remove_dir_all(fixture.run_dir(input.as_str())).unwrap();
    }
    let all = fixture.json(&["usage", "--days", "0", "--json"]);
    let rows = all.as_array().unwrap();
    assert_eq!(rows.len(), 2, "{all}");
    let first = rows
        .iter()
        .find(|row| row["artifact_key"] == prior.as_str())
        .unwrap();
    let second = rows
        .iter()
        .find(|row| row["artifact_key"] == current.as_str())
        .unwrap();
    assert_eq!(first["usage"]["input_tokens"], 21);
    assert!(first["usage"]["output_tokens"].is_null());
    assert_eq!(first["task_id"], Value::Null);
    assert_eq!(first["wave_id"], Value::Null);
    assert_eq!(first["harness"], "opencode");
    assert_eq!(first["recorded_outcome"], "failed");
    assert_eq!(first["usage"]["final_streams"], 0);
    assert_eq!(second["usage"]["input_tokens"], 0);
    assert_eq!(second["usage"]["output_tokens"], 0);
    assert_eq!(second["harness"], "claude");
    assert_eq!(second["recorded_outcome"], "completed");
    assert_eq!(second["usage"]["final_streams"], 1);
    assert_eq!(second["task_identifier"], "INF-123");
    assert_eq!(second["work_source"], "inherited");
    assert_eq!(
        fixture.json(&["usage", "--days", "0", "--task", "INF-123", "--json"]),
        json!([second])
    );
    assert_eq!(
        fixture.json(&["usage", "--days", "7", "--json"]),
        json!([second]),
        "recent continuation survives old Session creation"
    );
    assert_eq!(fixture.json(&["runs", "--json"]), json!([second]));
    assert_eq!(
        fixture.json(&["runs", "--parent", caller.as_str(), "--json"]),
        all,
        "exact parent reads both inputs without a date cap"
    );
    assert_eq!(
        fixture.json(&["runs", "--parent", &caller.as_str()[..16], "--json"]),
        all
    );
    assert_eq!(fixture.count("agent_sessions"), 1);
    assert_eq!(fixture.captures(), 2);
    assert_eq!(fixture.retired_run_tables(), 0);
    assert!(fixture.launches().is_empty());
}

#[test]
fn imported_final_and_events_survive_artifact_removal() {
    use loopflow::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
    use serde_json::json;

    let fixture = Fixture::new(false);
    let input = uuid::Uuid::new_v4().simple().to_string();
    let dir = fixture.run_dir(input.as_str());
    std::fs::create_dir_all(&dir).unwrap();
    let at = "2026-09-29T00:00:00Z";
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema_version":1,"run_id":input,"parent_run_id":null,"created_at":at,
            "harness":"opencode","model":null,"surface":"headless",
            "cwd":fixture.repo.path(),"repo":fixture.repo.path(),"worktree":fixture.repo.path(),
            "skill":"implement","subjects":[],"flow":{"kind":"independent"},
            "launch":null,"context":null,"runtime_path":null,"runtime_digest":null,
            "host":"fixture","boot_id":null
        }))
        .unwrap(),
    )
    .unwrap();
    let message = |turn: &str, text: &str, phase: &str| ConversationEvent::ItemCompleted {
        turn_id: turn.into(),
        item: ConversationItem::Message {
            id: format!("{turn}-{phase}"),
            text: text.into(),
            phase: Some(phase.into()),
        },
    };
    let events: Vec<_> = [
        message("success", "still working", "commentary"),
        message("success", "retained conclusion", "final_answer"),
        ConversationEvent::TurnCompleted {
            turn_id: "success".into(),
            status: Lifecycle::Completed,
        },
        message("failed", "failed conclusion", "final_answer"),
        ConversationEvent::TurnCompleted {
            turn_id: "failed".into(),
            status: Lifecycle::Failed,
        },
        message("unfinished", "unconfirmed conclusion", "final_answer"),
    ]
    .into_iter()
    .enumerate()
    .map(|(seq, event)| {
        json!({
            "schema_version":1,"seq":seq,"observed_at":at,"type":"conversation","event":event
        })
    })
    .collect();
    std::fs::write(
        dir.join("events.jsonl"),
        events
            .iter()
            .map(|event| format!("{event}\n"))
            .collect::<String>(),
    )
    .unwrap();
    let imported = fixture.json(&["session", "import", "--json"]);
    assert_eq!(imported["failed"], json!([]), "{imported}");
    let session = fixture.session_row(input.as_str()).0;
    let again = fixture.json(&["session", "import", "--json"]);
    assert_eq!(again["unchanged"], 1, "{again}");
    std::fs::remove_dir_all(&dir).unwrap();
    for selector in [input.as_str(), &session] {
        let answer = fixture.run(&["runs", selector, "--final"]);
        assert!(answer.status.success(), "{answer:?}");
        assert_eq!(
            String::from_utf8(answer.stdout).unwrap(),
            "retained conclusion\n"
        );
        let output = fixture.run(&["runs", selector, "--events"]);
        assert!(output.status.success(), "{output:?}");
        let retained: Vec<Value> = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(retained, events);
    }
    assert!(!dir.exists());
    assert!(fixture.launches().is_empty());
}

#[test]
fn import_retains_sql_only_members_after_their_artifacts_are_missing() {
    use loopflow::session::{AgentSession, SessionKind, TitleSource};

    let fixture = Fixture::new(false);
    let prior = format!("run_{}", uuid::Uuid::new_v4().simple());
    let current = format!("run_{}", uuid::Uuid::new_v4().simple());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &loopflow::store::StorageConfig::sqlite(fixture.home.path().join("loopflow.db")),
        ))
        .unwrap();
    let saved = runtime.block_on(async {
        let first = AgentSession {
            captured: None,
            id: "sql-conversation".into(),
            artifact_key: prior.clone(),
            caller_artifact_key: None,
            input_published: true,
            cwd: fixture.repo.path().into(),
            skill: Some("implement".into()),
            provider: Some("opencode".into()),
            model: None,
            node: None,
            iterations: None,
            task_id: None,
            wave_id: None,
            flow_session_id: None,
            work_source: None,
            bound_at: None,
            kind: SessionKind::Conversation,
            interactive: false,
            repo: None,
            title: "Kept name".into(),
            title_source: TitleSource::Human,
            request: None,
            ready_summary: Some("Kept feedback".into()),
            completed_at: None,
            created_at: 10,
        };
        let mut next = store.create_session(first, None).await.unwrap();
        next.artifact_key = current.clone();
        next.provider = Some("claude".into());
        store
            .replace_session_input(next.captured, next)
            .await
            .unwrap()
    });
    // These immutable source payloads are the final-schema import boundary;
    // the populated development migration proves their extraction from old SQL.
    let seed = |input: &String, changes: Value| {
        let mut evidence = serde_json::json!({
            "id":input.as_str(),"session_id":null,"invocation_id":null,"task_id":null,"wave_id":null,
            "work_source":null,"created_at":70,"published":1,"cwd":fixture.repo.path().to_string_lossy(),
            "skill":null,"node":null,"iterations":null,"attempt":null,"provider":null,
            "model":null,"caller_run_id":null,"outcome":null,"ended_at":null
        });
        for (key, value) in changes.as_object().unwrap() {
            evidence[key] = value.clone();
        }
        fixture
            .db()
            .execute(
                "INSERT INTO import_evidence(source,selector,payload) VALUES('runs',?1,?2)",
                rusqlite::params![input.as_str(), evidence.to_string()],
            )
            .unwrap();
    };
    for (id, at, outcome, provider) in [
        (&prior, 10, "failed", "opencode"),
        (&current, 30, "completed", "claude"),
    ] {
        seed(
            id,
            serde_json::json!({"session_id":saved.id,"created_at":at,"skill":"implement","provider":provider,"outcome":outcome,"ended_at":at+5}),
        );
    }
    let unpublished = format!("run_{}", uuid::Uuid::new_v4().simple());
    seed(
        &unpublished,
        serde_json::json!({"session_id":saved.id,"created_at":50,"published":0}),
    );
    let standalone = format!("run_{}", uuid::Uuid::new_v4().simple());
    seed(
        &standalone,
        serde_json::json!({"created_at":60,"skill":"research","provider":"codex","caller_run_id":prior.as_str()}),
    );
    let mechanical = format!("run_{}", uuid::Uuid::new_v4().simple());
    let ambiguous_kind = format!("run_{}", uuid::Uuid::new_v4().simple());
    for (id, provider) in [(&mechanical, Some("loopflow")), (&ambiguous_kind, None)] {
        seed(
            id,
            serde_json::json!({"skill":"publish","provider":provider}),
        );
    }
    assert_eq!(fixture.retired_run_tables(), 0);
    let preview = fixture.json(&["session", "import", "--dry-run", "--json"]);
    let unresolved = preview["failed"].as_array().unwrap();
    assert_eq!(unresolved.len(), 3, "{preview}");
    for input in [&mechanical, &ambiguous_kind, &standalone] {
        assert!(unresolved
            .iter()
            .any(|failure| failure["reason"].as_str().unwrap().contains(input.as_str())));
    }
    assert_eq!(preview["run"], 3, "{preview}");
    assert_eq!(fixture.captures(), 2);
    assert_eq!(fixture.count("agent_sessions"), 1);
    assert_eq!(fixture.count("import_evidence"), 6);
    let imported = fixture.json(&["session", "import", "--json"]);
    assert_eq!(imported["failed"], preview["failed"]);
    assert_eq!(imported["run"], preview["run"]);
    assert_eq!(
        fixture.json(&["session", "import", "--json"])["unchanged"],
        3
    );
    assert_eq!(
        runtime.block_on(store.session(&saved.id)).unwrap(),
        Some(saved.clone())
    );
    for input in [&mechanical, &ambiguous_kind, &standalone] {
        assert!(runtime
            .block_on(store.session_for_artifact(input))
            .unwrap()
            .is_none());
        assert_eq!(
            fixture
                .db()
                .query_row(
                    "SELECT count(*) FROM import_evidence WHERE source='runs' AND selector=?1 AND historical_session_id IS NULL",
                    [input.as_str()],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }
    let history = fixture.json(&["session", "history", &saved.id, "--json"]);
    assert_eq!(
        history
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["kind"] == "observed")
            .count(),
        3
    );
    let partial = history
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["payload"]["input_id"] == unpublished.as_str())
        .unwrap();
    assert_eq!(partial["payload"]["evidence"]["published"], 0);
    assert_eq!(partial["payload"]["evidence"]["outcome"], Value::Null);
    assert_eq!(partial["payload"]["evidence"]["ended_at"], Value::Null);
    assert_eq!(partial["exec_id"], Value::Null);
    assert_eq!(partial["provider_turn"], Value::Null);
    let usage = fixture.json(&["usage", "--days", "0", "--json"]);
    let rows = usage.as_array().unwrap();
    assert_eq!(rows.len(), 2, "{usage}");
    for (id, at, outcome, provider) in [
        (&prior, 10, "failed", "opencode"),
        (&current, 30, "completed", "claude"),
    ] {
        let row = rows
            .iter()
            .find(|row| row["artifact_key"] == id.as_str())
            .unwrap();
        assert_eq!(row["observed_at"], at);
        assert_eq!(row["recorded_at"], at + 5);
        assert_eq!(row["recorded_outcome"], outcome);
        assert_eq!(row["harness"], provider);
        assert_eq!(row["usage"]["input_tokens"], Value::Null);
        assert_eq!(fixture.json(&["runs", id.as_str(), "--json"]), *row);
        assert_eq!(fixture.json(&["runs", &id.as_str()[4..16], "--json"]), *row);
        assert!(
            row["evidence_gaps"].as_u64().unwrap() > 0,
            "missing payload remains explicit"
        );
    }
    assert_eq!(
        fixture.json(&["runs", &saved.id, "--json"]),
        *rows
            .iter()
            .find(|row| row["artifact_key"] == current.as_str())
            .unwrap()
    );
    let ambiguous = fixture.run(&["runs", "run_", "--json"]);
    assert!(!ambiguous.status.success());
    assert!(String::from_utf8_lossy(&ambiguous.stderr).contains("ambiguous"));
    assert!(
        runtime
            .block_on(store.session(standalone.as_str()))
            .unwrap()
            .is_none(),
        "A provider label without conversation membership cannot invent a Session"
    );
    assert!(fixture.db().execute(
        "UPDATE import_evidence SET payload=json_set(payload,'$.outcome','interrupted') WHERE source='runs' AND selector=?1",
        [prior.as_str()]).is_err());
    let replay = fixture.json(&["session", "import", "--json"]);
    assert_eq!(replay["unchanged"], 3);
    assert_eq!(replay["failed"], preview["failed"]);
    assert_eq!(
        fixture.json(&["session", "history", &saved.id, "--json"]),
        history
    );
    assert!(fixture
        .db()
        .execute("DELETE FROM import_evidence WHERE source='runs'", [])
        .is_err());
    assert_eq!(fixture.json(&["usage", "--days", "0", "--json"]), usage);
    assert_eq!(fixture.count("agent_sessions"), 1);
    assert_eq!(fixture.count("import_evidence"), 6);
    assert!(fixture.launches().is_empty());
}

#[test]
fn import_preserves_unopened_and_finished_review_identity_and_feedback() {
    use loopflow::engine::{ConcreteSkill, ConcreteStep, ExecutionCursor, OccurrencePolicy, Skill};
    use serde_json::json;

    let fixture = Fixture::new(false);
    let review = ConcreteStep::Skill(ConcreteSkill {
        skill: Skill::named("review-design"),
        policy: OccurrencePolicy {
            human: true,
            ..Default::default()
        },
        flow_parents: vec![],
    });
    let autonomous = ConcreteStep::Skill(ConcreteSkill {
        skill: Skill::named("implement"),
        policy: OccurrencePolicy::default(),
        flow_parents: vec![],
    });
    let mut saved = Vec::new();
    for (opened, finished) in [(false, false), (true, false), (true, true)] {
        let flow = uuid::Uuid::new_v4().to_string();
        let boundary = uuid::Uuid::new_v4().to_string();
        let session = format!("flow:{flow}:{boundary}");
        let input = opened.then(|| format!("run_{}", uuid::Uuid::new_v4().simple()));
        if let Some(input) = &input {
            let dir = fixture.run_dir(input.as_str());
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("manifest.json"),
                serde_json::to_vec(&serde_json::json!({
                    "schema_version": 1, "run_id": input, "parent_run_id": null,
                    "created_at": "2026-09-20T20:00:00Z", "harness": "opencode", "model": null,
                    "surface": "tui", "cwd": fixture.repo.path(), "repo": fixture.repo.path(),
                    "worktree": fixture.repo.path(), "skill": "review-design", "subjects": [],
                    "flow": {"kind": "step", "task_id": null, "boundary_key": "0",
                        "invocation_id": flow, "flow": "design", "step": "review-design",
                        "node": "0", "iterations": [[]]},
                    "launch": null, "context": null, "runtime_path": null,
                    "runtime_digest": null, "host": "fixture", "boot_id": null
                }))
                .unwrap(),
            )
            .unwrap();
        }
        let path = fixture
            .home
            .path()
            .join("flows")
            .join(&flow)
            .join("position.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let value = json!({
            "id": flow, "flow": "design", "cwd": fixture.repo.path(),
            "steps": [review, autonomous],
            "cursor": ExecutionCursor { index: if finished { 2 } else { usize::from(opened) }, ..Default::default() },
            "message": null, "model": "opencode", "wave": null, "task": null, "as_work": null,
            "active": {"id": boundary, "run_id": input, "completed": opened,
                "ready_summary": "Keep the saved review feedback"},
            "failure": null, "finished": finished
        });
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        saved.push((path, value, session, input));
    }
    // An active agent or operation is not necessarily a human review.
    let autonomous_input = uuid::Uuid::new_v4().simple().to_string();
    let autonomous_flow = uuid::Uuid::new_v4().to_string();
    let dir = fixture.run_dir(autonomous_input.as_str());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1, "run_id": autonomous_input, "parent_run_id": null,
            "created_at": "2026-09-20T20:00:00Z", "harness": "opencode", "model": null,
            "surface": "headless", "cwd": fixture.repo.path(), "repo": fixture.repo.path(),
            "worktree": fixture.repo.path(), "skill": "implement", "subjects": [],
            "flow": {"kind": "step", "task_id": null, "boundary_key": "1",
                "invocation_id": autonomous_flow, "flow": "design", "step": "implement",
                "node": "1", "iterations": [[]]},
            "launch": null, "context": null, "runtime_path": null,
            "runtime_digest": null, "host": "fixture", "boot_id": null
        }))
        .unwrap(),
    )
    .unwrap();
    for (id, steps, input, index) in [
        (
            autonomous_flow.clone(),
            vec![review, autonomous],
            Some(autonomous_input.clone()),
            1,
        ),
        (
            uuid::Uuid::new_v4().to_string(),
            vec![ConcreteStep::Op(loopflow::engine::ConcreteOp {
                item: loopflow::engine::Op {
                    command: "pr publish".into(),
                    args: vec![],
                },
                flow_parents: vec![],
            })],
            None,
            0,
        ),
    ] {
        let path = fixture
            .home
            .path()
            .join("flows")
            .join(&id)
            .join("position.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&serde_json::json!({
            "id": id, "flow": "design", "cwd": fixture.repo.path(), "steps": steps,
            "cursor": ExecutionCursor { index, ..Default::default() },
            "message": null, "model": "opencode", "wave": null, "task": null, "as_work": null,
            "active": {"id": "active", "run_id": input, "completed": false, "ready_summary": null},
            "failure": null, "finished": false
        })).unwrap()).unwrap();
    }
    let preview = fixture.json(&["session", "import", "--dry-run", "--json"]);
    assert_eq!(preview["failed"], json!([]));
    assert_eq!(preview["flow_review"], 3, "{preview}");
    assert_eq!(preview["flow"], 2);
    assert_eq!(preview["run"], 1);
    assert_eq!(fixture.count("agent_sessions"), 0);
    assert_eq!(fixture.count("flow_sessions"), 0);
    let imported = fixture.json(&["session", "import", "--json"]);
    assert_eq!(imported["failed"], json!([]));
    assert_eq!(imported["flow_review"], preview["flow_review"]);
    let again = fixture.json(&["session", "import", "--json"]);
    assert_eq!(again["failed"], json!([]));
    assert_eq!(again["unchanged"], 6, "{again}");
    for (path, value, session, input) in &saved {
        let (stored_input, published, feedback, completed, node, iterations):
            (String, bool, String, Option<i64>, Option<i64>, Option<String>) = fixture.db().query_row(
                "SELECT (SELECT receipt_key FROM session_events WHERE seq=agent_sessions.current_capture),input_published,ready_summary,completed_at,node,iterations FROM agent_sessions WHERE id=?1",
                [session], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
            ).unwrap();
        assert_eq!(published, input.is_some());
        if let Some(input) = input {
            assert_eq!(stored_input, input.as_str());
        }
        assert_eq!(feedback, "Keep the saved review feedback");
        assert_eq!(completed.is_some(), input.is_some());
        assert_eq!(node, Some(0));
        assert_eq!(
            serde_json::from_str::<Value>(&iterations.unwrap()).unwrap(),
            json!([[]])
        );
        let pending: Option<String> = fixture
            .db()
            .query_row(
                "SELECT pending_session_id FROM flow_sessions WHERE id=?1",
                [value["id"].as_str().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pending.as_ref(), input.is_none().then_some(session));
        let mut conflict = value.clone();
        conflict["active"]["ready_summary"] = json!("A different answer");
        std::fs::write(path, serde_json::to_vec(&conflict).unwrap()).unwrap();
        let rejected = fixture.json(&["session", "import", "--json"]);
        assert_eq!(
            rejected["failed"].as_array().unwrap().len(),
            1,
            "{rejected}"
        );
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    let listed = fixture.json(&["session", "list", "--history", "--all", "--json"]);
    for (_, _, id, _) in &saved {
        assert!(
            listed
                .as_array()
                .unwrap()
                .iter()
                .any(|session| session["id"] == *id),
            "{listed}"
        );
    }
    assert_eq!(fixture.retired_run_tables(), 0);
    assert_eq!(fixture.count("agent_sessions"), 4);
    let (interactive, kind, flow, node): (bool, String, String, i64) = fixture
        .db()
        .query_row(
            "SELECT interactive,kind,flow_session_id,node FROM agent_sessions WHERE current_capture=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?1)",
            [autonomous_input.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert!(!interactive);
    assert_eq!(kind, "conversation");
    assert_eq!(flow, autonomous_flow);
    assert_eq!(node, 1);
    assert!(fixture.launches().is_empty());
}

#[test]
fn import_keeps_tui_closure_without_native_identity() {
    import_tui_closure(false);
}

#[test]
fn import_keeps_earlier_tui_review_closure_and_membership() {
    import_tui_closure(true);
}

fn import_tui_closure(flow_member: bool) {
    use loopflow::engine::{ConcreteSkill, ConcreteStep, ExecutionCursor, OccurrencePolicy, Skill};
    use serde_json::json;

    let fixture = Fixture::new(false);
    let input = uuid::Uuid::new_v4().simple().to_string().to_string();
    let invocation = uuid::Uuid::new_v4().to_string();
    let dir = fixture.run_dir(&input);
    let write = |path: PathBuf, value: Value| {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    };
    let flow = if flow_member {
        write(
            fixture
                .home
                .path()
                .join("flows")
                .join(&invocation)
                .join("position.json"),
            json!({
                "id": invocation, "flow": "historical", "cwd": fixture.repo.path(),
                "steps": [ConcreteStep::Skill(ConcreteSkill {
                    skill: Skill::named("review-design"),
                    policy: OccurrencePolicy { human: true, ..Default::default() },
                    flow_parents: vec![]
                })],
                "cursor": ExecutionCursor { index: 1, ..Default::default() },
                "message": null, "model": "opencode", "wave": null,
                "task": null, "as_work": null, "active": null,
                "failure": null, "finished": true
            }),
        );
        json!({"kind": "step", "task_id": null, "boundary_key": "0",
            "invocation_id": invocation, "flow": "historical", "step": "review-design",
            "node": "0", "iterations": [[]]})
    } else {
        json!({"kind": "independent"})
    };
    write(
        dir.join("manifest.json"),
        json!({
            "schema_version": 1, "run_id": input, "parent_run_id": null,
            "created_at": "2026-09-20T20:00:00Z", "harness": "opencode", "model": null,
            "surface": "tui", "cwd": fixture.repo.path(), "repo": fixture.repo.path(),
            "worktree": fixture.repo.path(), "skill": "review-design", "subjects": [],
            "flow": flow, "launch": null, "context": null, "runtime_path": null,
            "runtime_digest": null, "host": "fixture", "boot_id": null
        }),
    );
    write(
        dir.join("session-resolution.json"),
        json!({
            "schema_version": 1, "resolved_at": "2026-09-20T20:05:00Z"
        }),
    );
    let report = fixture.json(&["session", "import", "--json"]);
    assert_eq!(report["failed"], json!([]), "{report}");
    let facts = || {
        fixture.db().query_row(
        "SELECT interactive,completed_at,flow_session_id,node,iterations FROM agent_sessions WHERE id=?1",
        [&input], |row| Ok((row.get::<_,bool>(0)?,row.get::<_,Option<i64>>(1)?,
            row.get::<_,Option<String>>(2)?,row.get::<_,Option<i64>>(3)?,
            row.get::<_,Option<String>>(4)?))).unwrap()
    };
    let expected = (
        true,
        Some(1_789_934_700),
        flow_member.then(|| invocation.clone()),
        flow_member.then_some(0),
        flow_member.then(|| "[[]]".to_string()),
    );
    assert_eq!(
        facts(),
        expected,
        "surface and closure are independent of native identity and current cursor"
    );
    let history = fixture.json(&["session", "history", &input, "--json"]);
    let resolution = history
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["payload"]["source"] == "session-resolution.json")
        .unwrap();
    assert_eq!(resolution["observed_at"], 1_789_934_700);
    assert!(
        history
            .as_array()
            .unwrap()
            .iter()
            .all(|event| event["kind"] == "observed" || event["kind"] == "captured"),
        "closure cannot invent native success"
    );
    let again = fixture.json(&["session", "import", "--json"]);
    assert_eq!(again["failed"], json!([]));
    assert_eq!(fixture.count("agent_sessions"), 1);
    assert_eq!(facts(), expected);
    std::fs::remove_dir_all(&dir).unwrap();
    if flow_member {
        std::fs::remove_dir_all(fixture.home.path().join("flows")).unwrap();
    }
    let listed = fixture.json(&["session", "list", "--history", "--all", "--json"]);
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["id"], input);
    assert_eq!(listed[0]["interactive"], true);
    assert_eq!(listed[0]["state"], "closed");
    assert_eq!(
        fixture.json(&["session", "history", &input, "--json"]),
        history
    );
    assert!(!dir.exists());
    assert!(fixture.launches().is_empty());
}

/// One Session of each origin, as an old Home kept them in files.
#[test]
fn import_retains_autonomous_and_finished_captures_and_rejects_changed_graphs() {
    use loopflow::engine::{ConcreteSkill, ConcreteStep, ExecutionCursor, OccurrencePolicy, Skill};
    use serde_json::json;

    let fixture = Fixture::new(false);
    let step = ConcreteStep::Skill(ConcreteSkill {
        skill: Skill {
            content: Some("Retained source".into()),
            ..Skill::named("implement")
        },
        policy: OccurrencePolicy::default(),
        flow_parents: vec![],
    });
    let mut paths = Vec::new();
    for finished in [false, true] {
        let id = uuid::Uuid::new_v4().to_string();
        let path = fixture
            .home
            .path()
            .join("flows")
            .join(&id)
            .join("position.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let cursor = ExecutionCursor {
            index: usize::from(finished),
            ..Default::default()
        };
        let failure = (!finished).then(|| {
            json!({
                "run_id": null, "reason": "provider failed", "restart_required": false,
                "observed_at": "2026-09-20T20:05:00Z"
            })
        });
        let saved = json!({
            "id": id, "flow": "captured", "cwd": fixture.repo.path(), "steps": [step],
            "cursor": cursor, "message": "Keep original input", "model": "opencode",
            "wave": null, "task": null, "as_work": null, "active": null,
            "failure": failure, "finished": finished
        });
        std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
        paths.push((path, saved));
    }
    let planned = fixture.json(&["session", "import", "--dry-run", "--json"]);
    assert_eq!(planned["flow"], 2, "{planned}");
    assert_eq!(fixture.count("flow_sessions"), 0);
    let imported = fixture.json(&["session", "import", "--json"]);
    assert_eq!(imported["flow"], 2, "{imported}");
    assert_eq!(imported["failed"], json!([]));
    let again = fixture.json(&["session", "import", "--json"]);
    assert_eq!(again["unchanged"], 2, "{again}");
    for (path, saved) in &paths {
        let (graph, cursor, state, failure): (String, String, String, Option<String>) = fixture.db().query_row(
            "SELECT invocation_json,review_json,state,failure_json FROM flow_sessions WHERE id=?1",
            [saved["id"].as_str().unwrap()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&graph).unwrap()["steps"],
            saved["steps"]
        );
        assert_eq!(
            serde_json::from_str::<Value>(&cursor).unwrap(),
            saved["cursor"]
        );
        assert_eq!(
            state,
            if saved["finished"] == true {
                "completed"
            } else {
                "current"
            }
        );
        assert_eq!(
            failure.map(|s| serde_json::from_str::<Value>(&s).unwrap()),
            saved["failure"].is_object().then(|| {
                let mut failure = saved["failure"].clone();
                failure.as_object_mut().unwrap().remove("run_id");
                failure["captured"] = Value::Null;
                failure
            })
        );
        let mut changed = saved.clone();
        changed["steps"][0] = json!(ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named("different"),
            policy: OccurrencePolicy::default(),
            flow_parents: vec![],
        }));
        std::fs::write(path, serde_json::to_vec(&changed).unwrap()).unwrap();
        let rejected = fixture.json(&["session", "import", "--json"]);
        assert!(
            !rejected["failed"].as_array().unwrap().is_empty(),
            "{rejected}"
        );
        let retained: String = fixture
            .db()
            .query_row(
                "SELECT invocation_json FROM flow_sessions WHERE id=?1",
                [saved["id"].as_str().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retained, graph);
        std::fs::remove_file(path).unwrap();
    }
    assert_eq!(fixture.count("agent_sessions"), 0);
    assert_eq!(
        fixture.count("session_events"),
        0,
        "import invents no native turns"
    );
    let fabricated: i64 = fixture
        .db()
        .query_row(
            "SELECT count(*) FROM execs WHERE caller_session_id IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        fabricated, 0,
        "import commands are actual Execs, never old agent processes"
    );
    assert!(fixture.launches().is_empty());
}

/// One Session of each origin, as an old Home kept them in files.
#[test]
fn import_stores_each_old_session_once_with_its_name() {
    use loopflow::durable::FlowSession;
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
        let id = uuid::Uuid::new_v4().simple().to_string().to_string();
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
        sources.borrow_mut().push(write(
            dir.join("provider-session.json"),
            json!({"schema_version": 1, "provider_session_id": format!("ses_{id}"), "account_id": null}),
        ));
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
                FlowSession {
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
        let session = loopflow::session::AgentSession {
            captured: None,
            caller_artifact_key: None,
            task_id: Some(task.task.id.clone()),
            wave_id: Some(task.task.wave_id.clone()),
            flow_session_id: Some(parked.id().to_owned()),
            work_source: Some(loopflow::session::WorkSource::Inherited),
            bound_at: None,
            id: review_id,
            artifact_key: str::parse::<String>(&task_review).unwrap(),
            input_published: true,
            cwd: task.task.worktree.clone(),
            skill: Some("review-design".into()),
            provider: Some("opencode".into()),
            model: None,
            node: None,
            iterations: None,
            kind: loopflow::session::SessionKind::FlowReview,
            interactive: true,
            repo: None,
            title: task.task.plan.title.clone(),
            title_source: loopflow::session::TitleSource::Generated,
            request: None,
            ready_summary: None,
            completed_at: None,
            created_at: 1,
        };
        task.store
            .create_session(session, Some(parked))
            .await
            .unwrap();
    });
    // Old Runs outside any Session had no rows: a settled headless Run a
    // conversation launched, and an earlier step of a Flow.
    let settled = |flow: Value, subjects: Value, parent: Value| -> String {
        let id = uuid::Uuid::new_v4().simple().to_string().to_string();
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
    let past_flow = uuid::Uuid::new_v4().to_string();
    sources.borrow_mut().push(write(
        home.join("flows").join(&past_flow).join("position.json"),
        json!({
            "id": past_flow, "flow": "historical", "cwd": sibling.worktree,
            "steps": [ConcreteStep::Skill(ConcreteSkill {
                skill: Skill::named("implement"), policy: OccurrencePolicy::default(), flow_parents: vec![]
            })],
            "cursor": ExecutionCursor { index: 1, ..Default::default() },
            "message": null, "model": "opencode", "wave": null,
            "task": sibling.id, "as_work": null, "active": null,
            "failure": null, "finished": true
        }),
    ));
    let step = settled(
        json!({
            "kind": "step", "task_id": null, "boundary_key": "0", "invocation_id": past_flow,
            "flow": "historical", "step": "implement", "node": "0", "iterations": [[]]
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
        let mut sessions = fixture.sessions();
        sessions.extend(
            serde_json::from_value::<Vec<Value>>(fixture.json(&[
                "session",
                "list",
                "--all",
                "--interactive",
                "false",
                "--json",
            ]))
            .unwrap(),
        );
        let mut names: Vec<_> = sessions
            .iter()
            .map(|session| {
                (
                    session["title"].as_str().unwrap().to_string(),
                    session["kind"].as_str().unwrap().to_string(),
                    fixture.artifact(session["id"].as_str().unwrap()),
                )
            })
            .collect();
        names.sort();
        let both: Vec<Value> = serde_json::from_value(fixture.json(&[
            "session",
            "list",
            "--all",
            "--interactive",
            "all",
            "--limit",
            "0",
            "--json",
        ]))
        .unwrap();
        let mut combined: Vec<_> = both
            .iter()
            .map(|session| {
                (
                    session["title"].as_str().unwrap().to_string(),
                    session["kind"].as_str().unwrap().to_string(),
                    fixture.artifact(session["id"].as_str().unwrap()),
                )
            })
            .collect();
        combined.sort();
        assert_eq!(combined, names);
        names
    };
    assert_eq!(
        names().len(),
        1,
        "before the import only the stored Task review lists"
    );

    let started = || {
        fixture
            .db()
            .query_row(
                "SELECT started_at FROM tasks WHERE id=?1",
                [task.task.id.as_str()],
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap()
    };
    let original_started = started();
    assert!(original_started.is_some());
    let planned = fixture.json(&["session", "import", "--dry-run", "--json"]);
    assert_eq!(
        fixture.count("agent_sessions"),
        1,
        "a dry run stores nothing"
    );
    assert_eq!(
        fixture.count("flow_sessions"),
        1,
        "dry-run writes no captured graph"
    );
    assert_eq!(
        fixture.count("session_events"),
        1,
        "dry-run preserves the existing capture without writing history"
    );
    let first = fixture.json(&["session", "import", "--json"]);
    assert_eq!(started(), original_started);
    for report in [&planned, &first] {
        for (kind, count) in [
            ("interactive", 1),
            ("ask", 1),
            ("flow_review", 1),
            ("flow", 1),
            ("task_review", 1),
            ("run", 2),
            ("unchanged", 0),
        ] {
            assert_eq!(report[kind], count, "{kind}: {report}");
        }
        assert_eq!(report["tasks_started"], json!([sibling.id]), "{report}");
        let failed = report["failed"].as_array().unwrap();
        assert_eq!(failed.len(), 1, "{report}");
        assert_eq!(
            std::path::Path::new(failed[0]["path"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            broken.canonicalize().unwrap()
        );
        assert!(!failed[0]["reason"].as_str().unwrap().is_empty());
    }
    let again = fixture.json(&["session", "import", "--json"]);
    for (kind, count) in [
        ("interactive", 0),
        ("ask", 0),
        ("flow_review", 0),
        ("task_review", 0),
        ("run", 0),
        ("unchanged", 7),
    ] {
        assert_eq!(again[kind], count, "{kind}: {again}");
    }
    assert_eq!(again["tasks_started"], json!([]));

    let expected = {
        let mut expected = vec![
            ("Design review".to_string(), "flow".to_string(), reviewed),
            (
                "Parser review".to_string(),
                "conversation".to_string(),
                interactive.clone(),
            ),
            ("Release target".to_string(), "ask".to_string(), asked),
            ("Task review".to_string(), "flow".to_string(), task_review),
            (
                "implement".to_string(),
                "conversation".to_string(),
                headless.clone(),
            ),
            (
                "implement".to_string(),
                "conversation".to_string(),
                step.clone(),
            ),
        ];
        expected.sort();
        expected
    };
    assert_eq!(names(), expected);
    assert_eq!(fixture.count("agent_sessions"), 6);
    assert_eq!(
        fixture.retired_run_tables(),
        0,
        "import does not recreate Run owners"
    );
    let membership =
        |input: &str| -> (Option<String>, Option<i64>, Option<String>, Option<String>) {
            fixture.db().query_row(
            "SELECT s.flow_session_id,s.node,s.iterations,json_extract(i.payload,'$.caller_key') FROM agent_sessions s
             JOIN session_events i ON i.seq=s.current_capture AND i.kind='captured' WHERE (SELECT receipt_key FROM session_events WHERE seq=s.current_capture)=?1", [input],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap()
        };
    assert_eq!(
        membership(&headless),
        (None, None, None, Some(interactive.clone()))
    );
    assert_eq!(
        membership(&step),
        (Some(past_flow), Some(0), Some("[[]]".into()), None),
        "recorded membership survives the later cursor"
    );
    for (input, task_id, source) in [
        (&headless, None, "declared"),
        (&step, Some(sibling.id.to_string()), "inherited"),
        (&interactive, Some(sibling.id.to_string()), "declared"),
    ] {
        let parents: (Option<String>, String, String) = fixture
            .db()
            .query_row(
                "SELECT task_id,wave_id,work_source FROM agent_sessions WHERE current_capture=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?1)",
                [input],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            parents,
            (task_id, task.task.wave_id.to_string(), source.into())
        );
    }
    let history = fixture.json(&["session", "history", &headless, "--json"]);
    let terminal = history
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["payload"]["source"] == "terminal.json")
        .unwrap();
    assert_eq!(terminal["payload"]["evidence"]["outcome"], "failed");
    assert_eq!(terminal["observed_at"], 1_789_934_700);
    assert_eq!(terminal["kind"], "observed");
    assert!(terminal["provider_turn"].is_null() && terminal["exec_id"].is_null());
    let mut sessions = Vec::new();
    for mode in ["true", "false"] {
        sessions.extend(
            serde_json::from_value::<Vec<Value>>(fixture.json(&[
                "session",
                "list",
                "--all",
                "--interactive",
                mode,
                "--task",
                "INF-124",
                "--json",
            ]))
            .unwrap(),
        );
    }
    let mut listed: Vec<String> = sessions
        .iter()
        .map(|session| fixture.artifact(session["id"].as_str().unwrap()))
        .collect();
    listed.sort();
    let mut expected_inputs = vec![interactive.clone(), step.clone()];
    expected_inputs.sort();
    assert_eq!(listed, expected_inputs);
    let feedback: String = fixture
        .db()
        .query_row(
            "SELECT ready_summary FROM agent_sessions WHERE id=?1",
            [format!("flow:{invocation}:{boundary}")],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(feedback, "Keep the old parser");

    let terminal_path = fixture.run_dir(&headless).join("terminal.json");
    let original_terminal = std::fs::read(&terminal_path).unwrap();
    let mut changed: Value = serde_json::from_slice(&original_terminal).unwrap();
    changed["outcome"] = json!("completed");
    std::fs::write(&terminal_path, serde_json::to_vec(&changed).unwrap()).unwrap();
    let conflict = fixture.json(&["session", "import", "--json"]);
    assert!(
        conflict["failed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure["reason"]
                .as_str()
                .unwrap()
                .contains("conflicting terminal.json evidence")),
        "{conflict}"
    );
    assert_eq!(
        fixture.json(&["session", "history", &headless, "--json"]),
        history,
        "a conflicting replay cannot rewrite retained failure evidence"
    );
    std::fs::write(&terminal_path, original_terminal).unwrap();
    assert!(
        fixture.launches().is_empty(),
        "offline import never starts a provider"
    );

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
    let task_path = fixture.repo.create_named_worktree("task-binding");
    let task = support::register_unrun_task(
        fixture.home.path(),
        &task_path,
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
                "SELECT s.flow_session_id,i.session_id,json_extract(i.payload,'$.caller_key')
                 FROM session_events i JOIN agent_sessions s ON s.id=i.session_id AND i.kind='captured'
                 WHERE i.receipt_key=?1",
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
    assert_eq!(
        membership(&of_wave),
        (None, Some(fixture.session_row(&of_wave).0), None)
    );

    // A Task conversation and a child agent it launches. The
    // child carries the explicit --as declaration, independently of its parent.
    let of_task = launch(&mut headless(&[
        "--task",
        "INF-123",
        "--__cwd",
        fixture.repo.path().to_str().unwrap(),
    ]));
    assert_eq!(
        fixture.run_parents(&of_task),
        (
            Some(task_id.clone()),
            Some(wave_id.clone()),
            declared.clone()
        )
    );
    let store = loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("loopflow.db"))
        .unwrap();
    let parent_session = fixture.session_row(&of_task).0;
    let caller = store
        .session_driver(&parent_session)
        .unwrap()
        .unwrap()
        .caller(parent_session);
    let child = launch(
        fixture
            .command(&HEADLESS)
            .env("LF_AS", format!("task:{task_id}"))
            .env("LF_AGENT_CALLER", serde_json::to_string(&caller).unwrap())
            .env("LF_RUN_ID", &of_task)
            .env("LF_RUN_DIR", fixture.run_dir(&of_task)),
    );
    assert_eq!(
        fixture.run_parents(&child),
        (
            Some(task_id.clone()),
            Some(wave_id.clone()),
            Some("declared".to_string())
        )
    );
    assert_eq!(
        membership(&child),
        (
            None,
            Some(fixture.session_row(&child).0),
            Some(of_task.clone())
        )
    );

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
            "SELECT (SELECT receipt_key FROM session_events WHERE seq=agent_sessions.current_capture),flow_session_id FROM agent_sessions WHERE kind='flow_review'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        membership(&step),
        (
            Some(invocation.clone()),
            Some(fixture.session_row(&step).0),
            None
        )
    );
    assert_eq!(
        fixture.run_parents(&step),
        // The command names the Wave; conversations inherit it from their Flow.
        (None, Some(wave_id.clone()), Some("inherited".to_string()))
    );
    assert_eq!(fixture.run_parents(&review), fixture.run_parents(&step));
    let positions = invocation_inputs(&fixture, &invocation);
    assert_eq!(positions.len(), 2, "{positions:?}");
    assert_eq!((&positions[0].0, positions[0].2), (&step, 1));
    assert_eq!((&positions[1].0, positions[1].2), (&review, 1));
    assert_ne!(positions[0].1, positions[1].1, "each step is its own node");

    // Each input retains its own completion in conversation history.
    for run in [&of_wave, &of_task, &child, &step] {
        let (outcome, ended): (Option<String>, Option<i64>) = fixture
            .db()
            .query_row(
                "SELECT json_extract(payload,'$.evidence.outcome'),observed_at
                 FROM session_events WHERE kind='observed' AND receipt_key=?1||':terminal.json'",
                [run],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(outcome.as_deref(), Some("completed"), "{run}");
        assert!(ended.is_some());
    }

    // One input per launch, and every reader lists each input once.
    assert_eq!(fixture.captures(), 5);
    assert_eq!(fixture.retired_run_tables(), 0);
    let listed =
        |args: &[&str]| -> Vec<Value> { serde_json::from_value(fixture.json(args)).unwrap() };
    let ids = |runs: &[Value]| -> Vec<String> {
        let mut ids: Vec<String> = runs
            .iter()
            .map(|run| run["artifact_key"].as_str().unwrap().to_string())
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
        assert_eq!(run["wave_name"], "task-pr-tests", "{run}");
        assert_eq!(
            run["task_identifier"] == "INF-123",
            [&of_task, &child].contains(&&run["artifact_key"].as_str().unwrap().to_string()),
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
    assert_eq!(children[0]["caller_artifact_key"], of_task.as_str());

    let activity = fixture.json(&["activity", "--wave", "task-pr-tests", "--json"]);
    let mut captured: Vec<_> = activity["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["fact"]["kind"] == "input_captured")
        .map(|item| {
            (
                item["fact"]["session_id"].as_str().unwrap().to_owned(),
                item["fact"]["captured"].as_i64().unwrap(),
            )
        })
        .collect();
    let mut expected: Vec<_> = of_wave_runs
        .iter()
        .map(|row| {
            (
                row["session_id"].as_str().unwrap().to_owned(),
                row["captured"].as_i64().unwrap(),
            )
        })
        .collect();
    captured.sort();
    expected.sort();
    assert_eq!(captured, expected);

    let sessions = fixture.sessions();
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].get("run_id").is_none());
}

/// A native provider fixture that returns a schema-constrained final result.
fn saved_flow_stand_in(fixture: &Fixture) {
    std::fs::write(fixture.home.path().join("decide-enabled"), "").unwrap();
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

/// (input, node, ordinal at that node, outcome, Task) from conversation history.
type AttemptRow = (String, i64, i64, Option<String>, Option<String>);

/// Retain earlier inputs independently of the conversation's current selection.
fn invocation_inputs(fixture: &Fixture, invocation: &str) -> Vec<AttemptRow> {
    fixture
        .db()
        .prepare(
            "WITH inputs AS (
                SELECT i.receipt_key AS input_id,
                    CAST(coalesce(json_extract(m.payload,'$.evidence.flow.node'),s.node) AS INTEGER) AS node,
                    coalesce(m.seq,9223372036854775807) AS sequence,
                    json_extract(t.payload,'$.evidence.outcome') AS outcome,
                    CASE WHEN m.seq IS NOT NULL THEN m.task_id ELSE s.task_id END AS task_id
                FROM session_events i JOIN agent_sessions s ON s.id=i.session_id AND i.kind='captured'
                LEFT JOIN session_events m ON m.session_id=s.id AND m.kind='observed'
                    AND m.receipt_key=i.receipt_key||':manifest.json'
                LEFT JOIN session_events t ON t.session_id=s.id AND t.kind='observed'
                    AND t.receipt_key=i.receipt_key||':terminal.json'
                WHERE s.flow_session_id=?1)
             SELECT input_id,node,row_number() OVER(PARTITION BY node ORDER BY sequence),outcome,task_id
             FROM inputs ORDER BY node,sequence",
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
    fixture.repo.create_branch("task-row-flow");
    saved_flow_stand_in(&fixture);
    let task = support::register_unrun_task(
        fixture.home.path(),
        fixture.repo.path(),
        "task-row-flow",
        &fixture.repo.head_sha(),
    );
    let task_id = task.task.id.to_string();
    // Claude supports the checkout boundary. Its native process is simulated;
    // the public lf command, account route and Session/Flow writes are real.
    let provider = fixture.home.path().join("bin/claude");
    std::fs::write(
        &provider,
        r#"#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

if "--version" in sys.argv:
    print("fixture Claude")
    raise SystemExit(0)
home = Path(__file__).resolve().parent.parent
with (home / "launched").open("a") as output:
    output.write(os.environ["LF_RUN_ID"] + "\n")
print(json.dumps({"type": "system", "subtype": "init", "session_id": "fixture-conversation"}))
failure = home / "fail-once"
failed = failure.exists()
failure.unlink(missing_ok=True)
print(json.dumps({"type": "result", "subtype": "error_during_execution" if failed else "success",
                  "is_error": failed, "result": "fixture failure" if failed else "Fixture complete",
                  "session_id": "fixture-conversation"}))
raise SystemExit(1 if failed else 0)
"#,
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let account_home = fixture.home.path().join("accounts/claude/fixture");
    std::fs::create_dir_all(&account_home).unwrap();
    std::fs::write(
        account_home.join(".credentials.json"),
        r#"{"claudeAiOauth":{"accessToken":"synthetic-fixture-token","expiresAt":4102444800000}}"#,
    )
    .unwrap();
    let store = loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("loopflow.db"))
        .unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let account_id = loopflow::store::ProviderAccountId::parse("fixture").unwrap();
    store
        .upsert_provider_account(&loopflow::store::ProviderAccount {
            provider: "claude".into(),
            account_id: account_id.clone(),
            home: Some(account_home),
            login_email: None,
            credential_state: loopflow::store::CredentialState::Connected,
            routing_state: loopflow::store::RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: now,
            updated_at: now,
        })
        .unwrap();
    store
        .set_provider_route(&loopflow::profile::ProviderRoute {
            scope: loopflow::profile::RouteScope::Default,
            provider: loopflow::provider_auth::Provider::Claude,
            accounts: vec![account_id],
            created_at: now,
            updated_at: now,
        })
        .unwrap();
    std::fs::write(fixture.home.path().join("fail-once"), "").unwrap();
    let blocked = fixture.run(&[
        "--task",
        "INF-123",
        "--model",
        "claude",
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
            "SELECT f.id, f.failure_json, t.current_invocation_id FROM flow_sessions f
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
    assert!(failure.contains("work-proof"), "{failure}; {stderr}");
    assert_eq!(pointer, None, "a Flow about the Task is not its Flow");
    let failed = invocation_inputs(&fixture, &invocation);
    assert_eq!(failed.len(), 1, "{failed:?}");
    assert_eq!(
        (failed[0].2, failed[0].3.as_deref(), failed[0].4.as_deref()),
        (1, Some("failed"), Some(task_id.as_str()))
    );
    let headless = fixture.json(&[
        "session",
        "list",
        "--all",
        "--interactive",
        "false",
        "--json",
    ]);
    assert_eq!(headless.as_array().unwrap().len(), 1);
    let conversation = headless[0]["id"].as_str().unwrap().to_string();
    assert!(headless[0].get("run_id").is_none());
    assert_eq!(headless[0]["flow_membership"]["invocation_id"], invocation);
    fixture.json(&[
        "session",
        "rename",
        &conversation,
        "Parser investigation",
        "--json",
    ]);

    let waiting = fixture
        .command(&["flow", "resume", &invocation, "--retry"])
        .env("LF_AS", format!("task:{task_id}"))
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&waiting.stderr).contains("waiting for human input"),
        "{waiting:?}"
    );
    no_position_file();
    let runs = invocation_inputs(&fixture, &invocation);
    assert_eq!(runs.len(), 3, "{runs:?}");
    assert_eq!(runs[0].0, failed[0].0);
    assert_eq!(
        (runs[1].1, runs[1].2, runs[1].3.as_deref()),
        (runs[0].1, 2, Some("completed"))
    );
    assert_eq!((runs[2].2, runs[2].3.as_deref()), (1, None));
    assert_ne!(runs[2].1, runs[1].1, "the review is its own node");
    let headless = fixture.json(&[
        "session",
        "list",
        "--all",
        "--interactive",
        "false",
        "--json",
    ]);
    assert_eq!(headless.as_array().unwrap().len(), 1);
    assert_eq!(headless[0]["id"], conversation);
    assert_eq!(headless[0]["title"], "Parser investigation");
    assert!(headless[0].get("run_id").is_none());
    for run in &runs {
        assert_eq!(run.4.as_deref(), Some(task_id.as_str()), "{run:?}");
    }
    let review = runs[2].0.clone();
    let listed: Vec<Value> =
        serde_json::from_value(fixture.json(&["runs", "--task", "INF-123", "--json"])).unwrap();
    let mut listed: Vec<&str> = listed
        .iter()
        .map(|run| run["artifact_key"].as_str().unwrap())
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
    assert!(session.get("run_id").is_none());
    assert_eq!(
        session["work"],
        serde_json::json!({"kind": "task", "id": task_id})
    );
    assert_eq!(session["flow_membership"]["flow"], "work-then-review");
    assert_eq!(session["flow_membership"]["node"], 1);

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
            "SELECT state FROM flow_sessions WHERE id=?1",
            [&invocation],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "completed");
    no_position_file();
    assert_eq!(invocation_inputs(&fixture, &invocation).len(), 3);
    assert_eq!(fixture.retired_run_tables(), 0);
    let launches = fixture.launches();
    let repeated = fixture.run(&["flow", "resume", &invocation]);
    assert!(repeated.status.success(), "{repeated:?}");
    assert_eq!(fixture.launches(), launches);
}

#[test]
fn taskless_structured_output_correction_is_bounded_and_preserves_the_conversation() {
    for exhausted in [false, true] {
        let fixture = Fixture::new(false);
        saved_flow_stand_in(&fixture);
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
            "--model",
            "opencode",
            "flow",
            "work-then-decide",
            "-b",
            "--no-loopflow",
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
        let (starts, consumed): (i64,i64) = fixture.db().query_row(
            "SELECT (SELECT count(*) FROM session_events WHERE kind='started'),(SELECT count(*) FROM flow_events WHERE kind='consumed')", [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(starts, if exhausted { 4 } else { 3 });
        assert_eq!(consumed, if exhausted { 1 } else { 2 });
        if exhausted {
            assert!(String::from_utf8_lossy(&output.stderr).contains("exhausted after 3"));
        }
    }
}

/// A taskless step consumes its selected native structured output once.
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
        log.lines().count(),
        1,
        "only the decision step has an output contract"
    );
    assert_eq!(
        serde_json::from_str::<Value>(log.trim()).unwrap()["decision"],
        "advance"
    );
    let (state, task, cursor): (String, Option<String>, String) = fixture
        .db()
        .query_row(
            "SELECT state, task_id, review_json FROM flow_sessions",
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
    let receipts: (i64, i64, i64) = fixture
        .db()
        .query_row(
            "SELECT (SELECT count(*) FROM session_events WHERE kind='started'),
                (SELECT count(*) FROM session_events WHERE kind='completed'),
                (SELECT count(*) FROM flow_events WHERE kind='consumed')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        receipts,
        (2, 2, 2),
        "each boundary consumes its exact native completion"
    );
    assert_eq!(fixture.retired_run_tables(), 0);
    for (index, input) in fixture.launches().iter().enumerate() {
        let answer = fixture.run(&["runs", input, "--final"]);
        assert!(answer.status.success(), "{answer:?}");
        if index == 0 {
            assert_eq!(
                String::from_utf8_lossy(&answer.stdout).trim(),
                "Fixture completed."
            );
        } else {
            assert_eq!(
                serde_json::from_slice::<Value>(&answer.stdout).unwrap()["decision"],
                "advance"
            );
        }
    }

    assert!(std::fs::read_dir(fixture.home.path().join("flows"))
        .map(|entries| entries.flatten().all(|entry| {
            std::fs::read_dir(entry.path())
                .unwrap()
                .flatten()
                .all(|file| file.file_name() == "driver.lock")
        }))
        .unwrap_or(true));
}

#[test]
fn custom_router_returns_a_captured_path_without_an_in_turn_command() {
    let fixture = Fixture::new(false);
    saved_flow_stand_in(&fixture);
    std::fs::write(fixture.repo.path().join(".lf/flows/choose.yaml"),
        "- xor:\n    router: decide-proof\n    paths:\n      alpha:\n        description: Do the selected work\n        skill: work-proof\n      zeta:\n        description: Leave this path untouched\n        skill: review-proof\n").unwrap();
    let output = fixture.run(&[
        "--model",
        "opencode",
        "flow",
        "choose",
        "-b",
        "--no-loopflow",
    ]);
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
    let (state, consumed): (String, i64) = fixture.db().query_row(
        "SELECT state,(SELECT count(*) FROM flow_events WHERE kind='consumed') FROM flow_sessions", [],
        |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
    assert_eq!((state.as_str(), consumed), ("completed", 2));
}

#[test]
fn public_taskless_flow_records_distinct_completed_loop_passes() {
    let fixture = Fixture::new(false);
    saved_flow_stand_in(&fixture);
    std::fs::write(fixture.home.path().join("remaining-passes"), "2").unwrap();
    let output = fixture.run(&[
        "--model",
        "opencode",
        "flow",
        "work-then-decide",
        "-b",
        "--no-loopflow",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let conn = fixture.db();
    let (root, state, iteration, count): (String, String, i64, i64) = conn
        .query_row(
            "SELECT id,state,iteration,(SELECT count(*) FROM flow_sessions) FROM flow_sessions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!((state.as_str(), iteration, count), ("completed", 2, 1));
    let positions: i64 = conn.query_row(
        "SELECT count(DISTINCT iterations) FROM flow_events WHERE kind='consumed' AND flow_id=?1",
        [&root], |row| row.get(0)).unwrap();
    assert_eq!(positions, 3);
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM flow_events WHERE kind='consumed'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        6
    );
    assert_eq!(fixture.launches().len(), 6);
    assert_eq!(fixture.retired_run_tables(), 0);
    assert!(fixture.run(&["flow", "resume", &root]).status.success());
    assert_eq!(
        fixture.launches().len(),
        6,
        "completed resumption must not launch another pass"
    );
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

#[test]
fn opencode_disconnect_after_tool_preserves_unknown_native_completion() {
    let fixture = Fixture::new(false);
    saved_flow_stand_in(&fixture);
    std::fs::write(fixture.home.path().join("disconnect-after-tool"), "").unwrap();
    let output = fixture.run(&[
        "--model",
        "opencode",
        "flow",
        "work-then-decide",
        "-b",
        "--no-loopflow",
    ]);
    assert!(!output.status.success(), "{output:?}");
    assert!(fixture.home.path().join("tool-effect").exists());
    let rows: (i64, i64, i64) = fixture
        .db()
        .query_row(
            "SELECT (SELECT count(*) FROM session_events WHERE kind='started'),
                (SELECT count(*) FROM session_events WHERE kind='completed'),
                (SELECT count(*) FROM flow_events WHERE kind='consumed')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(rows, (1, 0, 0));
    assert_eq!(
        fixture.launches().len(),
        1,
        "uncertain effect is not silently retried"
    );
    let outcome: Option<String> = fixture
        .db()
        .query_row(
            "SELECT outcome FROM execs WHERE caller_session_id IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(outcome.as_deref(), Some("failed"));
}

#[test]
fn opencode_automatic_retry_keeps_conversation_and_rejects_failed_turn_output() {
    let fixture = Fixture::new(false);
    saved_flow_stand_in(&fixture);
    std::fs::write(fixture.home.path().join("transient-once"), "").unwrap();
    let output = fixture.run(&[
        "--model",
        "opencode",
        "flow",
        "work-then-decide",
        "-b",
        "--no-loopflow",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let failed_consumed: i64 = fixture
        .db()
        .query_row(
            "SELECT count(*) FROM flow_events f JOIN session_events e ON e.seq=f.session_event
         WHERE f.kind='consumed' AND json_extract(e.payload,'$.status')!='completed'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(failed_consumed, 0);
    assert_eq!(fixture.count("agent_sessions"), 2);
    assert_eq!(fixture.captures(), 2);
    assert_eq!(fixture.retired_run_tables(), 0);
    let (threads,starts,done,consumed): (i64,i64,i64,i64) = fixture.db().query_row(
        "SELECT count(DISTINCT provider_thread),sum(kind='started'),sum(kind='completed'),
           (SELECT count(*) FROM flow_events WHERE kind='consumed') FROM session_events WHERE kind!='observed'", [],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap();
    assert_eq!((threads, starts, done, consumed), (2, 3, 3, 2));
    let usage = fixture.json(&["usage", "--json"]);
    assert_eq!(usage.as_array().unwrap().len(), 2);
    let launches = fixture.launches();
    assert_eq!(launches.len(), 3);
    assert_eq!(
        launches[1], launches[2],
        "automatic retry retains its admitted input and conversation"
    );
    let retried = usage
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["artifact_key"] == launches[1])
        .unwrap();
    assert_eq!(retried["usage"]["input_tokens"], 40);
    assert_eq!(retried["usage"]["output_tokens"], 10);
    let answer = fixture.run(&["runs", &fixture.launches()[1], "--final"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&answer.stdout).unwrap()["decision"],
        "advance"
    );
}
