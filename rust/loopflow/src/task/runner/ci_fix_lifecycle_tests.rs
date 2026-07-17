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

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex, MutexGuard, OnceLock,
};

use async_trait::async_trait;
use loopflow_test_support::TestRepo;
use tempfile::TempDir;
use time::OffsetDateTime;
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::child_session::{
    ChildCommand, ChildCommandId, ChildCommandKind, ChildCommandSource, ChildCommandState,
    ChildDirective, ChildLeaseState, ChildProcessGeneration, ChildRef, ChildWriteLease,
};
use crate::engine::agent::AgentConfig;
use crate::harness::{Capabilities, Harness as ProviderHarness};
use crate::id::WaveId;
use crate::project_session::{ProjectSession, ProjectSessionId, ProjectSessionStatus};
use crate::session_context::{
    LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
    ProjectLaunchReceipt, TaskLaunchReceipt,
};
use crate::store::{open_store, SharedStore, StorageConfig};
use crate::task::{
    AfterMerge, CiState, GithubPr, PrPhase, PrPublication, TaskPr, TaskPrId, TaskSession,
    TaskSessionId, TaskSessionStatus,
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

struct PushingHarness {
    events: mpsc::UnboundedSender<ConversationEvent>,
    gh_dir: std::path::PathBuf,
    store: SharedStore,
    task_id: TaskSessionId,
    lease: ChildWriteLease,
    /// Provider inputs this body spent. A repair is one turn; a lifecycle step
    /// taken in its name — a `task_clarify` after a Kickoff-to-Iterate — is a
    /// second send and nothing else about the Task would show it.
    sends: Arc<AtomicUsize>,
}

struct LiveIdleHarness {
    events: mpsc::UnboundedSender<ConversationEvent>,
    gh_dir: std::path::PathBuf,
    store: SharedStore,
    task_id: TaskSessionId,
    lease: ChildWriteLease,
    first_turn_active: Arc<AtomicBool>,
    repair_started_while_active: Arc<AtomicBool>,
    sends: Arc<AtomicUsize>,
    interrupts: Arc<AtomicUsize>,
}

#[async_trait]
impl ProviderHarness for PushingHarness {
    async fn start(&mut self, _config: &AgentConfig) -> anyhow::Result<()> {
        Ok(())
    }

    async fn send_input(&mut self, _content: &str) -> anyhow::Result<()> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        let gh = FakeGh::new(self.gh_dir.clone());
        gh.set_pr(1009, "open", "h2");
        gh.set_checks(
            &[("tests-result", "pending")],
            &[("tests-result", "pending"), ("cargo-fmt", "pending")],
        );

        let mut pr = self
            .store
            .active_task_pr(&self.task_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("scripted repair lost its active PR"))?;
        let observation = pr
            .github_observation
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("scripted repair has no cached PR observation"))?;
        observation.checked_at = OffsetDateTime::now_utc() - time::Duration::minutes(10);
        self.store
            .update_task_pr_for_lease(&pr, &self.lease)
            .await?;

        self.events
            .send(ConversationEvent::TurnStarted {
                turn_id: "ci-fix-turn".to_string(),
            })
            .map_err(|_| anyhow::anyhow!("runner dropped the ci-fix event stream"))?;
        self.events
            .send(ConversationEvent::TurnCompleted {
                turn_id: "ci-fix-turn".to_string(),
                status: Lifecycle::Completed,
            })
            .map_err(|_| anyhow::anyhow!("runner dropped the ci-fix event stream"))?;
        Ok(())
    }

    async fn interrupt(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_steer: false,
        }
    }

    fn provider_session_id(&self) -> Option<String> {
        Some("scripted-ci-fix".to_string())
    }
}

#[async_trait]
impl ProviderHarness for LiveIdleHarness {
    async fn start(&mut self, _config: &AgentConfig) -> anyhow::Result<()> {
        Ok(())
    }

