//! `--watch` on the ordinary reads shows another process's commits. Each test
//! drives the real binary against a seeded `LF_HOME` and writes from here.

use crate::test_linear_input::{InventoryFixture, SeedLinearSqlite};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use loopflow::id::WaveId;
use loopflow::lf::commands::waves::{Evidence, RoadmapSnapshot};
use loopflow::store::sqlite::SqliteStore;
#[cfg(target_os = "macos")]
use loopflow::work::task::{TaskEventKind, TaskPr, TaskPrId};
use loopflow::work::wave::Wave;

const PROJECT: &str = "95159066-9098-4d0b-8903-01459dc7ec14";

/// One running `lf … --watch`; its frames are `{answers, home, copy,
/// revisions, unavailable, body}`.
struct Watch {
    child: Child,
    stdin: Option<ChildStdin>,
    frames: Receiver<serde_json::Value>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Machine {
    dir: tempfile::TempDir,
    store: SqliteStore,
    wave: Wave,
}

fn lf(home: &Path, args: &[&str]) -> Command {
    let mut command = on_machine(home, Command::new(loopflow_test_support::lf_binary()));
    command.args(args);
    command
}

/// `command` as a process on the Machine at `home` runs, inheriting no `LF_*`.
fn on_machine(home: &Path, mut command: Command) -> Command {
    for (key, _) in std::env::vars_os() {
        let key = key.to_string_lossy().into_owned();
        if key.starts_with("LF_") || key.starts_with("LOOPFLOW_") || key.starts_with("LINEAR_") {
            command.env_remove(key);
        }
    }
    command
        .current_dir(home)
        .env("LF_HOME", home)
        .env("LF_BIN", loopflow_test_support::lf_binary())
        .env("HOME", home)
        .stderr(Stdio::inherit());
    command
}

impl Machine {
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
                "metric_targets": [], "status": "started", "krs": [],
                "initiative_ids": ["initiative-product"], "team_ids": ["team-product"]
            }],
            "items": items
        });
        self.store
            .seed_inventory(&InventoryFixture {
                wave_id: self.wave.id().clone(),
                provider: "linear".into(),
                initiative: "initiative-product".into(),
                synced_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                snapshot: serde_json::from_value(payload).unwrap(),
            })
            .unwrap();
        rusqlite::Connection::open(self.path().join("loopflow.db")).unwrap().execute(
            "UPDATE waves SET current_project_id=(SELECT id FROM projects WHERE external_project_id=?2) WHERE id=?1 AND current_project_id IS NULL",
            rusqlite::params![self.wave.id(),PROJECT]).unwrap();
    }

    /// Start `FIX-<index>` in a real Git checkout with one commit, and return it.
    #[cfg(target_os = "macos")]
    fn start(&self, index: usize) -> std::path::PathBuf {
        let worktree = repository(&self.path().join(format!("checkout-{index}")));
        let head = git(&worktree, &["rev-parse", "HEAD"]);
        let now = time::OffsetDateTime::now_utc();
        let mut task = self
            .store
            .task_by_issue(&format!("issue-{index}"))
            .unwrap()
            .unwrap();
        task.worktree = Some(worktree.clone());
        task.workspace_slug = format!("fix-{index}");
        git(&worktree, &["branch", "--move", &task.workspace_slug]);
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: task.workspace_slug.clone(),
            branch: task.workspace_slug.clone(),
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
        self.store
            .place_task(
                &task.id,
                task.worktree.as_ref().unwrap(),
                &task.workspace_slug,
                &pr,
            )
            .unwrap();
        self.store
            .append_task_event(
                &task.id,
                &TaskEventKind::CheckoutReady {
                    branch: pr.branch,
                    base_commit: pr.base_commit,
                },
            )
            .unwrap();
        worktree
    }

    fn raw(&self) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open(self.path().join("loopflow.db")).unwrap();
        conn.busy_timeout(Duration::from_secs(5)).unwrap();
        conn
    }

    fn repo(&self) -> &Path {
        Path::new(self.wave.repo())
    }

    /// A Linear-planned repository with Git, as its Work was registered.
    fn linear_repository(&self) {
        repository(self.repo());
        // This fixture registers Work before creating Git. Enroll that known
        // legacy checkout explicitly, just as recovery does for released Work.
        let id = self.store.repository_id(self.wave.repo()).unwrap().unwrap();
        loopflow::repository::location::reconcile(
            &self.store,
            &loopflow::repository::CanonicalRepo::discover(self.repo()).unwrap(),
            Some(&id),
            None,
        )
        .unwrap();
        let config = self.repo().join(".lf");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join("config.yaml"),
            "planning:\n  provider: linear\n  team: team-product\n",
        )
        .unwrap();
    }

    /// Run `lf <args> --json --watch` in the Wave's repository.
    fn watch(&self, args: &[&str]) -> Watch {
        Watch::open(self.path(), self.repo(), args)
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        let output = lf(self.path(), args)
            .current_dir(self.repo())
            .env("LF_USER_NAME", "Fixture Person")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}

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
fn repository(path: &Path) -> std::path::PathBuf {
    std::fs::create_dir_all(path).unwrap();
    let repo = path.canonicalize().unwrap();
    git(&repo, &["init", "--quiet", "--initial-branch=fix-one"]);
    git(&repo, &["commit", "--quiet", "--allow-empty", "-m", "base"]);
    repo
}

