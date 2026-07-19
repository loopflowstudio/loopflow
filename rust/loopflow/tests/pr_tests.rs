mod support;

use std::process::Command;

use loopflow::durable::WorkStatus;
use loopflow::ops::task::{pr_next, task_complete, task_snapshot, task_status};
use loopflow::ops::{
    create_or_update_pr, current_pr, present_pr_review, NullProgress, OpsError, PrOptions,
};
use loopflow::task::{AfterMerge, GithubPr, PrPhase, PrPublication};
use loopflow_test_support::TestRepo;
use support::{counting_open_script, presentation_attempts, register_task, EnvGuard};

fn write_gh_script(pr_list: &str, pr_diff: Option<&str>) -> String {
    let diff = pr_diff.unwrap_or("");
    format!(
        "#!/bin/sh\ncase \"$1 $2\" in\n  'pr list')\n    cat <<'JSON'\n{pr_list}\nJSON\n    exit 0;;\n  'pr diff') echo '{diff}'; exit 0;;\n  'pr create') echo 'https://example.com/pr/1'; exit 0;;\n  'pr edit') exit 0;;\n  'pr ready') exit 0;;\n  'pr view') echo 'OPEN'; exit 0;;\nesac\nexit 0\n"
    )
}

fn noop_script() -> &'static str {
    "#!/bin/sh\nexit 0\n"
}

fn agent_script() -> &'static str {
    "#!/bin/sh\necho '{\"title\":\"generated title\",\"body\":\"generated body\"}'\nexit 0\n"
}

fn mutating_agent_script() -> &'static str {
    "#!/bin/sh\nprintf 'provider mutation\\n' > provider.txt\ngit add provider.txt\ngit commit -m 'provider mutation' >/dev/null\necho '{\"title\":\"generated title\",\"body\":\"generated body\"}'\nexit 0\n"
}

fn codex_script(output: &str) -> String {
    format!("#!/bin/sh\ncat <<'EOF'\n{output}\nEOF\nexit 0\n")
}

fn write_gh_script_reject_base(expected_reject: &str) -> String {
    format!(
        "#!/bin/sh\ncase \"$1 $2\" in\n  'pr list')\n    echo '[]'; exit 0;;\n  'pr diff') exit 1;;\n  'pr create')\n    base=\"\"\n    while [ \"$#\" -gt 0 ]; do\n      if [ \"$1\" = \"--base\" ]; then\n        shift\n        base=\"$1\"\n      fi\n      shift\n    done\n    if [ \"$base\" = \"{expected_reject}\" ]; then\n      echo \"base branch matches head\" >&2\n      exit 1\n    fi\n    echo 'https://example.com/pr/1'\n    exit 0;;\n  'pr edit') exit 0;;\n  'pr ready') exit 0;;\n  'pr view') echo 'OPEN'; exit 0;;\nesac\nexit 0\n"
    )
}

fn gh_create_failure_script() -> &'static str {
    r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi
if [ "$1 $2" = "pr list" ]; then
  echo '[]'
  exit 0
fi
if [ "$1 $2" = "pr create" ]; then
  echo 'GitHub is unavailable' >&2
  exit 1
fi
exit 0
"#
}

fn gh_merged_pr_script() -> &'static str {
    r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi
if [ "$1" = "api" ]; then
  head=$(git rev-parse HEAD)
  printf '{"merged":true,"state":"closed","draft":false,"merge_commit_sha":"merge-912","number":912,"html_url":"https://example.com/pr/912","head":{"sha":"%s"}}\n' "$head"
  exit 0
fi
exit 0
"#
}

fn push_branch(repo: &TestRepo, name: &str) {
    let _ = Command::new("git")
        .args(["push", "-u", "origin", name])
        .current_dir(repo.path())
        .status();
}

fn create_changed_branch(repo: &TestRepo, name: &str) {
    repo.create_branch(name);
    repo.create_file("feature.txt", name);
    repo.stage_all();
    repo.commit("feature work");
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

#[test]
fn task_snapshot_reads_its_current_parent_project() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    let mut project = runtime
        .block_on(task.store.get_project(&task.task.project_id))
        .expect("read parent Project")
        .expect("parent Project exists");
    project.definition.slug = "current-project".to_string();
    project.definition.pm_snapshot_synced_at += 1;
    runtime
        .block_on(task.store.update_project(&project))
        .expect("update parent Project");

    let snapshot = task_snapshot(&task.task).expect("snapshot Task");

    assert_eq!(snapshot.project, "current-project");
    assert_eq!(snapshot.external_project_id, project.definition.id.as_str());
    assert_eq!(
        snapshot.pm_snapshot_synced_at,
        task.task.directive.pm_snapshot_synced_at
    );
}

