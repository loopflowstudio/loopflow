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
/// claim. A step that ends without a result releases the position; a
/// decision Run that failed without a verdict opens its one unblock Session.
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
    let result =
        crate::lf::commands::flow::drive(store.clone(), flow, Some(launch_claim), &cli).await;
    if result.as_ref().err().is_some_and(|error| {
        matches!(
            error.downcast_ref::<crate::lf::commands::flow::StepEnd>(),
            Some(crate::lf::commands::flow::StepEnd::StoreChanged(_))
        )
    }) {
        return result.map(|_| ());
    }
    if result.is_err() {
        if let Some(blocked) = store.task_flow(&task_id).await? {
            if blocked
                .failure
                .as_ref()
                .is_some_and(|failure| failure.captured.is_some())
                && blocked.is_decision()
            {
                let task = load_task(&store, &task_id).await?;
                crate::ops::human_session::task_unblock(&store, &task, &blocked).await?;
            }
        }
    }
    result.map(|_| ())
}

pub async fn run_worker(task_id: TaskId) -> Result<()> {
    crate::ops::task_destination::require_worker_destination()?;
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

/// The human completed the Task's review; the store advances the cursor with
/// the feedback, or ends the Flow when the review was its last step.
pub(crate) async fn complete_human_flow_step(
    store: &SharedStore,
    token: &crate::ops::human_session::FlowSessionToken,
    expected: &FlowSession,
) -> Result<()> {
    if !crate::ops::human_session::token_is_current(store, token).await? {
        anyhow::bail!("review session is stale");
    }
    crate::ops::human_session::require_current_review_actor(store, expected).await?;
    let text = expected
        .ready_summary
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            anyhow!("review is not ready; its agent must run `lf session ready` first")
        })?;
    let step = expected.current();
    if !step.human
        || step.invocation_id != token.invocation_id
        || step.id.as_deref() != Some(token.node_id.as_str())
        || step.step != token.skill.name
        || step.iteration != token.iteration
        || step.flow != token.flow
    {
        anyhow::bail!("review session no longer matches the Task Flow position");
    }
    store
        .complete_task_review(&token.task_id, expected, text)
        .await?;
    Ok(())
}