    async fn send_input(&mut self, _content: &str) -> anyhow::Result<()> {
        match self.sends.fetch_add(1, Ordering::SeqCst) {
            0 => {
                let session = self
                    .store
                    .get_task_session(&self.task_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("live-idle Task disappeared"))?;
                let pr = self
                    .store
                    .active_task_pr(&self.task_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("live-idle Task lost its PR"))?;
                crate::ops::task::queue_ci_fix_command(&self.store, &session, &pr)
                    .await
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                self.events
                    .send(ConversationEvent::TurnStarted {
                        turn_id: "gate-review-turn".to_string(),
                    })
                    .map_err(|_| anyhow::anyhow!("runner dropped the review event stream"))?;
                let events = self.events.clone();
                let first_turn_active = self.first_turn_active.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(350)).await;
                    first_turn_active.store(false, Ordering::SeqCst);
                    let _ = events.send(ConversationEvent::TurnCompleted {
                        turn_id: "gate-review-turn".to_string(),
                        status: Lifecycle::Completed,
                    });
                });
            }
            1 => {
                if self.first_turn_active.load(Ordering::SeqCst) {
                    self.repair_started_while_active
                        .store(true, Ordering::SeqCst);
                }
                let gh = FakeGh::new(self.gh_dir.clone());
                gh.set_pr(1009, "open", "h2");
                gh.set_checks(
                    &[("tests-result", "pending")],
                    &[("tests-result", "pending"), ("cargo-fmt", "pending")],
                );

                let mut pr = self
                    .store
                    .active_task_pr(&self.task_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("scripted repair lost its active PR"))?;
                let observation = pr.github_observation.as_mut().ok_or_else(|| {
                    anyhow::anyhow!("scripted repair has no cached PR observation")
                })?;
                observation.checked_at = OffsetDateTime::now_utc() - time::Duration::minutes(10);
                self.store
                    .update_task_pr_for_lease(&pr, &self.lease)
                    .await?;

                self.events
                    .send(ConversationEvent::TurnStarted {
                        turn_id: "live-idle-ci-fix".to_string(),
                    })
                    .map_err(|_| anyhow::anyhow!("runner dropped the ci-fix event stream"))?;
                self.events
                    .send(ConversationEvent::TurnCompleted {
                        turn_id: "live-idle-ci-fix".to_string(),
                        status: Lifecycle::Completed,
                    })
                    .map_err(|_| anyhow::anyhow!("runner dropped the ci-fix event stream"))?;
            }
            call => anyhow::bail!("unexpected provider input {}", call + 1),
        }
        Ok(())
    }

    async fn interrupt(&mut self) -> anyhow::Result<()> {
        self.interrupts.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_steer: false,
        }
    }

    fn provider_session_id(&self) -> Option<String> {
        Some("scripted-live-idle".to_string())
    }
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
        let directive = ChildDirective::initial(
            ChildRef::Task(task.id.clone()),
            task.launch.issue.description.clone(),
            ChildCommandSource::System,
        );
        store
            .reserve_task_session_with_directive(&task, &pr, &directive)
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
    ///
    /// Resolves the newest row, mirroring `reconcile_subject`: `active_task_pr`
    /// returns `None` for an abandoned row, which would skip the expiry and leave
    /// reconcile serving a cached reading the reopen never reached.
    async fn expire_read_cache(&self) {
        let Some(mut pr) = self
            .store
            .task_prs(&self.task.id)
            .await
            .expect("read task prs")
            .pop()
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

    /// What the Project runner's observation does: enqueue the wake the current
    /// reading warrants, if any. Mints the payload through the same
    /// `ci_fix_wake_kind` production uses, so a drift between the identity
    /// enqueued and the identity `arm` matches would fail these tests.
    ///
    /// Stops short of `queue_ci_fix_command` only because its tail launches a real
    /// process; `ops::child` covers the launch seam. Returns the surviving command
    /// id and whether this observation minted it.
    async fn enqueue(&self) -> Option<(ChildCommandId, bool)> {
        let pr = self
            .store
            .active_task_pr(&self.task.id)
            .await
            .expect("read active pr")?;
        let kind = crate::ops::task::ci_fix_wake_kind(&pr)?;
        let ChildCommandKind::CiFix {
            ref incident_identity,
            ..
        } = kind
        else {
            unreachable!("ci_fix_wake_kind returns a CiFix");
        };
        let identity = incident_identity.clone();
        let command = ChildCommand::new(
            ChildRef::Task(self.task.id.clone()),
            ChildCommandSource::System,
            kind,
        );
        let (command, created) = self
            .store
            .ensure_child_ci_fix_command(&command)
            .await
            .expect("ensure the ci-fix wake");
        self.store
            .mark_ci_incident_triggered(&identity, &command.id, OffsetDateTime::now_utc())
            .await
            .expect("link the wake to its incident");
        Some((command.id, created))
    }

    /// Kill this generation's body and boot its successor, as recovery does.
    ///
    /// Revokes and finishes the live process, then reserves and activates the
    /// next generation, replacing the harness lease. A real succession, not a
    /// re-arm: `arm()` alone reuses one lease, and `claim_child_commands_in`
    /// skips rows already claimed by the asking generation, so re-arming can
    /// never prove that a *successor* reclaims. Returns the new generation.
    async fn crash_and_relaunch(&mut self) -> u32 {
        let revoked = self
            .store
            .revoke_task_process(
                &self.task.id,
                &crate::child_session::ChildBodyOutcome::Lost {
                    reason: "body died mid-repair".to_string(),
                },
            )
            .await
            .expect("revoke the live generation");
        self.store
            .finish_revoked_task_process(&self.task.id, revoked.generation)
            .await
            .expect("finish the revoked generation");

        let mut successor = self
            .store
            .get_task_session(&self.task.id)
            .await
            .expect("read task")
            .expect("task exists");
        successor.set_status(TaskSessionStatus::Waiting, "body died; recovering");
        self.store
            .update_task_session(&successor)
            .await
            .expect("park the session for recovery");
        let generation = successor.begin_generation("lf-ci-fix-successor".to_string());
        successor.status = TaskSessionStatus::Running;
        successor.status_reason = "ci-fix successor body".to_string();
        let lease = self
            .store
            .reserve_task_process(&successor, TaskSessionStatus::Waiting)
            .await
            .expect("reserve the successor process")
            .expect("a waiting task reserves its next generation");
        // A reserved lease cannot write; the body activates it before touching
        // anything, exactly as the first generation did.
        if let Some(process) = successor.latest_process.as_mut() {
            process.state = ChildLeaseState::Active;
        }
        self.store
            .activate_task_process(&successor, &lease)
            .await
            .expect("activate the successor process");
        self.lease = lease;
        self.task = self
            .store
            .get_task_session(&self.task.id)
            .await
            .expect("read task")
            .expect("task exists");
        generation
    }

    async fn crash_and_reserve_successor(&mut self) -> u32 {
        let revoked = self
            .store
            .revoke_task_process(
                &self.task.id,
                &crate::child_session::ChildBodyOutcome::Lost {
                    reason: "body died after claiming its ci-fix wake".to_string(),
                },
            )
            .await
            .expect("revoke the predecessor generation");
        self.store
            .finish_revoked_task_process(&self.task.id, revoked.generation)
            .await
            .expect("finish the predecessor generation");

        let mut successor = self
            .store
            .get_task_session(&self.task.id)
            .await
            .expect("read task")
            .expect("task exists");
        successor.set_status(TaskSessionStatus::Waiting, "repair body is restarting");
        self.store
            .update_task_session(&successor)
            .await
            .expect("park the task before reserving its successor");
        let generation = successor.begin_generation("lf-ci-fix-real-runner".to_string());
        let lease = self
            .store
            .reserve_task_process(&successor, TaskSessionStatus::Waiting)
            .await
            .expect("reserve the successor process")
            .expect("a waiting task reserves its successor generation");
        self.lease = lease;
        self.task = self
            .store
            .get_task_session(&self.task.id)
            .await
            .expect("read reserved task")
            .expect("task exists");
        generation
    }

    /// Boot a real successor generation and let the production runner drive it
    /// to its own exit, with a scripted provider whose repair turn pushes a new
    /// head. Returns the successor generation and how many provider turns the
    /// body spent.
    ///
    /// The whole runner, not a hand-called settlement: the defect this pins is
    /// an *ordering* between the runner's parent-lifecycle paths and its repair
    /// exit, so a test that calls `settle_ci_fix_turn` itself asserts the
    /// ordering it is supposed to prove.
    async fn run_real_body(&mut self) -> (u32, usize) {
        let generation = self.crash_and_reserve_successor().await;
        let sends = Arc::new(AtomicUsize::new(0));
        let store = self.store.clone();
        let task_id = self.task.id.clone();
        let lease = self.lease.clone();
        let gh_dir = self._guard.state_dir.clone();
        let creator_store = store.clone();
        let creator_task_id = task_id.clone();
        let creator_lease = lease.clone();
        let creator_sends = sends.clone();
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            super::run_task_session_with(
                store,
                task_id,
                &lease,
                Box::new(move |_name, _approval, events| {
                    Ok(Box::new(PushingHarness {
                        events,
                        gh_dir: gh_dir.clone(),
                        store: creator_store.clone(),
                        task_id: creator_task_id.clone(),
                        lease: creator_lease.clone(),
                        sends: creator_sends.clone(),
                    }))
                }),
            ),
        )
        .await
        .expect("a bounded ci-fix body must reach its own exit")
        .expect("the real runner completes and settles the ci-fix turn");
        (generation, sends.load(Ordering::SeqCst))
    }

    async fn command(&self, id: &ChildCommandId) -> ChildCommand {
        self.store
            .get_child_command(id)
            .await
            .expect("read command")
            .expect("command exists")
    }

    /// What a body's boot does: claim, then select the flow from the claimed wake.
    /// Returns the armed wake, if this generation has one.
    async fn arm(&self) -> Option<super::CiFixWake> {
        let claimed = self
            .store
            .claim_child_commands_for_lease(&ChildRef::Task(self.task.id.clone()), &self.lease)
            .await
            .expect("claim commands");
        let (wake, _) = super::arm_ci_fix_wake(&self.store, &self.task, &self.lease, claimed)
            .await
            .expect("arm the ci-fix wake");
        wake
    }

    /// Observe, then enqueue: one supervision pass over a sleeping Task.
    async fn observe(&mut self) -> Option<(ChildCommandId, bool)> {
        self.reconcile().await;
        self.enqueue().await
    }

    async fn commands(&self) -> Vec<ChildCommand> {
        self.store
            .list_child_commands(&ChildRef::Task(self.task.id.clone()))
            .await
            .expect("read child commands")
    }

    async fn ci_fix_commands(&self) -> Vec<ChildCommand> {
        self.commands()
            .await
            .into_iter()
            .filter(|command| matches!(command.kind, ChildCommandKind::CiFix { .. }))
            .collect()
    }

    async fn command_state(&self, id: &ChildCommandId) -> ChildCommandState {
        self.store
            .get_child_command(id)
            .await
            .expect("read command")
            .expect("command exists")
            .state
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

    /// The measured shape of a PR carrying its own design doc: every real leaf
    /// green, red only on `scratch-clear`, and `tests-result` red *solely* as its
    /// roll-up (`ci.yml` gives it `needs: scratch-clear`). Taken verbatim from
    /// #1062 at head 904185190 — this Task's own PR, which armed the third wake.
    fn checks_scratch_clear_only(&self) {
        self.gh.set_checks(
            &[("tests-result", "fail")],
            &[
                ("tests-result", "fail"),
                ("scratch-clear", "fail"),
                ("rust-test", "pass"),
                ("clippy", "pass"),
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

    /// Every read fails the way an exhausted quota does, which
    /// `classify_pr_read_failure` turns into a `Degraded` observation.
    ///
    /// Replaces the script rather than deleting it: `AmbientGuard` prepends its
    /// bin dir to the real `PATH`, so a deleted fake falls through to the host's
    /// `gh`, which 404s on `test/repo` and lands in `NotFound` — freshness stays
    /// `Fresh` and the outage never happens.
    fn outage(&self) {
        write_fake_gh(
            self._guard._bin.path(),
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then echo "gh version 2.40.0 (fake)"; exit 0; fi
echo "API rate limit exceeded for installation" >&2
exit 1
"#,
        );
    }

    /// What the runner's exit does: reconcile the PR the turn just acted on, then
    /// settle the wake and park the body. Mirrors the production call site.
    async fn settle(
        &mut self,
        wake: &super::CiFixWake,
        head_before_turn: Option<&str>,
        status: Lifecycle,
    ) {
        let observed_pr = self.reconcile().await;
        super::settle_ci_fix_turn(
            &self.store,
            &mut self.task,
            &self.lease,
            wake,
            observed_pr.as_ref(),
            head_before_turn,
            status,
            None,
        )
        .await
        .expect("settle the ci-fix turn");
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
    assert!(
        harness.observe().await.is_none(),
        "a pending head enqueues no wake"
    );
    let observation = harness
        .observation()
        .await
        .expect("a pending reading lands");
    assert_eq!(observation.state, CiState::Pending);
    assert_eq!(observation.head_sha, "h1");
    assert!(
        harness.arm().await.is_none(),
        "a pending head must not wake a body"
    );

    // 2. Red: the gate fails and the seed names the broken leaves, not the
    //    `tests-result` aggregate whose link is only the roll-up.
    harness.checks_failing();
    let (first, created) = harness.observe().await.expect("a red head enqueues a wake");
    assert!(created, "the first observation of a failure mints the wake");
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
    assert_eq!(
        incidents[0].incident.trigger_command_id.as_ref(),
        Some(&first),
        "the evidence names the command that will wake the body, before one exists"
    );

    // 3. A duplicate observation is not a second wake. This is the whole dedup:
    //    a durable row keyed on the incident identity, not a marker stamped once
    //    a body already exists.
    let (again, created) = harness.observe().await.expect("the failure still stands");
    assert_eq!(
        again, first,
        "a repeat observation lands on the same command"
    );
    assert!(!created, "and mints nothing");
    assert_eq!(harness.ci_fix_commands().await.len(), 1);

    // 4. Exactly one body, and its wake stays Claimed for the whole repair turn.
    let wake = harness.arm().await.expect("a red head wakes a ci-fix body");
    assert_eq!(wake.command_id, first);
    assert_eq!(wake.head_sha, "h1", "the wake names the head that failed");
    assert_eq!(
        wake.failing_checks
            .iter()
            .map(|check| check.name.as_str())
            .collect::<Vec<_>>(),
        vec!["cargo-fmt", "clippy"],
        "the wake carries the failing leaves the body must repair"
    );
    assert_eq!(
        harness.command_state(&first).await,
        ChildCommandState::Claimed,
        "the wake is claimed, never accepted: settling it is ENG-19's, and \
         accepting here would strand a crashed repair"
    );
    assert!(
        harness.incidents().await[0].incident.responded_at.is_some(),
        "body birth records the response milestone"
    );

    // 5. Restart mid-repair: the successor generation reclaims the same command
    //    and lands on the same wake. No second command, no second failure.
    harness.reconcile().await;
    let resumed = harness
        .arm()
        .await
        .expect("a crashed repair resumes on its own wake");
    assert_eq!(
        resumed.command_id, first,
        "a restart between observation and settlement services the same command"
    );
    assert_eq!(
        harness.ci_fix_commands().await.len(),
        1,
        "and mints no second"
    );
    assert_eq!(
        harness.command_state(&first).await,
        ChildCommandState::Claimed
    );

    // 6. The push: a new head makes the old reading stale, so a fresh reading is
    //    taken and the still-red head rearms under a new identity.
    harness.head("h2");
    let (second, created) = harness.observe().await.expect("the new head is red too");
    assert!(
        created,
        "a new failing head is a new failure, not a duplicate"
    );
    assert_ne!(second, first);
    let observation = harness
        .observation()
        .await
        .expect("a reading for the new head");
    assert_eq!(observation.head_sha, "h2");
    let wake = harness
        .arm()
        .await
        .expect("a new failing head rearms rather than staying deduped against the old one");
    assert_eq!(
        wake.command_id, second,
        "the body services the current failure"
    );
    assert_eq!(wake.head_sha, "h2");
    assert_eq!(
        harness.command_state(&first).await,
        ChildCommandState::Superseded,
        "the wake for the head that was pushed past is stale, not a live race"
    );
    assert_eq!(
        harness.incidents().await.len(),
        2,
        "each failed repair head is one measurable attempt"
    );

    // 7. Green: the repair worked and the Task settles back to waiting.
    harness.checks_passing();
    assert!(
        harness.observe().await.is_none(),
        "a green head enqueues no wake"
    );
    let observation = harness
        .observation()
        .await
        .expect("a passing reading lands");
    assert_eq!(observation.state, CiState::Passing);
    assert!(
        harness.arm().await.is_none(),
        "a green head must not wake a body"
    );
    assert!(
        harness
            .incidents()
            .await
            .iter()
            .all(|incident| incident.incident.green_at.is_some()),
        "the passing head closes every open attempt on the PR"
    );
    assert!(
        harness
            .incidents()
            .await
            .iter()
            .all(|incident| incident.incident.trigger_command_id.is_some()),
        "every attempt names the command that woke it"
    );
}

/// A head red *only* on `scratch-clear` wakes nobody, through the real read path.
///
/// `scratch-clear` asserts a land-time precondition: `lf pr land` clears
/// `scratch/` as its first act, so the check fails on every PR carrying its own
/// design doc and no repair turn can green it. The only action a woken body could
/// take is deleting the artifact the reviewer reads. Measured live three times
/// (ENG-4 #1055, W2-297 #1060, and this Task's own #1062 at 904185190).
///
/// Both directions run here, and the second is what keeps this a classifier
/// rather than a mute button: sabotaging the `wake_legal` clause turns the first
/// half red while the real-leaf half stays green. A test using a head with a real
/// failure passes with the bug fully present — which is exactly why this defect
/// survived two live occurrences before anyone filed it.
#[tokio::test]
async fn a_scratch_clear_only_head_arms_no_ci_fix_wake() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_scratch_clear_only();

    assert!(
        harness.observe().await.is_none(),
        "a head red only on a land-time precondition enqueues no wake"
    );
    assert!(
        harness.arm().await.is_none(),
        "and no body is woken to repair what only `lf pr land` can green"
    );
    assert!(
        harness.ci_fix_commands().await.is_empty(),
        "no ci-fix command exists to be claimed later"
    );

    // The reading stays honest. This refuses the wake; it does not deny the red.
    // `lf ci` and `lf task status` still name the failure on this head.
    let observation = harness
        .observation()
        .await
        .expect("the failing reading still lands");
    assert_eq!(observation.state, CiState::Failing);
    let names: Vec<&str> = observation
        .failing_checks
        .iter()
        .map(|check| check.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["scratch-clear"],
        "the observation reports the real failure; only the repair is refused"
    );

    // The incident row survives too, and this is the healthy shape rather than a
    // gap: the head *was* red, and no repair was warranted. A NULL trigger means
    // nobody was woken, not that a wake was lost.
    let incidents = harness.incidents().await;
    assert_eq!(incidents.len(), 1, "the red head still opens an incident");
    assert_eq!(incidents[0].incident.failure_set, vec!["scratch-clear"]);
    assert!(
        incidents[0].incident.trigger_command_id.is_none(),
        "no command woke for it"
    );

    // Direction two: a real leaf breaking on the same head still earns exactly
    // one attributable body. Both, or the fix is a mute button.
    harness.checks_failing();
    let (id, created) = harness
        .observe()
        .await
        .expect("an actionable leaf still enqueues a wake");
    assert!(created, "the real failure mints its own wake");
    assert_eq!(
        harness
            .arm()
            .await
            .expect("and wakes a body to repair it")
            .command_id,
        id
    );
}

/// The identity is conditional on the failing *set*, not just the head: a check
/// that breaks after a wake already fired is a new failure and earns a new body.
#[tokio::test]
async fn a_changed_failing_set_on_the_same_head_rearms() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_failing();
    let (first, _) = harness.observe().await.expect("the first failing set");
    assert_eq!(
        harness
            .arm()
            .await
            .expect("the first failing set wakes a body")
            .command_id,
        first
    );
    assert!(
        harness
            .observe()
            .await
            .is_some_and(|(id, created)| id == first && !created),
        "and only one"
    );

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
    let (second, created) = harness
        .observe()
        .await
        .expect("a check that broke after the wake fired is a new failure, not a duplicate");
    assert!(created);
    assert_ne!(second, first);
    let wake = harness.arm().await.expect("the new failure wakes a body");
    assert_eq!(wake.command_id, second);
    assert!(
        wake.failing_checks
            .iter()
            .any(|check| check.name == "rust-test"),
        "the body repairs the failure that woke it, including the newly broken check"
    );
}

/// The window the whole `Claimed`-through-the-turn decision exists for, and the
/// executable form of the argument for it.
///
/// A body dies mid-repair. Its successor must land back on the *same* wake and
/// run ci-fix again. That works only because the command is still `Claimed`:
/// `claim_child_commands_in` reassigns `persisted`/`claimed` rows to the asking
/// generation and skips terminal ones.
///
/// Both rejected alternatives fail here, which is the point of the test:
/// - **Accept at arm** — the wake would be terminal, the successor would claim
///   nothing, `arm` would return `None`, the body would fall through to its
///   lifecycle phase, and the PR would stay red with nobody repairing it. A
///   silent strand.
/// - **Deliver at arm** — `reconcile_stale_deliveries` would flip the dead
///   generation's `Delivering` to `Uncertain`, and `plan_body_recovery` returns
///   `NeedsInput`, stranding an *automatic* wake on a human.
#[tokio::test]
async fn a_crash_after_arm_reclaims_the_same_command_and_reselects_ci_fix() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_failing();
    let (wake, _) = harness.observe().await.expect("a red head mints a wake");

    // Generation 1 arms it and is servicing the repair.
    let armed = harness.arm().await.expect("the first body arms the wake");
    assert_eq!(armed.command_id, wake);
    assert_eq!(harness.command(&wake).await.claimed_by_generation, Some(1));
    assert_eq!(
        harness.command_state(&wake).await,
        ChildCommandState::Claimed,
        "held for the repair turn, not settled at arm"
    );

    // The body dies mid-repair and recovery boots its successor.
    let generation = harness.crash_and_relaunch().await;
    assert_eq!(generation, 2, "a real succession, not a re-arm");

    let resumed = harness.arm().await.expect(
        "the successor reselects ci-fix rather than falling through to its lifecycle phase",
    );
    assert_eq!(
        resumed.command_id, wake,
        "the successor services the same command, not a new one"
    );
    assert_eq!(
        resumed.head_sha, "h1",
        "seeded from that same command's payload"
    );

    let command = harness.command(&wake).await;
    assert_eq!(
        command.claimed_by_generation,
        Some(generation),
        "the wake is reassigned to the successor generation"
    );
    assert_eq!(
        command.state,
        ChildCommandState::Claimed,
        "still Claimed across the crash — never Accepted, never Uncertain"
    );
    assert_eq!(
        harness.ci_fix_commands().await.len(),
        1,
        "a crash mints no second wake"
    );
    assert_eq!(
        harness.incidents().await.len(),
        1,
        "and opens no second incident"
    );
}

/// A wake is a command, not a direction — the sharpest trap in the change.
///
/// Minting a `ChildDirective` would bump `current_directive_version`, and
/// `has_pending_directive` gates `task_completion_gate` on
/// `current > incorporated`. A wake that minted one would block Task completion
/// until a body acknowledged a direction no human ever gave.
#[tokio::test]
async fn a_ci_fix_wake_mints_no_directive() {
    let mut harness = Harness::new().await;
    let before = harness
        .store
        .get_task_session(&harness.task.id)
        .await
        .expect("read task")
        .expect("task exists");

    harness.head("h1");
    harness.checks_failing();
    harness.observe().await.expect("a red head mints a wake");
    harness.arm().await.expect("the wake arms a body");

    let after = harness
        .store
        .get_task_session(&harness.task.id)
        .await
        .expect("read task")
        .expect("task exists");
    assert_eq!(
        after.current_directive_version, before.current_directive_version,
        "a wake must not version a direction nobody gave"
    );
    assert_eq!(
        after.incorporated_directive_version, before.incorporated_directive_version,
        "so nothing is left pending incorporation, and completion stays reachable"
    );
    let wake_id = harness.ci_fix_commands().await[0].id.clone();
    let directives = harness
        .store
        .child_directives(&ChildRef::Task(harness.task.id.clone()))
        .await
        .expect("read directives");
    assert!(
        directives
            .iter()
            .all(|directive| directive.command_id.as_ref() != Some(&wake_id)),
        "no directive is bound to the wake command"
    );
}

/// The Project runner's seam, through the real entry point.
///
/// `queue_ci_fix_command` is what the observation calls now; the direct
/// `wake_task_ci_fix` launch is deleted, so no caller can reach a body except
/// through the ledger. A healthy head must enqueue nothing — and because a wake
/// that is never minted is also never launched, this half of the seam is safe to
/// drive here. The red half launches a real process, so its ledger behaviour is
/// covered where the launch is barred (`ops::child`).
#[tokio::test]
async fn the_observer_enqueues_nothing_for_a_healthy_head() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_passing();
    harness.reconcile().await;

    let pr = harness
        .store
        .active_task_pr(&harness.task.id)
        .await
        .expect("read active pr")
        .expect("an active pr");
    crate::ops::task::queue_ci_fix_command(&harness.store, &harness.task, &pr)
        .await
        .expect("a healthy head is not an error, it is simply nothing to repair");

    assert!(
        harness.ci_fix_commands().await.is_empty(),
        "a green head mints no wake, so nothing can launch a body"
    );
    assert!(
        harness.incidents().await.is_empty(),
        "and opens no incident"
    );
}

