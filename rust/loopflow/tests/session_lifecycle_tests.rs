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
use sha2::{Digest, Sha256};

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

    fn captures(&self) -> i64 {
        self.db()
            .query_row(
                "SELECT count(*) FROM session_events WHERE kind='captured'",
                [],
                |row| row.get(0),
            )
            .unwrap()
    }

    /// (session id, kind, title, title_source, completed) of the captured input’s Session.
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

    /// `lf ask` as called from inside the caller's conversation.
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
}

/// An upper bound only: a debug `lf` on a busy machine takes tens of seconds to launch.
const PATIENCE: Duration = Duration::from_secs(180);

const LAUNCH: [&str; 6] = [
    "--mode",
    "tui",
    "--model",
    "opencode",
    ":",
    "Review the parser",
];

#[test]
fn conversation_keeps_its_name_and_identity_until_completed() {
    let fixture = Fixture::new(true);

    let (first, first_run) = fixture.attach(&LAUNCH);
    assert_eq!(fixture.sessions().len(), 1);
    let (id, kind, title, source, completed) = fixture.session_row(&first_run);
    assert_ne!(
        id, first_run,
        "conversation identity differs from its captured input"
    );
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

    // Open resumes the same conversation with its captured input.
    let described = fixture.json(&["session", "connect", &id, "--json"]);
    assert_eq!(described["id"], id.as_str());
    let (resumed, resumed_run) = fixture.attach(&["session", "connect", &id]);
    assert_eq!(resumed_run, first_run);
    assert_eq!(fixture.sessions()[0]["state"], "active");
    fixture.release(resumed);
    assert_eq!(fixture.sessions().len(), 1);

    // Another launch in the same checkout is another conversation.
    let (second, second_run) = fixture.attach(&LAUNCH);
    fixture.release(second);
    let (second_id, ..) = fixture.session_row(&second_run);
    assert_ne!(second_id, id);
    assert_eq!(fixture.sessions().len(), 2);
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
    let reopened = fixture.run(&["session", "connect", &id, "--json"]);
    assert!(!reopened.status.success(), "{reopened:?}");
    assert!(
        String::from_utf8_lossy(&reopened.stderr).contains("already complete"),
        "{reopened:?}"
    );
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
    {
        let command = vec!["usage", "--days", "0", "--json"];
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
            .json(&["usage", "--days", "0", "--task", issue, "--json"])
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
    assert_eq!(started(task.task.id.as_str()), Some(bound_at));
    assert!(task_runs("INF-123").is_empty());
    let listed = fixture.sessions();
    assert_eq!(listed.len(), 1);
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
    assert_eq!(started(sibling.id.as_str()), None);
    assert!(task_runs("INF-124").is_empty());

    std::thread::sleep(Duration::from_secs(2));
    let later = launch(&[
        "--task",
        "INF-123",
        "--mode",
        "tui",
        "--model",
        "opencode",
        ":",
        "Review the parser",
    ]);
    let started_at = started(task.task.id.as_str()).expect("future work starts the Task");
    assert_eq!(started_at, bound_at);
    assert_eq!(task_runs("INF-123"), vec![later]);
    assert_eq!(fixture.run_parents(&orphan), (None, None, None));
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
        "argv": [env!("CARGO_BIN_EXE_lf"), "--mode", "tui", "--model", "opencode", ":", "Work in Y"], "cwd": y,
    })).unwrap()).unwrap();
    let output = fixture.run(&[
        "--task",
        "INF-123",
        "--mode",
        "tui",
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
    support::bind_task_planning(&fixture.repo);
    let y = fixture.repo.create_named_worktree("task-y");
    let target =
        support::register_unrun_task(fixture.home.path(), &y, "task-y", &fixture.repo.head_sha());
    let x = fixture.repo.create_named_worktree("task-x");
    let caller = support::register_sibling_task(&target, "INF-124", "task-x", &x);
    std::fs::create_dir_all(y.join(".lf/flows")).unwrap();
    std::fs::write(
        y.join(".lf/flows/switch-proof.yaml"),
        "- cmd: task sync --plan\n",
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
            "argv": [env!("CARGO_BIN_EXE_lf"), "--task", "INF-123", "flow", "start", "--json"], "cwd": x,
        }))
        .unwrap(),
    )
    .unwrap();
    let output = fixture
        .command(&[
            "--task", "INF-124", "--mode", "tui", "--model", "opencode", ":", "Start Y",
        ])
        .env("LF_BIN", bin.join("lf"))
        .output()
        .unwrap();
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

const BOUND_LAUNCH: [&str; 8] = [
    "--task",
    "INF-123",
    "--mode",
    "tui",
    "--model",
    "opencode",
    ":",
    "Review the parser",
];

