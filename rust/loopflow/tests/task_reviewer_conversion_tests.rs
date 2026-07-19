mod support;

use std::process::{Command, Output};

use loopflow::child::ChildRef;
use loopflow::durable::{
    AdvanceReceipt, AttentionRoute, Containment, FlowPosition, LaunchRoute, RunAdvance, RunTrigger,
};
use loopflow::task::FeedbackReviewer;
use loopflow_test_support::TestRepo;
use support::{register_task, RegisteredTask};

#[derive(Debug, PartialEq, Eq)]
struct LifecycleConfig {
    first_flow: String,
    first_reviewer: FeedbackReviewer,
    loop_flow: String,
    loop_reviewer: FeedbackReviewer,
    finally_flow: String,
    finally_reviewer: FeedbackReviewer,
}

fn lifecycle_config(task: &RegisteredTask) -> LifecycleConfig {
    let runtime = tokio::runtime::Runtime::new().expect("task test runtime");
    let persisted_task = runtime
        .block_on(task.store.get_task(&task.task.id))
        .expect("read Task")
        .expect("Task");
    LifecycleConfig {
        first_flow: persisted_task.lifecycle.first.flow,
        first_reviewer: persisted_task.lifecycle.first.reviewer,
        loop_flow: persisted_task.lifecycle.loop_.flow,
        loop_reviewer: persisted_task.lifecycle.loop_.reviewer,
        finally_flow: persisted_task.lifecycle.finally.flow,
        finally_reviewer: persisted_task.lifecycle.finally.reviewer,
    }
}

fn run_with_reviewer(repo: &TestRepo, home: &std::path::Path, reviewer: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["task", "run", "INF-123", "--reviewer", reviewer])
        .current_dir(repo.path())
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_WAVE_ID")
        .output()
        .expect("run lf task run --reviewer")
}

fn seed_current_user_feedback(task: &RegisteredTask) {
    let runtime = tokio::runtime::Runtime::new().expect("task test runtime");
    runtime.block_on(async {
        let work = task
            .store
            .work_for_child(&ChildRef::Task(task.task.id.clone()))
            .await
            .expect("resolve Task Work");
        let boundary = task.store.boundary_seed(&work).await.expect("read Basis");
        let (_run, lease) = task
            .store
            .reserve_run(&work, RunTrigger::User)
            .await
            .expect("reserve Run");
        let launch = match task
            .store
            .advance_run(
                &lease,
                RunAdvance::LaunchStarting {
                    route: LaunchRoute {
                        provider: "opaque".to_string(),
                        model: None,
                        account_id: None,
                    },
                    containment: Containment::ProcessGroup {
                        // SAFETY: `getpgrp` has no preconditions and does not mutate memory.
                        id: i64::from(unsafe { libc::getpgrp() }),
                    },
                    cwd: task.task.worktree.clone(),
                    surface: "terminal".to_string(),
                    opaque: true,
                    resume_token: None,
                },
            )
            .await
            .expect("start Launch")
        {
            AdvanceReceipt::Launch(launch) => launch,
            receipt => panic!("expected Launch, got {receipt:?}"),
        };
        task.store
            .advance_run(
                &lease,
                RunAdvance::LaunchLive {
                    launch_id: launch.id.clone(),
                },
            )
            .await
            .expect("mark Launch live");
        task.store
            .set_flow_position(
                &lease,
                FlowPosition {
                    work,
                    epoch_id: boundary.basis.epoch_id,
                    flow: task.task.lifecycle.first.flow.clone(),
                    step: "review-design".to_string(),
                    step_index: 0,
                    iteration: 0,
                    feedback: true,
                    updated_at: time::OffsetDateTime::now_utc(),
                },
            )
            .await
            .expect("record flow position");
        task.store
            .route_feedback(&lease, &launch.id, AttentionRoute::User)
            .await
            .expect("route User attention");
    });
}

#[test]
fn task_run_explicit_parent_persists_all_reviewers() {
    let repo = TestRepo::new();
    let branch = "jack/task-parent-reviewer";
    repo.create_branch(branch);
    let home = tempfile::tempdir().expect("Task home");
    let task = register_task(home.path(), repo.path(), branch, &repo.head_sha());

    let output = run_with_reviewer(&repo, home.path(), "parent");
    assert!(
        output.status.success(),
        "lf task run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let persisted = lifecycle_config(&task);
    assert_eq!(persisted.first_reviewer, FeedbackReviewer::Parent);
    assert_eq!(persisted.loop_reviewer, FeedbackReviewer::Parent);
    assert_eq!(persisted.finally_reviewer, FeedbackReviewer::Parent);
}

#[test]
fn task_run_explicit_reviewer_changes_only_future_feedback() {
    let repo = TestRepo::new();
    let branch = "jack/task-current-user-feedback";
    repo.create_branch(branch);
    let home = tempfile::tempdir().expect("Task home");
    let task = register_task(home.path(), repo.path(), branch, &repo.head_sha());
    seed_current_user_feedback(&task);
    let output = run_with_reviewer(&repo, home.path(), "parent");
    assert!(
        output.status.success(),
        "lf task run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let persisted = lifecycle_config(&task);
    assert_eq!(persisted.first_reviewer, FeedbackReviewer::Parent);
    assert_eq!(persisted.loop_reviewer, FeedbackReviewer::Parent);
    assert_eq!(persisted.finally_reviewer, FeedbackReviewer::Parent);

    let runtime = tokio::runtime::Runtime::new().expect("task test runtime");
    let attention = runtime.block_on(async {
        let work = task
            .store
            .work_for_child(&ChildRef::Task(task.task.id.clone()))
            .await
            .expect("resolve Task Work");
        task.store
            .feedback(&work)
            .await
            .expect("read Feedback")
            .expect("open Feedback")
            .attention
    });
    assert_eq!(attention, AttentionRoute::User);
}