#[test]
fn pr_create_calls_gh() {
    let gh_script = write_gh_script("[]", None);
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_home(
        &[
            ("gh", gh_script.as_str()),
            ("open", noop_script()),
            ("codex", agent_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("test title".to_string()),
            body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("pr");

    assert!(result.created);
    assert_eq!(result.url, "https://example.com/pr/1");
}

#[test]
fn publish_makes_no_presentation_attempt() {
    let gh_script = write_gh_script("[]", None);
    let marker_dir = tempfile::TempDir::new().expect("marker dir");
    let marker = marker_dir.path().join("present.log");
    let open_script = counting_open_script(&marker);
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_home(
        &[
            ("gh", gh_script.as_str()),
            ("open", open_script.as_str()),
            ("xdg-open", open_script.as_str()),
            ("codex", agent_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("test title".to_string()),
            body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("pr");

    assert!(result.created);
    assert_eq!(
        presentation_attempts(&marker),
        0,
        "publication must not open any review surface"
    );
}

#[test]
fn publication_refuses_if_copy_generation_changes_the_pushed_head() {
    let gh_script = write_gh_script("[]", None);
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("codex", mutating_agent_script()),
    ]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");
    let pushed_head = repo.head_sha();

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: None,
            body: None,
            agent: None,
        },
        &NullProgress,
    );

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("changed the published branch/HEAD")),
        "a generated message cannot invalidate the pushed head: {result:?}"
    );
    let remote_head = Command::new("git")
        .arg("--git-dir")
        .arg(repo.bare_path())
        .args(["rev-parse", "refs/heads/feature"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .expect("read remote feature");
    assert_eq!(remote_head, pushed_head);
}

#[test]
fn present_pr_review_opens_the_pr_once() {
    let marker_dir = tempfile::TempDir::new().expect("marker dir");
    let marker = marker_dir.path().join("present.log");
    let open_script = counting_open_script(&marker);
    let _env = EnvGuard::new(&[
        ("open", open_script.as_str()),
        ("xdg-open", open_script.as_str()),
    ]);

    present_pr_review("https://example.com/pr/1").expect("present");

    assert_eq!(
        presentation_attempts(&marker),
        1,
        "pr open must present exactly once once a PR URL exists"
    );
    let log = std::fs::read_to_string(&marker).expect("marker");
    assert!(
        log.contains("https://example.com/pr/1"),
        "the presented URL is the published PR URL: {log}"
    );
}

#[test]
fn github_failure_leaves_publication_intent_observable() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(
        &[("gh", gh_create_failure_script()), ("open", noop_script())],
        home.path(),
    );
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "publication intent\n");
    repo.stage_all();
    repo.commit("add publication proof");
    repo.push_new_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &base);

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("Persist publication first".to_string()),
            body: Some("The GitHub call will fail.".to_string()),
            agent: None,
        },
        &NullProgress,
    );
    assert!(result.is_err());

    let runtime = tokio::runtime::Runtime::new().expect("read task runtime");
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert_eq!(pr.phase(), PrPhase::Publishing);
    let publication = pr.publication.expect("durable publication request");
    assert_eq!(publication.after_merge, AfterMerge::ContinueTask);
    assert!(publication.github.is_none());
}