#[test]
fn ask_returns_feedback_once_and_rejects_a_stale_answer() {
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
    assert_eq!(fixture.sessions().len(), 2);
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
    // Replacement captures another input in the same conversation.
    std::fs::remove_file(fixture.run_dir(&first_run).join("prepared")).unwrap();
    let (opened, second_run) = fixture.attach(&["session", "connect", &id]);
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
        "the replaced capture stays in the conversation's history"
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
        command.env("LF_HOME", &self.directory);
    }

    fn unlock(&self) -> rusqlite::Connection {
        std::fs::set_permissions(&self.directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        rusqlite::Connection::open(self.directory.join("loopflow.db")).unwrap()
    }
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
        &[
            "--mode",
            "batch",
            "--model",
            "opencode",
            ":",
            "Tidy the parser",
        ],
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
        &[
            "--mode",
            "batch",
            "--model",
            "opencode",
            ":",
            "Tidy the parser",
        ],
    ] {
        let output = fixture
            .command(args)
            .env("LF_AGENT_CALLER", "not-json")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("agent Exec requires an admitted Exec"),
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
fn unavailable_store_starts_no_provider_or_ask() {
    let fixture = Fixture::new(false);
    saved_flow_stand_in(&fixture);
    let store = LockedStore::new(&fixture);
    for args in [
        LAUNCH.as_slice(),
        &[
            "--mode",
            "batch",
            "--model",
            "opencode",
            ":",
            "Tidy the parser",
        ],
        &[
            "--model",
            "opencode",
            "flow",
            "work-then-decide",
            "--mode",
            "batch",
            "--no-loopflow",
        ],
        &["ask", "Which release target?"],
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

const REVIEW_FLOW: [&str; 7] = [
    "--model",
    "opencode",
    "flow",
    "review-first",
    "--mode",
    "batch",
    "--no-loopflow",
];

impl Fixture {
    /// A saved Flow whose only step is a human review, run until it waits.
    /// Returns (Session id, invocation id, first capture key).
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

    /// (title, title_source, ready_summary, current capture, completed) of a Session.
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
fn review_feedback_survives_replacement_and_resumes_the_flow() {
    let fixture = Fixture::new(true);
    let (id, invocation, first_run) = fixture.waiting_review();
    assert_eq!(fixture.sessions().len(), 1);
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
        "the review belongs to a taskless Flow"
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
    assert_eq!(fixture.sessions().len(), 1);
    let ready = inside(&first_run, &["session", "ready", "Ship the parser"]);
    assert!(ready.status.success(), "{ready:?}");
    let named = fixture.json(&["session", "rename", &id, "Parser review", "--json"]);
    assert_eq!(named["title"], "Parser review");
    assert_eq!(named["title_source"], "human");

    // A consumed launch without provider history is replaced on the next open.
    // Title and feedback belong to the Session, so both survive.
    std::fs::remove_file(fixture.run_dir(&first_run).join("prepared")).unwrap();
    let (opened, second_run) = fixture.attach(&["session", "connect", &id]);
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
        "the replaced capture stays in the conversation's history"
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
}

#[test]
fn headless_history_is_discoverable_without_entering_the_interactive_list() {
    let fixture = Fixture::new(false);
    support::register_unrun_task(
        fixture.home.path(),
        fixture.repo.path(),
        "history",
        &fixture.repo.head_sha(),
    );
    let output = fixture.run(&[
        "--wave",
        "task-pr-tests",
        "--mode",
        "batch",
        "--model",
        "opencode",
        ":",
        "Tidy the parser",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert!(fixture.sessions().is_empty());
    let sessions = fixture.json(&["session", "list", "--interactive", "false", "--json"]);
    assert_eq!(sessions.as_array().unwrap().len(), 1);
    let session = &sessions[0]["id"];
    {
        let rows = fixture.json(&["usage", "--wave", "task-pr-tests", "--json"]);
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert_eq!(&rows[0]["session_id"], session);
        assert_eq!(rows[0]["recorded_outcome"], "completed");
    }
    let activity = fixture.json(&["activity", "--wave", "task-pr-tests", "--json"]);
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

#[test]
fn task_flow_retries_the_conversation_then_waits_for_review() {
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
sys.stdin.readline()
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
    let credential =
        r#"{"claudeAiOauth":{"accessToken":"synthetic-fixture-token","expiresAt":4102444800000}}"#;
    std::fs::write(account_home.join(".credentials.json"), credential).unwrap();
    let store = loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("loopflow.db"))
        .unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let account_id = loopflow::store::ProviderAccountId::parse("fixture").unwrap();
    store
        .upsert_provider_account(&loopflow::store::ProviderAccount {
            provider: "claude".into(),
            account_id: account_id.clone(),
            home: Some(account_home),
            login_email: Some(
                loopflow::profile::EmailAddress::parse("fixture@example.com").unwrap(),
            ),
            observed_email: Some("fixture@example.com".into()),
            observed_subject: Some("fixture".into()),
            observed_credential_digest: Some(format!(
                "{:x}",
                Sha256::digest(credential.as_bytes())
            )),
            observed_plan: None,
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
        "--mode",
        "batch",
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
    for run in &runs {
        assert_eq!(run.4.as_deref(), Some(task_id.as_str()), "{run:?}");
    }
    let review = runs[2].0.clone();
    let listed: Vec<Value> = serde_json::from_value(
        fixture.json(&["usage", "--days", "0", "--task", "INF-123", "--json"]),
    )
    .unwrap();
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
    assert_eq!(invocation_inputs(&fixture, &invocation).len(), 3);
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
            "--mode",
            "batch",
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
        "--mode",
        "batch",
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
        "--mode",
        "batch",
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
    assert!(fixture.run(&["flow", "resume", &root]).status.success());
    assert_eq!(
        fixture.launches().len(),
        6,
        "completed resumption must not launch another pass"
    );
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
        "--mode",
        "batch",
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
        "--mode",
        "batch",
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
    let answer = fixture.run(&[
        "monitor",
        "show",
        retried["session_id"].as_str().unwrap(),
        "--input",
        &launches[1],
        "--final",
    ]);
    assert_eq!(
        serde_json::from_slice::<Value>(&answer.stdout).unwrap()["decision"],
        "advance"
    );
}
