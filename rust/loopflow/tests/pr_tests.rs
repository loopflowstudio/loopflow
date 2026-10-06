mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use loopflow::durable::{FlowSession, WorkStatus};
use loopflow::engine::flow::Command as FlowCommand;
use loopflow::engine::invocation::QueuedInvocation;
use loopflow::ops::task::{pr_next, task_complete, task_snapshot, task_status};
use loopflow::ops::{
    arm as land, commit_workflow, create_or_update_pr, current_pr, execute_flow_command,
    present_pr_review, CommitOptions, LandOptions, NullProgress, OpsError, PrOptions,
};
use loopflow::work::task::{
    AfterMerge, GithubPr, PrMergeMode, PrMergeRequest, PrPhase, PrPresentation, PrPublication,
};
use loopflow_test_support::TestRepo;
use support::{
    codex_app_server_script, counting_open_script, presentation_attempts, register_task,
    register_unrun_task, EnvGuard,
};

fn write_gh_script(pr_list: &str, pr_diff: Option<&str>) -> String {
    let diff = pr_diff.unwrap_or("");
    format!(
        "#!/bin/sh\ncase \"$1 $2\" in\n  'pr list')\n    cat <<'JSON'\n{pr_list}\nJSON\n    exit 0;;\n  'pr diff') echo '{diff}'; exit 0;;\n  'pr create') echo 'https://example.com/pr/1'; exit 0;;\n  'pr edit') exit 0;;\n  'pr ready') exit 0;;\n  'pr view') echo 'OPEN'; exit 0;;\nesac\nexit 0\n"
    )
}

fn draft_pr_script(state: &std::path::Path) -> String {
    r#"#!/bin/sh
state='@STATE@'
case "$1 $2" in
  'pr list')
    if [ ! -f "$state" ]; then echo '[]'; exit 0; fi
    draft=false
    if [ "$(cat "$state")" = draft ]; then draft=true; fi
    printf '[{"url":"https://example.com/pr/1","number":1,"state":"OPEN","isDraft":%s,"headRefOid":"%s"}]\n' "$draft" "$(git rev-parse HEAD)"
    ;;
  'pr create')
    readiness=open
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --draft) readiness=draft ;;
        --body) shift; printf '%s' "$1" > "$state.body" ;;
      esac
      shift
    done
    printf '%s' "$readiness" > "$state"
    echo 'https://example.com/pr/1'
    ;;
  'pr edit')
    while [ "$#" -gt 0 ]; do
      if [ "$1" = --body ]; then shift; printf '%s' "$1" > "$state.body"; fi
      shift
    done
    ;;
  'pr ready')
    if [ -f "$state.fail-ready" ]; then echo 'promotion failed' >&2; exit 1; fi
    printf open > "$state"
    ;;
esac
exit 0
"#
    .replace("@STATE@", state.to_str().unwrap())
}

