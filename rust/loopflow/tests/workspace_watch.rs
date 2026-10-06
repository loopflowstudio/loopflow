//! `lf monitor workspace --watch` shows another process's commits. Each test
//! drives the real binary against a seeded `LF_HOME` and writes from here.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use loopflow::id::WaveId;
use loopflow::lf::commands::waves::{Evidence, RoadmapSnapshot};
use loopflow::lf::commands::workspace_watch::{WorkspaceContent, WorkspaceFrame};
#[cfg(target_os = "macos")]
use loopflow::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::PmSnapshotRow;
#[cfg(target_os = "macos")]
use loopflow::work::project::{Project, ProjectId};
#[cfg(target_os = "macos")]
use loopflow::work::task::{Observation, PmWritebackState, Task, TaskId, TaskPr, TaskPrId};
use loopflow::work::wave::Wave;

const PROJECT: &str = "95159066-9098-4d0b-8903-01459dc7ec14";
const TRANSCRIPT_LINE: &str = r#"{"evidence":{"schema_version":1,"type":"provider_output"}}"#;
const INPUT: &str = "run_00000000000000000000000000000001";

struct Watch {
    child: Child,
    stdin: Option<ChildStdin>,
    frames: Receiver<WorkspaceFrame>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Home {
    dir: tempfile::TempDir,
    store: SqliteStore,
    wave: Wave,
}

fn lf(home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (key, _) in std::env::vars_os() {
        let key = key.to_string_lossy().into_owned();
        if key.starts_with("LF_") || key.starts_with("LOOPFLOW_") {
            command.env_remove(key);
        }
    }
    command
        .args(args)
        .current_dir(home)
        .env("LF_HOME", home)
        .stderr(Stdio::inherit());
    command
}

impl Home {
    fn new() -> Self {
        let home = Self::at(tempfile::tempdir().unwrap());
        home.plan(0);
        home
    }

    /// Create the store and one Wave in `dir`.
    fn at(dir: tempfile::TempDir) -> Self {
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let store = SqliteStore::new(&dir.path().join("loopflow.db")).unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".into(),
            repo.canonicalize().unwrap().display().to_string(),
        );
        store.create_wave(&wave).unwrap();
        Self { dir, store, wave }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Commit a chapter plan holding Tasks `FIX-1..=count`, as a sync would.
    fn plan(&self, count: usize) {
        let items: Vec<_> = (1..=count)
            .map(|index| {
                serde_json::json!({
                    "id": format!("issue-{index}"), "identifier": format!("FIX-{index}"),
                    "url": null, "name": format!("Task {index}"), "description": "",
                    "rank": index, "completed": false, "project_id": PROJECT,
                    "project": "reactive", "team_id": "team-product", "assignee": null
                })
            })
            .collect();
        let payload = serde_json::json!({
            "projects": [{
                "id": PROJECT, "slug": "reactive", "name": "Reactive", "summary": "",
                "metric_targets": [], "flow": "feature", "status": "started", "krs": [],
                "initiative_ids": ["initiative-product"], "team_ids": ["team-product"]
            }],
            "items": items
        });
        self.store
            .put_pm_snapshot(&PmSnapshotRow {
                wave_id: self.wave.id().clone(),
                provider: "linear".into(),
                initiative: "initiative-product".into(),
                synced_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                snapshot: serde_json::from_value(payload).unwrap(),
            })
            .unwrap();
    }

    /// Start FIX-1 in a real Git checkout with one commit, and return it.
    #[cfg(target_os = "macos")]
    fn checkout(&self) -> std::path::PathBuf {
        let worktree = repository(&self.path().join("checkout"));
        let head = git(&worktree, &["rev-parse", "HEAD"]);
        let now = time::OffsetDateTime::now_utc();
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                flow: "feature".into(),
                status: loopflow::pm::ProjectStatus::Started,
                id: LinearProjectId::new(PROJECT).unwrap(),
                slug: "reactive".into(),
                name: "Reactive".into(),
                prompt_context: String::new(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: self.wave.id().clone(),
            iteration: 0,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        self.store.insert_project(&project).unwrap();
        let task = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("issue-1").unwrap(),
                identifier: "FIX-1".into(),
                title: "Task 1".into(),
                description: String::new(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: self.wave.id().clone(),
            project_id: project.id.clone(),
            worktree: worktree.clone(),
            workspace_slug: "fix-one".into(),
            agent: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: task.workspace_slug.clone(),
            branch: "fix-one".into(),
            base_commit: head,
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };
        self.store.insert_task(&task, &pr).unwrap();
        worktree
    }

    fn raw(&self) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open(self.path().join("loopflow.db")).unwrap();
        conn.busy_timeout(Duration::from_secs(5)).unwrap();
        conn
    }