/// The selector's proof. Two wakes can be claimable at once — a head fails, is
/// pushed to, and fails again before any body boots. Taking the first claimed
/// command would seed an obsolete repair *and* spend the current wake's identity
/// as a stray, leaving the live failure permanently unrepairable.
#[tokio::test]
async fn a_moved_failure_arms_the_current_wake_and_supersedes_the_stale_one() {
    let mut harness = Harness::new().await;

    // Wake A: h1 is red. No body boots.
    harness.head("h1");
    harness.checks_failing();
    let (stale, _) = harness.observe().await.expect("h1 mints a wake");

    // The head moves and fails differently before anything claims A.
    harness.head("h2");
    harness.gh.set_checks(
        &[("tests-result", "fail")],
        &[("tests-result", "fail"), ("rust-test", "fail")],
    );
    let (current, created) = harness.observe().await.expect("h2 mints its own wake");
    assert!(created);
    assert_ne!(current, stale);
    assert_eq!(
        harness.ci_fix_commands().await.len(),
        2,
        "both wakes are unsettled and claimable"
    );

    // The body boots and claims both. It must service the current failure.
    let wake = harness
        .arm()
        .await
        .expect("a body arms for the live failure");
    assert_eq!(
        wake.command_id, current,
        "the body services the wake naming the PR's current failure, not the first claimed"
    );
    assert_eq!(wake.head_sha, "h2");
    assert_eq!(
        wake.failing_checks
            .iter()
            .map(|check| check.name.as_str())
            .collect::<Vec<_>>(),
        vec!["rust-test"],
        "and is seeded from that wake's payload"
    );
    assert_eq!(
        harness.command_state(&current).await,
        ChildCommandState::Claimed,
        "the live wake is held for the repair turn, not lost to the stale one"
    );
    assert_eq!(
        harness.command_state(&stale).await,
        ChildCommandState::Superseded,
        "the wake for the head the PR moved past is retired as stale"
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
    let (wake, _) = harness.observe().await.expect("a red head mints a wake");
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

    // The outage must not burn the wake. The reading is persisted, not re-read
    // live, so the identity still matches and the body still arms for it. If a
    // degraded read could make a wake look stale, its identity would be spent and
    // the failure would never be repaired.
    assert_eq!(
        harness
            .arm()
            .await
            .expect("the wake survives a degraded read")
            .command_id,
        wake
    );
    assert_eq!(
        harness.command_state(&wake).await,
        ChildCommandState::Claimed
    );
}

// ---------------------------------------------------------------------------
// Reopening
// ---------------------------------------------------------------------------

/// A reopened PR is the same PR: it returns to active at its own identity, its
/// red head is visible and wakes one body, and it mints no successor.
///
/// Pins ENG-20/#1026, where `abandoned_at` was stamped from GitHub's closed
/// state and never cleared, so the reopened PR stayed unobserved while an empty
/// sequence 2 was cut at main.
#[tokio::test]
async fn a_reopened_pr_is_restored_red_woken_once_and_mints_no_successor() {
    let mut harness = Harness::new().await;
    let original = harness
        .store
        .active_task_pr(&harness.task.id)
        .await
        .expect("read active pr")
        .expect("the fixture PR is active");

    harness.head("h1");
    harness.checks_pending();
    harness.reconcile().await;

    // Closed by GitHub, the real writer of `abandoned_at`.
    harness.gh.set_pr(harness.pr_number, "closed", "h1");
    harness.reconcile().await;
    assert!(
        harness
            .store
            .active_task_pr(&harness.task.id)
            .await
            .expect("read active pr")
            .is_none(),
        "a closed PR leaves the active set — the state that used to rotate"
    );

    // Reopened, and red.
    harness.gh.set_pr(harness.pr_number, "open", "h1");
    harness.checks_failing();
    harness.reconcile().await;

    let reopened = harness
        .store
        .active_task_pr(&harness.task.id)
        .await
        .expect("read active pr")
        .expect("a reopened PR is active again");
    assert_eq!(reopened.phase(), PrPhase::Open);
    assert_eq!(
        reopened.id, original.id,
        "the reopened PR keeps its identity"
    );
    assert_eq!(reopened.sequence, 1);
    assert_eq!(reopened.branch, original.branch);
    assert_eq!(reopened.abandoned_at, None);

    let observation = harness.observation().await.expect("the head is read again");
    assert_eq!(observation.state, CiState::Failing);
    let incidents = harness.incidents().await;
    assert_eq!(incidents.len(), 1, "the red head is visible to `lf ci`");
    assert_eq!(incidents[0].incident.pr_number, harness.pr_number);

    let (wake, created) = harness.observe().await.expect("the red head wakes a body");
    assert!(created, "the reopened failure mints its wake");
    let (again, created_again) = harness.observe().await.expect("the head is still red");
    assert_eq!(again, wake);
    assert!(!created_again, "a repeat observation mints no second wake");
    assert_eq!(
        harness
            .arm()
            .await
            .expect("the body arms for it")
            .command_id,
        wake
    );

    crate::ops::task::ensure_working_pr_for_lease(
        &harness.store,
        &mut harness.task,
        &harness.lease,
    )
    .await
    .expect("a reopened PR needs no rotation");
    let prs = harness
        .store
        .task_prs(&harness.task.id)
        .await
        .expect("read task prs");
    assert_eq!(prs.len(), 1, "a reopened PR mints no empty successor");
    assert_eq!(prs[0].id, original.id);
}

/// A predecessor GitHub could not be re-read is not a settled predecessor: under
/// an outage the stale `abandoned_at` stands, and rotating on it would mint the
/// same empty successor.
#[tokio::test]
async fn a_degraded_read_refuses_to_rotate_past_an_abandoned_pr() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_pending();
    harness.reconcile().await;
    harness.gh.set_pr(harness.pr_number, "closed", "h1");
    harness.reconcile().await;

    harness.outage();
    harness.expire_read_cache().await;

    let error = crate::ops::task::ensure_working_pr_for_lease(
        &harness.store,
        &mut harness.task,
        &harness.lease,
    )
    .await
    .expect_err("an unconfirmable predecessor must not be rotated past");

    // The message, not merely `is_err`: this path also touches git, and a
    // rotation that failed on an unrelated git error would pass a bare `is_err`
    // while minting the branch this test forbids.
    let message = error.to_string();
    assert!(
        message.contains(&format!("#{}", harness.pr_number)) && message.contains("is closed"),
        "the refusal must name the PR it could not confirm, got: {message}"
    );
    let prs = harness
        .store
        .task_prs(&harness.task.id)
        .await
        .expect("read task prs");
    assert_eq!(prs.len(), 1, "a refused rotation mints no successor");
}