#[test]
fn task_delivery_works_on_an_ordinary_branch_without_registration() {
    let home = tempfile::tempdir().unwrap();
    let state = home.path().join("pr-state");
    let gh = draft_pr_script(&state);
    let marker = home.path().join("present.log");
    let open = counting_open_script(&marker);
    let _env = EnvGuard::with_lf_home(
        &[("gh", &gh), ("open", &open), ("xdg-open", &open)],
        home.path(),
    );
    let repo = TestRepo::new();
    repo.create_branch("ordinary");
    repo.create_file("feature.txt", "local work");
    let before = repo.head_sha();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .env_clear()
            .env("HOME", home.path())
            .env("LF_HOME", home.path())
            .env("PATH", std::env::var_os("PATH").unwrap())
            .args(args)
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    assert_eq!(
        String::from_utf8(run(&["pr"])).unwrap().trim(),
        "No open PR for the current branch."
    );
    run(&["commit", "-m", "Record local work"]);
    let committed = repo.head_sha();
    assert_ne!(before, committed);
    assert!(!state.exists(), "commit published a PR");
    let remote = Command::new("git")
        .args(["ls-remote", "origin", "refs/heads/ordinary"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(remote.status.success());
    assert!(remote.stdout.is_empty(), "commit pushed the branch");
    run(&["sync", "--plan"]);
    assert_eq!(repo.head_sha(), committed);
    let worktrees: serde_json::Value =
        serde_json::from_slice(&run(&["wt", "list", "--json"])).unwrap();
    assert_eq!(worktrees.as_array().unwrap().len(), 1);
    for (verb, expected) in [("open", "draft"), ("publish", "open")] {
        run(&[
            "pr",
            verb,
            "--title",
            "Ordinary branch",
            "--body",
            "No Task required.",
        ]);
        assert_eq!(fs::read_to_string(&state).unwrap(), expected);
        let status = String::from_utf8(run(&["pr"])).unwrap();
        assert!(status.contains("#1") && status.contains("https://example.com/pr/1"));
    }
    let remote = Command::new("git")
        .args(["ls-remote", "origin", "refs/heads/ordinary"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(remote.status.success());
    assert!(String::from_utf8_lossy(&remote.stdout).starts_with(&repo.head_sha()));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let store = loopflow::store::open_ephemeral_store(&loopflow::store::StorageConfig::sqlite(
            home.path().join("loopflow.db"),
        ))
        .await
        .unwrap();
        assert!(store.list_tasks(None).await.unwrap().is_empty());
    });
}

#[test]
fn draft_open_stays_draft_until_publish_in_cli_and_flow() {
    for headless in [false, true] {
        let home = tempfile::TempDir::new().unwrap();
        let state = home.path().join("pr-state");
        let gh = draft_pr_script(&state);
        let marker = home.path().join("present.log");
        let open = counting_open_script(&marker);
        let _env = EnvGuard::with_lf_home(
            &[("gh", &gh), ("open", &open), ("xdg-open", &open)],
            home.path(),
        );
        let repo = TestRepo::new();
        let base = repo.head_sha();
        create_changed_branch(&repo, "feature");
        let task = register_task(home.path(), repo.path(), "feature", &base);
        let runtime = tokio::runtime::Runtime::new().unwrap();

        for (command, expected) in [
            ("open", "draft"),
            ("open", "draft"),
            ("publish", "open"),
            ("open", "open"),
        ] {
            let args = [
                command,
                "--title",
                "Review the work",
                "--body",
                "Current work.",
            ];
            if headless {
                execute_flow_command(
                    repo.path(),
                    &FlowCommand {
                        command: "pr".to_string(),
                        args: args.iter().map(|arg| (*arg).to_string()).collect(),
                    },
                    &NullProgress,
                )
                .unwrap();
            } else {
                let output = Command::new(env!("CARGO_BIN_EXE_lf"))
                    .args(["pr"])
                    .args(args)
                    .current_dir(repo.path())
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            assert_eq!(current_pr(repo.path()).unwrap().unwrap().state, expected);
            let pr = runtime
                .block_on(task.store.active_task_pr(&task.task.id))
                .unwrap()
                .unwrap();
            let publication = pr.publication.unwrap();
            assert_eq!(publication.github.unwrap().number, 1);
            let body = fs::read_to_string(state.with_extension("body")).unwrap();
            assert!(body.contains(if expected == "draft" {
                "is a draft"
            } else {
                "published for review"
            }));
            assert_eq!(publication.presentation.unwrap().body, body);
        }
        assert_eq!(presentation_attempts(&marker), if headless { 0 } else { 3 });
    }
}

#[test]
fn failed_draft_promotion_stays_draft_and_can_retry() {
    let home = tempfile::TempDir::new().unwrap();
    let state = home.path().join("pr-state");
    let failure = state.with_extension("fail-ready");
    let gh = draft_pr_script(&state);
    let _env = EnvGuard::with_lf_home(&[("gh", &gh)], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    create_changed_branch(&repo, "feature");
    let task = register_task(home.path(), repo.path(), "feature", &base);
    let mut options = PrOptions {
        title: Some("Ready work".to_string()),
        body: Some("Current work.".to_string()),
        agent: None,
        draft: true,
    };
    create_or_update_pr(repo.path(), &options, &NullProgress).unwrap();
    let draft_body = fs::read_to_string(state.with_extension("body")).unwrap();
    assert!(draft_body.contains("is a draft"));

    options.draft = false;
    fs::write(&failure, "fail").unwrap();
    assert!(create_or_update_pr(repo.path(), &options, &NullProgress).is_err());
    assert_eq!(current_pr(repo.path()).unwrap().unwrap().state, "draft");
    assert_eq!(
        fs::read_to_string(state.with_extension("body")).unwrap(),
        draft_body
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let retained = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .unwrap()
        .unwrap();
    assert_eq!(
        retained.publication.unwrap().presentation.unwrap().body,
        draft_body
    );

    fs::remove_file(failure).unwrap();
    create_or_update_pr(repo.path(), &options, &NullProgress).unwrap();
    assert_eq!(current_pr(repo.path()).unwrap().unwrap().state, "open");
    let ready_body = fs::read_to_string(state.with_extension("body")).unwrap();
    assert!(ready_body.contains("published for review"));
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .unwrap()
        .unwrap();
    assert_eq!(
        pr.publication.unwrap().presentation.unwrap().body,
        ready_body
    );
}

fn noop_script() -> &'static str {
    "#!/bin/sh\nexit 0\n"
}

#[test]
fn existing_draft_without_local_publication_retains_identity_when_promotion_fails() {
    let home = tempfile::TempDir::new().unwrap();
    let state = home.path().join("pr-state");
    let failure = state.with_extension("fail-ready");
    let gh = draft_pr_script(&state);
    let _env = EnvGuard::with_lf_home(&[("gh", &gh)], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    create_changed_branch(&repo, "feature");
    let task = register_task(home.path(), repo.path(), "feature", &base);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let options = PrOptions {
        title: Some("Adopt existing work".into()),
        body: Some("Current work.".into()),
        agent: None,
        draft: false,
    };
    fs::write(&state, "draft").unwrap();
    fs::write(&failure, "fail").unwrap();

    let error = create_or_update_pr(repo.path(), &options, &NullProgress).unwrap_err();
    assert!(error.to_string().contains("promotion failed"), "{error}");
    let retained = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .unwrap()
        .unwrap()
        .publication
        .unwrap();
    assert_eq!(retained.github.unwrap().number, 1);
    assert!(retained.presentation.is_none());
    assert!(retained.merge.is_none());
    assert_eq!(current_pr(repo.path()).unwrap().unwrap().state, "draft");

    fs::remove_file(failure).unwrap();
    create_or_update_pr(repo.path(), &options, &NullProgress).unwrap();
    assert_eq!(current_pr(repo.path()).unwrap().unwrap().state, "open");
    let published = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .unwrap()
        .unwrap()
        .publication
        .unwrap();
    assert_eq!(published.github.unwrap().number, 1);
    assert!(published
        .presentation
        .unwrap()
        .body
        .contains("published for review"));
    assert!(published.merge.is_none());
}

fn reviewer_copy(head_sha: &str) -> PrPresentation {
    PrPresentation {
        title: "Ship it".to_string(),
        body: "Reviewer context".to_string(),
        head_sha: head_sha.to_string(),
    }
}

fn agent_script() -> String {
    codex_app_server_script(r#"{"title":"generated title","body":"generated body"}"#, "")
}

fn mutating_agent_script() -> String {
    codex_app_server_script(
        r#"{"title":"generated title","body":"generated body"}"#,
        "printf 'provider mutation\\n' > provider.txt\ngit add provider.txt\ngit commit -m 'provider mutation' >/dev/null",
    )
}

fn codex_script(output: &str) -> String {
    codex_app_server_script(output, "")
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
  printf '{"merged":true,"state":"closed","draft":false,"merge_commit_sha":"merge-912","merged_at":"2026-07-21T19:00:00Z","number":912,"html_url":"https://example.com/pr/912","head":{"sha":"%s"}}\n' "$head"
  exit 0
fi
exit 0
"#
}

fn gh_merged_pr_without_time_script(log_path: &str) -> String {
    r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "__LOG_PATH__"
if [ "$1" = "api" ]; then
  head=$(git rev-parse HEAD)
  printf '{"merged":true,"state":"closed","draft":false,"merge_commit_sha":"merge-912","number":912,"html_url":"https://example.com/pr/912","head":{"sha":"%s"}}\n' "$head"
  exit 0
fi
if [ "$1 $2" = "pr list" ]; then
  echo '[]'
  exit 0
fi
if [ "$1 $2" = "pr create" ]; then
  echo 'https://example.com/pr/1'
  exit 0
fi
exit 0
"#
    .replace("__LOG_PATH__", log_path)
}

fn gh_merged_pr_logging_script(log_path: &str) -> String {
    format!(
        r#"#!/bin/sh
echo "$@" >> "{log_path}"
if [ "$1" = "--version" ]; then
  exit 0
fi
if [ "$1" = "api" ]; then
  head=$(git rev-parse HEAD)
  printf '{{"merged":true,"state":"closed","draft":false,"merge_commit_sha":"merge-912","merged_at":"2026-07-21T19:00:00Z","number":912,"html_url":"https://example.com/pr/912","head":{{"sha":"%s"}}}}\n' "$head"
  exit 0
fi
if [ "$1 $2" = "pr list" ]; then
  echo '[]'
  exit 0
fi
exit 0
"#
    )
}

fn gh_changed_head_script(log_path: &str) -> String {
    let armed = support::github_merge_response(912, "fixture-head", "OPEN", "CLEAN", Some("auto"));
    format!(
        r#"#!/bin/sh
echo "$@" >> "{log_path}"
if [ "$1" = "--version" ]; then
  exit 0
fi
if [ "$1 $2" = "api graphql" ]; then
  echo '{armed}'
  exit 0
fi
if [ "$1" = "api" ]; then
  echo '{{"merged":false,"state":"open","draft":false,"merge_commit_sha":null,"number":912,"html_url":"https://example.com/pr/912","head":{{"sha":"new-head"}}}}'
  exit 0
fi
if [ "$1 $2 $3 $4" = "pr merge 912 --disable-auto" ]; then
  exit 0
fi
exit 0
"#
    )
}

fn gh_open_auto_script(log_path: &str) -> String {
    let armed = support::github_merge_response(912, "fixture-head", "OPEN", "CLEAN", Some("auto"));
    format!(
        r#"#!/bin/sh
echo "$@" >> "{log_path}"
if [ "$1" = "--version" ]; then
  exit 0
fi
if [ "$1 $2" = "api graphql" ]; then
  echo '{armed}'
  exit 0
fi
if [ "$1" = "api" ]; then
  head="$(git rev-parse HEAD)"
  printf '{{"merged":false,"state":"open","draft":false,"merge_commit_sha":null,"number":912,"html_url":"https://example.com/pr/912","head":{{"sha":"%s"}}}}\n' "$head"
  exit 0
fi
if [ "$1 $2 $3 $4" = "pr merge 912 --disable-auto" ]; then
  exit 0
fi
exit 0
"#
    )
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
    project.plan.slug = "current-project".to_string();
    project.plan.pm_snapshot_synced_at += 1;
    runtime
        .block_on(task.store.update_project(&project))
        .expect("update parent Project");

    let snapshot = task_snapshot(&task.task).expect("snapshot Task");

    assert_eq!(snapshot.project, "current-project");
    assert_eq!(snapshot.external_project_id, project.plan.id.as_str());
    assert_eq!(
        snapshot.pm_snapshot_synced_at,
        task.task.plan.pm_snapshot_synced_at
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
            ("codex", &agent_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
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
            ("codex", &agent_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
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
fn gate_artifacts_never_reach_the_published_head() {
    let gh_script = write_gh_script("[]", None);
    let marker_dir = tempfile::TempDir::new().expect("marker dir");
    let agent_marker = marker_dir.path().join("agent-called");
    let codex = format!(
        "#!/bin/sh\nprintf called > '{}'\nexit 1\n",
        agent_marker.display()
    );
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_home(
        &[("gh", gh_script.as_str()), ("codex", codex.as_str())],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    let scratch = repo.path().join("scratch");
    fs::create_dir_all(&scratch).expect("create scratch");
    fs::write(
        scratch.join("security-review.md"),
        "independent audit evidence",
    )
    .expect("write independent review");
    repo.stage_all();
    repo.commit("preserve independent audit evidence");
    push_branch(&repo, "feature");
    let implementation_head = repo.head_sha();

    fs::write(scratch.join(".pr-copy-ref"), &implementation_head).expect("write copy ref");
    fs::write(scratch.join("pr-title.txt"), "cached gate title").expect("write title");
    fs::write(scratch.join("pr-body.md"), "cached gate body").expect("write body");

    create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
            title: None,
            body: None,
            agent: Some("codex".to_string()),
        },
        &NullProgress,
    )
    .expect("publish cached gate output");

    assert_eq!(
        repo.head_sha(),
        implementation_head,
        "gate handoff must not manufacture an artifact-only prepare commit"
    );
    let remote_head = Command::new("git")
        .arg("--git-dir")
        .arg(repo.bare_path())
        .args(["rev-parse", "refs/heads/feature"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .expect("read remote feature");
    assert_eq!(remote_head, implementation_head);
    for artifact in [".pr-copy-ref", "pr-title.txt", "pr-body.md"] {
        assert!(
            !scratch.join(artifact).exists(),
            "gate artifact survived publication: {artifact}"
        );
    }
    assert_eq!(
        fs::read_to_string(scratch.join("security-review.md")).unwrap(),
        "independent audit evidence"
    );
    assert!(
        !agent_marker.exists(),
        "valid gate copy must be consumed without launching another provider"
    );
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo.path())
        .output()
        .expect("read worktree status");
    assert!(
        status.stdout.is_empty(),
        "publication must leave a clean tree"
    );
}

#[test]
fn publication_refuses_if_copy_generation_changes_the_pushed_head() {
    let gh_script = write_gh_script("[]", None);
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("codex", &mutating_agent_script()),
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
            draft: false,
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
            draft: false,
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
    let presentation = publication
        .presentation
        .as_ref()
        .expect("reviewer-facing Task copy");
    assert_eq!(presentation.title, "Persist publication first");
    assert!(presentation.body.contains(
        "> **Task:** [Prove Task PR transitions · INF-123](https://linear.app/loopflow/issue/INF-123/prove-task-pr-transitions)"
    ));
    assert!(!presentation.body.contains("Task cycle:"));
    assert!(presentation.body.contains(
        "> **PR lifecycle:** PR 1 is published for review; no Task settlement is requested."
    ));
    assert!(presentation
        .body
        .starts_with("The GitHub call will fail.\n\n<!--"));
    assert!(publication.github.is_none());
    assert!(publication.merge.is_none());
}

#[test]
fn configured_feature_generation_adds_task_intent_and_lifecycle_to_pr_copy() {
    let home = tempfile::TempDir::new().expect("temp home");
    let gh = write_gh_script("[]", None);
    let agent = codex_script(
        r###"{"title":"Understand what merging this PR will do","body":"Reviewers can see what work remains without reconstructing Task state.\n\n## What changes\n\nPublication adds durable Task context.\n\n## Evaluate\n\nSuggested check: inspect the Task link and merge consequence."}"###,
    );
    let _env = EnvGuard::with_lf_home(
        &[("gh", gh.as_str()), ("codex", agent.as_str())],
        home.path(),
    );
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-copy-proof";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "configured generation\n");
    repo.stage_all();
    repo.commit("add configured generation proof");
    repo.push_new_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &base);

    create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
            title: None,
            body: None,
            agent: Some("codex".to_string()),
        },
        &NullProgress,
    )
    .expect("generate and publish Task PR copy");

    let runtime = tokio::runtime::Runtime::new().expect("read task runtime");
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    let presentation = pr
        .publication
        .as_ref()
        .and_then(|publication| publication.presentation.as_ref())
        .expect("generated Task PR copy");
    assert_eq!(
        presentation.title,
        "Understand what merging this PR will do"
    );
    assert_eq!(
        presentation.body,
        "Reviewers can see what work remains without reconstructing Task state.\n\n\
<!-- loopflow:task-pr-context:start -->\n\
> [!NOTE]\n\
> **Task:** [Prove Task PR transitions · INF-123](https://linear.app/loopflow/issue/INF-123/prove-task-pr-transitions)\n\
> **PR lifecycle:** PR 1 is published for review; no Task settlement is requested.\n\
<!-- loopflow:task-pr-context:end -->\n\n\
## What changes\n\n\
Publication adds durable Task context.\n\n\
## Evaluate\n\n\
Suggested check: inspect the Task link and merge consequence."
    );
}

#[test]
fn task_pr_generation_does_not_require_a_controller() {
    let home = tempfile::TempDir::new().expect("temp home");
    let gh = write_gh_script("[]", None);
    let agent = codex_script(
        r###"{"title":"generated title","body":"## Evaluate\n\nRun the configured fix proof."}"###,
    );
    let _env = EnvGuard::with_lf_home(
        &[("gh", gh.as_str()), ("codex", agent.as_str())],
        home.path(),
    );
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-fix-copy-proof";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "configured fix generation\n");
    repo.stage_all();
    repo.commit("add configured fix generation proof");
    repo.push_new_branch(branch);
    let registered = register_task(home.path(), repo.path(), branch, &base);
    let runtime = tokio::runtime::Runtime::new().expect("update Task runtime");

    create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
            title: None,
            body: None,
            agent: Some("codex".to_string()),
        },
        &NullProgress,
    )
    .expect("generate and publish fix Task PR copy");

    let pr = runtime
        .block_on(registered.store.active_task_pr(&registered.task.id))
        .expect("read active PR")
        .expect("active PR");
    let presentation = pr
        .publication
        .as_ref()
        .and_then(|publication| publication.presentation.as_ref())
        .expect("generated fix Task PR copy");
    assert_eq!(presentation.title, "generated title");
    assert!(!presentation.body.contains("Task cycle:"));
    assert!(presentation.body.contains(
        "> **PR lifecycle:** PR 1 is published for review; no Task settlement is requested."
    ));
}

#[test]
fn task_pr_missing_cached_linear_url_refuses_before_remote_mutation() {
    let markers = tempfile::TempDir::new().expect("markers");
    let github_marker = markers.path().join("github");
    let gh = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then exit 0; fi\nprintf called > '{}'\nexit 1\n",
        github_marker.display()
    );
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[("gh", gh.as_str())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "identity preflight\n");
    repo.stage_all();
    repo.commit("add identity proof");
    let remote_head_before = repo.head_sha();
    push_branch(&repo, branch);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    let mut snapshot = runtime
        .block_on(task.store.pm_snapshot(&task.task.wave_id))
        .expect("read PM snapshot")
        .expect("PM snapshot");
    snapshot.snapshot.items[0].url = None;
    snapshot.snapshot.items[0].revision = Some("2026-09-30T00:00:00Z".into());
    runtime
        .block_on(task.store.put_pm_snapshot(snapshot))
        .expect("remove cached Task URL");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
            title: Some("Reviewer context".to_string()),
            body: Some("Proof".to_string()),
            agent: None,
        },
        &NullProgress,
    );

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("no valid provider URL") && message.contains("lf repo refresh")),
        "missing provider identity should be actionable: {result:?}"
    );
    assert!(!github_marker.exists(), "GitHub must not mutate");
    let remote_branch = Command::new("git")
        .arg("--git-dir")
        .arg(repo.bare_path())
        .args(["rev-parse", "--verify", &format!("refs/heads/{branch}")])
        .output()
        .expect("inspect remote branch");
    assert!(
        remote_branch.status.success(),
        "Task branch must remain remote"
    );
    assert_eq!(
        String::from_utf8_lossy(&remote_branch.stdout).trim(),
        remote_head_before,
        "identity refusal must not mutate the remote Task branch"
    );
}

