//! The failed-PR ci-fix lifecycle, driven end to end as one deterministic state
//! machine: pending -> failing -> exactly one armed body -> same-PR push ->
//! new-head rearm -> green -> waiting, plus restart and infra-blocked behaviour.
//!
//! **Why this lives in-crate rather than in `tests/`.** Every function the
//! lifecycle turns on is private or `pub(crate)` — `arm_ci_fix_wake`,
//! `reconcile_task_pr_with_authority`, `observe_required_checks`,
//! `read_check_set`. A `tests/` binary links `loopflow` as an external consumer
//! and can reach none of them. Widening that surface so a test could see it is
//! what CLAUDE.md forbids ("Never reshape production code for tests"), so the
//! proof moves to the code instead. As a descendant module of `task::runner`
//! this sees `super::arm_ci_fix_wake` for free: private items are visible to
//! descendants. `ops/child.rs`'s own wake test is in-crate for the same reason.
//!
//! **Why the env guard is duplicated.** `tests/support/mod.rs`'s `EnvGuard`
//! does exactly this job, but it compiles into each *integration test binary*,
//! not into the lib — a `#[cfg(test)]` module in `src/` cannot see it, and a lib
//! test binary is a separate process from every `tests/` binary, so even its
//! mutex would not be shared. The duplication is forced by Rust's test
//! architecture, not chosen. Do not try to DRY the two together.

use std::sync::{Mutex, MutexGuard, OnceLock};

use loopflow_test_support::TestRepo;
use tempfile::TempDir;
use time::OffsetDateTime;

use crate::child_session::{ChildLeaseState, ChildProcessGeneration, ChildWriteLease};
use crate::id::WaveId;
use crate::project_session::{ProjectSession, ProjectSessionId, ProjectSessionStatus};
use crate::session_context::{
    LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
    ProjectLaunchReceipt, TaskLaunchReceipt,
};
use crate::store::{open_store, SharedStore, StorageConfig};
use crate::task::{
    AfterMerge, CiState, GithubPr, PrPublication, TaskPr, TaskPrId, TaskSession, TaskSessionId,
    TaskSessionStatus,
};
use crate::wave::Wave;

// ---------------------------------------------------------------------------
// Ambient environment isolation
// ---------------------------------------------------------------------------

/// Mirrors `AMBIENT_TASK_ENV` in `rust/loopflow/tests/support/mod.rs`. **That
/// constant is the source of truth**; this copy exists only because that module
/// is unreachable from the lib (see the module header). Keep the two in sync.
///
/// These are not incidental: a live `lf __task` exports them, and production
/// reads them on the exact paths this proof drives. `ops/task.rs` gates task
/// control on `LF_PROJECT_SESSION_ID` before it consults the ambient Wave, and
/// `resolve_child_command_source` refuses when the ambient Wave does not own the
/// Task ("Wave X cannot control Task Y"). Left set, this suite would fail inside
/// a Task Session and pass in CI.
const AMBIENT_TASK_ENV: [&str; 5] = [
    "LF_TASK_SESSION_ID",
    "LF_TASK_GENERATION",
    "LF_TASK_LEASE_TOKEN",
    "LF_WAVE_ID",
    "LF_PROJECT_SESSION_ID",
];

// This guard deliberately does NOT touch `LF_HOME`/`LF_CONTROL_*`. An earlier
// draft redirected them at a temp dir, reasoning that `resolve_task_authority`
// resolves the global registry and could reach a developer's live control DB.
// Measured: that path is never driven here. `reconcile_task_pr_with_authority`
// takes `Some(lease)` and reads only the store it is handed, and this suite
// opens its own SQLite store in a `TempDir`, so nothing resolves a home. The
// redirect was not merely redundant — it was harmful: `trace.rs` reads `LF_HOME`
// and the lib test binary runs its ~1400 tests in threads, so pointing that var
// at a temp dir failed `trace::tests::capture_persists_private_artifacts_and_
// queryable_rows` from another thread. Mutate the minimum; a private mutex
// serializes this guard against itself, never against the rest of the binary.

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Puts a fake `gh` on `PATH`, clears the ambient Task identity, and points the
/// store home at a temp dir. Env is process-global and the lib test binary runs
/// tests in threads, so the guard serializes on a process-wide mutex and
/// restores every prior value on drop — including on panic.
struct AmbientGuard {
    _lock: MutexGuard<'static, ()>,
    previous: Vec<(String, Option<std::ffi::OsString>)>,
    _bin: TempDir,
    _state: TempDir,
    state_dir: std::path::PathBuf,
}