fn identifiers(roadmap: &RoadmapSnapshot) -> Vec<String> {
    roadmap
        .waves
        .iter()
        .flat_map(|wave| match roadmap.wave_tasks(wave) {
            Evidence::Ok { items, .. } => items
                .iter()
                .map(|task| task.task.identifier.clone())
                .collect::<Vec<_>>(),
            other => panic!("planning unavailable: {other:?}"),
        })
        .collect()
}

impl Watch {
    /// Start a watch on a Machine, which need not hold a store yet.
    fn open(home: &Path, cwd: &Path, args: &[&str]) -> Self {
        let mut child = lf(home, args)
            .args(["--json", "--watch"])
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let (send, frames) = mpsc::channel();
        std::thread::spawn(move || {
            for line in stdout.lines() {
                let line = line.unwrap();
                let frame: serde_json::Value = serde_json::from_str(&line)
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

    fn next(&self, within: Duration) -> Option<serde_json::Value> {
        self.frames.recv_timeout(within).ok()
    }

    /// The next frame whose body satisfies `accept`.
    fn body(
        &self,
        within: Duration,
        accept: impl Fn(&serde_json::Value) -> bool,
    ) -> serde_json::Value {
        let deadline = Instant::now() + within;
        let mut last = None;
        loop {
            let Some(frame) = self.next(deadline.saturating_duration_since(Instant::now())) else {
                panic!("no matching frame in time; last: {last:?}");
            };
            if accept(&frame["body"]) {
                return frame["body"].clone();
            }
            last = Some(frame);
        }
    }

    /// The next `lf wave show --watch` frame whose Tasks satisfy `accept`.
    fn planning(&self, within: Duration, accept: impl Fn(&[String]) -> bool) -> Vec<String> {
        let deadline = Instant::now() + within;
        loop {
            let frame = self
                .next(deadline.saturating_duration_since(Instant::now()))
                .expect("no matching planning frame in time");
            assert_eq!(frame["unavailable"], serde_json::Value::Null);
            let roadmap: RoadmapSnapshot = serde_json::from_value(frame["body"].clone()).unwrap();
            let tasks = identifiers(&roadmap);
            if accept(&tasks) {
                return tasks;
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
            let roadmap: RoadmapSnapshot = serde_json::from_value(frame["body"].clone()).unwrap();
            let Evidence::Ok { items, .. } = roadmap.wave_tasks(&roadmap.waves[0]) else {
                continue;
            };
            let progress = &items[0].condition.local_progress;
            if (progress.dirty, progress.authored_commits) == (Some(expected.0), Some(expected.1)) {
                return;
            }
        }
    }
}

const PLANNING: &[&str] = &["desktop", "plan", "--all"];

#[test]
fn a_frame_is_its_answer_and_body() {
    let home = Machine::new();
    let watch = home.watch(PLANNING);
    let frame = watch.next(Duration::from_secs(30)).expect("no frame");
    let mut keys: Vec<_> = frame.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["answers", "body", "unavailable"]);
}

#[test]
fn a_store_created_after_the_watch_started_is_shown() {
    let dir = tempfile::tempdir().unwrap();
    let watch = Watch::open(dir.path(), dir.path(), PLANNING);
    assert!(watch.planning(Duration::from_secs(30), |_| true).is_empty());

    let home = Machine::at(dir);
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
    let watch = Watch::open(dir.path(), dir.path(), PLANNING);
    let frame = watch.next(Duration::from_secs(30)).expect("watch ended");
    assert_eq!(frame["body"], serde_json::Value::Null);
    assert!(frame["unavailable"].is_string());
}

#[test]
fn a_steady_writer_is_shown_while_it_writes() {
    let home = Machine::new();
    let watch = home.watch(PLANNING);
    watch.planning(Duration::from_secs(30), |_| true);
    let mut shown = None;
    let mut latest = 0;
    for count in 1..=60 {
        home.plan(count);
        std::thread::sleep(Duration::from_millis(50));
        while let Ok(frame) = watch.frames.try_recv() {
            let roadmap: RoadmapSnapshot = serde_json::from_value(frame["body"].clone()).unwrap();
            latest = identifiers(&roadmap).len();
            shown.get_or_insert(count);
        }
    }
    let count = shown.expect("no planning frame until the writer stopped");
    assert!(
        count < 60,
        "first frame arrived only as the writer finished"
    );
    if latest != 60 {
        watch.planning(Duration::from_secs(10), |tasks| tasks.len() == 60);
    }
}

#[test]
fn a_paused_watch_and_a_truncated_log_both_catch_up() {
    let home = Machine::new();
    let watch = home.watch(PLANNING);
    watch.planning(Duration::from_secs(30), |_| true);

    // SAFETY: signals this test's own child, which `Watch` still owns.
    unsafe { libc::kill(watch.child.id() as libc::pid_t, libc::SIGSTOP) };
    home.plan(1);
    std::thread::sleep(Duration::from_millis(500));
    // SAFETY: as above.
    unsafe { libc::kill(watch.child.id() as libc::pid_t, libc::SIGCONT) };
    watch.planning(Duration::from_secs(5), |tasks| tasks == ["FIX-1"]);

    home.raw()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    home.plan(2);
    watch.planning(Duration::from_secs(5), |tasks| tasks.len() == 2);
}

#[test]
fn a_confirmation_is_answered_even_when_nothing_changed() {
    let home = Machine::new();
    home.plan(1);
    let mut watch = home.watch(PLANNING);
    watch.planning(Duration::from_secs(30), |_| true);
    watch.request(serde_json::json!({"id": 7}));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let frame = watch
            .next(deadline.saturating_duration_since(Instant::now()))
            .expect("confirmation never answered");
        if frame["answers"] == 7 {
            assert!(frame["body"].is_object());
            break;
        }
    }
}

#[test]
fn a_watch_ends_with_its_parent_while_stdin_stays_open() {
    let home = Machine::new();
    // The parent hands its own stdin to the watch, then waits. This test keeps
    // that pipe's write end open, so only the parent's exit can end the watch.
    let mut parent = on_machine(home.path(), Command::new("/bin/sh"))
        .args([
            "-c",
            r#"exec 3<&0; "$0" desktop plan --all --json --watch <&3 >/dev/null & echo $!; exec sleep 600"#,
        ])
        .arg(loopflow_test_support::lf_binary())
        .current_dir(home.repo())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(parent.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let watch: libc::pid_t = line.trim().parse().unwrap();
    let alive = |pid: libc::pid_t| {
        let state = Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .unwrap();
        let state = String::from_utf8_lossy(&state.stdout);
        !state.trim().is_empty() && !state.trim_start().starts_with('Z')
    };
    std::thread::sleep(Duration::from_secs(1));
    assert!(alive(watch), "the watch never started");

    parent.kill().unwrap();
    parent.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while alive(watch) {
        if Instant::now() > deadline {
            // SAFETY: the orphaned watch this test started.
            unsafe { libc::kill(watch, libc::SIGKILL) };
            panic!("the watch outlived its parent");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    drop(parent.stdin.take());
}

#[cfg(target_os = "macos")]
#[test]
fn a_checkout_changed_on_disk_is_shown() {
    let home = Machine::at(tempfile::tempdir().unwrap());
    home.plan(1);
    let worktree = home.start(1);
    let watch = home.watch(PLANNING);
    watch.checkout((false, false));

    std::fs::write(worktree.join("note.txt"), "draft").unwrap();
    watch.checkout((true, false));

    git(&worktree, &["add", "note.txt"]);
    git(&worktree, &["commit", "--quiet", "-m", "note"]);
    watch.checkout((false, true));
}

/// An offline completion and reopening reach an open client without a refresh:
/// the Task leaves and rejoins the open Tasks, and its status follows.
#[test]
fn offline_completion_and_reopening_reach_an_open_watch() {
    let home = Machine::new();
    home.linear_repository();
    home.plan(1);
    // An open window keeps the repository's planning exchanged; no Task is selected.
    let _sync = home.watch(&["planning", "status"]);
    let tasks = home.watch(&["task", "list"]);
    let status = home.watch(&["desktop", "task", "FIX-1"]);
    let listed = |body: &serde_json::Value, listed: bool| {
        body.as_array()
            .is_some_and(|rows| rows.iter().any(|row| row["identifier"] == "FIX-1") == listed)
    };
    tasks.body(Duration::from_secs(30), |body| listed(body, true));
    for (operation, completed, open) in [("complete", true, false), ("reopen", false, true)] {
        home.run(&["task", operation, "FIX-1"]);
        tasks.body(Duration::from_secs(10), |body| listed(body, open));
        // The tracker is unreachable: the saved decision shows with its failed delivery.
        status.body(Duration::from_secs(10), |body| {
            body["task"]["completed_at"].is_null() != completed
                && body["sync"]["changes"].as_array().is_some_and(|changes| {
                    changes
                        .iter()
                        .any(|change| change["field"] == "state" && !change["error"].is_null())
                })
        });
    }
    let reopened = home.watch(&["task", "list"]);
    reopened.body(Duration::from_secs(30), |body| listed(body, true));
    let task = home.store.task_by_issue("FIX-1").unwrap().unwrap();
    assert!(task.worktree.is_none());
    assert!(home.store.task_prs(&task.id).unwrap().is_empty());
}

#[test]
fn an_offline_comment_reaches_the_open_task_status() {
    let home = Machine::new();
    home.linear_repository();
    home.plan(1);
    let watch = home.watch(&["task", "status", "FIX-1"]);
    let thread = |watch: &Watch, count: usize| {
        let body = watch.body(Duration::from_secs(30), |body| {
            body["comments"]["comments"].as_array().map(Vec::len) == Some(count)
        });
        serde_json::from_value::<loopflow::lf::commands::task_row::TaskThread>(
            body["comments"].clone(),
        )
        .unwrap()
    };
    assert!(thread(&watch, 0).pending_sync.is_empty());
    let output = home.run(&["task", "comment", "FIX-1", "Saved during outage", "--json"]);
    let saved: loopflow::work::task::comment::TaskComments =
        serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(saved.pending_sync, [saved.comments[0].id.clone()]);
    let saved = loopflow::lf::commands::task_row::TaskThread::from(saved);
    assert_eq!(thread(&watch, 1), saved);
    let reopened = home.watch(&["task", "status", "FIX-1"]);
    assert_eq!(thread(&reopened, 1), saved);
}

#[test]
fn peer_planning_receipts_reach_an_open_watch() {
    use loopflow::planning_git::PlanningDestination;
    use loopflow::store::PeerPlanningStatus;

    let home = Machine::new();
    let watch = home.watch(&["planning", "status"]);
    let repo = home.wave.repo();
    let read = |accept: &dyn Fn(&[PeerPlanningStatus]) -> bool| {
        let body = watch.body(Duration::from_secs(10), |body| {
            serde_json::from_value::<Vec<PeerPlanningStatus>>(body["destinations"].clone())
                .is_ok_and(|statuses| accept(&statuses))
        });
        serde_json::from_value::<Vec<PeerPlanningStatus>>(body["destinations"].clone()).unwrap()
    };
    assert!(read(&|statuses| statuses.is_empty()).is_empty());
    // Non-Git Work folders still display stored status; foreground acquisition
    // reports the unavailable destination without discarding local planning.
    let destination = PlanningDestination::new(
        home.path().join("offline.git").to_str().unwrap(),
        "refs/loopflow/planning/shared/team",
    )
    .unwrap();
    let id = home.store.bind_peer_planning(repo, &destination).unwrap();
    home.store.export_peer_planning(repo, &id).unwrap();
    let statuses = read(&|statuses| {
        statuses
            .iter()
            .any(|status| status.id == id && status.selected_records > 0)
    });
    assert!(!statuses[0].active);
    assert_eq!(statuses.len(), 1);
    assert!(statuses[0].pending_local);

    home.raw()
        .execute(
            "UPDATE planning_destinations SET fetched_revision='fetched',
         publication_revision='attempted',publication_state='unconfirmed',
         publication_error='reply lost' WHERE id=?1",
            [&id],
        )
        .unwrap();
    let statuses = read(&|statuses| {
        statuses[0].publication_state.as_deref() == Some("unconfirmed")
            && statuses[0].fetched_revision.as_deref() == Some("fetched")
    });
    assert_eq!(statuses[0].imported_revision, None);
    let config_dir = Path::new(repo).join(".lf");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.yaml"),
        "planning: {provider: linear}\n",
    )
    .unwrap();
    assert!(read(&|statuses| statuses.is_empty()).is_empty());
    std::fs::write(
        config_dir.join("config.yaml"),
        "planning: {provider: git, shared: team}\n",
    )
    .unwrap();
    let statuses = read(&|statuses| !statuses.is_empty());
    assert_eq!(
        statuses[0].publication_state.as_deref(),
        Some("unconfirmed")
    );
}

/// The `git` commands one `lf wave show --all --json` runs, recorded by a shim
/// ahead of the real `git` on PATH.
fn planning_git_commands(home: &Path) -> Vec<String> {
    let shim = tempfile::tempdir().unwrap();
    let log = shim.path().join("calls");
    let path = std::env::var_os("PATH").unwrap();
    let git = std::env::split_paths(&path)
        .map(|dir| dir.join("git"))
        .find(|candidate| candidate.is_file())
        .unwrap();
    let script = shim.path().join("git");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
            log.display(),
            git.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let mut search = vec![shim.path().to_path_buf()];
    search.extend(std::env::split_paths(&path));
    let output = lf(home, &["desktop", "plan", "--all", "--json"])
        .env("PATH", std::env::join_paths(search).unwrap())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice::<RoadmapSnapshot>(&output.stdout).expect("no planning reading");
    std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Every Desktop window waits on this reading. On October 18, 2026 Jack Heart's
/// Machine took over a minute to produce it, mostly spawning `git` for Waves
/// whose repositories were deleted test directories, so the window stayed on
/// “Updating…” with no working Tasks.
#[test]
fn planning_runs_git_per_repository_not_per_wave() {
    let home = Machine::new();
    home.plan(3);
    let baseline = planning_git_commands(home.path());

    for index in 0..8 {
        let gone = home.path().join(format!("deleted-{index}"));
        let wave = Wave::new(
            WaveId::new(),
            format!("gone-{index}"),
            gone.display().to_string(),
        );
        home.store.create_wave(&wave).unwrap();
        let sibling = Wave::new(
            WaveId::new(),
            format!("sibling-{index}"),
            home.wave.repo().into(),
        );
        home.store.create_wave(&sibling).unwrap();
    }
    let grown = planning_git_commands(home.path());

    assert!(
        grown.len() <= baseline.len(),
        "baseline: {baseline:#?}\ngrown: {grown:#?}"
    );
}

/// A planning reading asks Git only whether each live checkout is dirty; its
/// HEAD and Git layout are read from disk. On October 18, 2026 Jack Heart's
/// 55 live checkouts cost three commands each.
#[cfg(target_os = "macos")]
#[test]
fn planning_asks_git_once_per_live_checkout() {
    let home = Machine::new();
    home.plan(4);
    home.start(1);
    let one = planning_git_commands(home.path());
    for index in 2..=4 {
        home.start(index);
    }
    let four = planning_git_commands(home.path());

    assert!(
        four.len() <= one.len() + 3,
        "one checkout: {one:#?}\nfour checkouts: {four:#?}"
    );
}

/// Remove the fields that restate when a result was read.
fn without_reading_time(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            fields.remove("generated_at");
            fields.remove("evidence_age_secs");
            if let Some(serde_json::Value::Object(condition)) = fields.get_mut("condition") {
                condition.remove("observed_at");
            }
            fields.values_mut().for_each(without_reading_time);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(without_reading_time),
        _ => {}
    }
}

#[test]
fn an_operation_served_in_process_reads_what_its_lf_command_prints() {
    use loopflow::lf::commands::activity::HistoryWindow;
    use loopflow::lf::commands::operation::Operation;

    let home = Machine::new();
    home.plan(2);
    let history = Operation::History(HistoryWindow {
        since: "7d".into(),
        limit: 50,
        wave: None,
        project: None,
        task: None,
    });
    // A new Operation fails to compile here until it is compared too.
    let operations = match history {
        Operation::History(_) | Operation::MonitorPs | Operation::DesktopPlan => {
            [history, Operation::MonitorPs, Operation::DesktopPlan]
        }
    };
    let _env = loopflow_test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let saved = std::env::var_os("LF_HOME");
    std::env::set_var("LF_HOME", home.path());
    for operation in operations {
        let argv = operation.argv();
        let (sender, wakes) = mpsc::channel();
        let sender = std::sync::Mutex::new(sender);
        let subscription = operation
            .subscribe(move || {
                let _ = sender.lock().unwrap().send(());
            })
            .unwrap();
        // The app pulls on its own thread after a data-free wakeup.
        wakes.recv_timeout(Duration::from_secs(30)).unwrap();
        let mut served: serde_json::Value =
            serde_json::from_slice(&subscription.pull().expect("a first frame")).unwrap();
        drop(subscription);
        assert!(served["unavailable"].is_null(), "{argv:?}: {served}");

        let mut args: Vec<&str> = argv.iter().map(String::as_str).collect();
        args.push("--json");
        let output = lf(home.path(), &args).output().unwrap();
        assert!(output.status.success(), "{argv:?} failed");
        let mut printed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

        without_reading_time(&mut served["body"]);
        without_reading_time(&mut printed);
        assert_eq!(served["body"], printed, "{argv:?}");
    }
    match saved {
        Some(saved) => std::env::set_var("LF_HOME", saved),
        None => std::env::remove_var("LF_HOME"),
    }
}

/// The `processes` revision every Work reader wakes on.
fn processes_revision(home: &Path) -> i64 {
    rusqlite::Connection::open(home.join("loopflow.db"))
        .unwrap()
        .query_row(
            "SELECT COALESCE((SELECT revision FROM store_revisions WHERE domain = 'processes'), 0)",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn reads_record_no_process_and_wake_no_reader() {
    let home = Machine::new();
    home.plan(1);
    let before = processes_revision(home.path());
    for args in [
        &["history", "--json"][..],
        &["monitor", "ps", "--json"],
        &["wave", "list", "--all", "--json"],
        &["desktop", "plan", "--all", "--json"],
        &["history", "list", "--json", "--all"],
    ] {
        let output = lf(home.path(), args).output().unwrap();
        assert!(output.status.success(), "{args:?} failed");
    }
    assert_eq!(processes_revision(home.path()), before);
}