// ---------------------------------------------------------------------------
// The exit: settlement and parking
// ---------------------------------------------------------------------------

/// The repair worked: the body pushed a new head, so the wake it was born for is
/// accepted and the Task waits on CI again. Nothing about the Task advances — a
/// repair is not Task progress.
#[tokio::test]
async fn a_repaired_head_accepts_the_wake_and_parks_without_a_gate() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_failing();
    let (command, _) = harness.observe().await.expect("a red head mints a wake");
    let wake = harness.arm().await.expect("a red head wakes a body");
    let phase = harness.task.lifecycle_phase;
    let gate_cycle = harness.task.gate_cycle;

    // The repair turn's push: the head moves, and the new head is not yet red.
    harness.head("h2");
    harness.checks_pending();
    harness
        .settle(&wake, Some("h1"), Lifecycle::Completed)
        .await;

    assert_eq!(
        harness.command_state(&command).await,
        ChildCommandState::Accepted,
        "a repair that moved the head settles the exact wake it was born for"
    );
    let session = harness
        .store
        .get_task_session(&harness.task.id)
        .await
        .expect("read task")
        .expect("task exists");
    assert_eq!(
        session.status,
        TaskSessionStatus::Waiting,
        "and the Task durably waits on CI for the head it just pushed"
    );
    assert_eq!(
        session.lifecycle_phase, phase,
        "a ci-fix body must not advance the Task's lifecycle phase"
    );
    assert_eq!(session.gate_cycle, gate_cycle, "nor open a gate cycle");
    assert!(
        session.gate_proposal.is_none(),
        "nor propose a Task outcome"
    );
    let process = session.latest_process.expect("a generation");
    assert_eq!(
        process.state,
        ChildLeaseState::Finished,
        "the body is over and the Session is reservable again"
    );
    assert_eq!(
        process.outcome,
        Some(crate::child_session::ChildBodyOutcome::Completed),
        "a repair that ran to the end is Completed; reporting it as Interrupted \
         would file finished work as abandoned"
    );
    assert_eq!(
        harness
            .store
            .task_prs(&harness.task.id)
            .await
            .expect("read task prs")
            .len(),
        1,
        "a repair pushes to the branch it was given; it never rotates to a successor PR"
    );
}