#[test]
fn merged_continue_task_rotates_to_a_working_pr_without_review_state() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[("gh", gh_merged_pr_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    point_origin_at_github(&repo);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: time::OffsetDateTime::now_utc(),
        after_merge: AfterMerge::ContinueTask,
        next_slug: None,
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: None,
        }),
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("mark PR as published");

    let persisted_task = task_status("INF-123").expect("reconcile Task PR");
    let snapshot = loopflow::ops::task::task_snapshot(&persisted_task).expect("snapshot Task");
    assert!(!matches!(snapshot.status, WorkStatus::Done));
    assert!(
        matches!(
            persisted_task.observation,
            loopflow::task::Observation::Fresh { .. }
        ),
        "manual merge reconciliation should use the bounded REST observation: {persisted_task:?}"
    );

    let prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read Task PRs");
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].phase(), PrPhase::Merged);
    let publication = prs[0].publication.as_ref().expect("adopted publication");
    assert_eq!(publication.after_merge, AfterMerge::ContinueTask);
    assert_eq!(publication.github.as_ref().map(|pr| pr.number), Some(912));
    let work = runtime
        .block_on(
            task.store
                .work_for_child(&loopflow::child::ChildRef::Task(task.task.id.clone())),
        )
        .expect("resolve reconciled Task Work");
    assert!(!matches!(
        runtime.block_on(task.store.work_status(&work)).unwrap(),
        WorkStatus::Done
    ));

    let restore = Command::new("git")
        .current_dir(repo.path())
        .args([
            "remote",
            "set-url",
            "origin",
            repo.bare_path().to_str().expect("bare origin path"),
        ])
        .status()
        .expect("restore local origin");
    assert!(restore.success());

    let next = pr_next(repo.path(), None).expect("rotate merged continuation");
    assert_eq!(next.sequence, 2);
    assert_eq!(next.phase(), PrPhase::Working);
    let prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read rotated PR chain");
    assert_eq!(prs.len(), 2);
    assert_eq!(prs[0].phase(), PrPhase::Merged);
    assert_eq!(prs[1].id, next.id);
    assert_eq!(prs[1].sequence, next.sequence);
    assert_eq!(prs[1].branch, next.branch);
    assert_eq!(prs[1].phase(), PrPhase::Working);

    let current_branch = Command::new("git")
        .current_dir(repo.path())
        .args(["branch", "--show-current"])
        .output()
        .expect("read rotated branch");
    assert!(current_branch.status.success());
    assert_eq!(
        String::from_utf8_lossy(&current_branch.stdout).trim(),
        next.branch
    );

    let durable_task = runtime
        .block_on(task.store.get_task(&task.task.id))
        .expect("read rotated Task")
        .expect("Task remains present");
    assert!(durable_task.gate_proposal.is_none());
    assert!(runtime
        .block_on(task.store.feedback(&work))
        .expect("read Task Feedback")
        .is_none());
    assert!(!matches!(
        runtime.block_on(task.store.work_status(&work)).unwrap(),
        WorkStatus::Done
    ));
}

#[test]
fn observed_merge_completes_a_pr_marked_to_complete_the_task() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[("gh", gh_merged_pr_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    point_origin_at_github(&repo);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: time::OffsetDateTime::now_utc(),
        after_merge: AfterMerge::CompleteTask,
        next_slug: None,
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: None,
        }),
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("mark PR as completing");

    let persisted_task = task_status("INF-123").expect("reconcile completing PR");
    assert!(
        matches!(
            persisted_task.observation,
            loopflow::task::Observation::Fresh { .. }
        ),
        "completion should use the bounded REST observation: {persisted_task:?}"
    );
    let snapshot = loopflow::ops::task::task_snapshot(&persisted_task).expect("snapshot Task");
    assert!(matches!(snapshot.status, WorkStatus::Done));
    let prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read completing PR");
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].phase(), PrPhase::Merged);
}

#[test]
fn task_complete_refuses_while_a_working_pr_is_unsettled() {
    // W2-151: a Task must not be completed in the PM while it still owns an
    // unsettled PR. Previously `lf task complete` would delete an unpublished
    // working PR and complete; now the completion gate refuses it so the PR
    // cannot be published later into a Task the PM already calls done.
    let home = tempfile::TempDir::new().expect("temp home");
    let gh_script = write_gh_script("[]", None);
    let _env = EnvGuard::with_lf_home(&[("gh", gh_script.as_str())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &base);

    let result = task_complete("INF-123", "done".to_string());
    let message = result
        .expect_err("an unpublished working PR must block completion")
        .to_string();
    assert!(
        message.contains("cannot complete") && message.contains("unpublished"),
        "expected a gate refusal naming the unpublished PR, got: {message}"
    );

    // The Task and PR are unchanged: no premature completion, no deleted PR.
    let runtime = tokio::runtime::Runtime::new().expect("read runtime");
    let work = runtime
        .block_on(
            task.store
                .work_for_child(&loopflow::child::ChildRef::Task(task.task.id.clone())),
        )
        .expect("resolve Task Work");
    assert!(!matches!(
        runtime.block_on(task.store.work_status(&work)).unwrap(),
        WorkStatus::Done
    ));
    let prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read PRs");
    assert_eq!(prs.len(), 1, "working PR must survive the refusal");
}

#[test]
fn canonical_checkout_refuses_pr_before_committing_or_pushing() {
    let repo = TestRepo::new();

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("must not ship".to_string()),
            body: None,
            agent: None,
        },
        &NullProgress,
    );

    assert!(matches!(
        result,
        Err(OpsError::Message(message))
            if message.contains("canonical checkout")
                && message.contains("lf task run")
    ));
}