#[test]
fn serial_task_pr_publication_restores_task_context() {
    let home = tempfile::TempDir::new().expect("temp home");
    let gh_log = home.path().join("gh.log");
    let gh = gh_merged_pr_without_time_script(gh_log.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(&[("gh", gh.as_str())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    push_branch(&repo, branch);
    point_origin_at_github(&repo);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: time::OffsetDateTime::now_utc(),
        presentation: None,
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: None,
        }),
        merge: None,
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("mark PR as published");

    let persisted_task = task_status(repo.path(), Some("INF-123"))
        .expect("reconcile Task PR")
        .execution
        .expect("execution");
    assert!(!matches!(persisted_task.status, WorkStatus::Done));
    assert!(
        matches!(
            persisted_task.observation,
            loopflow::work::task::Observation::Fresh { .. }
        ),
        "manual merge reconciliation should use the bounded REST observation: {persisted_task:?}"
    );
    let cached_task = task_status(repo.path(), Some("INF-123"))
        .expect("reuse partial merge-time observation")
        .execution
        .expect("execution");
    assert!(
        matches!(
            cached_task.observation,
            loopflow::work::task::Observation::NotRequired
        ),
        "partial timing evidence must not degrade Task correctness: {cached_task:?}"
    );

    let prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read Task PRs");
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].phase(), PrPhase::Merged);
    let publication = prs[0].publication.as_ref().expect("adopted publication");
    assert_eq!(prs[0].after_merge(), AfterMerge::ContinueTask);
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

    assert!(!matches!(
        runtime.block_on(task.store.work_status(&work)).unwrap(),
        WorkStatus::Done
    ));

    repo.create_file("serial-proof.txt", "second PR\n");
    repo.stage_all();
    repo.commit("add serial proof");
    create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
            title: Some("Second delivery slice".to_string()),
            body: Some("Serial reviewer context".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("publish serial Task PR");
    let serial = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read serial PR")
        .expect("active serial PR");
    assert_eq!(serial.sequence, 2);
    let presentation = serial
        .publication
        .as_ref()
        .and_then(|publication| publication.presentation.as_ref())
        .expect("serial reviewer copy");
    assert_eq!(presentation.title, "Second delivery slice");
    assert!(presentation.body.starts_with(
        "Serial reviewer context\n\n<!-- loopflow:task-pr-context:start -->\n> [!NOTE]\n> **Task:** [Prove Task PR transitions · INF-123](https://linear.app/loopflow/issue/INF-123/prove-task-pr-transitions)"
    ));
    assert!(!presentation.body.contains("Task cycle:"));
    assert!(presentation.body.contains(
        "> **PR lifecycle:** PR 2 is published for review; no Task settlement is requested."
    ));
    let gh_calls = fs::read_to_string(gh_log).expect("read GitHub calls");
    assert!(gh_calls.contains("--title Second delivery slice"));
    assert!(gh_calls.contains("--body Serial reviewer context"));
}