/// W2-280 generation 5 and W2-298 generation 3 both reached the real runner
/// with a durable Gate cursor and an out-of-band `ci-fix` playhead. The generic
/// completion path must finish that repair without validating or copying its
/// cursor into `task-gate` before the dedicated settlement exit runs.
#[tokio::test]
async fn a_real_ci_fix_turn_preserves_the_gate_cursor_and_settles_its_wake() {
    let mut harness = Harness::new().await;
    let proposal = crate::task::TaskGateProposal {
        status: TaskSessionStatus::Completed,
        reason: "implementation is ready once CI passes".to_string(),
    };
    harness.task.lifecycle_phase = crate::task::TaskLifecyclePhase::Gate;
    harness.task.phase_epoch = 4;
    harness.task.phase_cursor = 1;
    harness.task.phase_iteration = 2;
    harness.task.gate_cycle = 3;
    harness.task.gate_proposal = Some(proposal.clone());
    harness
        .store
        .update_task_session_for_lease(&harness.task, &harness.lease)
        .await
        .expect("persist the Gate cursor before the repair");

    harness.head("h1");
    harness.checks_failing();
    let (command, _) = harness.observe().await.expect("a red head mints a wake");
    assert!(
        harness.arm().await.is_some(),
        "the predecessor generation claims and arms the wake"
    );
    assert_eq!(
        harness.command_state(&command).await,
        ChildCommandState::Claimed
    );

    let (generation, sends) = harness.run_real_body().await;
    assert_eq!(
        sends, 1,
        "the repair spends one provider turn, and no gate step"
    );

    let session = harness
        .store
        .get_task_session(&harness.task.id)
        .await
        .expect("read task")
        .expect("task exists");
    assert_eq!(
        session.lifecycle_phase,
        crate::task::TaskLifecyclePhase::Gate
    );
    assert_eq!(session.phase_epoch, 4);
    assert_eq!(session.phase_cursor, 1);
    assert_eq!(session.phase_iteration, 2);
    assert_eq!(session.gate_cycle, 3);
    assert_eq!(session.gate_proposal, Some(proposal));
    assert_eq!(session.status, TaskSessionStatus::Waiting);
    let process = session.latest_process.expect("the successor generation");
    assert_eq!(process.generation, generation);
    assert_eq!(process.state, ChildLeaseState::Finished);
    assert_eq!(
        process.outcome,
        Some(crate::child_session::ChildBodyOutcome::Completed)
    );
    assert_eq!(
        harness.command_state(&command).await,
        ChildCommandState::Accepted,
        "settlement terminates the exact wake reclaimed from the predecessor"
    );
}

