mod support;

use std::fs;
use std::process::Command;

use loopflow::durable::{WorkRef, WorkStatus};
use loopflow::ops::task::{task_interrupt, task_status};
use loopflow::work::task::{GithubPr, Observation, PrPublication};
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard, RegisteredTask};
use time::{Duration, OffsetDateTime};

fn publish_task(task: &mut RegisteredTask) {
    task.pr.publication = Some(PrPublication {
        requested_at: OffsetDateTime::now_utc(),
        presentation: None,
        github: Some(GithubPr {
            number: 928,
            url: "https://github.com/loopflowstudio/loopflow/pull/928".to_string(),
            head_sha: None,
        }),
        merge: None,
    });
    task.pr.updated_at = OffsetDateTime::now_utc() - Duration::hours(1);
    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    runtime
        .block_on(task.store.update_task_pr(&task.pr))
        .expect("publish test PR");
}

fn tmux_script() -> &'static str {
    "#!/bin/sh\nexit 0\n"
}

fn point_origin_at_github(repo: &TestRepo) {
    let status = Command::new("git")
        .current_dir(repo.path())
        .args([
            "remote",
            "set-url",
            "origin",
            "https://github.com/loopflowstudio/loopflow.git",
        ])
        .status()
        .expect("set GitHub origin");
    assert!(status.success());
}

fn gh_success_script(log: &str) -> String {
    format!(
        r#"#!/bin/sh
echo "$*" >> "{log}"
if [ "$1" = "--version" ]; then
  exit 0
fi
if [ "$1 $2" = "pr list" ]; then
  echo "GraphQL: API rate limit already exceeded" >&2
  exit 1
fi
if [ "$1" = "api" ]; then
  cat <<'JSON'
{{"merged":false,"state":"open","draft":false,"merge_commit_sha":null,"number":928,"html_url":"https://github.com/loopflowstudio/loopflow/pull/928","head":{{"sha":"head-928"}}}}
JSON
  exit 0
fi
if [ "$1 $2" = "pr checks" ]; then
  echo '[]'
  exit 0
fi
echo "unexpected gh invocation: $*" >&2
exit 1
"#
    )
}

fn gh_failure_script(log: &str) -> String {
    format!(
        r#"#!/bin/sh
echo "$*" >> "{log}"
if [ "$1" = "--version" ]; then
  exit 0
fi
if [ "$1 $2" = "pr list" ]; then
  echo "GraphQL: API rate limit already exceeded" >&2
  exit 1
fi
if [ "$1" = "api" ]; then
  echo "gh: Internal Server Error (HTTP 500)" >&2
  exit 1
fi
echo "unexpected gh invocation: $*" >&2
exit 1
"#
    )
}

fn github_reads(log: &str) -> Vec<String> {
    fs::read_to_string(log)
        .expect("read gh log")
        .lines()
        .filter(|line| line.starts_with("api "))
        .map(str::to_string)
        .collect()
}

#[test]
fn graph_ql_exhaustion_never_blocks_task_control_or_forces_pr_enumeration() {
    let home = tempfile::tempdir().expect("task home");
    let repo = TestRepo::new();
    repo.create_branch("jack/task-pr-proof");
    point_origin_at_github(&repo);
    let base = repo.head_sha();
    let mut task = register_task(home.path(), repo.path(), "jack/task-pr-proof", &base);
    publish_task(&mut task);
    let log = home.path().join("gh.log");
    let script = gh_success_script(log.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("tmux", tmux_script())],
        home.path(),
    );

    let first = task_status("INF-123").expect("REST status succeeds despite GraphQL exhaustion");
    assert!(matches!(&first.observation, Observation::Fresh { .. }));
    let status = loopflow::ops::task::task_snapshot(&first).expect("snapshot Task");
    assert_eq!(status.status, WorkStatus::Ready, "{first:?}");

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let work = WorkRef::Task(task.task.id.clone());
    let previous_interrupt = runtime
        .block_on(task.store.latest_interrupt_id(&work))
        .unwrap();
    let interrupt = task_interrupt("INF-123").expect("interrupt remains local and durable");
    assert!(matches!(interrupt.observation, Observation::NotRequired));
    assert!(
        runtime
            .block_on(task.store.latest_interrupt_id(&work))
            .unwrap()
            > previous_interrupt
    );
    assert!(runtime
        .block_on(task.store.flow_position(&task.task.id))
        .unwrap()
        .is_none());
    let cached = task_status("INF-123").expect("cached status succeeds");
    assert!(matches!(cached.observation, Observation::Cached { .. }));

    let log_text = fs::read_to_string(&log).expect("read gh log");
    assert!(!log_text.lines().any(|line| line.starts_with("pr list")));
    let reads = github_reads(log.to_string_lossy().as_ref());
    assert_eq!(reads.len(), 1, "rapid controls share one bounded REST read");
    assert!(reads[0].contains("repos/loopflowstudio/loopflow/pulls/928"));
}

#[test]
fn rest_failure_opens_one_durable_circuit_while_local_controls_continue() {
    let home = tempfile::tempdir().expect("task home");
    let repo = TestRepo::new();
    repo.create_branch("jack/task-pr-proof");
    point_origin_at_github(&repo);
    let base = repo.head_sha();
    let mut task = register_task(home.path(), repo.path(), "jack/task-pr-proof", &base);
    publish_task(&mut task);
    let cached_as_of = task.pr.updated_at;
    let log = home.path().join("gh.log");
    let script = gh_failure_script(log.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("tmux", tmux_script())],
        home.path(),
    );

    let first = task_status("INF-123").expect("REST failure degrades instead of failing");
    let status = loopflow::ops::task::task_snapshot(&first).expect("snapshot Task");
    assert_eq!(status.status, WorkStatus::Ready, "{first:?}");
    let (reason, first_retry_at) = match first.observation {
        Observation::Degraded {
            reason,
            cached_as_of: observed_cache,
            retry_at,
        } => {
            assert_eq!(
                observed_cache.unix_timestamp(),
                cached_as_of.unix_timestamp()
            );
            (reason, retry_at)
        }
        other => panic!("expected degraded observation, got {other:?}"),
    };
    assert!(reason.contains("Internal Server Error"));

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let work = WorkRef::Task(task.task.id.clone());
    let previous_interrupt = runtime
        .block_on(task.store.latest_interrupt_id(&work))
        .unwrap();
    let interrupt = task_interrupt("INF-123").expect("interrupt survives REST failure");
    assert!(matches!(interrupt.observation, Observation::NotRequired));
    assert!(
        runtime
            .block_on(task.store.latest_interrupt_id(&work))
            .unwrap()
            > previous_interrupt
    );
    let cached = task_status("INF-123").expect("cached degraded status succeeds");
    match &cached.observation {
        Observation::Degraded {
            reason, retry_at, ..
        } => {
            assert!(reason.contains("Internal Server Error"));
            assert_eq!(*retry_at, first_retry_at);
        }
        other => panic!("expected cached degradation, got {other:?}"),
    }
    let status =
        loopflow::ops::task::task_snapshot(&cached).expect("snapshot Task after interrupt");
    assert_eq!(status.status, WorkStatus::Ready);
    assert_eq!(
        github_reads(log.to_string_lossy().as_ref()).len(),
        1,
        "the degraded circuit suppresses repeat REST reads"
    );
}