#[test]
fn completing_land_discards_an_empty_successor_without_a_controller() {
    let home = tempfile::TempDir::new().expect("temp home");
    let log_path = home.path().join("gh.log");
    let script = gh_merged_pr_logging_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(&[("gh", script.as_str())], home.path());
    let repo = TestRepo::new();
    fs::create_dir_all(repo.path().join("scratch")).expect("create scratch");
    fs::write(repo.path().join("scratch/.gitkeep"), "").expect("write gitkeep");
    repo.stage_all();
    repo.commit("track scratch");
    push_branch(&repo, "main");
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    push_branch(&repo, branch);
    point_origin_at_github(&repo);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: time::OffsetDateTime::now_utc(),
        presentation: None,
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: None,
        }),
        merge: None,
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("mark PR as published");
    task_status(repo.path(), Some("INF-123"))
        .expect("reconcile merged Task PR")
        .execution
        .expect("execution");

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
    let successor = pr_next(repo.path(), None).expect("rotate merged continuation");
    assert_eq!(successor.phase(), PrPhase::Working);

    let options = LandOptions {
        strict: false,
        local: false,
        create_pr: true,
        complete: true,
        next_slug: None,
        worktree: None,
        commit_message: None,
        pr_title: None,
        pr_body: None,
        agent: None,
    };
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
    fs::create_dir_all(repo.path().join("scratch")).expect("recreate scratch after rotation");
    fs::write(repo.path().join("scratch/review.md"), "final gate evidence")
        .expect("write final evidence");
    let calls_before = fs::read_to_string(&log_path).expect("read setup calls");

    let result = land(repo.path(), &options, &NullProgress).expect("complete final Task");

    assert!(result.is_none(), "no empty GitHub PR should be created");
    let calls_after = fs::read_to_string(&log_path).expect("read final calls");
    let final_calls = calls_after
        .strip_prefix(&calls_before)
        .expect("setup calls remain a prefix");
    for mutation in ["pr create", "pr edit", "pr ready", "pr merge"] {
        assert!(
            !final_calls.contains(mutation),
            "direct completion must not mutate GitHub: {final_calls}"
        );
    }
    assert_eq!(
        runtime.block_on(task.store.work_status(&work)).unwrap(),
        WorkStatus::Done
    );
    let prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read completed PR chain");
    assert_eq!(prs.len(), 2, "retain branch identity for cleanup retries");
    assert_eq!(prs[0].phase(), PrPhase::Merged);
    assert_eq!(prs[1].phase(), PrPhase::Abandoned);
    assert!(!repo.path().join("scratch/review.md").exists());
}