impl AmbientGuard {
    fn new(gh_script: &str) -> Self {
        let lock = env_lock().lock().unwrap_or_else(|err| err.into_inner());
        let bin = TempDir::new().expect("temp bin dir");
        let state = TempDir::new().expect("temp gh state dir");

        let mut previous = Vec::new();
        let mut remember = |name: &str| {
            previous.push((name.to_string(), std::env::var_os(name)));
        };
        for name in AMBIENT_TASK_ENV {
            remember(name);
        }
        remember("PATH");
        remember("LF_TEST_GH_DIR");

        write_fake_gh(bin.path(), gh_script);

        for name in AMBIENT_TASK_ENV {
            std::env::remove_var(name);
        }

        let path = match std::env::var("PATH") {
            Ok(prev) => format!("{}:{}", bin.path().display(), prev),
            Err(_) => bin.path().display().to_string(),
        };
        std::env::set_var("PATH", path);
        std::env::set_var("LF_TEST_GH_DIR", state.path());

        let state_dir = state.path().to_path_buf();
        Self {
            _lock: lock,
            previous,
            _bin: bin,
            _state: state,
            state_dir,
        }
    }
}

impl Drop for AmbientGuard {
    fn drop(&mut self) {
        for (name, value) in &self.previous {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

fn write_fake_gh(dir: &std::path::Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("gh");
    std::fs::write(&path, body).expect("write fake gh");
    let mut perms = std::fs::metadata(&path)
        .expect("stat fake gh")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).expect("chmod fake gh");
}

// ---------------------------------------------------------------------------
// The fake `gh`
// ---------------------------------------------------------------------------

/// Answers the two calls the observation path makes, from state files the test
/// rewrites between phases.
///
/// Two properties are deliberate. It **fails loudly** on any unexpected
/// invocation (`release_tests.rs`'s style, not `pr_tests.rs`'s `exit 0`): a
/// permissive fake answers a wrong call with empty JSON, which parses as *green*
/// and silently inverts this suite's verdict. And it **honours `--required`**,
/// serving a different list than the full read, because production reads the
/// gate from the required set but seeds the ci-fix turn from the full set minus
/// the required names — this repo's only merge-gating check is the
/// `tests-result` roll-up, and the full read is what makes the seed name the
/// job that actually broke. No `jq`: the test pre-renders the JSON, so the fake
/// is a `cat` and runs on a host without it.
const FAKE_GH: &str = r#"#!/bin/sh
if [ "$1" = "--version" ]; then echo "gh version 2.40.0 (fake)"; exit 0; fi
case "$1" in
  api)
    cat "$LF_TEST_GH_DIR/pr.json"; exit 0;;
  pr)
    if [ "$2" = "checks" ]; then
      for arg in "$@"; do
        if [ "$arg" = "--required" ]; then cat "$LF_TEST_GH_DIR/required.json"; exit 0; fi
      done
      cat "$LF_TEST_GH_DIR/full.json"; exit 0
    fi;;
esac
echo "fake gh: unexpected invocation: $*" >&2
exit 90
"#;

struct FakeGh {
    dir: std::path::PathBuf,
}

impl FakeGh {
    fn new(dir: std::path::PathBuf) -> Self {
        Self { dir }
    }

    /// The PR read (`gh api repos/{nwo}/pulls/{n}`).
    fn set_pr(&self, number: u32, state: &str, head_sha: &str) {
        let body = serde_json::json!({
            "number": number,
            "state": state,
            "merged_at": serde_json::Value::Null,
            "merge_commit_sha": serde_json::Value::Null,
            "html_url": format!("https://github.com/test/repo/pull/{number}"),
            "head": { "sha": head_sha },
        });
        std::fs::write(
            self.dir.join("pr.json"),
            serde_json::to_string(&body).unwrap(),
        )
        .expect("write pr.json");
    }

