mod support;

use std::process::{Command, Output};

use loopflow::child::ChildRef;
use loopflow::durable::{
    AdvanceReceipt, AttentionRoute, Containment, FlowPosition, LaunchRoute, RunAdvance, RunTrigger,
};
use loopflow::engine::InteractionPolicy;
use loopflow_test_support::TestRepo;
use support::{register_task, RegisteredTask};

#[derive(Debug, PartialEq, Eq)]
struct LifecycleConfig {
    kickoff_flow: String,
    kickoff_policy: InteractionPolicy,
    iterate_flow: String,
    iterate_policy: InteractionPolicy,
    gate_flow: String,
    gate_policy: InteractionPolicy,
}

fn lifecycle_config(task: &RegisteredTask) -> LifecycleConfig {
    let runtime = tokio::runtime::Runtime::new().expect("task test runtime");
    let session = runtime
        .block_on(task.store.get_task(&task.session.id))
        .expect("read Task")
        .expect("Task");
    LifecycleConfig {
        kickoff_flow: session.lifecycle.kickoff.flow,
        kickoff_policy: session.lifecycle.kickoff.interaction_policy,
        iterate_flow: session.lifecycle.iterate.flow,
        iterate_policy: session.lifecycle.iterate.interaction_policy,
        gate_flow: session.lifecycle.gate.flow,
        gate_policy: session.lifecycle.gate.interaction_policy,
    }
}

fn run_headless(repo: &TestRepo, home: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["task", "run", "INF-123", "--headless"])
        .current_dir(repo.path())
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_WAVE_ID")
        .output()
        .expect("run lf task run --headless")
}

fn seed_current_user_feedback(task: &RegisteredTask) {
    let runtime = tokio::runtime::Runtime::new().expect("task test runtime");
    runtime.block_on(async {
        let work = task
            .store
            .work_for_child(&ChildRef::Task(task.session.id.clone()))
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
                    containment: Containment::Tmux {
                        name: "task-headless-feedback".to_string(),
                    },
                    cwd: task.session.worktree.clone(),
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
                    flow: task.session.lifecycle.kickoff.flow.clone(),
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
fn task_run_headless_existing_task_persists_all_policies() {
    let repo = TestRepo::new();
    let branch = "jack/task-headless-persistence";
    repo.create_branch(branch);
    let home = tempfile::tempdir().expect("Task home");
    let task = register_task(home.path(), repo.path(), branch, &repo.head_sha());

    let output = run_headless(&repo, home.path());
    assert!(
        output.status.success(),
        "lf task run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let persisted = lifecycle_config(&task);
    assert_eq!(persisted.kickoff_policy, InteractionPolicy::Defer);
    assert_eq!(persisted.iterate_policy, InteractionPolicy::Defer);
    assert_eq!(persisted.gate_policy, InteractionPolicy::Defer);
}

#[test]
fn task_run_headless_existing_task_refuses_current_user_feedback() {
    let repo = TestRepo::new();
    let branch = "jack/task-headless-human-feedback";
    repo.create_branch(branch);
    let home = tempfile::tempdir().expect("Task home");
    let task = register_task(home.path(), repo.path(), branch, &repo.head_sha());
    seed_current_user_feedback(&task);
    let before = lifecycle_config(&task);

    let output = run_headless(&repo, home.path());
    assert!(
        !output.status.success(),
        "current User Feedback must refuse"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("current Feedback"), "{stderr}");
    assert_eq!(lifecycle_config(&task), before);
}
