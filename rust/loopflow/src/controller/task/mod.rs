use crate::durable::{FlowSession, TaskWorkerClaim};
use crate::engine::invocation::QueuedInvocation;
use crate::store::SharedStore;
use crate::work::task::{Task, TaskId};
use anyhow::{anyhow, Result};
use clap::Parser;

async fn load_task(store: &SharedStore, task_id: &TaskId) -> Result<Task> {
    store
        .get_task(task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {task_id} not found"))
}

pub(crate) async fn run(store: SharedStore, task_id: TaskId) -> Result<()> {
    let launch_claim =
        take_task_exec_env(crate::durable::TASK_WORKER_CLAIM_ENV, "Task worker claim")?
            .map(|value| serde_json::from_str::<TaskWorkerClaim>(&value))
            .transpose()
            .map_err(|error| anyhow!("invalid Task worker claim: {error}"))?
            .ok_or_else(|| anyhow!("Task boundary launch is missing its worker claim"))?;
    let owner = crate::journal::current_process_identity()
        .ok_or_else(|| anyhow!("Task worker requires a registered Exec"))?;
    let launch_claim = store
        .sqlite
        .handoff_task_worker(&task_id, &launch_claim, &owner)?;
    drive_task(store, task_id, launch_claim).await
}

/// Drive the Task's Flow through the shared executor with this process's
/// claim. A step that ends without a result records failure and releases the position.
async fn drive_task(
    store: SharedStore,
    task_id: TaskId,
    launch_claim: TaskWorkerClaim,
) -> Result<()> {
    let flow = store
        .task_flow(&task_id)
        .await?
        .ok_or_else(|| anyhow!("Task has no active Flow"))?;
    if flow.claim.as_ref() != Some(&launch_claim) {
        anyhow::bail!("Task driver launch claim is stale");
    }
    let options: Vec<String> =
        take_task_exec_env(crate::lf::TASK_SKILL_OPTIONS_ENV, "Task skill options")?
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .unwrap_or_default();
    let cli = crate::lf::Cli::try_parse_from(std::iter::once("lf".to_string()).chain(options))?;
    crate::lf::commands::flow::drive(store, flow, Some(launch_claim), &cli)
        .await
        .map(|_| ())
}

pub async fn run_worker(task_id: TaskId) -> Result<()> {
    let store = std::sync::Arc::new(
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    run(store, task_id).await
}

fn take_task_exec_env(key: &'static str, label: &str) -> Result<Option<String>> {
    let Some(value) = std::env::var_os(key) else {
        return Ok(None);
    };
    std::env::remove_var(key);
    value
        .into_string()
        .map(Some)
        .map_err(|_| anyhow!("{label} is not valid UTF-8"))
}

/// The Task's Flow to launch: the one it points at, or `selected_flow` when
/// the Task has none or points at a different Flow. Selecting a different
/// Flow replaces the current one in one transaction. A review without its Session parks.
pub(crate) async fn ensure_flow_position(
    store: &SharedStore,
    task_id: &TaskId,
    selected_flow: Option<&str>,
) -> Result<FlowSession> {
    let task = load_task(store, task_id).await?;
    let current = match (store.task_flow(&task.id).await?, selected_flow) {
        (Some(current), Some(selected)) if current.invocation.flow != selected => {
            store
                .start_task_flow(&task.id, start_task_flow(&task, selected)?)
                .await?
        }
        (Some(current), _) => current,
        (None, Some(selected)) => {
            store
                .start_task_flow(&task.id, start_task_flow(&task, selected)?)
                .await?
        }
        (None, None) => anyhow::bail!(
            "Task {} has no active Flow; run `lf --task {} flow start [TEMPLATE]` to select one",
            task.plan.identifier,
            task.plan.identifier
        ),
    };
    if current.is_human() && current.pending_session_id.is_none() {
        return park_at_review(store, &task, &current).await;
    }
    Ok(current)
}

/// Park the Task's Flow at its review: the review Session exists, the
/// worktree is checkpointed, and the Session's terminal is launched.
pub(crate) async fn park_at_review(
    store: &SharedStore,
    task: &Task,
    flow: &FlowSession,
) -> Result<FlowSession> {
    let flow = store.reserve_task_review(flow.id(), flow.version).await?;
    let node_id = flow
        .current()
        .id
        .ok_or_else(|| anyhow!("Task review step has no stable node id"))?;
    checkpoint_worktree_before_human(task, &node_id).await;
    crate::ops::human_session::prepare(store, task, &flow).await?;
    Ok(flow)
}

/// A review FlowStep can outlive this machine's uptime; nothing the Task produced
/// may exist only in the local worktree while it waits. Failure to checkpoint
/// (offline, no remote) must never block the park itself.
async fn checkpoint_worktree_before_human(task: &Task, node_id: &str) {
    if let Err(error) = crate::ops::checkpoint_task_worktree(
        task.worktree.clone(),
        task.plan.identifier.clone(),
        format!("checkpoint: park at review node {node_id}"),
    )
    .await
    {
        tracing::warn!(task = %task.id, %error, "Task parks at a review node without a pushed checkpoint");
    }
}

/// A fresh invocation of `selected_flow` for the Task, at its first step.
fn start_task_flow(task: &Task, selected_flow: &str) -> Result<FlowSession> {
    Ok(FlowSession {
        invocation: QueuedInvocation::load(&task.worktree, selected_flow)?,
        cursor: crate::engine::ExecutionCursor::default(),
        version: 0,
        task_id: Some(task.id.clone()),
        wave_id: Some(task.wave_id.clone()),
        cwd: task.worktree.clone(),
        message: None,
        model: None,
        current_attempt: None,
        pending_session_id: None,
        worker_generation: 0,
        claim: None,
        failure: None,
        finished: false,
        updated_at: time::OffsetDateTime::now_utc(),
    })
}

#[cfg(test)]
struct TestLfBinGuard {
    _ambient: crate::test_ambient::EnvGuard,
    previous: Option<std::ffi::OsString>,
    _ledger: crate::journal::TestLedgerGuard,
}

#[cfg(test)]
impl TestLfBinGuard {
    fn pin() -> Self {
        let ledger = crate::journal::TestLedgerGuard::new();
        let ambient = crate::test_ambient::EnvGuard::new();
        let previous = std::env::var_os("LF_BIN");
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        Self {
            _ambient: ambient,
            previous,
            _ledger: ledger,
        }
    }
}

#[cfg(test)]
impl Drop for TestLfBinGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("LF_BIN", value),
            None => std::env::remove_var("LF_BIN"),
        }
    }
}