/// One bounded repair turn, driven through the real runner from a given
/// lifecycle phase, asserting the two properties that make the boundary real:
/// the parent's cursor is exactly where the repair found it, and the wake
/// terminalizes once.
///
/// `sends` is the load-bearing one and the reason this drives the real runner
/// rather than calling settlement directly: the Kickoff defect spent a
/// `task_clarify` turn and left the wake `Claimed`, which every cursor
/// assertion here would have happily passed. A second send means a parent
/// lifecycle path ran ahead of the repair's exit.
async fn a_ci_fix_turn_preserves_the_cursor_of(
    phase: crate::task::TaskLifecyclePhase,
    epoch: u32,
    cursor: u32,
    iteration: u32,
) {
    let mut harness = Harness::new().await;
    harness.task.lifecycle_phase = phase;
    harness.task.phase_epoch = epoch;
    harness.task.phase_cursor = cursor;
    harness.task.phase_iteration = iteration;
    harness
        .store
        .update_task_session_for_lease(&harness.task, &harness.lease)
        .await
        .expect("persist the parent cursor before the repair");

    harness.head("h1");
    harness.checks_failing();
    let (command, _) = harness.observe().await.expect("a red head mints a wake");
    assert!(
        harness.arm().await.is_some(),
        "the predecessor generation claims and arms the wake"
    );

    let (generation, sends) = harness.run_real_body().await;
    assert_eq!(
        sends, 1,
        "one bounded repair turn; a second send is the lifecycle spending a turn \
         the repair was never asked for"
    );

    let session = harness
        .store
        .get_task_session(&harness.task.id)
        .await
        .expect("read task")
        .expect("task exists");
    assert_eq!(
        session.lifecycle_phase, phase,
        "a repair is not Task progress: the phase is still where the Task stands"
    );
    assert_eq!(
        session.phase_epoch, epoch,
        "entering another phase would bump the epoch"
    );
    assert_eq!(
        session.phase_cursor, cursor,
        "the transient playhead never lands in the durable cursor"
    );
    assert_eq!(session.phase_iteration, iteration);
    assert_eq!(session.status, TaskSessionStatus::Waiting);
    let process = session.latest_process.expect("the successor generation");
    assert_eq!(process.generation, generation);
    assert_eq!(process.state, ChildLeaseState::Finished);
    assert_eq!(
        process.outcome,
        Some(crate::child_session::ChildBodyOutcome::Completed)
    );
    assert_eq!(
        harness.command_state(&command).await,
        ChildCommandState::Accepted,
        "settlement terminates the exact wake reclaimed from the predecessor"
    );
    assert_eq!(
        harness.ci_fix_commands().await.len(),
        1,
        "and no second wake exists to mint a second body"
    );
}