    /// The two check reads. `required` is the gate; `full` is what the ci-fix
    /// seed is built from.
    fn set_checks(&self, required: &[(&str, &str)], full: &[(&str, &str)]) {
        for (file, checks) in [("required.json", required), ("full.json", full)] {
            let body: Vec<serde_json::Value> = checks
                .iter()
                .map(|(name, bucket)| {
                    serde_json::json!({
                        "name": name,
                        "bucket": bucket,
                        "link": format!("https://ci.example/{name}"),
                    })
                })
                .collect();
            std::fs::write(self.dir.join(file), serde_json::to_string(&body).unwrap())
                .expect("write checks");
        }
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn make_wave(repo: &str) -> Wave {
    let id = WaveId::new();
    Wave::new(id.clone(), format!("wave-{id}"), repo.to_string())
}

fn make_project(wave: &Wave) -> ProjectSession {
    let now = OffsetDateTime::now_utc();
    ProjectSession {
        id: ProjectSessionId::new(),
        launch: ProjectLaunchReceipt {
            project: LinearProjectSnapshot {
                id: LinearProjectId::new(format!("project-{}", WaveId::new())).unwrap(),
                slug: "developer-efficiency".to_string(),
                name: "Developer Efficiency".to_string(),
                prompt_context: "Keep the loop fast.".to_string(),
            },
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        wave_id: wave.id().clone(),
        current_directive_version: 0,
        incorporated_directive_version: 0,
        status: ProjectSessionStatus::Waiting,
        status_reason: "ci-fix lifecycle fixture".to_string(),
        status_at: now,
        iteration: 1,
        observation_cursor: 0,
        last_state_fingerprint: None,
        agent: "claude".to_string(),
        provider: "claude".to_string(),
        provider_session_id: None,
        latest_process: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    }
}

fn make_task(wave: &Wave, project: &ProjectSession, worktree: &std::path::Path) -> TaskSession {
    let now = OffsetDateTime::now_utc();
    let id = WaveId::new();
    TaskSession {
        id: TaskSessionId::new(),
        launch: TaskLaunchReceipt {
            issue: LinearIssueSnapshot {
                id: LinearIssueId::new(format!("issue-{id}")).unwrap(),
                identifier: "W2-229".to_string(),
                title: "Prove failed-PR ci-fix recovery".to_string(),
                description: "One failed head wakes exactly one ci-fix body.".to_string(),
            },
            project: project.launch.project.clone(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        pm_writeback: crate::task::PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_session_id: project.id.clone(),
        current_directive_version: 1,
        incorporated_directive_version: 1,
        status: TaskSessionStatus::Waiting,
        status_reason: "waiting on review".to_string(),
        status_at: now,
        worktree: worktree.to_path_buf(),
        workspace_slug: "ci-fix-lifecycle".to_string(),
        lifecycle: crate::task::TaskLifecyclePlan::standard("task"),
        lifecycle_phase: crate::task::TaskLifecyclePhase::Iterate,
        phase_epoch: 1,
        phase_cursor: 0,
        phase_iteration: 0,
        gate_cycle: 0,
        gate_proposal: None,
        agent: "claude".to_string(),
        provider: "claude".to_string(),
        provider_session_id: None,
        observation: crate::task::Observation::NotRequired,
        latest_process: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    }
}

/// A sequence-1 **Working** PR: the store refuses to create a Task Session with
/// anything else ("Task Session requires its sequence-1 Working PR"). The
/// publication is applied by [`publish`] afterwards, which is the real order —
/// a PR exists before it is published.
fn make_task_pr(task: &TaskSession) -> TaskPr {
    let now = OffsetDateTime::now_utc();
    TaskPr {
        id: TaskPrId::new(),
        task_session_id: task.id.clone(),
        sequence: 1,
        slug: task.workspace_slug.clone(),
        branch: format!("test/{}", task.workspace_slug),
        base_commit: "0".repeat(40),
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
    }
}

/// Publish the PR onto GitHub, the state this lifecycle starts from: a Task
/// sleeping on an open PR whose head is about to be observed.
fn publish(pr: &mut TaskPr, number: u32, head_sha: &str) {
    pr.publication = Some(PrPublication {
        requested_at: pr.updated_at,
        after_merge: AfterMerge::Review,
        next_slug: None,
        github: Some(GithubPr {
            number,
            url: format!("https://github.com/test/repo/pull/{number}"),
            head_sha: Some(head_sha.to_string()),
        }),
    });
}

/// `observe_pr_by_number` resolves owner/repo from `remote.origin.url`, and
/// `github_repo_nwo` parses only github.com URLs — a `TestRepo`'s origin is a
/// local bare path, so the read would degrade before CI is ever consulted. This
/// is `pr_tests.rs`'s `point_origin_at_github` idiom. It destroys push, which
/// costs nothing here: this proof's "push" is a fake-`gh` head change.
fn point_origin_at_github(repo: &TestRepo) {
    let status = std::process::Command::new("git")
        .args([
            "remote",
            "set-url",
            "origin",
            "https://github.com/test/repo.git",
        ])
        .current_dir(repo.path())
        .status()
        .expect("git remote set-url");
    assert!(status.success(), "git remote set-url failed");
}

struct Harness {
    _guard: AmbientGuard,
    _dir: TempDir,
    // Kept alive: the worktree gh runs in must outlive the harness.
    _repo: TestRepo,
    gh: FakeGh,
    store: SharedStore,
    task: TaskSession,
    lease: ChildWriteLease,
    pr_number: u32,
}

impl Harness {
    async fn new() -> Self {
        let guard = AmbientGuard::new(FAKE_GH);
        let gh = FakeGh::new(guard.state_dir.clone());
        let dir = TempDir::new().expect("temp store dir");
        let repo = TestRepo::new();
        point_origin_at_github(&repo);

        let store: SharedStore = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .expect("open store"),
        );

        let wave = make_wave(repo.path().to_str().unwrap());
        store.create_wave(&wave).await.expect("create wave");
        let project = make_project(&wave);
        store
            .create_project_session(&project)
            .await
            .expect("create project session");

        let mut task = make_task(&wave, &project, repo.path());
        let pr_number = 1009;
        let mut pr = make_task_pr(&task);
        store
            .create_task_session(&task, &pr)
            .await
            .expect("create task session");
        publish(&mut pr, pr_number, "h1");
        store.update_task_pr(&pr).await.expect("publish the pr");

        // A real lease from the real reservation path rather than a hand-built
        // one — a fabricated token would not match the stored lease and every
        // write under it would be refused. This is the production launch: a
        // Waiting Task reserves generation 1 and starts running, which is the
        // state a body is in when it arms its own ci-fix wake
        // (`runner.rs` calls `arm_ci_fix_wake` with exactly this lease).
        let mut launching = task.clone();
        launching.status = TaskSessionStatus::Running;
        launching.status_reason = "ci-fix lifecycle body".to_string();
        launching.latest_process = Some(ChildProcessGeneration {
            generation: 1,
            pid: None,
            process_group_id: None,
            tmux_name: "lf-ci-fix-lifecycle".to_string(),
            agent: "claude".to_string(),
            provider: "claude".to_string(),
            provider_session_id: None,
            started_at: OffsetDateTime::now_utc(),
            state: ChildLeaseState::Active,
            outcome: None,
            provenance: None,
        });
        let lease = store
            .reserve_task_process(&launching, TaskSessionStatus::Waiting)
            .await
            .expect("reserve task process")
            .expect("a waiting task reserves generation 1");
        // A reserved lease cannot write yet; the body activates it before it
        // touches anything, exactly as `runner.rs` does before arming.
        store
            .activate_task_process(&launching, &lease)
            .await
            .expect("activate the reserved process");
        task = store
            .get_task_session(&task.id)
            .await
            .expect("read task")
            .expect("task exists");

        Self {
            _guard: guard,
            _dir: dir,
            _repo: repo,
            gh,
            store,
            task,
            lease,
            pr_number,
        }
    }

    /// Reconcile, having first let the read cache expire.
    ///
    /// **Without the expiry this whole suite is a tautology.** The observation
    /// path coalesces GitHub reads for `PR_OBSERVATION_TTL` (60s, `ops/task.rs`)
    /// — `cached_github_observation` short-circuits before `gh` is ever
    /// spawned. Every phase here lands within milliseconds of the last, so a
    /// plain reconcile loop would serve the first reading back forever: the
    /// checks would "never go red", and the gh-outage test would pass without
    /// gh being consulted at all. Backdating `checked_at` past the TTL is how
    /// the suite models the passage of a minute (`ops/task.rs` tests use the
    /// same trick from the other side, pinning that 59s is still fresh).
    async fn reconcile(&mut self) -> Option<TaskPr> {
        self.expire_read_cache().await;
        crate::ops::task::reconcile_task_pr_for_lease(&self.store, &mut self.task, &self.lease)
            .await
            .expect("reconcile the task PR")
    }

    /// Age the last GitHub read past both the fresh TTL and the degraded
    /// backoff, so the next reconcile really talks to the fake `gh`.
    async fn expire_read_cache(&self) {
        let Some(mut pr) = self
            .store
            .active_task_pr(&self.task.id)
            .await
            .expect("read active pr")
        else {
            return;
        };
        let Some(observation) = pr.github_observation.as_mut() else {
            return;
        };
        observation.checked_at = OffsetDateTime::now_utc() - time::Duration::minutes(10);
        self.store
            .update_task_pr_for_lease(&pr, &self.lease)
            .await
            .expect("expire the read cache");
    }

    async fn arm(&self) -> bool {
        super::arm_ci_fix_wake(&self.store, &self.task, &self.lease)
            .await
            .expect("arm the ci-fix wake")
    }

    async fn observation(&self) -> Option<crate::task::CiObservation> {
        self.store
            .active_task_pr(&self.task.id)
            .await
            .expect("read active pr")
            .and_then(|pr| pr.ci_observation)
    }

    async fn incidents(&self) -> Vec<crate::store::ci_incidents::CiIncidentReportRow> {
        self.store
            .ci_incidents_since(
                self.task.created_at - time::Duration::seconds(1),
                None,
                Some("test/repo"),
            )
            .await
            .expect("read CI incidents")
    }

    /// The gate is the `tests-result` roll-up; the leaves are what actually
    /// broke. Mirrors this repo's real branch protection.
    fn checks_failing(&self) {
        self.gh.set_checks(
            &[("tests-result", "fail")],
            &[
                ("tests-result", "fail"),
                ("cargo-fmt", "fail"),
                ("clippy", "fail"),
                ("docs", "pass"),
            ],
        );
    }

    fn checks_pending(&self) {
        self.gh.set_checks(
            &[("tests-result", "pending")],
            &[("tests-result", "pending"), ("cargo-fmt", "pending")],
        );
    }

    fn checks_passing(&self) {
        self.gh.set_checks(
            &[("tests-result", "pass")],
            &[("tests-result", "pass"), ("cargo-fmt", "pass")],
        );
    }

    fn head(&self, sha: &str) {
        self.gh.set_pr(self.pr_number, "open", sha);
    }
}

// ---------------------------------------------------------------------------
// The lifecycle
// ---------------------------------------------------------------------------

/// The whole claim in one pass: a red head wakes exactly one body, a duplicate
/// delivery does not wake a second, a restart does not either, a push rearms,
/// and green settles back to waiting.
#[tokio::test]
async fn a_failed_head_wakes_exactly_one_ci_fix_body_and_rearms_until_green() {
    let mut harness = Harness::new().await;

    // 1. Pending: nothing to repair yet.
    harness.head("h1");
    harness.checks_pending();
    harness.reconcile().await;
    let observation = harness
        .observation()
        .await
        .expect("a pending reading lands");
    assert_eq!(observation.state, CiState::Pending);
    assert_eq!(observation.head_sha, "h1");
    assert!(!harness.arm().await, "a pending head must not wake a body");

    // 2. Red: the gate fails and the seed names the broken leaves, not the
    //    `tests-result` aggregate whose link is only the roll-up.
    harness.checks_failing();
    harness.reconcile().await;
    let observation = harness
        .observation()
        .await
        .expect("a failing reading lands");
    assert_eq!(observation.state, CiState::Failing);
    let names: Vec<&str> = observation
        .failing_checks
        .iter()
        .map(|check| check.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["cargo-fmt", "clippy"],
        "the seed carries the failing leaves parsed from gh's full read"
    );
    let incidents = harness.incidents().await;
    assert_eq!(
        incidents.len(),
        1,
        "the first failed head opens an incident"
    );
    assert_eq!(incidents[0].incident.responded_at, None);

    // 3. Exactly one body.
    assert!(harness.arm().await, "a red head wakes a ci-fix body");
    assert!(
        harness.incidents().await[0].incident.responded_at.is_some(),
        "body birth records the response milestone"
    );
    assert!(
        !harness.arm().await,
        "a duplicate delivery must not wake a second body"
    );

    // 4. Restart: reconciling again on the same head with the same failing set
    //    carries the dedup marker forward, so it still does not re-arm.
    harness.reconcile().await;
    assert!(
        !harness.arm().await,
        "a restart between observation and settlement must not wake a second body"
    );

    // 5. The push: a new head makes the old reading stale, so a fresh reading is
    //    taken and the still-red head rearms.
    harness.head("h2");
    harness.reconcile().await;
    let observation = harness
        .observation()
        .await
        .expect("a reading for the new head");
    assert_eq!(observation.head_sha, "h2");
    assert!(
        harness.arm().await,
        "a new failing head rearms rather than staying deduped against the old one"
    );
    assert_eq!(
        harness.incidents().await.len(),
        2,
        "each failed repair head is one measurable attempt"
    );

    // 6. Green: the repair worked and the Task settles back to waiting.
    harness.checks_passing();
    harness.reconcile().await;
    let observation = harness
        .observation()
        .await
        .expect("a passing reading lands");
    assert_eq!(observation.state, CiState::Passing);
    assert!(!harness.arm().await, "a green head must not wake a body");
    assert!(
        harness
            .incidents()
            .await
            .iter()
            .all(|incident| incident.incident.green_at.is_some()),
        "the passing head closes every open attempt on the PR"
    );
}

/// The carry-forward is conditional on the failing *set*, not just the head: a
/// check that breaks after a wake already fired is a new failure and earns a
/// new body.
#[tokio::test]
async fn a_changed_failing_set_on_the_same_head_rearms() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_failing();
    harness.reconcile().await;
    assert!(harness.arm().await, "the first failing set wakes a body");
    assert!(!harness.arm().await, "and only one");

    // Same head, a different failing set.
    harness.gh.set_checks(
        &[("tests-result", "fail")],
        &[
            ("tests-result", "fail"),
            ("cargo-fmt", "fail"),
            ("clippy", "fail"),
            ("rust-test", "fail"),
        ],
    );
    harness.reconcile().await;
    assert!(
        harness.arm().await,
        "a check that broke after the wake fired is a new failure, not a duplicate"
    );
}

/// Infra-blocked: with `gh` gone the PR read degrades. Today's contract is that
/// nothing is invented — the prior observation stands, no false green appears,
/// and local control survives. (W2-231 may turn this into an explicit `Blocked`
/// transition; this pins what main does now.)
#[tokio::test]
async fn a_gh_outage_degrades_the_read_without_inventing_a_reading() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_failing();
    harness.reconcile().await;
    let before = harness
        .observation()
        .await
        .expect("a failing reading lands");
    assert_eq!(before.state, CiState::Failing);

    // Take gh away: PATH keeps the temp bin dir, but the binary is gone.
    std::fs::remove_file(harness._guard._bin.path().join("gh")).expect("remove fake gh");

    harness.reconcile().await;
    let after = harness
        .observation()
        .await
        .expect("the prior reading still stands");
    assert_eq!(
        after.state,
        CiState::Failing,
        "a degraded read must not overwrite the last real reading"
    );
    assert_ne!(
        after.state,
        CiState::Passing,
        "a degraded read must never read as green"
    );
    assert_eq!(after.head_sha, before.head_sha);
}