#[test]
fn changed_head_revokes_auto_merge_and_clears_the_stale_request() {
    let home = tempfile::TempDir::new().expect("temp home");
    let log_path = home.path().join("gh.log");
    let script = gh_changed_head_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(&[("gh", script.as_str())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    point_origin_at_github(&repo);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let now = time::OffsetDateTime::now_utc();
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: now,
        presentation: Some(reviewer_copy("old-head")),
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: Some("old-head".to_string()),
        }),
        merge: Some(PrMergeRequest {
            mode: PrMergeMode::Auto,
            requested_at: now,
            head_sha: "old-head".to_string(),
            after_merge: AfterMerge::CompleteTask,
            next_slug: None,
        }),
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("store auto-merge request");

    task_status(repo.path(), Some("INF-123"))
        .expect("reconcile changed head")
        .execution
        .expect("execution");

    let persisted = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert_eq!(persisted.head_sha(), Some("new-head"));
    assert!(persisted.merge_request().is_none());
    assert_eq!(persisted.after_merge(), AfterMerge::ContinueTask);
    let log = std::fs::read_to_string(log_path).expect("read gh log");
    assert!(log.contains("pr merge 912 --disable-auto"));
}

#[test]
fn task_resume_revokes_auto_merge_before_returning_to_human_review() {
    let home = tempfile::TempDir::new().expect("temp home");
    let log_path = home.path().join("gh.log");
    let script = gh_open_auto_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(&[("gh", script.as_str())], home.path());
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    let base = repo.head_sha();
    let branch = "jack/task-resume-proof";
    repo.create_branch(branch);
    point_origin_at_github(&repo);
    // Planning lookup uses the canonical repository path, including macOS /var aliases.
    let canonical_repo = repo.path().canonicalize().expect("canonical fixture repo");
    let task = register_task(home.path(), &canonical_repo, branch, &base);
    let now = time::OffsetDateTime::now_utc();
    let head = repo.head_sha();
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: now,
        presentation: Some(reviewer_copy(&head)),
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: Some(head.clone()),
        }),
        merge: Some(PrMergeRequest {
            mode: PrMergeMode::Auto,
            requested_at: now,
            head_sha: head,
            after_merge: AfterMerge::CompleteTask,
            next_slug: None,
        }),
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    let position = FlowSession {
        invocation: QueuedInvocation::load(repo.path(), "task-design").expect("Task design Flow"),
        cursor: loopflow::engine::ExecutionCursor {
            index: 1,
            iteration: 0,
            ..Default::default()
        },
        version: 0,
        task_id: Some(task.task.id.clone()),
        wave_id: Some(task.task.wave_id.clone()),
        cwd: task.task.worktree.clone(),
        message: None,
        model: None,
        current_attempt: None,
        pending_session_id: None,
        ready_summary: None,
        worker_generation: 0,
        claim: None,
        failure: None,
        finished: false,
        updated_at: now,
    };
    assert!(position.is_human());
    let position = runtime
        .block_on(task.store.start_task_flow(&task.task.id, position))
        .expect("persist review boundary");
    let position = runtime
        .block_on(
            task.store
                .reserve_task_review(position.id(), position.version),
        )
        .expect("reserve the review Session");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("store auto merge request");

    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["--task", "INF-123", "flow", "start", "--json"])
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: loopflow::ops::task::TaskSnapshot = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result.task_id, task.task.id.to_string());
    let resumed = runtime
        .block_on(task.store.task_flow(&task.task.id))
        .unwrap()
        .expect("review boundary remains available");
    assert_eq!(resumed.invocation, position.invocation);
    assert_eq!(resumed.cursor, position.cursor);
    assert!(resumed.is_human());
    assert!(resumed.pending_session_id.is_some());

    let persisted = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert!(persisted.merge_request().is_none());
    let log = std::fs::read_to_string(log_path).expect("read gh log");
    assert!(log.contains("pr merge 912 --disable-auto"));
}