/// W2-303 generation 4 reached the real runner from Iterate: durable flow
/// `task`, transient playhead `ci-fix`. The generic completion path rejected
/// that playhead against the parent's flow before the repair could settle.
///
/// The Gate proof above is not evidence about this one. Each phase reaches the
/// parent's cursor through a different path, and only running the phase proves
/// the phase.
#[tokio::test]
async fn a_real_ci_fix_turn_preserves_the_iterate_cursor_and_settles_its_wake() {
    a_ci_fix_turn_preserves_the_cursor_of(crate::task::TaskLifecyclePhase::Iterate, 2, 1, 3).await;
}

/// W2-309 generation 2 (PR #1062, 2026-07-17) reached the real runner from
/// Kickoff and failed *silently*: the Kickoff-completion path ran ahead of the
/// repair's exit, entered Iterate, discarded the `ci-fix` playhead for a fresh
/// `task` flow, and spent a `task_clarify` turn — leaving the wake `Claimed`
/// with its incident already stamped `responded_at`.
///
/// Nothing rejected anything, which is why this phase needs its own case: the
/// Iterate and Gate proofs pass with this hole fully present. Kickoff does not
/// validate the cursor, it replaces it.
#[tokio::test]
async fn a_kickoff_ci_fix_turn_settles_before_iterate_and_spends_no_lifecycle_turn() {
    a_ci_fix_turn_preserves_the_cursor_of(crate::task::TaskLifecyclePhase::Kickoff, 1, 1, 0).await;
}

/// The parent's interactive rendezvous is the third way a `ci-fix` playhead
/// reaches the durable cursor, and the quietest: a repair body's boot skips
/// `reconcile_interactive_rendezvous_at_birth` on purpose, so a prior body's
/// pending handoff is still open when the repair turn completes — and the park
/// that handles it records the playhead as the phase's position.
///
/// A repair neither opened that rendezvous nor can answer it. It settles and
/// parks; the handoff stays pending for the next parent body's birth reconcile,
/// which is the one place allowed to claim its wake exactly once.
#[tokio::test]
async fn a_ci_fix_turn_settles_past_a_pending_handoff_it_does_not_own() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_failing();
    let (command, _) = harness.observe().await.expect("a red head mints a wake");
    assert!(
        harness.arm().await.is_some(),
        "the predecessor generation claims and arms the wake"
    );
    let (handoff, created) = harness
        .store
        .open_interactive_handoff(crate::interactive_handoff::OpenInteractiveHandoff {
            parent: crate::interactive_handoff::InteractiveHandoffParent::Task(
                harness.task.id.clone(),
            ),
            home: crate::engine::wave_home::WaveHome::parse("jack@local").unwrap(),
            cwd: harness.task.worktree.clone(),
            provider: harness.task.provider.clone(),
            provider_session_id: None,
            body_generation: harness.lease.generation,
            reason: "the parent's iterate step needs an interactive login".to_string(),
            environment: std::collections::BTreeMap::new(),
            attach_argv: vec!["tmux".to_string(), "attach".to_string()],
        })
        .await
        .expect("open the parent's handoff");
    assert!(created);

    let (_, sends) = harness.run_real_body().await;
    assert_eq!(sends, 1, "the repair still spends exactly one turn");

    assert_eq!(
        harness.command_state(&command).await,
        ChildCommandState::Accepted,
        "the repair reaches its own exit rather than parking on a rendezvous it \
         cannot answer"
    );
    let session = harness
        .store
        .get_task_session(&harness.task.id)
        .await
        .expect("read task")
        .expect("task exists");
    assert_eq!(
        session.lifecycle_phase,
        crate::task::TaskLifecyclePhase::Iterate
    );
    assert_eq!(session.status, TaskSessionStatus::Waiting);
    assert!(
        super::parked_on_interactive_handoff(&harness.store, &session)
            .await
            .expect("read the handoff"),
        "and the human's rendezvous is still pending, untouched, for the next \
         parent body to reconcile at birth"
    );
    let handoffs = harness
        .store
        .list_interactive_handoffs(Some(
            &crate::interactive_handoff::InteractiveHandoffParent::Task(harness.task.id.clone()),
        ))
        .await
        .expect("read the handoffs");
    assert_eq!(handoffs.len(), 1);
    assert_eq!(handoffs[0].id, handoff.id);
    assert!(
        handoffs[0].wake_claimed_by_generation.is_none(),
        "the repair never claims the wake the parent's birth reconcile owes"
    );
}