#[test]
fn empty_non_task_range_refuses_before_copy_or_github_mutation() {
    let markers = tempfile::TempDir::new().expect("markers");
    let agent_marker = markers.path().join("agent");
    let github_marker = markers.path().join("github");
    let codex = format!(
        "#!/bin/sh\nprintf called > '{}'\nexit 1\n",
        agent_marker.display()
    );
    let gh = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then exit 0; fi\ncase \"$1 $2\" in\n  'pr create'|'pr edit'|'pr ready') printf called > '{}';;\nesac\nif [ \"$1 $2\" = \"pr list\" ]; then echo '[]'; fi\nexit 0\n",
        github_marker.display()
    );
    let _env = EnvGuard::new(&[("gh", gh.as_str()), ("codex", codex.as_str())]);
    let repo = TestRepo::new();
    repo.create_branch("already-landed");
    push_branch(&repo, "already-landed");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: None,
            body: None,
            agent: Some("codex".to_string()),
        },
        &NullProgress,
    );

    assert!(matches!(
        result,
        Err(OpsError::Message(message))
            if message.contains("no changes")
                && message.contains("before PR copy generation or GitHub mutation")
    ));
    assert!(!agent_marker.exists(), "PR-copy agent must not launch");
    assert!(!github_marker.exists(), "GitHub must not mutate");
}

#[test]
fn pr_update_refreshes_body() {
    let gh_script = write_gh_script(
        r#"[{"url":"https://example.com/pr/1","state":"OPEN","isDraft":false,"number":1}]"#,
        Some("diff"),
    );
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_home(
        &[
            ("gh", gh_script.as_str()),
            ("open", noop_script()),
            ("codex", agent_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("updated title".to_string()),
            body: Some("updated body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("pr");

    assert!(!result.created);
}

#[test]
fn pr_create_uses_default_base_when_upstream_matches_head() {
    let gh_script = write_gh_script_reject_base("feature");
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_home(
        &[
            ("gh", gh_script.as_str()),
            ("open", noop_script()),
            ("codex", agent_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("test title".to_string()),
            body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("pr");

    assert!(result.created);
    assert_eq!(result.url, "https://example.com/pr/1");
}

#[test]
fn current_pr_surfaces_gh_list_errors() {
    let _env = EnvGuard::new(&[(
        "gh",
        "#!/bin/sh\nif [ \"$1\" = \"pr\" ] && [ \"$2\" = \"list\" ]; then\n  echo \"gh pr list failed\" >&2\n  exit 1\nfi\nexit 0\n",
    )]);
    let repo = TestRepo::new();

    let result = current_pr(repo.path());
    assert!(matches!(
        result,
        Err(OpsError::CommandFailed { stderr, .. }) if stderr.contains("gh pr list failed")
    ));
}

#[test]
fn pr_auto_generates_title_when_missing() {
    let gh_script = write_gh_script("[]", None);
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_home(
        &[
            ("gh", gh_script.as_str()),
            ("open", noop_script()),
            ("codex", agent_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: None,
            body: Some("some body".to_string()),
            agent: None,
        },
        &NullProgress,
    );

    let Ok(result) = result else {
        panic!("expected auto-generated title to succeed");
    };
    assert!(result.created);
}

#[test]
fn pr_auto_generates_title_from_labeled_codex_output() {
    let gh_script = write_gh_script("[]", None);
    let codex_output = r#"Title: generated title
Body:
## Usage

- generated body"#;
    let codex = codex_script(codex_output);
    let home = tempfile::TempDir::new().expect("temp home");
    std::fs::create_dir_all(home.path().join(".lf")).expect("config dir");
    std::fs::write(home.path().join(".lf/config.yaml"), "agent: codex\n").expect("config");
    let _env = EnvGuard::with_home(
        &[
            ("gh", gh_script.as_str()),
            ("open", noop_script()),
            ("codex", codex.as_str()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: None,
            body: None,
            agent: None,
        },
        &NullProgress,
    );

    let Ok(result) = result else {
        panic!("expected labeled codex output to succeed");
    };
    assert!(result.created);
}
