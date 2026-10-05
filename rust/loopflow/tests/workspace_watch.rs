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
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::PmSnapshotRow;
use loopflow::work::wave::Wave;

const PROJECT: &str = "95159066-9098-4d0b-8903-01459dc7ec14";

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
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let store = SqliteStore::new(&dir.path().join("loopflow.db")).unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".into(),
            repo.canonicalize().unwrap().display().to_string(),
        );
        store.create_wave(&wave).unwrap();
        let home = Self { dir, store, wave };
        home.plan(0);
        home
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

    fn raw(&self) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open(self.path().join("loopflow.db")).unwrap();
        conn.busy_timeout(Duration::from_secs(5)).unwrap();
        conn
    }

    fn execs(&self) -> i64 {
        self.raw()
            .query_row("SELECT COUNT(*) FROM execs", [], |row| row.get(0))
            .unwrap()
    }

    fn watch(&self) -> Watch {
        let mut child = lf(self.path(), &["monitor", "workspace", "--watch", "--json"])
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
        Watch {
            stdin: child.stdin.take(),
            child,
            frames,
        }
    }
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
    assert!(watch
        .planning(Duration::from_secs(30), |_| true)
        .is_empty());

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
    let watch = home.watch();
    watch.planning(Duration::from_secs(30), |_| true);
    let conn = home.raw();
    conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd) VALUES('conversation','Conversation','human',1,0,?1)", [home.wave.repo()]).unwrap();
    // The Session row itself is a displayed change; let it settle.
    watch.parts(Duration::from_secs(1));
    let before = watch.heartbeat();

    for line in 0..2000 {
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('conversation','observed',?1,1,'{}')", [format!("input:events.jsonl:{line}")]).unwrap();
        if line % 100 == 0 {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    watch.parts(Duration::from_millis(500));
    let after = watch.heartbeat();
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
                conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('conversation','observed',?1,1,'{}')", [format!("input:events.jsonl:{line}")]).unwrap();
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    });
    home.plan(1);
    watch.planning(Duration::from_secs(2), |tasks| tasks == ["FIX-1"]);
    writer.join().unwrap();
}

#[test]
fn a_steady_writer_is_shown_while_it_writes() {
    let home = Home::new();
    let watch = home.watch();
    watch.planning(Duration::from_secs(30), |_| true);
    let started = Instant::now();
    let mut shown = None;
    for count in 1..=60 {
        home.plan(count);
        std::thread::sleep(Duration::from_millis(50));
        while let Ok(frame) = watch.frames.try_recv() {
            if matches!(frame.content, WorkspaceContent::Planning(Some(_))) && shown.is_none() {
                shown = Some((count, started.elapsed()));
            }
        }
    }
    let (count, elapsed) = shown.expect("no planning frame until the writer stopped");
    assert!(count < 60, "first frame arrived only as the writer finished");
    eprintln!("first frame after {elapsed:?}, at write {count}");
    watch.planning(Duration::from_secs(3), |tasks| tasks.len() == 60);
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
    watch.request(serde_json::json!({"action": "scope", "id": 1, "repo": home.wave.repo(),
        "headless": false, "task": null, "wave": null, "activity": null}));
    assert_eq!(sessions(&watch, 1).repo, home.wave.repo());
    watch.request(serde_json::json!({"action": "scope", "id": 2, "repo": other,
        "headless": true, "task": null, "wave": home.wave.id().as_str(), "activity": null}));
    let part = sessions(&watch, 2);
    assert_eq!((part.repo.as_str(), part.includes_headless), (other.as_str(), true));
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
        assert!(answered.iter().any(|name| name == part), "{part}: {answered:?}");
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