/// The Task's Flow to launch: the one it points at, or `selected_flow` when
/// the Task has none or points at a different Flow. Selecting a different
/// Flow replaces the current one in one transaction. A blocked decision opens
/// its unblock Session; a review without its Session parks.
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
            "Task {} has no active Flow; run `lf task run {} [--flow FLOW]` to select one",
            task.plan.identifier,
            task.plan.identifier
        ),
    };
    if current
        .failure
        .as_ref()
        .is_some_and(|failure| failure.captured.is_some())
        && current.is_decision()
    {
        crate::ops::human_session::task_unblock(store, &task, &current).await?;
        return Ok(current);
    }
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
        ready_summary: None,
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
    ledger: crate::journal::TestLedgerGuard,
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
            ledger,
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
    use crate::ops::human_session::{self, SessionFlowMembership};
    use crate::ops::task_input::task_seed;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::session_record::SessionTitleSource;
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

    /// The Run the claim reserved for the current step.
    async fn reserved_capture(store: &SharedStore, task: &Task) -> String {
        let flow = store.task_flow(&task.id).await.unwrap().unwrap();
        store
            .reserve_attempt(flow.id(), flow.version, flow.claim.as_ref(), None)
            .await
            .unwrap()
            .current_attempt
            .unwrap()
            .run_id
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
            for expected in ["implement", "compress", "rebase", "realign"] {
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
        position.cursor.progress.direction = Some("Human requested a delivery correction".into());
        finish(&mut position).unwrap();
        assert_eq!(position.current().step, "loop-decide");
        assert!(position.cursor.progress.verdict.is_none());
        assert_eq!(position.cursor.iteration, 9);
        position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Iterate,
            summary: "Human requested a delivery correction".into(),
        });
        assert!(!finish(&mut position).unwrap());
        assert_eq!(position.current().step, "implement");
        assert_eq!(
            position.cursor.progress.direction.as_deref(),
            Some("Human requested a delivery correction")
        );
        for _ in 0..4 {
            finish(&mut position).unwrap();
        }
        assert_eq!(position.current().step, "loop-decide");
        position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Iterate,
            summary: "Continue the human-requested revision".into(),
        });
        finish(&mut position).unwrap();
        assert_eq!(position.current().step, "implement");
        assert_eq!(position.cursor.progress.repeats["decide"], 10);
        // Final Advance is the only edge into queue and landing. This traverses
        // the authored plan without invoking any publication operation.
        for _ in 0..4 {
            finish(&mut position).unwrap();
        }
        position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Advance,
            summary: "Revision proved".into(),
        });
        finish(&mut position).unwrap();
        assert_eq!(position.current().step, "pr-publish");
        finish(&mut position).unwrap();
        assert_eq!(position.current().step, "demo");
        finish(&mut position).unwrap();
        position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Advance,
            summary: "Human feedback addressed".into(),
        });
        finish(&mut position).unwrap();
        for expected in ["compress", "rebase", "realign", "gate", "pr land -c"] {
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

    fn pursue_decision(task: &Task) -> FlowSession {
        let mut flow = super::start_task_flow(task, "pursue").unwrap();
        while !flow.is_decision() {
            assert!(!finish(&mut flow).unwrap());
        }
        assert_eq!(flow.current().step, "loop-decide");
        flow
    }

    #[test]
    fn task_decision_live_unblock_returns_feedback_without_navigation() {
        let _guard = super::TestLfBinGuard::pin();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (store, task, _) = runtime.block_on(human_task_fixture_at(
            &_guard.ledger.home().join("loopflow.db"),
        ));
        crate::journal::with_runtime(&task.worktree, &["live-unblock-proof".into()], || {
            runtime.block_on(async {
                let flow = pursue_decision(&task);
                let flow = store.start_task_flow(&task.id, flow).await.unwrap();
                let owner = crate::journal::current_process_identity().unwrap();
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
                    panic!("decision claim")
                };
                let run = reserved_capture(&store, &task).await;
                let publish = {
                    let (store, id, version, claim) = (
                        store.clone(),
                        flow.id().to_owned(),
                        flow.version,
                        claim.clone(),
                    );
                    move |run: &String| {
                        store.sqlite.publish_attempt(
                            &id,
                            version,
                            store.sqlite.captured_sequence(run).unwrap().unwrap(),
                            Some(&claim),
                            "proof",
                            None,
                        )
                    }
                };
                let capture = crate::session_record::CaptureHandle::begin_reserved_with_context(
                    crate::session_record::SessionCaptureSpec {
                        harness: "proof".into(),
                        model: None,
                        surface: "headless".into(),
                        cwd: task.worktree.clone(),
                        repo: None,
                        worktree: None,
                        skill: Some("loop-decide".into()),
                        subjects: vec![],
                        flow: crate::session_record::SessionFlowMembership::Step(
                            crate::session_record::SessionFlowStep::of(&flow).unwrap(),
                        ),
                        work: None,
                    },
                    run.clone(),
                    None,
                    &crate::trace::PreparedTurnContext::from_prompts("system", "decide"),
                    publish,
                )
                .unwrap();
                capture.record_input("initial", "Fixture decision is active");
                std::env::set_var(crate::durable::RUN_ID_ENV, run.as_str());
                std::env::set_var(crate::session_record::RUN_DIR_ENV, capture.artifact_dir());
                let before = store.task_flow(&task.id).await.unwrap().unwrap();
                let key = crate::ops::human_session::task_unblock_key(&before).unwrap();
                let ask = async {
                    crate::ops::human_session::ask_once(
                        &store,
                        &key,
                        "Choose the consumer to replace",
                        Some("unblock"),
                    )
                    .await
                    .unwrap_or_else(|error| panic!("live decision Ask failed: {error:#}"))
                };
                let human = async {
                    let session = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        loop {
                            let sessions = crate::ops::human_session::list(
                                &store,
                                &crate::session::SessionFilter::default(),
                            )
                            .await
                            .unwrap();
                            if let Some(session) = sessions
                                .into_iter()
                                .find(|s| s.kind == crate::ops::human_session::SessionKind::Ask)
                            {
                                break session;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                    })
                    .await
                    .unwrap();
                    let (execution, graph) =
                        crate::ops::task_execution::task_execution_and_flow(&store, &task.id)
                            .await
                            .unwrap();
                    assert_eq!(
                        execution.state,
                        crate::ops::task_execution::TaskExecutionState::Blocked
                    );
                    assert_eq!(
                        execution.captured,
                        store.sqlite.captured_sequence(&run).unwrap()
                    );
                    assert!(execution.reason.contains(&session.id));
                    assert!(execution.reason.contains("without Task resume"));
                    let crate::ops::task_flow::TaskFlowRecord::Pinned(graph) = graph else {
                        panic!("pinned")
                    };
                    assert_eq!(graph.reason, execution.reason);
                    assert_eq!(graph.execution, execution.state);
                    assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), before);
                    std::env::set_var(
                        crate::durable::RUN_ID_ENV,
                        store
                            .session(&session.id)
                            .await
                            .unwrap()
                            .unwrap()
                            .artifact_key
                            .as_str(),
                    );
                    std::env::set_var(
                        crate::ops::human_session::HUMAN_SESSION_ENV,
                        serde_json::to_string(&crate::ops::human_session::HumanSessionToken::Ask {
                            id: session.id.clone(),
                        })
                        .unwrap(),
                    );
                    crate::ops::human_session::mark_ready(&store, "Switch the existing reader")
                        .await
                        .unwrap();
                    crate::ops::human_session::complete(&store, &session.id)
                        .await
                        .unwrap();
                };
                let (feedback, ()) = tokio::join!(ask, human);
                assert_eq!(feedback, "Switch the existing reader");
                assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), before);
                assert_eq!(
                    crate::ops::task_execution::task_execution(&store, &task.id)
                        .await
                        .unwrap()
                        .state,
                    crate::ops::task_execution::TaskExecutionState::Running
                );
                assert!(before.cursor.leaf().progress.verdict.is_none());
                std::env::remove_var(crate::durable::RUN_ID_ENV);
                std::env::remove_var(crate::session_record::RUN_DIR_ENV);
                std::env::remove_var(crate::ops::human_session::HUMAN_SESSION_ENV);
                Ok(())
            })
        })
        .unwrap();
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
    async fn active_task_invocation_ignores_later_flow_and_skill_edits() {
        let (store, task, _) = human_task_fixture().await;
        let flow_dir = task.worktree.join(".lf/flows");
        let skill_dir = task.worktree.join(".lf/skills");
        std::fs::create_dir_all(&flow_dir).unwrap();
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            flow_dir.join("persisted-proof.yaml"),
            "- original-proof\n- cmd: rebase --plan\n",
        )
        .unwrap();
        std::fs::write(
            skill_dir.join("original-proof.md"),
            "# Original\n\nExecute the definition captured at Flow start.\n",
        )
        .unwrap();
        let flow = super::start_task_flow(&task, "persisted-proof").unwrap();
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
        assert_eq!(active_op.item.command, "rebase");
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

    async fn park_human_task(
        store: &SharedStore,
        task: &Task,
        flow: &FlowSession,
    ) -> crate::ops::human_session::FlowSessionToken {
        if store.task_flow(&task.id).await.unwrap().is_none() {
            parked(store, task, flow.clone()).await;
        }
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        if position.is_human() && position.pending_session_id.is_none() {
            super::park_at_review(store, task, &position).await.unwrap();
        }
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        let step = position.current();
        let crate::engine::ConcreteStep::Skill(planned) = position.current_plan() else {
            panic!("human position must select a Skill")
        };
        crate::ops::human_session::FlowSessionToken {
            task_id: task.id.clone(),
            invocation_id: position.invocation.id.clone(),
            flow: step.flow,
            node_id: step.id.unwrap(),
            skill: planned.skill.clone(),
            iteration: position.cursor.iteration,
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Serializes fixture executable and Home.
    async fn restarting_a_human_node_reuses_the_same_task_position() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let original = park_human_task(&store, &task, &flow).await;
        ready_review(&store, &task, "ready").await;
        let persisted = store.task_flow(&task.id).await.unwrap().unwrap();
        let run_id = persisted.review_artifact_key().cloned().unwrap();
        let restarted_flow = super::ensure_flow_position(&store, &task.id, None)
            .await
            .unwrap();
        assert_eq!(restarted_flow, persisted);
        let recovered = park_human_task(&store, &task, &restarted_flow).await;

        assert_eq!(recovered, original);
        assert_eq!(
            store
                .sessions(&crate::session::SessionFilter::default())
                .await
                .unwrap()
                .len(),
            1
        );
        let recovered = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(recovered.review_artifact_key(), Some(&run_id));
        assert_eq!(recovered.ready_summary.as_deref(), Some("ready"));
    }

    #[tokio::test]
    async fn review_actions_preserve_selected_attempt_across_replacement() {
        use crate::ops::human_session::action_test::{LookupPause, NativeClients};

        let _lf_bin = super::TestLfBinGuard::pin();
        for action in ["complete", "rename", "open"] {
            for selector_kind in ["run", "prefix", "session"] {
                let (store, task, flow) = human_task_fixture().await;
                let position = parked(&store, &task, flow).await;
                let id = human_session::flow_id(&position).unwrap();
                let (position, first) = store.reserve_review_run(&position).await.unwrap();
                let capture = crate::session_record::CaptureHandle::begin_reserved_with_context(
                    crate::session_record::SessionCaptureSpec {
                        harness: "codex".into(),
                        model: None,
                        surface: "tui".into(),
                        cwd: task.worktree.clone(),
                        repo: None,
                        worktree: None,
                        skill: None,
                        subjects: Vec::new(),
                        flow: crate::session_record::SessionFlowMembership::Step(
                            crate::session_record::SessionFlowStep::of(&position).unwrap(),
                        ),
                        work: None,
                    },
                    first.artifact_key.clone(),
                    None,
                    &crate::trace::PreparedTurnContext::from_prompts("system", "review"),
                    |run| {
                        store.sqlite.publish_review_capture(
                            &id,
                            store.sqlite.captured_sequence(run).unwrap().unwrap(),
                            position.version,
                            "codex",
                            None,
                        )
                    },
                )
                .unwrap();
                store
                    .ready_session(&id, first.captured, "Retain this feedback")
                    .await
                    .unwrap();
                let before = store.task_flow(&task.id).await.unwrap().unwrap();
                let original = store.session(&id).await.unwrap().unwrap();
                let selector = match selector_kind {
                    "run" => first.artifact_key.to_string(),
                    "prefix" => first.artifact_key.as_str()[..20].to_string(),
                    _ => id.clone(),
                };
                let clients = NativeClients::new(&id, std::slice::from_ref(&first.artifact_key));
                let pause = LookupPause::at(action, &selector);
                // Rename starts acquiring this lock after its initial lookup.
                // Replacement owns it until B and retained feedback are published.
                let replacement_lock = human_session::lock_session_exec(&id).unwrap();
                let request = async {
                    match action {
                        "complete" => human_session::complete(&store, &selector).await,
                        "rename" => {
                            human_session::rename(
                                &store,
                                &selector,
                                "Chosen conversation",
                                SessionTitleSource::Human,
                            )
                            .await
                        }
                        _ => {
                            human_session::open(
                                &store,
                                &selector,
                                human_session::OpenMode::Refuse,
                                true,
                            )
                            .await
                        }
                    }
                };
                let replace = async {
                    pause.reached.notified().await;
                    let (reserved, replacement) = store.reserve_review_run(&before).await.unwrap();
                    store
                        .sqlite
                        .publish_review_capture(
                            &id,
                            store
                                .sqlite
                                .captured_sequence(&replacement.artifact_key)
                                .unwrap()
                                .unwrap(),
                            reserved.version,
                            "codex",
                            None,
                        )
                        .unwrap();
                    clients.add(&id, &replacement.artifact_key);
                    let position = store.task_flow(&task.id).await.unwrap().unwrap();
                    drop(replacement_lock);
                    pause.proceed.notify_one();
                    (position, replacement)
                };
                let (result, (expected, replacement)) =
                    tokio::time::timeout(std::time::Duration::from_secs(10), async {
                        tokio::join!(request, replace)
                    })
                    .await
                    .unwrap();
                if action == "rename" && selector_kind == "session" {
                    assert_eq!(result.unwrap().title, "Chosen conversation");
                } else {
                    assert!(
                        result.is_err(),
                        "{action} via {selector_kind} selected a replacement"
                    );
                    assert_eq!(
                        store.session(&id).await.unwrap().unwrap().title,
                        original.title
                    );
                }
                assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), expected);
                let session = store.session(&id).await.unwrap().unwrap();
                let current = session.clone();
                assert_eq!(current.artifact_key, replacement.artifact_key);
                assert_eq!(
                    session.ready_summary.as_deref(),
                    Some("Retain this feedback")
                );
                assert!(session.completed_at.is_none());
                assert_eq!(
                    clients.active(),
                    [first.artifact_key.clone(), replacement.artifact_key.clone()].into()
                );
                assert!(store
                    .rename_session(
                        &id,
                        first.captured,
                        "Stale direct write",
                        crate::session::TitleSource::Human,
                    )
                    .await
                    .is_err());

                // Native resume must hold the same lock through handoff, then release
                // it so Complete can settle and stop exactly B. A remains untouched.
                let opened = human_session::open(
                    &store,
                    replacement.artifact_key.as_str(),
                    human_session::OpenMode::Refuse,
                    true,
                )
                .await
                .unwrap();
                assert_eq!(opened.id, replacement.id);
                let completed = human_session::complete(&store, replacement.artifact_key.as_str())
                    .await
                    .unwrap();
                assert_eq!(completed.id, replacement.id);
                assert_eq!(clients.active(), [first.artifact_key.clone()].into());
                assert!(store.task_flow(&task.id).await.unwrap().is_none());
                let closed = store.session(&id).await.unwrap().unwrap();
                assert!(closed.completed_at.is_some());
                assert_eq!(
                    closed.ready_summary.as_deref(),
                    Some("Retain this feedback")
                );
                assert!(human_session::complete(&store, &id).await.is_err());
                let events = store.recent_task_events(&task.id, 20).await.unwrap();
                assert_eq!(
                    events
                        .iter()
                        .filter(|event| matches!(
                            &event.kind, TaskEventKind::FlowFinished { summary, .. }
                                if summary == "Retain this feedback"
                        ))
                        .count(),
                    1
                );
                assert_eq!(store.session_inputs(&id).await.unwrap().len(), 2);
                drop(capture);
            }
        }
    }

    #[tokio::test]
    async fn flow_session_name_and_membership_survive_sql_run_replacement() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let position = parked(&store, &task, flow).await;
        let id = human_session::flow_id(&position).unwrap();
        let first = store.session(&id).await.unwrap().unwrap();
        let named = human_session::rename(
            &store,
            first.artifact_key.as_str(),
            "Parser review",
            SessionTitleSource::Human,
        )
        .await
        .unwrap();
        assert_eq!(named.id, id);
        assert_eq!(
            store.session(&id).await.unwrap().unwrap().captured,
            first.captured
        );
        assert_eq!(named.work, Some(WorkRef::Task(task.id.clone())));
        assert!(matches!(
            &named.flow_membership,
            SessionFlowMembership::Step { invocation_id, .. }
                if invocation_id == &position.invocation.id
        ));

        let home = crate::store::observability_home_dir();
        let first_dir = crate::session_record::record_dir(&home, &first.artifact_key).unwrap();
        std::fs::create_dir_all(&first_dir).unwrap();
        let manifest = crate::session_record::SessionCaptureManifest {
            schema_version: 1,
            artifact_key: first.artifact_key.clone(),
            caller_artifact_key: None,
            created_at: time::OffsetDateTime::now_utc(),
            harness: "codex".into(),
            model: None,
            surface: "tui".into(),
            cwd: task.worktree.clone(),
            repo: None,
            worktree: None,
            skill: None,
            subjects: Vec::new(),
            flow: Some(crate::session_record::SessionFlowMembership::Step(
                crate::session_record::SessionFlowStep::of(&position).unwrap(),
            )),
            exec: None,
            context: None,
            runtime_path: None,
            runtime_digest: None,
            host: "test".into(),
            boot_id: None,
        };
        std::fs::write(
            first_dir.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        ready_review(&store, &task, "Keep this answer").await;
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        let (_, replacement) = store.reserve_review_run(&position).await.unwrap();
        let retained = human_session::rename(
            &store,
            replacement.artifact_key.as_str(),
            "Generated suggestion",
            SessionTitleSource::Generated,
        )
        .await
        .unwrap();
        assert_eq!(retained.id, id);
        assert_ne!(replacement.captured, first.captured);
        assert_eq!(
            store.session(&id).await.unwrap().unwrap().captured,
            replacement.captured
        );
        assert_eq!(retained.title, "Parser review");
        assert_eq!(retained.title_source, SessionTitleSource::Human);
        assert_eq!(retained.flow_membership, named.flow_membership);
        assert_eq!(retained.ready_summary.as_deref(), Some("Keep this answer"));
        let listed = human_session::list(&store, &crate::session::SessionFilter::default())
            .await
            .unwrap()
            .into_iter()
            .find(|session| session.id == id)
            .unwrap();
        assert_eq!(listed, retained);
        let history = store.session_inputs(&id).await.unwrap();
        assert_eq!(history.len(), 2);
        assert!(history.contains(&first.artifact_key));
        assert!(history.contains(&replacement.artifact_key));
        let historical = human_session::rename(
            &store,
            first.artifact_key.as_str(),
            "Old actor",
            SessionTitleSource::Human,
        )
        .await
        .unwrap_err();
        assert!(historical
            .to_string()
            .contains(&format!("historical attempt of Session {id}")));
        assert!(!first_dir.join("session-name.json").exists());
        let prefix = &first.artifact_key.as_str()[..16];
        assert!(
            human_session::rename(&store, prefix, "Old prefix", SessionTitleSource::Human)
                .await
                .unwrap_err()
                .to_string()
                .contains("historical attempt of Session")
        );
        assert!(human_session::open(
            &store,
            first.artifact_key.as_str(),
            human_session::OpenMode::Refuse,
            false
        )
        .await
        .is_err());
        assert!(human_session::complete(&store, first.artifact_key.as_str())
            .await
            .is_err());

        ready_review(&store, &task, "Keep this answer").await;
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        let (_, third) = store.reserve_review_run(&position).await.unwrap();
        ready_review(&store, &task, "Keep this answer").await;
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(position.review_artifact_key(), Some(&third.artifact_key));
        assert!(store
            .ready_session(&id, first.captured, "stale answer")
            .await
            .is_err());
        let previous_actor = std::env::var_os("LF_RUN_ID");
        std::env::set_var("LF_RUN_ID", first.artifact_key.as_str());
        let stale_actor = human_session::require_current_review_actor(&store, &position).await;
        match previous_actor {
            Some(value) => std::env::set_var("LF_RUN_ID", value),
            None => std::env::remove_var("LF_RUN_ID"),
        }
        assert!(stale_actor.is_err());
        store
            .complete_task_review(&task.id, &position, "complete once")
            .await
            .unwrap();
        assert!(store
            .complete_task_review(&task.id, &position, "late completion")
            .await
            .is_err());
        let error = human_session::rename(
            &store,
            first.artifact_key.as_str(),
            "Closed actor",
            SessionTitleSource::Human,
        )
        .await
        .unwrap_err();
        assert!(error
            .to_string()
            .contains(&format!("historical attempt of Session {id}")));
        assert!(!first_dir.join("session-name.json").exists());
        assert!(human_session::rename(
            &store,
            third.artifact_key.as_str(),
            "Closed actor",
            SessionTitleSource::Human
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("already complete"));
        let closed = store.session(&id).await.unwrap().unwrap();
        let current = closed.clone();
        assert!(closed.completed_at.is_some());
        assert_eq!(closed.title, "Parser review");
        assert_eq!(closed.ready_summary.as_deref(), Some("Keep this answer"));
        assert_eq!(current.artifact_key, third.artifact_key);
        assert_eq!(store.session_inputs(&id).await.unwrap().len(), 3);
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
        let dir = crate::session_record::record_dir(
            &crate::store::authority_home_dir(),
            &run.artifact_key,
        )
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
    async fn session_list_and_open_preserve_valid_reviews_beside_unreadable_captures() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow, database) = human_task_fixture_with_database().await;
        let position = parked(&store, &task, flow).await;
        let id = human_session::flow_id(&position).unwrap();
        let conn = rusqlite::Connection::open(database).unwrap();
        let mut broken_review_id = None;
        for human in [false, true] {
            let broken_id = TaskId::new();
            conn.execute(
                "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,
                issue_title,issue_description,pm_snapshot_synced_at,pm_writeback_json,
                worktree,workspace_slug,created_at,updated_at)
                SELECT ?1,project_id,?1,?1,issue_title,issue_description,
                pm_snapshot_synced_at,pm_writeback_json,?1,?1,created_at,updated_at
                FROM tasks WHERE id=?2",
                rusqlite::params![broken_id.as_str(), task.id.as_str()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO work_placements(task_id,home_id,enabled,placed_at)
                SELECT ?1,home_id,enabled,placed_at FROM work_placements WHERE task_id=?2",
                rusqlite::params![broken_id.as_str(), task.id.as_str()],
            )
            .unwrap();
            let mut broken = position.clone();
            broken.cursor = Default::default();
            broken.task_id = Some(broken_id.clone());
            broken.wave_id = None;
            broken.invocation = crate::durable::test_flow_invocation(
                "broken",
                0,
                "review-design",
                Some("review"),
                human,
            );
            broken.version = 0;
            broken.current_attempt = None;
            broken.pending_session_id = None;
            broken.ready_summary = None;
            let broken = store.start_task_flow(&broken_id, broken).await.unwrap();
            if human {
                store
                    .reserve_task_review(broken.id(), broken.version)
                    .await
                    .unwrap();
                broken_review_id = Some(human_session::flow_id(&broken).unwrap());
            }
            let corrupt = serde_json::json!({"id": broken.invocation.id, "flow": "broken", "steps": "unreadable"}).to_string();
            conn.execute(
                "UPDATE flow_sessions SET invocation_json=?2 WHERE task_id=?1",
                rusqlite::params![broken_id.as_str(), corrupt],
            )
            .unwrap();
            let records = human_session::list(&store, &crate::session::SessionFilter::default())
                .await
                .unwrap();
            assert!(records.iter().any(|record| record.id == id));
            let opened = human_session::open(&store, &id, human_session::OpenMode::Refuse, false)
                .await
                .unwrap();
            assert_eq!(opened.id, id);
            if let Some(broken_id) = &broken_review_id {
                let record = records
                    .iter()
                    .find(|record| &record.id == broken_id)
                    .unwrap();
                assert!(
                    matches!(&record.flow_membership, SessionFlowMembership::Step { flow, .. } if flow == "broken")
                );
                // Inventory exposes recorded metadata; exact open validates the capture.
                assert!(record.actions.iter().any(|action| action.kind
                    == human_session::SessionActionKind::Open
                    && action.unavailable_reason.is_none()));
                assert!(record.actions.iter().any(|action| action.kind
                    == human_session::SessionActionKind::Complete
                    && action.unavailable_reason.is_some()));
                assert!(human_session::open(
                    &store,
                    broken_id,
                    human_session::OpenMode::Refuse,
                    false
                )
                .await
                .unwrap_err()
                .to_string()
                .contains("saved Invocation is unreadable"));
            }
            let retained: String = conn
                .query_row(
                    "SELECT invocation_json FROM flow_sessions WHERE task_id=?1",
                    [broken_id.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(retained, corrupt);
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Serializes fixture executable and Home.
    async fn stale_human_decisions_cannot_target_a_replacement_invocation() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let stale = park_human_task(&store, &task, &flow).await;
        let mut replacement = store.task_flow(&task.id).await.unwrap().unwrap();
        let step = replacement.current();
        replacement.invocation = crate::durable::test_flow_invocation(
            &step.flow,
            replacement.cursor.index as u32,
            &step.step,
            step.id.as_deref(),
            true,
        );
        store
            .restart_task_flow(
                &task,
                store.task_flow(&task.id).await.unwrap().as_ref(),
                "fixture-checkpoint",
            )
            .await
            .unwrap();
        replacement.version = 0;
        replacement.current_attempt = None;
        replacement.pending_session_id = None;
        replacement.ready_summary = None;
        let replacement = parked(&store, &task, replacement).await;

        assert!(
            super::complete_human_flow_step(&store, &stale, &replacement)
                .await
                .is_err()
        );
        let current = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(current.invocation.id, replacement.invocation.id);
        assert_eq!(current.version, replacement.version);
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
    async fn final_skill_completion_removes_the_flow_without_restarting_it() {
        let (store, task, _) = human_task_fixture().await;
        let mut position = super::start_task_flow(&task, "task-design").unwrap();
        position.invocation.steps.truncate(1);
        position.cursor.iteration = 3;
        let position = store.start_task_flow(&task.id, position).await.unwrap();
        let held = claim(&store, &task, &position, 101).await;
        let mut position = store.task_flow(&task.id).await.unwrap().unwrap();
        let completed = finish(&mut position).unwrap();
        assert!(completed);
        assert_eq!(position.cursor.iteration, 3);
        store
            .end_flow(position.id(), position.version, Some(&held), "done")
            .await
            .unwrap();

        assert!(store.task_flow(&task.id).await.unwrap().is_none());
        assert_eq!(
            store
                .work_status(&WorkRef::Task(task.id.clone()))
                .await
                .unwrap(),
            crate::durable::WorkStatus::Ready
        );
        let events = store.task_events_after(&task.id, 0).await.unwrap();
        assert!(events.iter().any(|event| matches!(
            &event.kind, TaskEventKind::FlowFinished { summary, .. } if summary == "done"
        )));
    }

    async fn ready_review(store: &SharedStore, task: &Task, feedback: &str) {
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        let id = crate::ops::human_session::flow_id(&position).unwrap();
        let (position, run) = store.reserve_review_run(&position).await.unwrap();
        store
            .sqlite
            .publish_review_capture(&id, run.captured.unwrap(), position.version, "codex", None)
            .unwrap();
        store
            .ready_session(&id, run.captured, feedback)
            .await
            .unwrap();
    }

    async fn finish_test_decision(
        store: &SharedStore,
        flow: &FlowSession,
        verdict: crate::engine::transitions::FlowVerdict,
    ) -> FlowSession {
        let saved = store
            .reserve_attempt(flow.id(), flow.version, None, None)
            .await
            .unwrap();
        let input = &saved.current_attempt.as_ref().unwrap().run_id;
        store
            .sqlite
            .publish_attempt(
                saved.id(),
                saved.version,
                store.sqlite.captured_sequence(input).unwrap().unwrap(),
                None,
                "proof",
                None,
            )
            .unwrap();
        let actor = store.sqlite.test_flow_turn(input);
        store.sqlite.test_decision_output(&actor, &verdict).unwrap();
        store.sqlite.test_finish_flow_turn(&actor, "completed");
        let mut saved = store.flow(saved.id()).await.unwrap().unwrap();
        finish(&mut saved).unwrap();
        store
            .checkpoint_flow(saved.id(), saved.version, &saved.cursor, None, None)
            .await
            .unwrap()
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // serializes LF_BIN while assembling the real next prompt
    async fn nested_review_returns_feedback_before_the_outer_decision() {
        let _lf_bin = super::TestLfBinGuard::pin();
        use crate::engine::transitions::{FlowDecision, FlowVerdict};
        use crate::engine::{ConcretePath, ConcreteStep, ConcreteXor, Skill};
        let (store, task, _) = human_task_fixture().await;
        let mut flow = super::start_task_flow(&task, "pursue").unwrap();
        let body = flow.invocation.steps.clone();
        let suffix = body[0].clone();
        flow.invocation.steps = vec![
            ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("xor-route"),
                paths: [(
                    "work".into(),
                    ConcretePath {
                        description: "implementation and review".into(),
                        steps: body,
                    },
                )]
                .into(),
                sources: vec![],
            }),
            suffix,
        ];
        flow.cursor.route = Some("work".into());
        finish(&mut flow).unwrap();
        while !flow.is_decision() {
            assert!(!finish(&mut flow).unwrap());
        }
        flow.cursor.leaf_mut().progress.verdict = Some(FlowVerdict {
            decision: FlowDecision::Advance,
            summary: "ready to demonstrate".into(),
        });
        finish(&mut flow).unwrap();
        assert_eq!(flow.current().step, "pr-publish");
        finish(&mut flow).unwrap();
        assert!(flow.is_human());
        assert_eq!(flow.current().step, "demo");
        let token = park_human_task(&store, &task, &flow).await;
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        assert!(super::complete_human_flow_step(&store, &token, &expected)
            .await
            .is_err());
        let notes = task.worktree.join("scratch/search");
        std::fs::create_dir_all(&notes).unwrap();
        let observation = "The human lost their place when clearing a search. Agreed: keep results on one screen. Next: implement clearing without a navigation change and prove focus stays in the search field.";
        std::fs::write(
            notes.join("feedback.md"),
            format!("# Search feedback\n\n{observation}\n\nDesign: [search design](design.md)\n"),
        )
        .unwrap();
        let design =
            "The search field and results share one screen; clearing preserves keyboard focus.";
        std::fs::write(notes.join("design.md"), design).unwrap();
        let feedback = "See scratch/search/feedback.md and scratch/search/design.md; implement the agreed search interaction";
        ready_review(&store, &task, feedback).await;
        let mut ordinary = flow.cursor.clone();
        ordinary.leaf_mut().progress.direction = Some(feedback.into());
        ordinary.finish(&flow.invocation.steps).unwrap();
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        super::complete_human_flow_step(&store, &token, &expected)
            .await
            .unwrap();
        let mut saved = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(saved.cursor, ordinary);
        assert_eq!(saved.current().step, "loop-decide");
        assert_eq!(saved.cursor.iteration, 0);
        assert_eq!(
            saved.cursor.leaf().progress.direction.as_deref(),
            Some(feedback)
        );
        assert_eq!(
            saved.cursor.leaf().progress.direction.as_deref(),
            Some(feedback)
        );
        assert!(saved.cursor.leaf().progress.verdict.is_none());
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        assert!(super::complete_human_flow_step(&store, &token, &expected)
            .await
            .is_err());
        assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), saved);
        saved = finish_test_decision(&store, &saved, FlowVerdict {
            decision: FlowDecision::Iterate,
            summary: "Implement scratch/search/design.md using the findings and proof in scratch/search/feedback.md".into(),
        }).await;
        assert_eq!(saved.current().step, "implement");
        assert_eq!(saved.cursor.iteration, 1);
        assert!(saved
            .cursor
            .leaf()
            .progress
            .direction
            .as_deref()
            .unwrap()
            .contains("Implement scratch/search/design.md"));
        assert_eq!(
            std::fs::read_to_string(notes.join("design.md")).unwrap(),
            design
        );
        while !saved.is_decision() {
            assert!(!finish(&mut saved).unwrap());
            saved = store
                .checkpoint_flow(saved.id(), saved.version, &saved.cursor, None, None)
                .await
                .unwrap();
        }
        saved = finish_test_decision(
            &store,
            &saved,
            FlowVerdict {
                decision: FlowDecision::Advance,
                summary: "revision demonstrated".into(),
            },
        )
        .await;
        assert_eq!(saved.current().step, "pr-publish");
        finish(&mut saved).unwrap();
        store
            .checkpoint_flow(saved.id(), saved.version, &saved.cursor, None, None)
            .await
            .unwrap();
        let later = park_human_task(&store, &task, &flow).await;
        assert_ne!(later.iteration, token.iteration);
        ready_review(&store, &task, "the interaction works").await;
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        super::complete_human_flow_step(&store, &later, &expected)
            .await
            .unwrap();
        let mut saved = store.task_flow(&task.id).await.unwrap().unwrap();
        saved.cursor.leaf_mut().progress.verdict = Some(FlowVerdict {
            decision: FlowDecision::Advance,
            summary: "human feedback and proof agree".into(),
        });
        finish(&mut saved).unwrap();
        assert_eq!(saved.cursor.index, 1);
        assert!(saved.cursor.child.is_none());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // serializes the fixture executable and Home
    async fn completing_a_final_review_finishes_the_flow() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let token = park_human_task(&store, &task, &flow).await;
        ready_review(&store, &task, "design clarified").await;
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        super::complete_human_flow_step(&store, &token, &expected)
            .await
            .unwrap();
        assert!(store.task_flow(&task.id).await.unwrap().is_none());
        assert!(!crate::ops::human_session::token_is_current(&store, &token)
            .await
            .unwrap());
        assert!(store.recent_task_events(&task.id, 10).await.unwrap().iter().any(|event|
            matches!(&event.kind, TaskEventKind::FlowFinished { summary, .. } if summary == "design clarified")
        ));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // serializes the fixture executable and Home
    async fn runtime_child_review_releases_the_worker_and_retains_the_managed_root() {
        let guard = super::TestLfBinGuard::pin();
        let db = guard.ledger.home().join("loopflow.db");
        let (store, task, mut flow) = human_task_fixture_at(&db).await;
        flow.invocation.steps = vec![flow.current_plan().clone()];
        flow.cursor = Default::default();
        flow.invocation
            .steps
            .push(crate::engine::ConcreteStep::Skill(
                crate::engine::ConcreteSkill {
                    skill: crate::engine::Skill::named("decide"),
                    sources: vec![],
                    id: Some("decision".into()),
                    human: false,
                    repeat: Some(crate::engine::flow::RepeatPolicy {
                        from: flow.current().id.unwrap(),
                    }),
                },
            ));
        flow.cursor.index = 1;
        let flow = store.start_task_flow(&task.id, flow).await.unwrap();
        let held = claim(&store, &task, &flow, 303).await;
        let input = reserved_capture(&store, &task).await;
        store
            .sqlite
            .publish_attempt(
                flow.id(),
                flow.version,
                store.sqlite.captured_sequence(&input).unwrap().unwrap(),
                Some(&held),
                "proof",
                None,
            )
            .unwrap();
        let actor = store.sqlite.test_flow_turn(&input);
        store
            .sqlite
            .test_decision_output(
                &actor,
                &crate::engine::transitions::FlowVerdict {
                    decision: crate::engine::transitions::FlowDecision::Iterate,
                    summary: "review the revision".into(),
                },
            )
            .unwrap();
        store.sqlite.test_finish_flow_turn(&actor, "completed");
        let saved = store.flow(flow.id()).await.unwrap().unwrap();
        let mut cursor = saved.cursor.clone();
        cursor.finish(&saved.invocation.steps).unwrap();
        let child = store
            .checkpoint_flow(saved.id(), saved.version, &cursor, Some(&held), None)
            .await
            .unwrap();
        assert!(child.is_human());
        assert!(child.claim.is_none());
        assert_eq!(
            store.task_flow(&task.id).await.unwrap().unwrap().id(),
            child.id()
        );
        let conn = rusqlite::Connection::open(db).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT current_invocation_id FROM tasks WHERE id=?1",
                [task.id.as_str()],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            flow.id()
        );
        super::park_at_review(&store, &task, &child).await.unwrap();
        ready_review(&store, &task, "revision accepted").await;
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        store
            .complete_task_review(&task.id, &expected, "revision accepted")
            .await
            .unwrap();
        let continued = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(continued.id(), child.id());
        assert_eq!(continued.current().step, "decide");
        assert_eq!(
            continued.cursor.leaf().progress.direction.as_deref(),
            Some("revision accepted")
        );
        assert!(store
            .complete_task_review(&task.id, &expected, "late completion")
            .await
            .is_err());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // serializes the fixture executable and Home
    async fn concurrent_review_completions_settle_once() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let token = park_human_task(&store, &task, &flow).await;
        ready_review(&store, &task, "review feedback").await;
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        let (first, second) = tokio::join!(
            super::complete_human_flow_step(&store, &token, &expected),
            super::complete_human_flow_step(&store, &token, &expected),
        );
        assert_ne!(first.is_ok(), second.is_ok());
        assert!(!crate::ops::human_session::token_is_current(&store, &token)
            .await
            .unwrap());
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