    /// An interactive conversation in the Wave's repository, with one captured
    /// input: what `lf` writes before a provider starts.
    fn conversation(&self) {
        self.raw()
            .execute_batch(&format!(
                "BEGIN;
                 INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,repo,driver_generation)
                 VALUES('conversation','Conversation','human',1,1,'{repo}','{repo}',1);
                 INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
                 VALUES('conversation','captured','{INPUT}',1,'{{}}');
                 UPDATE agent_sessions SET current_capture=last_insert_rowid() WHERE id='conversation';
                 COMMIT;",
                repo = self.wave.repo()
            ))
            .unwrap();
    }

    fn execs(&self) -> i64 {
        self.raw()
            .query_row("SELECT COUNT(*) FROM execs", [], |row| row.get(0))
            .unwrap()
    }

    fn watch(&self) -> Watch {
        Watch::open(self.path())
    }
}

#[cfg(target_os = "macos")]
fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// A new repository at `path` holding one commit.
#[cfg(target_os = "macos")]
fn repository(path: &Path) -> std::path::PathBuf {
    std::fs::create_dir_all(path).unwrap();
    let repo = path.canonicalize().unwrap();
    git(&repo, &["init", "--quiet", "--initial-branch=fix-one"]);
    git(&repo, &["commit", "--quiet", "--allow-empty", "-m", "base"]);
    repo
}

/// One uninteresting transcript line, as a provider's output is recorded.
fn transcript(conn: &rusqlite::Connection, line: usize) {
    conn.execute(
        "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
         VALUES('conversation','observed',?1,1,?2)",
        [
            format!("{INPUT}:events.jsonl:{line}"),
            TRANSCRIPT_LINE.to_owned(),
        ],
    )
    .unwrap();
}

fn identifiers(roadmap: &RoadmapSnapshot) -> Vec<String> {
    roadmap
        .waves
        .iter()
        .flat_map(|wave| match &wave.tasks {
            Evidence::Ok { items, .. } => items
                .iter()
                .map(|task| task.task.identifier.clone())
                .collect::<Vec<_>>(),
            other => panic!("planning unavailable: {other:?}"),
        })
        .collect()
}

impl Watch {
    /// Start the reader on a Home, which need not hold a store yet.
    fn open(home: &Path) -> Self {
        let mut child = lf(home, &["monitor", "workspace", "--watch", "--json"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let (send, frames) = mpsc::channel();
        std::thread::spawn(move || {
            for line in stdout.lines() {
                let line = line.unwrap();
                let frame: WorkspaceFrame = serde_json::from_str(&line)
                    .unwrap_or_else(|error| panic!("invalid frame: {error}: {line}"));
                if send.send(frame).is_err() {
                    break;
                }
            }
        });
        Self {
            stdin: child.stdin.take(),
            child,
            frames,
        }
    }

    fn request(&mut self, request: serde_json::Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{request}").unwrap();
        stdin.flush().unwrap();
    }

    fn next(&self, within: Duration) -> Option<WorkspaceFrame> {
        self.frames.recv_timeout(within).ok()
    }

    /// The next planning frame whose Tasks satisfy `accept`.
    fn planning(&self, within: Duration, accept: impl Fn(&[String]) -> bool) -> Vec<String> {
        let deadline = Instant::now() + within;
        loop {
            let frame = self
                .next(deadline.saturating_duration_since(Instant::now()))
                .expect("no matching planning frame in time");
            assert_eq!(frame.unavailable, None);
            if let WorkspaceContent::Planning(Some(part)) = frame.content {
                let tasks = identifiers(&part.roadmap);
                if accept(&tasks) {
                    return tasks;
                }
            }
        }
    }

    /// The next planning frame in which FIX-1's observed checkout is
    /// `(dirty, has commits past its PR base)`.
    #[cfg(target_os = "macos")]
    fn checkout(&self, expected: (bool, bool)) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let frame = self
                .next(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|| panic!("no planning frame showing {expected:?} in time"));
            let WorkspaceContent::Planning(Some(part)) = frame.content else {
                continue;
            };
            let Evidence::Ok { items, .. } = &part.roadmap.waves[0].tasks else {
                continue;
            };
            let progress = &items[0].condition.local_progress;
            if (progress.dirty, progress.authored_commits) == (Some(expected.0), Some(expected.1)) {
                return;
            }
        }
    }

