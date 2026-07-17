mod support;

use std::process::{Command, Output};

use loopflow::engine::InteractionPolicy;
use loopflow::interaction_review::InteractionReviewId;
use loopflow_test_support::TestRepo;
use rusqlite::{params, Connection};
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
        .block_on(task.store.get_task_session(&task.session.id))
        .expect("read Task Session")
        .expect("Task Session");
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
        .env_remove("LF_TASK_SESSION_ID")
        .env_remove("LF_TASK_GENERATION")
        .env_remove("LF_TASK_LEASE_TOKEN")
        .env_remove("LF_PROJECT_SESSION_ID")
        .env_remove("LF_WAVE_ID")
        .output()
        .expect("run lf task run --headless")
}

fn seed_current_human_review(home: &std::path::Path, task: &RegisteredTask) -> InteractionReviewId {
    let connection = Connection::open(home.join("loopflow.db")).expect("open Task store");
    connection
        .execute(
            "UPDATE task_sessions SET lifecycle_phase='kickoff' WHERE id=?1",
            [task.session.id.as_str()],
        )
        .expect("move Task to kickoff waitpoint");

    let review_id = InteractionReviewId::new();
    connection
        .execute(
            "INSERT INTO interaction_reviews (
                id, wave_id, project_session_id, task_session_id,
                phase, phase_epoch, flow, step, step_index, phase_iteration, policy,
                reviewer_kind, reviewer_id, status, reason, prompt,
                worktree, branch, base_commit, head_commit, worktree_fingerprint,
                pr_number, pr_url, requested_by_generation, reviewer_generation,
                disposition, outcome, requested_at, completed_at
             ) VALUES (
                ?1, ?2, ?3, ?4,
                'kickoff', ?5, ?6, 'review-design', ?7, ?8, 'require',
                'human', NULL, 'requested', 'Review the Task design', 'Review the Task design',
                ?9, ?10, ?11, ?11, 'test-fingerprint',
                NULL, NULL, 1, NULL,
                NULL, NULL, ?12, NULL
             )",
            params![
                review_id.as_str(),
                task.session.wave_id.as_str(),
                task.session.project_session_id.as_str(),
                task.session.id.as_str(),
                i64::from(task.session.phase_epoch),
                task.session.lifecycle.kickoff.flow,
                i64::from(task.session.phase_cursor),
                i64::from(task.session.phase_iteration),
                task.session.worktree.display().to_string(),
                task.pr.branch,
                task.pr.base_commit,
                time::OffsetDateTime::now_utc().unix_timestamp(),
            ],
        )
        .expect("seed current Human review");
    review_id
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
fn task_run_headless_existing_task_refuses_current_human_review() {
    let repo = TestRepo::new();
    let branch = "jack/task-headless-human-review";
    repo.create_branch(branch);
    let home = tempfile::tempdir().expect("Task home");
    let task = register_task(home.path(), repo.path(), branch, &repo.head_sha());
    let review_id = seed_current_human_review(home.path(), &task);
    let before = lifecycle_config(&task);

    let output = run_headless(&repo, home.path());
    assert!(!output.status.success(), "current Human review must refuse");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(review_id.as_str()), "{stderr}");
    assert_eq!(lifecycle_config(&task), before);
}
