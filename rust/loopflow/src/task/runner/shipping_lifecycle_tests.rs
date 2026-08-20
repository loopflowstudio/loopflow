use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use time::OffsetDateTime;

use super::{arm_ci_fix_wake, prepare_task_flow_step, resume_task_phase, settle_ci_fix_turn};
use crate::chat::types::Lifecycle;
use crate::child::ChildRef;
use crate::durable::{
    BoundaryState, Containment, InvocationRoute, RunAdvance, RunTrigger, WorkStatus,
};
use crate::engine::agent::{
    build_claude_session_turn_args, build_codex_command, AgentWriteScope, ProcessConfig,
};
use crate::id::WaveId;
use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
use crate::project::{Project, ProjectId};
use crate::store::{open_store, SharedStore, StorageConfig};
use crate::task::{
    AfterMerge, CiCheck, CiObservation, CiState, GithubPr, Observation, PmWritebackState,
    PrMergeMode, PrMergeRequest, PrPresentation, PrPublication, Task, TaskEventKind,
    TaskGateProposal, TaskLifecyclePhase, TaskLifecyclePlan, TaskPr, TaskPrId,
};
use crate::wave::Wave;

struct GitFixture {
    _root: tempfile::TempDir,
    main: PathBuf,
    worktree: PathBuf,
    base: String,
    upstream: String,
    failed_head: String,
}

impl GitFixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let main = root.path().join("repo");
        let remote = root.path().join("origin.git");
        let worktree = root.path().join("repo.task");
        fs::create_dir(&main).unwrap();
        git(&main, &["init", "-b", "main"]);
        git(&main, &["config", "user.email", "test@example.com"]);
        git(&main, &["config", "user.name", "Loopflow Test"]);
        fs::write(main.join("base.txt"), "base\n").unwrap();
        git(&main, &["add", "."]);
        git(&main, &["commit", "-m", "base"]);
        let base = git(&main, &["rev-parse", "HEAD"]);

        let output = Command::new("git")
            .args(["clone", "--bare"])
            .arg(&main)
            .arg(&remote)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git clone --bare failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        git(
            &main,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        git(&main, &["push", "-u", "origin", "main"]);
        git(
            &main,
            &[
                "worktree",
                "add",
                "-b",
                "task/shipping-proof",
                worktree.to_str().unwrap(),
            ],
        );
        git(&worktree, &["config", "user.email", "test@example.com"]);
        git(&worktree, &["config", "user.name", "Loopflow Test"]);
        fs::write(worktree.join("task.txt"), "task\n").unwrap();
        git(&worktree, &["add", "."]);
        git(&worktree, &["commit", "-m", "task change"]);

        fs::write(main.join("upstream.txt"), "upstream\n").unwrap();
        git(&main, &["add", "."]);
        git(&main, &["commit", "-m", "upstream change"]);
        git(&main, &["push", "origin", "main"]);
        let upstream = git(&main, &["rev-parse", "HEAD"]);

        git(&worktree, &["fetch", "origin", "main"]);
        git(&worktree, &["rebase", "origin/main"]);
        let failed_head = git(&worktree, &["rev-parse", "HEAD"]);

        Self {
            _root: root,
            main,
            worktree,
            base,
            upstream,
            failed_head,
        }
    }

    fn commit_repair(&self) -> String {
        fs::write(self.worktree.join("repair.txt"), "repair\n").unwrap();
        git(&self.worktree, &["add", "."]);
        git(&self.worktree, &["commit", "-m", "repair CI"]);
        git(&self.worktree, &["rev-parse", "HEAD"])
    }
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

