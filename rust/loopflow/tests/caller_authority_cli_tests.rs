mod support;

use std::process::{Command, Output};

use loopflow::child_session::{CallerAuthority, ChildCommandSource, ChildRef};
use loopflow::id::WaveId;
use loopflow::task::{
    AfterMerge, GithubObservation, GithubObservationResult, GithubPr, PrPublication,
};
use loopflow::wave::Wave;
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard, RegisteredTask};
use time::OffsetDateTime;

const MANAGED_MARKERS: [&str; 4] = [
    "LF_WAVE_ID",
    "LF_PROJECT_SESSION_ID",
    "LF_TASK_SESSION_ID",
    "LF_CHANNEL",
];

fn open_pr(task: &mut RegisteredTask) {
    let now = OffsetDateTime::now_utc();
    task.pr.publication = Some(PrPublication {
        requested_at: now,
        after_merge: AfterMerge::Review,
        next_slug: None,
        github: Some(GithubPr {
            number: 1079,
            url: "https://github.com/loopflowstudio/loopflow/pull/1079".to_string(),
            head_sha: Some("a".repeat(40)),
        }),
    });
    task.pr.github_observation = Some(GithubObservation {
        checked_at: now,
        result: GithubObservationResult::Fresh,
    });
    task.pr.updated_at = now;
    tokio::runtime::Runtime::new()
        .expect("test runtime")
        .block_on(task.store.update_task_pr(&task.pr))
        .expect("open test PR");
}

fn resume(repo: &TestRepo, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command.current_dir(repo.path());
    for marker in MANAGED_MARKERS {
        command.env_remove(marker);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    command.args(args).output().expect("run lf task resume")
}

fn command_count(task: &RegisteredTask) -> usize {
    tokio::runtime::Runtime::new()
        .expect("test runtime")
        .block_on(
            task.store
                .list_child_commands(&ChildRef::Task(task.session.id.clone())),
        )
        .expect("read Task commands")
        .len()
}

#[test]
fn task_resume_threads_wave_project_and_operator_authority_from_the_cli() {
    let home = tempfile::tempdir().expect("test home");
    let repo = TestRepo::new();
    let base = repo.head_sha();
    repo.create_branch("jack/caller-authority-proof");
    let mut task = register_task(
        home.path(),
        repo.path(),
        "jack/caller-authority-proof",
        &base,
    );
    open_pr(&mut task);
    let foreign = Wave::new(
        WaveId::new(),
        "foreign".to_string(),
        repo.path().display().to_string(),
    );
    tokio::runtime::Runtime::new()
        .expect("test runtime")
        .block_on(task.store.create_wave(&foreign))
        .expect("register foreign Wave");
    let _env = EnvGuard::with_lf_home(&[("tmux", "#!/bin/sh\nexit 0\n")], home.path());
    let initial_count = command_count(&task);

    let wave = resume(
        &repo,
        &["task", "resume", "INF-123", "--json"],
        &[
            ("LF_WAVE_ID", task.session.wave_id.as_str()),
            ("LF_CHANNEL", "task-pr-tests"),
        ],
    );
    assert!(!wave.status.success(), "Wave resume must be barred");
    let wave_error = String::from_utf8_lossy(&wave.stderr);
    assert!(
        wave_error.contains("A supervisor cannot restart a submitted Task"),
        "Wave invocation must reach the typed supervisor bar: {wave_error}"
    );
    assert_eq!(command_count(&task), initial_count);

    let project = resume(
        &repo,
        &["task", "resume", "INF-123", "--json"],
        &[
            (
                "LF_PROJECT_SESSION_ID",
                task.session.project_session_id.as_str(),
            ),
            ("LF_WAVE_ID", task.session.wave_id.as_str()),
            ("LF_CHANNEL", "task-pr-tests"),
        ],
    );
    assert!(!project.status.success(), "Project resume must be barred");
    let project_error = String::from_utf8_lossy(&project.stderr);
    assert!(
        project_error.contains("A supervisor cannot restart a submitted Task"),
        "Project invocation must reach the same typed supervisor bar: {project_error}"
    );
    assert_eq!(command_count(&task), initial_count);

    let explicit = resume(
        &repo,
        &[
            "--wave",
            foreign.name(),
            "task",
            "resume",
            "INF-123",
            "--json",
        ],
        &[
            (
                "LF_PROJECT_SESSION_ID",
                task.session.project_session_id.as_str(),
            ),
            ("LF_WAVE_ID", task.session.wave_id.as_str()),
            ("LF_CHANNEL", "task-pr-tests"),
        ],
    );
    assert!(
        !explicit.status.success(),
        "foreign explicit Wave must fail"
    );
    let explicit_error = String::from_utf8_lossy(&explicit.stderr);
    assert!(
        explicit_error.contains("Wave foreign cannot control Task INF-123"),
        "explicit --wave must override inherited Project context before ops: {explicit_error}"
    );
    assert_eq!(command_count(&task), initial_count);

    let operator = resume(&repo, &["task", "resume", "INF-123", "--json"], &[]);
    assert!(
        operator.status.success(),
        "clean operator must be allowed to answer review: {}",
        String::from_utf8_lossy(&operator.stderr)
    );
    let commands = tokio::runtime::Runtime::new()
        .expect("test runtime")
        .block_on(
            task.store
                .list_child_commands(&ChildRef::Task(task.session.id.clone())),
        )
        .expect("read operator Resume");
    assert_eq!(commands.len(), initial_count + 1);
    assert_eq!(commands.last().unwrap().source, ChildCommandSource::Human);
    assert_eq!(
        CallerAuthority::Operator.into_source(),
        commands.last().unwrap().source
    );
    let persisted = tokio::runtime::Runtime::new()
        .expect("test runtime")
        .block_on(task.store.get_task_session(&task.session.id))
        .expect("read resumed Task")
        .expect("Task still exists");
    assert_eq!(
        persisted
            .latest_process
            .as_ref()
            .map(|body| body.generation),
        Some(1),
        "operator Resume must reserve the review generation"
    );
}