/// A red observation can arrive while the Task control body is alive and a
/// provider turn still owns the transcript. The wake belongs to that same
/// generation, but only after the provider turn releases ownership.
#[tokio::test]
async fn a_live_generation_holds_ci_fix_until_its_provider_turn_is_idle() {
    let mut harness = Harness::new().await;
    harness.task.lifecycle_phase = crate::task::TaskLifecyclePhase::Gate;
    harness.task.phase_epoch = 4;
    harness.task.phase_cursor = 0;
    harness.task.phase_iteration = 0;
    harness.task.gate_cycle = 3;
    harness.task.gate_proposal = Some(crate::task::TaskGateProposal {
        status: TaskSessionStatus::Completed,
        reason: "implementation waits for green CI".to_string(),
    });
    harness
        .store
        .update_task_session_for_lease(&harness.task, &harness.lease)
        .await
        .expect("persist the Gate waitpoint");

    harness.head("h1");
    harness.checks_failing();
    harness.reconcile().await;
    let incident = harness
        .incidents()
        .await
        .into_iter()
        .next()
        .expect("the failing head opens an incident");
    assert_eq!(incident.incident.trigger_command_id, None);

    let generation = harness.crash_and_reserve_successor().await;
    let first_turn_active = Arc::new(AtomicBool::new(true));
    let repair_started_while_active = Arc::new(AtomicBool::new(false));
    let sends = Arc::new(AtomicUsize::new(0));
    let interrupts = Arc::new(AtomicUsize::new(0));
    let store = harness.store.clone();
    let task_id = harness.task.id.clone();
    let lease = harness.lease.clone();
    let gh_dir = harness._guard.state_dir.clone();
    let creator_store = store.clone();
    let creator_task_id = task_id.clone();
    let creator_lease = lease.clone();
    let creator_first_turn_active = first_turn_active.clone();
    let creator_repair_started_while_active = repair_started_while_active.clone();
    let creator_sends = sends.clone();
    let creator_interrupts = interrupts.clone();

    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        super::run_task_session_with(
            store.clone(),
            task_id.clone(),
            &lease,
            Box::new(move |_name, _approval, events| {
                Ok(Box::new(LiveIdleHarness {
                    events,
                    gh_dir: gh_dir.clone(),
                    store: creator_store.clone(),
                    task_id: creator_task_id.clone(),
                    lease: creator_lease.clone(),
                    first_turn_active: creator_first_turn_active.clone(),
                    repair_started_while_active: creator_repair_started_while_active.clone(),
                    sends: creator_sends.clone(),
                    interrupts: creator_interrupts.clone(),
                }))
            }),
        ),
    )
    .await
    .expect("the retained wake must start after the provider becomes idle")
    .expect("the same runner services and settles the wake");

    assert_eq!(
        sends.load(Ordering::SeqCst),
        2,
        "one review turn is followed by exactly one ci-fix turn"
    );
    assert_eq!(
        interrupts.load(Ordering::SeqCst),
        0,
        "the active provider turn is never interrupted"
    );
    assert!(
        !repair_started_while_active.load(Ordering::SeqCst),
        "the repair seed reaches the provider only after its prior turn ends"
    );

    let commands = harness.ci_fix_commands().await;
    assert_eq!(
        commands.len(),
        1,
        "the failing head keeps one wake identity"
    );
    let command = &commands[0];
    assert_eq!(command.state, ChildCommandState::Accepted);
    assert_eq!(command.claimed_by_generation, Some(generation));

    let incident = harness
        .incidents()
        .await
        .into_iter()
        .next()
        .expect("the attributed incident remains recorded");
    assert_eq!(
        incident.incident.trigger_command_id.as_ref(),
        Some(&command.id),
        "the repair services the command linked by supervision"
    );
    assert!(incident.incident.responded_at.is_some());

    let session = store
        .get_task_session(&task_id)
        .await
        .expect("read Task")
        .expect("Task exists");
    let process = session
        .latest_process
        .expect("the live generation remains recorded");
    assert_eq!(
        process.generation, generation,
        "ci-fix starts in the same control body, not a second generation"
    );
    assert_eq!(process.state, ChildLeaseState::Finished);
}

/// The repair ran and the head is still red: the wake fails with the reason a
/// human needs, and that failure is final. One failing head earns one automatic
/// repair — the bound that keeps a broken PR from spinning bodies.
#[tokio::test]
async fn an_unrepaired_head_fails_the_wake_and_never_rearms_the_same_failure() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_failing();
    let (command, _) = harness.observe().await.expect("a red head mints a wake");
    let wake = harness.arm().await.expect("a red head wakes a body");

    // The turn ends with the head exactly where it started, still red.
    harness
        .settle(&wake, Some("h1"), Lifecycle::Completed)
        .await;

    let settled = harness.command(&command).await;
    assert_eq!(
        settled.state,
        ChildCommandState::Failed,
        "a repair that did not move the head is a failure, not a success"
    );
    let error = settled.error.as_deref().expect("a failed wake names why");
    assert!(
        error.contains("did not repair the head"),
        "the wake carries the operator-facing reason: {error}"
    );
    assert_eq!(
        harness.task.status,
        TaskSessionStatus::Blocked,
        "and the Task blocks for a human rather than looping"
    );
    assert_eq!(harness.task.status_reason, error, "saying the same thing");

    // The bound. The head is still red, and the next supervision pass tries to
    // wake a body for it. (Only `enqueue`: the park finished this body's lease,
    // so nothing may reconcile under it any more — which is itself the shape of a
    // parked body.)
    let (again, created) = harness.enqueue().await.expect("the failure still stands");
    assert_eq!(again, command, "the observation lands on the spent wake");
    assert!(
        !created,
        "a settled identity mints no second wake, however it settled — so no body \
         can launch, and this failure is a human's now"
    );
    assert_eq!(harness.ci_fix_commands().await.len(), 1);
}

/// An interrupted turn reached no outcome, so it reports none: the wake stays
/// `Claimed`, the same state a crash leaves and recovered the same way (see
/// `a_crash_after_arm_reclaims_the_same_command_and_reselects_ci_fix`).
#[tokio::test]
async fn an_interrupted_turn_leaves_the_wake_claimed() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_failing();
    let (command, _) = harness.observe().await.expect("a red head mints a wake");
    let wake = harness.arm().await.expect("a red head wakes a body");

    harness
        .settle(&wake, Some("h1"), Lifecycle::Interrupted)
        .await;

    assert_eq!(
        harness.command_state(&command).await,
        ChildCommandState::Claimed,
        "an interrupted repair reports no outcome, so a successor can still run it"
    );
    assert_eq!(
        harness.task.status,
        TaskSessionStatus::Waiting,
        "the body still parks; it just settles nothing"
    );
    assert!(
        matches!(
            harness
                .task
                .latest_process
                .as_ref()
                .expect("a generation")
                .outcome,
            Some(crate::child_session::ChildBodyOutcome::Interrupted { .. })
        ),
        "a turn cut short is Interrupted — the outcome and the unsettled wake have \
         to tell the same story"
    );
}

/// The park-before-terminalize invariant (argued on `settle_ci_fix_turn`), read
/// off the durable record: both writes append to the event stream in the order
/// they land, so a settlement that ever preceded its park would show here.
#[tokio::test]
async fn the_wake_never_settles_while_the_session_still_reads_running() {
    let mut harness = Harness::new().await;
    harness.head("h1");
    harness.checks_failing();
    let (command, _) = harness.observe().await.expect("a red head mints a wake");
    let wake = harness.arm().await.expect("a red head wakes a body");
    assert_eq!(
        harness.task.status,
        TaskSessionStatus::Running,
        "the body is running as the turn ends"
    );

    harness
        .settle(&wake, Some("h1"), Lifecycle::Completed)
        .await;

    let events = harness
        .store
        .task_events_after(&harness.task.id, 0)
        .await
        .expect("read task events");
    let parked = events
        .iter()
        .position(|event| {
            matches!(
                &event.kind,
                crate::task::TaskEventKind::StatusChanged { to, .. }
                    if *to == TaskSessionStatus::Blocked
            )
        })
        .expect("the body records its park");
    let settled = events
        .iter()
        .position(|event| {
            matches!(
                &event.kind,
                crate::task::TaskEventKind::CommandChanged { command_id, state, .. }
                    if *command_id == command && state.is_terminal()
            )
        })
        .expect("the body records its settlement");
    assert!(
        parked < settled,
        "the Session must park before the wake goes terminal: a death in that window \
         has to leave a Claimed wake, not a spent one under a Running Session"
    );
}