    /// The next Sessions frame in which the conversation satisfies `accept`.
    /// `Null` stands for a frame that no longer lists it.
    fn session(&self, accept: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let frame = self
                .next(deadline.saturating_duration_since(Instant::now()))
                .expect("no matching Sessions frame in time");
            if let WorkspaceContent::Sessions(Some(part)) = frame.content {
                let record = part
                    .entries
                    .iter()
                    .map(|entry| serde_json::to_value(entry).unwrap())
                    .find(|entry| entry["id"] == "conversation")
                    .unwrap_or(serde_json::Value::Null);
                if accept(&record) {
                    return record;
                }
            }
        }
    }

    fn heartbeat(&self) -> BTreeMap<String, u64> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let frame = self
                .next(deadline.saturating_duration_since(Instant::now()))
                .expect("no heartbeat in time");
            if let WorkspaceContent::Heartbeat(heartbeat) = frame.content {
                return heartbeat.projections;
            }
        }
    }

    /// Every frame that arrives within `window`, heartbeats aside.
    fn parts(&self, window: Duration) -> Vec<WorkspaceFrame> {
        let deadline = Instant::now() + window;
        let mut parts = Vec::new();
        while let Some(frame) = self.next(deadline.saturating_duration_since(Instant::now())) {
            if !matches!(frame.content, WorkspaceContent::Heartbeat(_)) {
                parts.push(frame);
            }
        }
        parts
    }
}

fn stable(mut value: serde_json::Value) -> serde_json::Value {
    fn strip(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                fields.remove("generated_at");
                fields.remove("evidence_age_secs");
                if let Some(serde_json::Value::Object(condition)) = fields.get_mut("condition") {
                    condition.remove("observed_at");
                }
                fields.values_mut().for_each(strip);
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut value);
    value
}

#[test]
fn a_task_committed_elsewhere_appears_once_and_bursts_converge() {
    let home = Home::new();
    let watch = home.watch();
    assert!(watch.planning(Duration::from_secs(30), |_| true).is_empty());

    let committed = Instant::now();
    home.plan(1);
    assert_eq!(
        watch.planning(Duration::from_secs(2), |tasks| !tasks.is_empty()),
        ["FIX-1"]
    );
    eprintln!("write-to-frame: {:?}", committed.elapsed());

    // Twenty transactions of many rows each: few frames, and the last one is
    // what a fresh read prints.
    for count in 2..=21 {
        home.plan(count);
    }
    let mut frames = 0;
    let mut last = None;
    for frame in watch.parts(Duration::from_secs(3)) {
        if let WorkspaceContent::Planning(Some(part)) = frame.content {
            frames += 1;
            last = Some(part.roadmap);
        }
    }
    assert!((1..=5).contains(&frames), "{frames} planning frames");
    let last = last.unwrap();
    assert_eq!(identifiers(&last).len(), 21);
    let fresh = lf(home.path(), &["roadmap", "--all", "--json"])
        .output()
        .unwrap();
    assert!(fresh.status.success());
    assert_eq!(
        stable(serde_json::to_value(&last).unwrap()),
        stable(serde_json::from_slice(&fresh.stdout).unwrap())
    );
}