#[cfg(test)]
mod planning_tests {
    use crate::durable::{
        Author, FlowSession, TaskWorkerClaim, TaskWorkerClaimOutcome, TaskWorkerOwner, WorkRef,
    };
    use crate::id::{ExecId, TraceId};
    use crate::ops::human_session;
    use crate::ops::task_input::task_seed;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::store::{SharedStore, StorageConfig};
    use crate::work::project::{Project, ProjectId};
    use crate::work::task::{
        Observation, PmWritebackState, Task, TaskEventKind, TaskId, TaskPr, TaskPrId,
    };
    use crate::work::wave::Wave;

    /// Settle one completed step of a position in memory: the engine's traversal.
    fn finish(position: &mut FlowSession) -> anyhow::Result<bool> {
        position.cursor.finish(&position.invocation.steps)
    }

    /// An interrupted step keeps its position and discards its candidate.
    fn interrupt(position: &mut FlowSession) -> bool {
        let leaf = position.cursor.leaf_mut();
        leaf.progress.verdict = None;
        leaf.route = None;
        false
    }

    async fn claim(
        store: &SharedStore,
        task: &Task,
        flow: &FlowSession,
        pid: u32,
    ) -> TaskWorkerClaim {
        let owner = TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid,
            started_at: 1_700_000_000,
        };
        match store
            .claim_task_worker(
                &task.id,
                &flow.invocation.id,
                flow.version,
                &owner,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        }
    }

    /// Start `flow` as the Task's Flow and park it when its first step is a review.
    async fn parked(store: &SharedStore, task: &Task, flow: FlowSession) -> FlowSession {
        let flow = store.start_task_flow(&task.id, flow).await.unwrap();
        if flow.is_human() {
            return super::park_at_review(store, task, &flow).await.unwrap();
        }
        flow
    }

    #[tokio::test]
    async fn feature_repeats_the_whole_slice_then_stops_for_delivery() {
        let (_, task, _) = human_task_fixture().await;
        let mut position = super::start_task_flow(&task, "feature").unwrap();
        assert_eq!(position.current().step, "kickoff");
        finish(&mut position).unwrap();
        assert!(position.is_human());
        assert_eq!(position.current().step, "review-design");
        position.cursor.progress.direction = Some("Design clarified with the human".into());
        finish(&mut position).unwrap();
        for pass in 0..10 {
            for expected in ["implement", "compress", "sync", "realign"] {
                assert_eq!(position.current().step, expected);
                assert!(!finish(&mut position).unwrap());
            }
            assert_eq!(position.current().step, "loop-decide");
            position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
                decision: if pass < 9 {
                    crate::engine::transitions::FlowDecision::Iterate
                } else {
                    crate::engine::transitions::FlowDecision::Advance
                },
                summary: if pass < 9 {
                    "repair the demonstrated gap"
                } else {
                    "all design claims demonstrated"
                }
                .into(),
            });
            assert!(!finish(&mut position).unwrap());
        }
        assert_eq!(position.current().step, "pr-publish");
        assert!(!position.is_human());
        assert!(!finish(&mut position).unwrap());
        assert!(position.is_human());
        assert_eq!(position.current().step, "demo");
        assert_eq!(position.cursor.iteration, 9);
        assert!(position.cursor.progress.verdict.is_none());
        position.cursor.progress.direction = Some("Delivery reviewed".into());
        assert!(!finish(&mut position).unwrap());
        assert_eq!(
            position.cursor.progress.direction.as_deref(),
            Some("Delivery reviewed")
        );
        assert!(position.cursor.progress.verdict.is_none());
        assert_eq!(position.cursor.iteration, 9);
        for expected in ["compress", "sync", "realign", "gate", "pr land -c"] {
            assert_eq!(position.current().step, expected);
            let finished = finish(&mut position).unwrap();
            assert_eq!(finished, expected == "pr land -c");
        }
    }

    #[tokio::test]
    async fn loop_requires_a_decision_and_discards_it_on_interrupt() {
        let (_, task, _) = human_task_fixture().await;
        let mut position = super::start_task_flow(&task, "feature").unwrap();
        while !position.is_decision() {
            assert!(!finish(&mut position).unwrap());
        }
        assert_eq!(position.current().step, "loop-decide");
        let decision_index = position.cursor.index;
        assert!(finish(&mut position)
            .unwrap_err()
            .to_string()
            .contains("requires a decision"));
        assert_eq!(position.cursor.index, decision_index);
        let review = &mut position.cursor.progress;
        review.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Advance,
            summary: "discard on interrupt".into(),
        });
        assert!(!interrupt(&mut position));
        assert_eq!(position.cursor.index, decision_index);
        assert!(position.cursor.progress.verdict.is_none());
    }

    async fn human_task_fixture() -> (SharedStore, Task, FlowSession) {
        let (store, task, position, _) = human_task_fixture_with_database().await;
        (store, task, position)
    }

    async fn human_task_fixture_with_database(
    ) -> (SharedStore, Task, FlowSession, std::path::PathBuf) {
        let database = tempfile::tempdir().unwrap().keep().join("registry.db");
        let (store, task, position) = human_task_fixture_at(&database).await;
        (store, task, position, database)
    }

    async fn human_task_fixture_at(database: &std::path::Path) -> (SharedStore, Task, FlowSession) {
        let repository =
            std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .unwrap();
        // The Task worktree must never be the real checkout: parking at a
        // review node checkpoint-commits the worktree, and a test must not
        // commit or push the developer's repo.
        let worktree = tempfile::tempdir().unwrap().keep();
        let git = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .current_dir(&worktree)
                .args(args)
                .output()
                .unwrap()
                .status
                .success());
        };
        git(&["init", "-q"]);
        git(&[
            "-c",
            "user.email=test@loopflow.dev",
            "-c",
            "user.name=Loopflow Test",
            "commit",
            "--allow-empty",
            "-q",
            "-m",
            "init",
        ]);
        let base_commit = String::from_utf8(
            std::process::Command::new("git")
                .current_dir(&worktree)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(database.to_path_buf()))
                .await
                .unwrap(),
        );
        let now = time::OffsetDateTime::now_utc();
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "human-task-proof".to_string(),
            repository.display().to_string(),
        );
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                flow: "feature".into(),
                status: crate::pm::ProjectStatus::Started,
                id: LinearProjectId::new("human-task-project").unwrap(),
                slug: "human-task-proof".to_string(),
                name: "Review Task proof".to_string(),
                prompt_context: "Prove durable review steps.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        let task = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("human-task-issue").unwrap(),
                identifier: "TEST-1".to_string(),
                title: "Review Task proof".to_string(),
                description: "Stop at review_kickoff.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree: worktree.clone(),
            workspace_slug: "human-task-proof".to_string(),
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
            branch: "test/human-task-proof".to_string(),
            base_commit,
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
        store.create_wave(&wave).await.unwrap();
        store.create_project(&project).await.unwrap();
        store.create_task(&task, &pr).await.unwrap();
        let mut flow = super::start_task_flow(&task, "task-design").unwrap();
        flow.cursor.index = 1;
        (store, task, flow)
    }

    #[tokio::test]
    async fn posted_direction_on_an_idle_task_does_not_start_advancement() {
        let (store, task, _) = human_task_fixture().await;
        assert!(store.task_flow(&task.id).await.unwrap().is_none());
        let id = crate::ops::linear_observe::tests::with_posted_comment(
            &store,
            &task,
            "keep the API",
            crate::ops::linear_observe::publish_task_steer(&store, &task, "keep the API"),
        )
        .await
        .unwrap();
        assert_eq!(id, "comment-1");
        assert!(store.task_flow(&task.id).await.unwrap().is_none());
        let steers = store.task_steers(&task.id).await.unwrap();
        assert_eq!(steers.len(), 1);
        assert!(steers[0].text.contains("keep the API"));
    }

    #[tokio::test]
    async fn comment_sync_recovers_history_and_deduplicates_edits_and_webhooks() {
        let (store, task, _) = human_task_fixture().await;
        let now = time::OffsetDateTime::now_utc();
        let observation = |issue_revision: &str, comment_revision: &str, body: &str| {
            crate::pm::IssueObservation {
                revision: issue_revision.into(),
                title: task.plan.title.clone(),
                description: task.plan.description.clone(),
                comments: vec![crate::pm::IssueComment {
                    author_name: None,
                    id: "c-1".into(),
                    created_at: None,
                    revision: Some(comment_revision.into()),
                    body: body.into(),
                    author_id: Some("my-account".into()),
                }],
            }
        };
        let first = observation(
            "2026-09-23T00:00:00Z",
            "2026-09-22T00:00:00Z",
            "first advice",
        );
        let imported = crate::ops::linear_observe::reconcile_linear_observation(
            &store,
            &task,
            first.clone(),
            "my-account",
            now,
        )
        .await
        .unwrap();
        assert_eq!(imported.follow_ups_created.len(), 1);
        assert!(crate::ops::linear_observe::reconcile_linear_observation(
            &store,
            &task,
            first.clone(),
            "my-account",
            now
        )
        .await
        .unwrap()
        .follow_ups_created
        .is_empty());
        // A stale issue revision can carry a newer comment edit.
        let edited = observation(
            "2026-09-21T00:00:00Z",
            "2026-09-24T00:00:00Z",
            "revised advice",
        );
        assert_eq!(
            crate::ops::linear_observe::reconcile_linear_observation(
                &store,
                &task,
                edited,
                "my-account",
                now
            )
            .await
            .unwrap()
            .follow_ups_created
            .len(),
            1
        );
        assert!(crate::ops::linear_observe::reconcile_linear_observation(
            &store,
            &task,
            first,
            "my-account",
            now
        )
        .await
        .unwrap()
        .follow_ups_created
        .is_empty());
        let steers = store.task_steers(&task.id).await.unwrap();
        assert_eq!(steers.len(), 2);
        assert!(steers[1].text.contains("revised advice"));
    }

    #[tokio::test]
    async fn restart_only_failure_cannot_be_cleared_as_a_retry() {
        let (store, task, _) = human_task_fixture().await;
        let started = super::start_task_flow(&task, "task-design").unwrap();
        let started = store.start_task_flow(&task.id, started).await.unwrap();
        let blocked = store
            .fail_flow(
                started.id(),
                started.version,
                None,
                &crate::durable::TaskFlowBlocker {
                    captured: None,
                    reason: "old Flow definition is unavailable".to_string(),
                    restart_required: true,
                    observed_at: time::OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();

        let error = store.retry_flow(blocked.id(), None).await.unwrap_err();

        assert!(error.to_string().contains("explicit Flow restart"));
        assert_eq!(store.task_flow(&task.id).await.unwrap(), Some(blocked));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // isolates review Run capture and executable resolution
    async fn task_agent_changed_during_worker_applies_to_next_review() {
        let _guard = super::TestLfBinGuard::pin();
        let (store, task, _) = human_task_fixture().await;
        let flow = super::start_task_flow(&task, "task-design").unwrap();
        let mut flow = store.start_task_flow(&task.id, flow).await.unwrap();
        let owner = TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid: 303,
            started_at: 1_700_000_000,
        };
        let TaskWorkerClaimOutcome::Claimed(claim) = store
            .claim_task_worker(
                &task.id,
                &flow.invocation.id,
                flow.version,
                &owner,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap()
        else {
            panic!("fixture worker must acquire its claim");
        };
        store
            .set_task_agent(&task.id, "claude:sonnet")
            .await
            .unwrap();
        finish(&mut flow).unwrap();
        // The checkpoint onto the review releases the claim; parking reserves
        // the review Run with the Task's current choice.
        store
            .checkpoint_flow(
                flow.id(),
                flow.version,
                &flow.cursor,
                Some(&claim),
                Some("ready"),
            )
            .await
            .unwrap();
        let next = store.task_flow(&task.id).await.unwrap().unwrap();
        assert!(next.is_human());
        assert!(next.claim.is_none());
        let next = super::park_at_review(&store, &task, &next).await.unwrap();
        let review = store
            .session(&crate::ops::human_session::flow_id(&next).unwrap())
            .await
            .unwrap()
            .unwrap();
        assert!(!review.input_published);
        assert_eq!(
            (review.provider.as_deref(), review.model.as_deref()),
            (Some("claude"), Some("sonnet"))
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // isolates Run capture and executable resolution
    async fn task_agent_survives_refresh_and_selects_autonomous_and_human_steps() {
        let _guard = super::TestLfBinGuard::pin();
        let (store, task, mut flow) = human_task_fixture().await;
        std::fs::create_dir_all(task.worktree.join(".lf")).unwrap();
        std::fs::write(task.worktree.join(".lf/config.yaml"), "agent: codex\n").unwrap();
        let crate::engine::ConcreteStep::Skill(step) = &mut flow.invocation.steps[1] else {
            panic!("review skill");
        };
        step.skill.agent = Some("codex:step-model".into());
        step.skill.default_agent = Some("claude:haiku".into());

        store
            .set_task_agent(&task.id, "claude:sonnet")
            .await
            .unwrap();
        // A planning write made from an older snapshot cannot undo the choice.
        store.update_task(&task).await.unwrap();
        let resumed = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(resumed.agent.as_deref(), Some("claude:sonnet"));
        // Parking at the review reserves its Run; the choice lands on that row.
        let mut flow = store.start_task_flow(&task.id, flow.clone()).await.unwrap();
        flow = store
            .reserve_task_review(flow.id(), flow.version)
            .await
            .unwrap();
        let review_id = crate::ops::human_session::flow_id(&flow).unwrap();
        let agent = crate::ops::human_session::select_review_agent(&store, &resumed, &flow)
            .await
            .unwrap();
        assert_eq!(agent, "claude:sonnet");
        let review = store.session(&review_id).await.unwrap().unwrap();
        assert_eq!(
            (review.provider.as_deref(), review.model.as_deref()),
            (Some("claude"), Some("sonnet"))
        );

        // A later choice re-targets the same unlaunched Run in place.
        store
            .set_task_agent(&task.id, "codex:replacement")
            .await
            .unwrap();
        crate::ops::human_session::retarget_prepared_task_review(&store, &task)
            .await
            .unwrap();
        let retargeted = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(retargeted.cursor, flow.cursor);
        let retargeted_run = store.session(&review_id).await.unwrap().unwrap();
        assert_eq!(retargeted_run.artifact_key, review.artifact_key);
        assert!(!retargeted_run.input_published);
        assert_eq!(
            (
                retargeted_run.provider.as_deref(),
                retargeted_run.model.as_deref()
            ),
            (Some("codex"), Some("replacement"))
        );
        // A launched Run keeps the provider it launched with.
        store
            .sqlite
            .publish_review_capture(
                &review_id,
                store
                    .sqlite
                    .captured_sequence(&retargeted_run.artifact_key)
                    .unwrap()
                    .unwrap(),
                retargeted.version,
                "codex",
                Some("replacement"),
            )
            .unwrap();
        store.set_task_agent(&task.id, "claude:opus").await.unwrap();
        crate::ops::human_session::retarget_prepared_task_review(&store, &task)
            .await
            .unwrap();
        let published = store.session(&review_id).await.unwrap().unwrap();
        assert_eq!(published.model.as_deref(), Some("replacement"));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn interrupted_step_restarts_in_place_with_fresh_direction() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, mut flow) = human_task_fixture().await;
        let interrupted_step = flow.current().step.clone();

        assert!(!interrupt(&mut flow));
        store
            .append_steer(
                &WorkRef::Task(task.id.clone()),
                Author::User,
                "direction after interrupt",
            )
            .await
            .unwrap();

        let prepared = crate::ops::task_input::prepare(&store, &task, "human-task-proof")
            .await
            .unwrap();

        assert_eq!(flow.current().step, interrupted_step);
        assert!(prepared.message.contains("direction after interrupt"));
    }

    #[tokio::test]
    async fn selecting_another_flow_replaces_only_the_managed_invocation() {
        let (store, task, _) = human_task_fixture().await;
        let original = super::ensure_flow_position(&store, &task.id, Some("code"))
            .await
            .unwrap();
        let contribution = super::start_task_flow(&task, "code").unwrap();
        let contribution = store.create_flow(contribution).await.unwrap();
        let replacement = super::ensure_flow_position(&store, &task.id, Some("pursue"))
            .await
            .unwrap();
        assert_ne!(original.id(), replacement.id());
        assert!(store.flow(original.id()).await.unwrap().unwrap().finished);
        assert_eq!(
            store.flow(contribution.id()).await.unwrap().unwrap(),
            contribution
        );
        assert_eq!(
            store.task_flow(&task.id).await.unwrap().unwrap(),
            replacement
        );
        assert_eq!(
            super::ensure_flow_position(&store, &task.id, Some("pursue"))
                .await
                .unwrap(),
            replacement
        );
        assert!(store
            .reserve_attempt(original.id(), original.version, None, None)
            .await
            .is_err());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Account launch intent is process-scoped.
    async fn active_task_invocation_ignores_later_flow_and_skill_edits() {
        let _lock = crate::journal::test_env_lock();
        let accounts = crate::provider_account::lease::AccountSelection::from_flags(
            &["claude=first@".into(), "codex=second@".into()],
            &[],
        )
        .unwrap();
        let selected = accounts.activate().unwrap();
        let (store, task, _) = human_task_fixture().await;
        let flow_dir = task.worktree.join(".lf/flows");
        let skill_dir = task.worktree.join(".lf/skills");
        std::fs::create_dir_all(&flow_dir).unwrap();
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            flow_dir.join("persisted-proof.yaml"),
            "- original-proof\n- cmd: sync --plan\n",
        )
        .unwrap();
        std::fs::write(
            skill_dir.join("original-proof.md"),
            "# Original\n\nExecute the definition captured at Flow start.\n",
        )
        .unwrap();
        let flow = super::start_task_flow(&task, "persisted-proof").unwrap();
        drop(selected);
        store.start_task_flow(&task.id, flow.clone()).await.unwrap();

        std::fs::write(
            flow_dir.join("persisted-proof.yaml"),
            "- replacement-proof\n- cmd: doctor\n",
        )
        .unwrap();
        std::fs::write(
            skill_dir.join("original-proof.md"),
            "# Mutated\n\nThis must not affect the active invocation.\n",
        )
        .unwrap();
        std::fs::write(
            skill_dir.join("replacement-proof.md"),
            "# Replacement\n\nA future invocation may use this.\n",
        )
        .unwrap();

        let persisted = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(persisted.invocation.accounts.as_deref(), Some(&accounts));
        let crate::engine::ConcreteStep::Skill(active_skill) = persisted.current_plan() else {
            panic!("active first step is a skill")
        };
        assert_eq!(active_skill.skill.name, "original-proof");
        assert!(active_skill
            .skill
            .content
            .as_deref()
            .unwrap()
            .contains("captured at Flow start"));
        let crate::engine::ConcreteStep::Command(active_op) = &persisted.invocation.steps[1] else {
            panic!("active second step is an op")
        };
        assert_eq!(active_op.item.command, "sync");
        assert_eq!(active_op.item.args, ["--plan"]);

        let future = super::start_task_flow(&task, "persisted-proof").unwrap();
        let crate::engine::ConcreteStep::Skill(future_skill) = future.current_plan() else {
            panic!("future first step is a skill")
        };
        assert_eq!(future_skill.skill.name, "replacement-proof");
        let crate::engine::ConcreteStep::Command(future_op) = &future.invocation.steps[1] else {
            panic!("future second step is an op")
        };
        assert_eq!(future_op.item.command, "doctor");
    }

    #[tokio::test]
    async fn review_publication_retry_preserves_identity_and_claims_sql_once() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let position = parked(&store, &task, flow).await;
        let session_id = human_session::flow_id(&position).unwrap();
        let (reserved, run) = store.reserve_review_run(&position).await.unwrap();
        let spec = crate::session_record::SessionCaptureSpec {
            harness: "codex".into(),
            model: None,
            surface: "tui".into(),
            cwd: task.worktree.clone(),
            repo: None,
            worktree: None,
            skill: None,
            subjects: Vec::new(),
            flow: crate::session_record::SessionFlowMembership::Step(
                crate::session_record::SessionFlowStep::of(&reserved).unwrap(),
            ),
            work: None,
        };
        let context = crate::trace::PreparedTurnContext::from_prompts("system", "review");
        let interrupted = crate::session_record::CaptureHandle::begin_reserved_with_context(
            spec.clone(),
            run.artifact_key.clone(),
            None,
            &context,
            |_| {
                Err(crate::store::StoreError::InvalidAuthority(
                    "interrupted before SQL publication".into(),
                ))
            },
        );
        assert!(interrupted.is_err());
        let dir =
            crate::session_record::record_dir(&crate::store::lf_home_dir(), &run.artifact_key)
                .unwrap();
        let original = std::fs::read(dir.join("manifest.json")).unwrap();
        assert!(!dir.join("terminal.json").exists());
        let (retry, same_run) = store.reserve_review_run(&reserved).await.unwrap();
        assert_eq!(same_run, run);
        assert_eq!(retry, reserved);
        let capture = crate::session_record::CaptureHandle::begin_reserved_with_context(
            spec.clone(),
            run.artifact_key.clone(),
            None,
            &context,
            |id| {
                store.sqlite.publish_review_capture(
                    &session_id,
                    store.sqlite.captured_sequence(id).unwrap().unwrap(),
                    retry.version,
                    "codex",
                    None,
                )
            },
        )
        .unwrap();
        assert_eq!(capture.artifact_key(), run.artifact_key);
        assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), original);
        assert_eq!(store.session_inputs(&session_id).await.unwrap().len(), 1);
        assert!(
            store
                .session(&session_id)
                .await
                .unwrap()
                .unwrap()
                .input_published
        );
        assert!(
            crate::session_record::CaptureHandle::begin_reserved_with_context(
                spec,
                run.artifact_key.clone(),
                None,
                &context,
                |id| store.sqlite.publish_review_capture(
                    &session_id,
                    store.sqlite.captured_sequence(id).unwrap().unwrap(),
                    retry.version,
                    "codex",
                    None
                ),
            )
            .is_err()
        );
        assert!(!dir.join("terminal.json").exists());
        // Publication alone does not establish that a provider started or died.
        // The real Open path must retain this uncertain attempt, not replace it.
        let error = human_session::open(&store, &session_id, human_session::OpenMode::Refuse, true)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("launch status is unresolved"));
        assert_eq!(
            store
                .session(&session_id)
                .await
                .unwrap()
                .unwrap()
                .artifact_key,
            run.artifact_key
        );
        assert_eq!(store.session_inputs(&session_id).await.unwrap().len(), 1);
        capture.finish("interrupted").unwrap();
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn claimed_autonomous_boundary_settles_once_at_the_human_node() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, _) = human_task_fixture().await;
        let flow = super::start_task_flow(&task, "task-design").unwrap();
        let initial = store.start_task_flow(&task.id, flow).await.unwrap();
        let held = claim(&store, &task, &initial, 101).await;
        let mut flow = store.task_flow(&task.id).await.unwrap().unwrap();
        let completed = finish(&mut flow).unwrap();
        assert!(!completed);
        // The checkpoint onto the review releases the claim in the same write.
        store
            .checkpoint_flow(
                flow.id(),
                flow.version,
                &flow.cursor,
                Some(&held),
                Some("design ready"),
            )
            .await
            .unwrap();
        assert!(store
            .checkpoint_flow(flow.id(), flow.version, &flow.cursor, Some(&held), None)
            .await
            .is_err());

        let settled = store.task_flow(&task.id).await.unwrap().unwrap();
        assert!(settled.is_human());
        assert!(settled.claim.is_none());
        assert_eq!(settled.version, initial.version + 1);
        super::park_at_review(&store, &task, &settled)
            .await
            .unwrap();
        assert_eq!(
            store
                .sessions(&crate::session::SessionFilter::default())
                .await
                .unwrap()
                .len(),
            1
        );
        let events = store.task_events_after(&task.id, 0).await.unwrap();
        assert!(events.iter().any(|event| matches!(
            &event.kind, TaskEventKind::Progress { summary } if summary == "design ready"
        )));
    }

    #[tokio::test]
    async fn finished_task_or_final_skill_retires_the_worker_without_restarting() {
        for task_done in [false, true] {
            let (store, task, _) = human_task_fixture().await;
            let mut position = super::start_task_flow(&task, "task-design").unwrap();
            if !task_done {
                position.invocation.steps.truncate(1);
            }
            position.cursor.iteration = 3;
            let position = store.start_task_flow(&task.id, position).await.unwrap();
            let held = claim(&store, &task, &position, 101).await;
            let mut position = store.task_flow(&task.id).await.unwrap().unwrap();
            if task_done {
                store.complete_task(&task, None).await.unwrap();
                let launcher = <crate::lf::Cli as clap::Parser>::try_parse_from(["lf"]).unwrap();
                let result = crate::lf::commands::flow::drive(
                    store.clone(),
                    position,
                    Some(held),
                    &launcher,
                )
                .await;
                assert!(result.unwrap_err().to_string().contains("unsettled PR"));
                assert!(task.worktree.exists());
            } else {
                assert!(finish(&mut position).unwrap());
                assert_eq!(position.cursor.iteration, 3);
                store
                    .end_flow(position.id(), position.version, Some(&held), "done")
                    .await
                    .unwrap();
            }
            assert!(store.task_flow(&task.id).await.unwrap().is_none());
            assert_eq!(
                store
                    .work_status(&WorkRef::Task(task.id.clone()))
                    .await
                    .unwrap(),
                if task_done {
                    crate::durable::WorkStatus::Done
                } else {
                    crate::durable::WorkStatus::Ready
                }
            );
            let events = store.task_events_after(&task.id, 0).await.unwrap();
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event.kind, TaskEventKind::FlowFinished { .. }))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn task_seed_uses_the_current_chapter_plan() {
        let now = time::OffsetDateTime::UNIX_EPOCH;
        let task = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("issue-1").unwrap(),
                identifier: "INF-123".to_string(),
                title: "Ship it".to_string(),
                description: "Task direction".to_string(),
                pm_snapshot_synced_at: 11,
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: crate::id::WaveId::new(),
            project_id: crate::work::project::ProjectId::new(),
            worktree: "/tmp/task".into(),
            workspace_slug: "ship-it".to_string(),
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
            slug: "ship-it".to_string(),
            branch: "jack/ship-it".to_string(),
            base_commit: "deadbeef".to_string(),
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
        let project = ProjectPlan {
            flow: "feature".into(),
            status: crate::pm::ProjectStatus::Started,
            id: LinearProjectId::new("project-1").unwrap(),
            slug: "runtime".to_string(),
            name: "Current project name".to_string(),
            prompt_context: "Current project definition".to_string(),
            pm_snapshot_synced_at: 22,
        };
        let seed = task_seed(&task, &project, &pr, "wave", &[]);

        assert!(seed.contains("Current project name"));
        assert!(seed.contains("Current project definition"));
        assert!(seed.contains("Task directive snapshot synced at: 11"));
        assert!(seed.contains("Chapter plan snapshot synced at: 22"));
        assert!(!seed.contains("metric-portfolio"));
        assert!(!seed.contains("project-owned-metrics"));
    }
}