#[test]
fn pushed_task_commit_revokes_auto_before_exposing_the_new_head() {
    let home = tempfile::TempDir::new().expect("temp home");
    let log_path = home.path().join("push.log");
    let script = gh_open_auto_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(&[("gh", script.as_str())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-commit-push";
    repo.create_branch(branch);
    repo.create_file("feature.txt", "first head");
    repo.stage_all();
    repo.commit("first head");
    push_branch(&repo, branch);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let now = time::OffsetDateTime::now_utc();
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: now,
        presentation: Some(reviewer_copy(&repo.head_sha())),
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: Some(repo.head_sha()),
        }),
        merge: Some(PrMergeRequest {
            mode: PrMergeMode::Auto,
            requested_at: now,
            head_sha: repo.head_sha(),
            after_merge: AfterMerge::CompleteTask,
            next_slug: None,
        }),
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("store Auto request");

    let hook = repo.bare_path().join("hooks/pre-receive");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\necho git-push >> '{}'\ncat >/dev/null\n",
            log_path.display()
        ),
    )
    .expect("write push hook");
    let mut permissions = fs::metadata(&hook).expect("hook metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&hook, permissions).expect("make hook executable");

    repo.create_file("follow-up.txt", "new head");
    commit_workflow(
        repo.path(),
        &CommitOptions {
            add: true,
            push: true,
            create_draft_pr: false,
            task: "commit".to_string(),
            sources: Vec::new(),
            message: Some("new Task head".to_string()),
            agent: None,
        },
        &NullProgress,
        &|_| {},
    )
    .expect("commit and push new head");

    let persisted = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert!(persisted.merge_request().is_none());
    let log = fs::read_to_string(log_path).expect("read operation log");
    let disable = log
        .find("pr merge 912 --disable-auto")
        .expect("Auto is revoked");
    let push = log.find("git-push").expect("new head is pushed");
    assert!(disable < push, "Auto must be revoked before push:\n{log}");
}