async fn registry(
    fixture: &GitFixture,
) -> (SharedStore, tempfile::TempDir, Wave, Project, Task, TaskPr) {
    let database = tempfile::tempdir().unwrap();
    let store = Arc::new(
        open_store(&StorageConfig::sqlite(database.path().join("registry.db")))
            .await
            .unwrap(),
    );
    let now = OffsetDateTime::now_utc();
    let wave = Wave::new(
        WaveId::new(),
        "shipping-proof".to_string(),
        fixture.main.display().to_string(),
    );
    let project = Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            id: LinearProjectId::new("shipping-proof-project").unwrap(),
            slug: "shipping-proof".to_string(),
            name: "Shipping proof".to_string(),
            prompt_context: "A Task ships without manual repair.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        wave_id: wave.id().clone(),
        iteration: 0,
        observation_cursor: 0,
        last_state_fingerprint: None,
        agent: "codex".to_string(),
        provider: "codex".to_string(),
        provider_session_id: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    let task = Task {
        id: crate::task::TaskId::new(),
        plan: TaskPlan {
            id: LinearIssueId::new("shipping-proof-issue").unwrap(),
            identifier: "TEST-55".to_string(),
            title: "Ship without repair".to_string(),
            description: "Exercise the complete authority chain.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        pm_writeback: PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_id: project.id.clone(),
        worktree: fixture.worktree.clone(),
        workspace_slug: "shipping-proof".to_string(),
        lifecycle: TaskLifecyclePlan::new(
            crate::task::TaskOutcome::Delivery,
            "task-kickoff",
            "slice",
            "ship",
        ),
        lifecycle_phase: TaskLifecyclePhase::First,
        phase_epoch: 1,
        phase_cursor: 0,
        phase_iteration: 0,
        gate_cycle: 0,
        gate_proposal: None,
        agent: "codex".to_string(),
        provider: "codex".to_string(),
        provider_session_id: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
        observation: Observation::NotRequired,
    };
    let mut pr = TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        sequence: 1,
        slug: task.workspace_slug.clone(),
        branch: "task/shipping-proof".to_string(),
        base_commit: fixture.base.clone(),
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
    pr.publication = Some(PrPublication {
        requested_at: now,
        presentation: Some(PrPresentation {
            title: "Ship without repair".to_string(),
            body: "Proves autonomous Task settlement.".to_string(),
            head_sha: fixture.failed_head.clone(),
        }),
        github: Some(GithubPr {
            number: 55,
            url: "https://github.com/loopflowstudio/loopflow/pull/55".to_string(),
            head_sha: Some(fixture.failed_head.clone()),
        }),
        merge: None,
    });
    pr.ci_observation = Some(CiObservation {
        head_sha: fixture.failed_head.clone(),
        state: CiState::Failing,
        failing_checks: vec![CiCheck {
            name: "task-shipping".to_string(),
            url: Some("https://example.com/check/55".to_string()),
        }],
        observed_at: now,
    });
    store.update_task_pr(&pr).await.unwrap();
    (store, database, wave, project, task, pr)
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
async fn task_ships_on_one_authority_chain_without_manual_repair() {
    let _lf_bin = super::TestLfBinGuard::pin();
    let fixture = GitFixture::new();
    let (store, _database, _wave, project, mut task, _pr) = registry(&fixture).await;
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .unwrap();
    let (_run, lease) = store.reserve_run(&work, RunTrigger::User).await.unwrap();
    store
        .advance_run(
            &lease,
            RunAdvance::RunStarting {
                containment: Containment::Tmux {
                    name: "shipping-proof".to_string(),
                },
                cwd: fixture.worktree.clone(),
            },
        )
        .await
        .unwrap();
    store
        .advance_run(
            &lease,
            RunAdvance::InvocationStarting {
                route: InvocationRoute {
                    provider: "codex".to_string(),
                    model: None,
                    account_id: None,
                },
                surface: "headless".to_string(),
                resume_token: None,
                answer_ask_id: None,
            },
        )
        .await
        .unwrap();

    let mut flow = resume_task_phase(&task).unwrap();
    let prepared =
        prepare_task_flow_step(&store, &mut task, &lease, "shipping-proof", &mut flow, None)
            .await
            .unwrap()
            .expect("shipping proof starts on an autonomous step");
    assert_eq!(prepared.turn.config.write_scope, AgentWriteScope::Worktree);
    assert!(prepared.turn.config.skip_permissions);
    let codex = build_codex_command(
        &prepared.turn.config,
        &ProcessConfig {
            auto: true,
            ..Default::default()
        },
        None,
    );
    assert!(codex
        .iter()
        .any(|arg| arg == "--dangerously-bypass-approvals-and-sandbox"));
    let boundary = prepared
        .turn
        .config
        .execution_boundary
        .as_ref()
        .expect("Task delivery boundary");
    assert!(boundary
        .writable_roots
        .contains(&fixture.main.join(".git").canonicalize().unwrap()));
    let claude = build_claude_session_turn_args("ship", &prepared.turn.config, None);
    assert!(claude
        .iter()
        .any(|arg| arg == "--dangerously-skip-permissions"));
    crate::ops::task::verify_task_pr_range_with_authority(
        &store,
        &task,
        Some(&lease),
        &fixture.worktree,
    )
    .await
    .unwrap();
    let pr = store.active_task_pr(&task.id).await.unwrap().unwrap();
    assert_eq!(pr.base_commit, fixture.upstream);
    assert_eq!(
        git(&fixture.worktree, &["merge-base", "origin/main", "HEAD"]),
        fixture.upstream,
        "origin/main advancement is the authoritative base, not contamination"
    );

    let incident = crate::ops::task::current_ci_incident(&pr).unwrap();
    store.observe_ci_incident(&incident).await.unwrap();
    let wake = arm_ci_fix_wake(&store, &task, &lease)
        .await
        .unwrap()
        .expect("failed authoritative head wakes ci-fix");
    assert_eq!(wake.head_sha, fixture.failed_head);
    assert_eq!(wake.failing_checks[0].name, "task-shipping");

    let project_work = store
        .work_for_child(&ChildRef::Project(project.id.clone()))
        .await
        .unwrap();
    let (_project_run, project_lease) = store
        .reserve_run(&project_work, RunTrigger::User)
        .await
        .unwrap();
    assert!(
        !store
            .claim_ci_incident(
                &wake.incident_identity,
                &project_lease.run_id,
                OffsetDateTime::now_utc(),
            )
            .await
            .unwrap(),
        "a second live Run cannot steal the bounded repair"
    );
    store
        .finish_project_run(&project, &project_lease, BoundaryState::Succeeded)
        .await
        .unwrap();

    let repaired_head = fixture.commit_repair();
    let mut repaired_pr = pr.clone();
    repaired_pr
        .publication
        .as_mut()
        .unwrap()
        .github
        .as_mut()
        .unwrap()
        .head_sha = Some(repaired_head.clone());
    repaired_pr.ci_observation = Some(CiObservation {
        head_sha: repaired_head.clone(),
        state: CiState::Pending,
        failing_checks: Vec::new(),
        observed_at: OffsetDateTime::now_utc(),
    });
    repaired_pr.updated_at = OffsetDateTime::now_utc();
    store
        .update_task_pr_for_run(&repaired_pr, &lease)
        .await
        .unwrap();
    settle_ci_fix_turn(
        &store,
        &mut task,
        &lease,
        &wake,
        Some(&repaired_pr),
        Some(&wake.head_sha),
        Lifecycle::Completed,
        None,
    )
    .await
    .unwrap();
    assert!(store.current_run(&work).await.unwrap().is_none());

    let reports = store
        .ci_incidents_since(
            OffsetDateTime::now_utc() - time::Duration::hours(1),
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].incident.claimed_run_id.as_ref(),
        Some(&lease.run_id)
    );
    assert_eq!(
        reports[0].incident.repaired_head_sha.as_deref(),
        Some(repaired_head.as_str())
    );

    let now = OffsetDateTime::now_utc();
    repaired_pr.ci_observation = Some(CiObservation {
        head_sha: repaired_head.clone(),
        state: CiState::Passing,
        failing_checks: Vec::new(),
        observed_at: now,
    });
    repaired_pr
        .publication
        .as_mut()
        .unwrap()
        .presentation
        .as_mut()
        .unwrap()
        .head_sha = repaired_head.clone();
    repaired_pr.publication.as_mut().unwrap().merge = Some(PrMergeRequest {
        mode: PrMergeMode::Auto,
        requested_at: now,
        head_sha: repaired_head.clone(),
        after_merge: AfterMerge::CompleteTask,
        next_slug: None,
    });
    repaired_pr.updated_at = now;
    store.update_task_pr(&repaired_pr).await.unwrap();
    store
        .mark_ci_incidents_green(&repaired_pr.id, now)
        .await
        .unwrap();

    let mut merged_pr = repaired_pr;
    merged_pr.merge_commit = Some(repaired_head);
    merged_pr.ci_observation = None;
    merged_pr.updated_at = now;
    store
        .mark_ci_incidents_merged(&merged_pr.id, now)
        .await
        .unwrap();
    task.enter_loop().unwrap();
    task.enter_finally(TaskGateProposal {
        done: true,
        reason: "pull request #55 merged and completed the Task".to_string(),
    })
    .unwrap();
    store
        .complete_task_after_pr(&task, &merged_pr)
        .await
        .unwrap();

    assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);
    assert!(store.current_run(&work).await.unwrap().is_none());
    let reports = store
        .ci_incidents_since(
            OffsetDateTime::now_utc() - time::Duration::hours(1),
            None,
            None,
        )
        .await
        .unwrap();
    assert!(reports[0].incident.green_at.is_some());
    assert!(reports[0].incident.merged_at.is_some());
    let events = store.recent_task_events(&task.id, 20).await.unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, TaskEventKind::Completed { .. }))
            .count(),
        1
    );
}