#[test]
fn transcript_lines_read_nothing_and_do_not_delay_a_task() {
    let home = Home::new();
    let mut watch = home.watch();
    watch.planning(Duration::from_secs(30), |_| true);
    let conn = home.raw();
    home.conversation();
    // Select every scoped part, so a transcript line has all of them to wake.
    watch.request(
        serde_json::json!({"action": "scope", "id": 1, "repo": home.wave.repo(),
        "headless": true, "task": null, "wave": home.wave.id().as_str(),
        "activity": {"wave": home.wave.id().as_str(), "project": null, "task": null}}),
    );
    watch.session(|_| true);
    watch.parts(Duration::from_secs(1));
    let before = watch.heartbeat();

    for line in 0..2000 {
        transcript(&conn, line);
        if line % 100 == 0 {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    watch.parts(Duration::from_millis(500));
    let after = watch.heartbeat();
    // `activity` is absent: it observes this machine's processes on a 2 s
    // clock and reads nothing a transcript line can change.
    for part in ["planning", "sessions", "task", "wave", "work_activity"] {
        assert_eq!(before.get(part), after.get(part), "{part} was read again");
    }

    // An Exec can change a planning condition, so planning is read; nothing
    // displayed changed, so nothing is sent.
    conn.execute("INSERT INTO execs(id,trace_id,cwd,started_at,completed_at,outcome) VALUES('exec_00000000000000000000000000000001','trace_00000000000000000000000000000001','/elsewhere',1,2,'succeeded')", []).unwrap();
    assert!(watch
        .parts(Duration::from_millis(1500))
        .iter()
        .all(|frame| !matches!(frame.content, WorkspaceContent::Planning(_))));
    assert!(watch.heartbeat()["planning"] > after["planning"]);

    let writer = std::thread::spawn({
        let conn = home.raw();
        move || {
            for line in 2000..3000 {
                transcript(&conn, line);
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    });
    home.plan(1);
    watch.planning(Duration::from_secs(2), |tasks| tasks == ["FIX-1"]);
    writer.join().unwrap();
}

/// The only guard on the transcript exemption being too wide: a displayed
/// fact that bumps nothing is never read again, by any clock.
#[test]
fn every_displayed_session_fact_committed_elsewhere_is_shown() {
    let home = Home::new();
    let mut watch = home.watch();
    watch.planning(Duration::from_secs(30), |_| true);
    watch.request(
        serde_json::json!({"action": "scope", "id": 1, "repo": home.wave.repo(),
        "headless": false, "task": null, "wave": null, "activity": null}),
    );
    home.conversation();
    let first = watch.session(|record| !record.is_null());
    assert_eq!(first["title"], "Conversation");
    assert_eq!(first["flow_membership"]["kind"], "unknown");
    assert_eq!(first["attention"], serde_json::Value::Null);

    let conn = home.raw();
    let write = |sql: &str| {
        conn.execute_batch(sql).unwrap();
    };
    let event = |kind: &str, receipt: &str, turn: Option<&str>, payload: serde_json::Value| {
        conn.execute(
            "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload,captured_event)
             SELECT id,?1,?1,?2,?3,2,?4,current_capture FROM agent_sessions WHERE id='conversation'",
            rusqlite::params![turn, kind, receipt, payload.to_string()],
        )
        .unwrap();
    };

    write("UPDATE agent_sessions SET title='Renamed',title_source='generated' WHERE id='conversation'");
    let record = watch.session(|record| record["title"] == "Renamed");
    assert_eq!(record["title_source"], "generated");

    write("UPDATE agent_sessions SET provider='claude',model='opus' WHERE id='conversation'");
    watch.session(|record| record["detail"] == "claude:opus" && record["provider"] == "claude");

    // Flow membership is an `observed` event, like a transcript line.
    event(
        "observed",
        &format!("{INPUT}:manifest.json"),
        None,
        serde_json::json!({"source": "manifest.json", "input_id": INPUT, "evidence":
            {"schema_version": 1, "artifact_key": INPUT, "flow": {"kind": "independent"}}}),
    );
    watch.session(|record| record["flow_membership"]["kind"] == "independent");

    // A finished turn asks for a reply.
    event("started", "", Some("turn"), serde_json::json!({}));
    event(
        "completed",
        "",
        Some("turn"),
        serde_json::json!({"status": "completed"}),
    );
    watch.session(|record| record["attention"] == "reply");

    event(
        "observed",
        "driver:0:exit",
        None,
        serde_json::json!({"outcome": "interrupted"}),
    );
    let record = watch.session(|record| record["state"] == "interrupted");
    assert_eq!(record["attention"], serde_json::Value::Null);

    write("UPDATE agent_sessions SET ready_summary='Ready for review' WHERE id='conversation'");
    let record = watch.session(|record| record["state"] == "ready");
    assert_eq!(record["ready_summary"], "Ready for review");
    assert_eq!(record["attention"], "review");

    // Transcript lines between those facts were never a reason to read.
    let before = watch.heartbeat()["sessions"];
    for line in 0..50 {
        transcript(&conn, line);
    }
    watch.parts(Duration::from_millis(1500));
    assert_eq!(watch.heartbeat()["sessions"], before);

    write("UPDATE agent_sessions SET completed_at=4 WHERE id='conversation'");
    watch.session(|record| record.is_null() || record["state"] == "closed");
}

#[test]
fn a_store_created_after_the_reader_started_is_shown() {
    let dir = tempfile::tempdir().unwrap();
    let watch = Watch::open(dir.path());
    assert!(watch.planning(Duration::from_secs(30), |_| true).is_empty());

    let home = Home::at(dir);
    home.plan(1);
    assert_eq!(
        watch.planning(Duration::from_secs(5), |tasks| !tasks.is_empty()),
        ["FIX-1"]
    );
}

#[test]
fn a_store_that_cannot_be_opened_is_unavailable_not_empty() {
    let dir = tempfile::tempdir().unwrap();
    // Some other schema: this build must not adopt, upgrade or read it.
    rusqlite::Connection::open(dir.path().join("loopflow.db"))
        .unwrap()
        .execute_batch("CREATE TABLE elsewhere(x)")
        .unwrap();
    let watch = Watch::open(dir.path());
    let frame = loop {
        let frame = watch.next(Duration::from_secs(30)).expect("reader ended");
        if matches!(frame.content, WorkspaceContent::Planning(_)) {
            break frame;
        }
    };
    assert!(matches!(frame.content, WorkspaceContent::Planning(None)));
    assert!(frame.unavailable.is_some());
}

#[test]
fn a_steady_writer_is_shown_while_it_writes() {
    let home = Home::new();
    let watch = home.watch();
    watch.planning(Duration::from_secs(30), |_| true);
    let started = Instant::now();
    let mut shown = None;
    let mut latest = 0;
    for count in 1..=60 {
        home.plan(count);
        std::thread::sleep(Duration::from_millis(50));
        while let Ok(frame) = watch.frames.try_recv() {
            if let WorkspaceContent::Planning(Some(part)) = frame.content {
                latest = identifiers(&part.roadmap).len();
                shown.get_or_insert((count, started.elapsed()));
            }
        }
    }
    let (count, elapsed) = shown.expect("no planning frame until the writer stopped");
    assert!(
        count < 60,
        "first frame arrived only as the writer finished"
    );
    eprintln!("first frame after {elapsed:?}, at write {count}");
    if latest != 60 {
        watch.planning(Duration::from_secs(5), |tasks| tasks.len() == 60);
    }
}

#[test]
fn a_paused_reader_and_a_truncated_log_both_catch_up() {
    let home = Home::new();
    let watch = home.watch();
    watch.planning(Duration::from_secs(30), |_| true);

    // SAFETY: signals this test's own child, which `Watch` still owns.
    unsafe { libc::kill(watch.child.id() as libc::pid_t, libc::SIGSTOP) };
    home.plan(1);
    std::thread::sleep(Duration::from_millis(500));
    // SAFETY: as above.
    unsafe { libc::kill(watch.child.id() as libc::pid_t, libc::SIGCONT) };
    watch.planning(Duration::from_secs(3), |tasks| tasks == ["FIX-1"]);

    home.raw()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    home.plan(2);
    watch.planning(Duration::from_secs(3), |tasks| tasks.len() == 2);
}

#[test]
fn scope_selects_sessions_and_idle_sends_nothing() {
    let home = Home::new();
    let other = home.path().join("other");
    std::fs::create_dir_all(&other).unwrap();
    let other = other.canonicalize().unwrap().display().to_string();
    let mut watch = home.watch();
    watch.planning(Duration::from_secs(30), |_| true);

    let sessions = |watch: &Watch, id: u64| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let frame = watch
                .next(deadline.saturating_duration_since(Instant::now()))
                .expect("no Sessions frame in time");
            if let WorkspaceContent::Sessions(Some(part)) = frame.content {
                if frame.answers == Some(id) {
                    return part;
                }
                assert!(frame.answers < Some(id));
            }
        }
    };
    watch.request(
        serde_json::json!({"action": "scope", "id": 1, "repo": home.wave.repo(),
        "headless": false, "task": null, "wave": null, "activity": null}),
    );
    assert_eq!(sessions(&watch, 1).repo, home.wave.repo());
    watch.request(
        serde_json::json!({"action": "scope", "id": 2, "repo": other,
        "headless": true, "task": null, "wave": home.wave.id().as_str(), "activity": null}),
    );
    let part = sessions(&watch, 2);
    assert_eq!(
        (part.repo.as_str(), part.includes_headless),
        (other.as_str(), true)
    );
    // The first scope's repository is never read again.
    home.raw().execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd) VALUES('conversation','Conversation','human',1,0,?1)", [home.wave.repo()]).unwrap();
    for frame in watch.parts(Duration::from_secs(2)) {
        match frame.content {
            WorkspaceContent::Sessions(Some(part)) => assert_eq!(part.repo, other),
            WorkspaceContent::Wave(Some(part)) => assert_eq!(part.wave, home.wave.id().as_str()),
            _ => {}
        }
    }

    // A refresh answers every selected part even though nothing changed.
    watch.request(serde_json::json!({"action": "refresh", "id": 3}));
    let answered: Vec<_> = watch
        .parts(Duration::from_secs(3))
        .into_iter()
        .filter(|frame| frame.answers == Some(3))
        .map(|frame| serde_json::to_value(&frame).unwrap()["part"].clone())
        .collect();
    for part in ["planning", "sessions", "wave", "activity"] {
        assert!(
            answered.iter().any(|name| name == part),
            "{part}: {answered:?}"
        );
    }

    let execs = home.execs();
    // Process liveness is this machine's, observed on a clock; everything
    // read from the store stays silent.
    let idle: Vec<_> = watch
        .parts(Duration::from_secs(5))
        .into_iter()
        .filter(|frame| !matches!(frame.content, WorkspaceContent::Activity(_)))
        .collect();
    assert!(idle.is_empty(), "{idle:?}");
    assert_eq!(home.execs(), execs);

    watch.stdin = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    while watch.child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "reader outlived its stdin");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Nothing here commits to the store: the reader sees the files and Git
/// metadata change.
#[cfg(target_os = "macos")]
#[test]
fn a_checkout_changed_on_disk_is_shown() {
    let home = Home::at(tempfile::tempdir().unwrap());
    let worktree = home.checkout();
    home.plan(1);
    let watch = home.watch();
    watch.checkout((false, false));

    std::fs::write(worktree.join("note.txt"), "draft").unwrap();
    watch.checkout((true, false));

    git(&worktree, &["add", "note.txt"]);
    git(&worktree, &["commit", "--quiet", "-m", "note"]);
    watch.checkout((false, true));
}

/// Agents record usage every few seconds. Only the part showing token totals
/// follows it, and however often it moves that part is read once per rest.
#[test]
fn usage_rereads_only_wave_detail_and_not_for_every_row() {
    let home = Home::new();
    let mut watch = home.watch();
    watch.planning(Duration::from_secs(30), |_| true);
    let conn = home.raw();
    home.conversation();
    watch.request(
        serde_json::json!({"action": "scope", "id": 1, "repo": home.wave.repo(),
        "headless": true, "task": null, "wave": home.wave.id().as_str(),
        "activity": {"wave": home.wave.id().as_str(), "project": null, "task": null}}),
    );
    watch.session(|_| true);
    watch.parts(Duration::from_secs(1));
    let before = watch.heartbeat();

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(13) {
        conn.execute(
            "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
             VALUES('conversation','usage',?1,1,'{}')",
            [format!("turn-{}", started.elapsed().as_millis())],
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(100));
    }
    // Heartbeats sent while writing are still waiting to be received.
    watch.parts(Duration::from_millis(200));
    let after = watch.heartbeat();
    for part in ["planning", "sessions", "task", "work_activity"] {
        assert_eq!(before.get(part), after.get(part), "{part} was read again");
    }
    let readings = after["wave"] - before["wave"];
    assert!((1..=2).contains(&readings), "wave read {readings} times");
}