#[test]
fn observed_merge_does_not_complete_a_task_from_status() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[("gh", gh_merged_pr_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    point_origin_at_github(&repo);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let head = repo.head_sha();
    let now = time::OffsetDateTime::now_utc();
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: now,
        presentation: Some(reviewer_copy(&head)),
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: Some(head.clone()),
        }),
        merge: Some(PrMergeRequest {
            mode: PrMergeMode::User,
            requested_at: now,
            head_sha: head,
            after_merge: AfterMerge::CompleteTask,
            next_slug: None,
        }),
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("mark PR as completing");

    let persisted_task = task_status(repo.path(), Some("INF-123"))
        .expect("reconcile completing PR")
        .execution
        .expect("execution");
    assert!(
        matches!(
            persisted_task.observation,
            loopflow::work::task::Observation::Fresh { .. }
        ),
        "status should use the bounded REST observation: {persisted_task:?}"
    );
    assert!(matches!(persisted_task.status, WorkStatus::Ready));
    let prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read completing PR");
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].phase(), PrPhase::Merged);
}

#[test]
fn observed_auto_merge_waits_for_watched_landing_to_complete_the_task() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[("gh", gh_merged_pr_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    point_origin_at_github(&repo);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let head = repo.head_sha();
    let now = time::OffsetDateTime::now_utc();
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: now,
        presentation: Some(reviewer_copy(&head)),
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: Some(head.clone()),
        }),
        merge: Some(PrMergeRequest {
            mode: PrMergeMode::Auto,
            requested_at: now,
            head_sha: head,
            after_merge: AfterMerge::CompleteTask,
            next_slug: None,
        }),
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("mark PR as completing");

    let persisted_task = task_status(repo.path(), Some("INF-123"))
        .expect("reconcile watched PR merge")
        .execution
        .expect("execution");
    assert!(!matches!(persisted_task.status, WorkStatus::Done));
    let prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read completing PR");
    assert_eq!(prs[0].phase(), PrPhase::Merged);
}

#[test]
fn repeated_status_of_merged_task_never_completes_work() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[("gh", gh_merged_pr_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    point_origin_at_github(&repo);
    let task = register_unrun_task(home.path(), repo.path(), branch, &base);
    let head = repo.head_sha();
    let now = time::OffsetDateTime::now_utc();
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: now,
        presentation: Some(reviewer_copy(&head)),
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: Some(head.clone()),
        }),
        merge: Some(PrMergeRequest {
            mode: PrMergeMode::User,
            requested_at: now,
            head_sha: head,
            after_merge: AfterMerge::CompleteTask,
            next_slug: None,
        }),
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("mark PR as completing");

    let first = task_status(repo.path(), Some("INF-123"))
        .expect("first merged-PR status")
        .execution
        .expect("execution");
    assert_eq!(first.status, WorkStatus::Ready);
    let first_events = runtime
        .block_on(task.store.task_events_after(&task.task.id, 0))
        .expect("read first Task events");
    let conn =
        rusqlite::Connection::open(home.path().join("loopflow.db")).expect("open test registry");
    let first_state: (String, Option<i64>) = conn
        .query_row(
            "SELECT work_state, work_terminal_at FROM tasks WHERE id=?1",
            [task.task.id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read Work state");
    let second = task_status(repo.path(), Some("INF-123"))
        .expect("repeated merged-PR status")
        .execution
        .expect("execution");
    let second_state: (String, Option<i64>) = conn
        .query_row(
            "SELECT work_state, work_terminal_at FROM tasks WHERE id=?1",
            [task.task.id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("reread Work state");
    let second_events = runtime
        .block_on(task.store.task_events_after(&task.task.id, 0))
        .expect("reread Task events");

    assert_eq!(second.status, WorkStatus::Ready);
    assert_eq!(
        second_state, first_state,
        "status must not mutate Work state"
    );
    assert_eq!(
        second_events, first_events,
        "completion must be recorded once"
    );
    let completion_count = second_events
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                loopflow::work::task::TaskEventKind::Completed { .. }
            )
        })
        .count();
    assert_eq!(completion_count, 0);
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

    let result = task_complete(repo.path(), "INF-123", "done".to_string(), &[]);
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
fn default_branch_refuses_pr_before_committing_or_pushing() {
    let repo = TestRepo::new();
    let head = repo.head_sha();
    repo.create_file("notes.md", "unpublished\n");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
            title: Some("must not ship".to_string()),
            body: None,
            agent: None,
        },
        &NullProgress,
    );

    assert!(matches!(
        result,
        Err(OpsError::Message(message))
            if message.contains("default branch")
                && message.contains("lf wt create")
    ));
    assert_eq!(repo.head_sha(), head);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("notes.md")).unwrap(),
        "unpublished\n"
    );
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
            draft: false,
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
            ("codex", &agent_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
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
            ("codex", &agent_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
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
            ("codex", &agent_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    create_changed_branch(&repo, "feature");
    push_branch(&repo, "feature");

    let result = create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
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
            draft: false,
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

#[test]
fn persistent_publication_pushes_committed_docs_and_preserves_local_files() {
    let home = tempfile::tempdir().unwrap();
    let state = home.path().join("pr-state");
    let gh = draft_pr_script(&state);
    let _env = EnvGuard::with_lf_home(&[("gh", &gh)], home.path());
    let repo = TestRepo::new();
    let persistent = loopflow::engine::worktrees::ensure_agent_worktree(
        repo.path(),
        loopflow::engine::worktrees::WorktreeSegment::parse("repo").unwrap(),
    )
    .unwrap();
    fs::create_dir_all(persistent.path.join("scratch")).unwrap();
    fs::write(persistent.path.join("scratch/design.md"), "private\n").unwrap();
    fs::write(persistent.path.join("memory.md"), "accepted\n").unwrap();
    loopflow::ops::commit_selected(&persistent.path, &["memory.md".into()], Some("Save memory"))
        .unwrap();
    fs::write(persistent.path.join("memory.md"), "next decision\n").unwrap();
    fs::write(persistent.path.join("unrelated.md"), "unpublished\n").unwrap();
    let options = PrOptions {
        draft: false,
        title: Some("Document accepted decisions".into()),
        body: Some("Durable repository memory.".into()),
        agent: None,
    };
    let first = create_or_update_pr(&persistent.path, &options, &NullProgress).unwrap();
    let second = create_or_update_pr(&persistent.path, &options, &NullProgress).unwrap();
    assert_eq!(first.url, second.url);
    let published = Command::new("git")
        .current_dir(&persistent.path)
        .args(["show", &format!("origin/{}:memory.md", persistent.branch)])
        .output()
        .unwrap();
    assert!(published.status.success());
    assert_eq!(published.stdout, b"accepted\n");
    let scratch = Command::new("git")
        .current_dir(&persistent.path)
        .args([
            "ls-tree",
            "-r",
            "--name-only",
            &format!("origin/{}", persistent.branch),
            "--",
            "scratch",
            "unrelated.md",
        ])
        .output()
        .unwrap();
    assert!(scratch.stdout.is_empty());
    assert_eq!(
        fs::read_to_string(persistent.path.join("memory.md")).unwrap(),
        "next decision\n"
    );
    assert!(persistent.path.join("scratch/design.md").exists());
    assert!(persistent.path.join("unrelated.md").exists());
}
