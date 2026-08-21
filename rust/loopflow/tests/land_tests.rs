mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

use loopflow::engine::worktrees::create_named_worktree;
use loopflow::ops::{
    create_or_update_pr, land, submit, LandOptions, NullProgress, OpsError, PrOptions,
};
use loopflow::task::{AfterMerge, PrMergeMode, PrPhase};
use loopflow_test_support::TestRepo;
use support::{
    codex_app_server_script, counting_open_script, presentation_attempts, register_task, EnvGuard,
};

fn push_branch(repo: &TestRepo, name: &str) {
    let _ = Command::new("git")
        .args(["push", "-u", "origin", name])
        .current_dir(repo.path())
        .status();
}

fn local_branch_exists(repo: &TestRepo, name: &str) -> bool {
    Command::new("git")
        .args(["show-ref", "--verify", &format!("refs/heads/{name}")])
        .current_dir(repo.path())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn remote_branch_exists(repo: &TestRepo, name: &str) -> bool {
    Command::new("git")
        .arg("--git-dir")
        .arg(repo.bare_path())
        .args(["show-ref", "--verify", &format!("refs/heads/{name}")])
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn gh_no_pr_script() -> &'static str {
    r#"#!/bin/sh
if [ "$1" = "--version" ]; then
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

if [ "$1 $2" = "api graphql" ]; then
  echo 'false'
  exit 0
fi

if [ "$1 $2" = "pr view" ]; then
  echo 'https://example.com/pr/1'
  exit 0
fi

if [ "$1 $2" = "pr merge" ]; then
  exit 0
fi

exit 0
"#
}

fn noop_open_script() -> &'static str {
    "#!/bin/sh\nexit 0\n"
}

fn gh_land_script(log_path: &str) -> String {
    format!(
        r#"#!/bin/sh
auto_state="{log_path}.auto"
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "{log_path}"

if [ "$1 $2" = "pr list" ]; then
  echo '[]'
  exit 0
fi

if [ "$1 $2" = "pr create" ]; then
  echo "https://example.com/pr/1"
  exit 0
fi

if [ "$1 $2" = "api graphql" ]; then
  if [ -f "$auto_state" ]; then echo 'true'; else echo 'false'; fi
  exit 0
fi

if [ "$1 $2" = "pr view" ]; then
  echo "https://example.com/pr/1"
  exit 0
fi

if [ "$1 $2" = "pr merge" ]; then
  touch "$auto_state"
  exit 0
fi

exit 0
"#
    )
}

fn gh_existing_pr_script(log_path: &str) -> String {
    format!(
        r#"#!/bin/sh
auto_state="{log_path}.auto"
queue_state="{log_path}.queued"
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "{log_path}"
if [ "$1 $2" = "pr list" ]; then
  head="$(git rev-parse HEAD)"
  echo "[{{\"url\":\"https://example.com/pr/912\",\"state\":\"OPEN\",\"isDraft\":false,\"number\":912,\"mergeCommit\":null,\"headRefOid\":\"$head\"}}]"
  exit 0
fi
if [ "$1 $2" = "api graphql" ]; then
  if [ -f "$auto_state" ] || [ -f "$queue_state" ]; then echo 'true'; else echo 'false'; fi
  exit 0
fi
if [ "$1 $2" = "pr view" ]; then
  echo 'https://example.com/pr/912'
  exit 0
fi
if [ "$1 $2 $3 $4" = "pr merge 912 --disable-auto" ]; then
  rm -f "$auto_state" "$queue_state"
  exit 0
fi
if [ "$1 $2" = "pr merge" ]; then
  touch "$auto_state"
  exit 0
fi
exit 0
"#
    )
}

fn gh_ready_failure_script(log_path: &str) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "{log_path}"
if [ "$1 $2" = "pr list" ]; then
  echo '[]'
  exit 0
fi
if [ "$1 $2" = "pr create" ]; then
  echo 'https://example.com/pr/1'
  exit 0
fi
if [ "$1 $2" = "pr view" ]; then
  echo 'https://example.com/pr/1'
  exit 0
fi
if [ "$1 $2" = "pr ready" ]; then
  echo 'ready failed' >&2
  exit 1
fi
exit 0
"#
    )
}

fn gh_auto_failure_script(log_path: &str) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "{log_path}"
if [ "$1 $2" = "pr list" ]; then
  head="$(git rev-parse HEAD)"
  echo "[{{\"url\":\"https://example.com/pr/912\",\"state\":\"OPEN\",\"isDraft\":false,\"number\":912,\"mergeCommit\":null,\"headRefOid\":\"$head\"}}]"
  exit 0
fi
if [ "$1 $2" = "api graphql" ]; then
  echo 'false'
  exit 0
fi
if [ "$1 $2" = "pr view" ]; then
  echo 'https://example.com/pr/912'
  exit 0
fi
if [ "$1 $2" = "pr merge" ]; then
  echo 'auto arm failed' >&2
  exit 1
fi
exit 0
"#
    )
}

fn agent_script() -> String {
    codex_app_server_script(r#"{"title":"generated title","body":"generated body"}"#, "")
}

#[test]
fn land_local_squash_merges_to_main() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect("land");

    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo.path())
        .output()
        .expect("git rev-parse");
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(branch, "main");
    assert!(!local_branch_exists(&repo, "feature"));
    assert!(repo.path().join("feature.txt").exists());
}

#[test]
fn land_preserves_main_on_failure() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("conflict.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    repo.checkout("main");
    repo.create_file("conflict.txt", "main");
    repo.stage_all();
    repo.commit("main work");
    repo.push();
    let main_head = repo.head_sha();

    repo.checkout("feature");
    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    );

    assert!(result.is_err());
    let _ = Command::new("git")
        .args(["rebase", "--abort"])
        .current_dir(repo.path())
        .status();
    let _ = Command::new("git")
        .args(["reset", "--hard"])
        .current_dir(repo.path())
        .status();
    repo.checkout("main");
    assert_eq!(repo.head_sha(), main_head);
}

#[test]
fn land_cleans_up_remote_branch() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect("land");

    assert!(!remote_branch_exists(&repo, "feature"));
}

#[test]
fn land_clears_scratch_and_preserves_gitkeep() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let scratch = repo.path().join("scratch");
    fs::create_dir_all(scratch.join("nested")).expect("create nested scratch dir");
    fs::write(scratch.join("notes.md"), "review notes").expect("write scratch note");
    fs::write(scratch.join("nested").join("todo.md"), "todo").expect("write nested scratch note");
    let status = Command::new("git")
        .args(["add", "scratch"])
        .current_dir(repo.path())
        .status()
        .expect("git add scratch");
    assert!(status.success(), "git add scratch should succeed");
    let status = Command::new("git")
        .args(["commit", "-m", "add scratch docs"])
        .current_dir(repo.path())
        .status()
        .expect("git commit scratch");
    assert!(status.success(), "git commit scratch should succeed");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect("land should clear scratch");

    let scratch_entries = fs::read_dir(repo.path().join("scratch"))
        .expect("read scratch after land")
        .map(|entry| {
            entry
                .expect("scratch entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(scratch_entries, vec![".gitkeep"]);
}

#[test]
fn land_collapses_checkpoint_history_and_pushes_the_final_tree_once() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("first.txt", "first");
    repo.stage_all();
    repo.commit("checkpoint: first slice");
    repo.create_file("second.txt", "second");
    repo.stage_all();
    repo.commit("feature behavior");
    let scratch = repo.path().join("scratch");
    fs::create_dir_all(&scratch).expect("create scratch");
    fs::write(scratch.join("working.md"), "discard me").expect("write scratch");
    repo.stage_all();
    repo.commit("checkpoint: notes");
    push_branch(&repo, "feature");

    let push_log = repo.path().join("push.log");
    let hook = repo.bare_path().join("hooks/update");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\n",
            push_log.display()
        ),
    )
    .expect("write update hook");
    let mut permissions = fs::metadata(&hook).expect("hook metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&hook, permissions).expect("make hook executable");

    let gh_log = repo.bare_path().join("gh.log");
    let script = gh_land_script(gh_log.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str()), ("open", noop_open_script())]);
    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("collapsed change".to_string()),
            pr_body: Some("proof".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("land collapsed history");

    let output = |args: &[&str]| {
        let result = Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .output()
            .expect("run git proof");
        assert!(result.status.success(), "git {:?} failed", args);
        String::from_utf8_lossy(&result.stdout).trim().to_string()
    };
    assert_eq!(output(&["rev-list", "--count", "origin/main..HEAD"]), "1");
    assert_eq!(
        output(&["rev-parse", "HEAD"]),
        Command::new("git")
            .arg("--git-dir")
            .arg(repo.bare_path())
            .args(["rev-parse", "refs/heads/feature"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .expect("read remote head")
    );
    assert!(repo.path().join("first.txt").exists());
    assert!(repo.path().join("second.txt").exists());
    assert!(!repo.path().join("scratch/working.md").exists());
    assert_eq!(
        fs::read_to_string(&push_log)
            .expect("one final push receipt")
            .lines()
            .collect::<Vec<_>>(),
        vec!["refs/heads/feature"],
        "submit/land must push only the verified final head"
    );
    assert!(
        !output(&[
            "for-each-ref",
            "--format=%(refname)",
            "refs/loopflow/recovery/"
        ])
        .is_empty(),
        "the pre-collapse head must remain recoverable"
    );
}

#[test]
fn land_refuses_when_clearing_scratch_leaves_no_authored_change() {
    let repo = TestRepo::new();
    repo.create_file("scratch/.gitkeep", "");
    repo.stage_all();
    repo.commit("track scratch");
    repo.push();
    repo.create_branch("feature");
    repo.create_file("scratch/notes.md", "notes only");
    repo.stage_all();
    repo.commit("checkpoint: notes");
    push_branch(&repo, "feature");
    let remote_head = repo.head_sha();
    let gh_log = repo.bare_path().join("gh.log");
    let script = gh_land_script(gh_log.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str())]);

    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("notes only".to_string()),
            pr_body: Some("proof".to_string()),
            agent: None,
        },
        &NullProgress,
    );

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("no authored changes remain")),
        "scratch-only finalization must refuse actionably: {result:?}"
    );
    assert_eq!(
        Command::new("git")
            .arg("--git-dir")
            .arg(repo.bare_path())
            .args(["rev-parse", "refs/heads/feature"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .expect("read untouched remote"),
        remote_head,
        "empty finalization must not push"
    );
    assert!(
        !fs::read_to_string(gh_log)
            .unwrap_or_default()
            .contains("pr create"),
        "empty finalization must not create a PR"
    );
}

#[test]
fn land_does_not_push_when_target_already_contains_the_authored_patch() {
    let repo = TestRepo::new();
    repo.create_file("scratch/.gitkeep", "");
    repo.create_file("shared.txt", "base\n");
    repo.stage_all();
    repo.commit("shared base");
    repo.push();
    repo.create_branch("feature");
    repo.create_file("shared.txt", "same final content\n");
    repo.stage_all();
    repo.commit("feature patch");
    push_branch(&repo, "feature");
    let remote_feature = repo.head_sha();

    repo.checkout("main");
    repo.create_file("shared.txt", "same final content\n");
    repo.stage_all();
    repo.commit("upstream equivalent patch");
    repo.push();
    repo.checkout("feature");

    let gh_log = repo.bare_path().join("gh.log");
    let script = gh_land_script(gh_log.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str()), ("open", noop_open_script())]);
    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("already upstream".to_string()),
            pr_body: Some("proof".to_string()),
            agent: None,
        },
        &NullProgress,
    );

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("expected" ) && message.contains("empty")),
        "an empty final replay must fail before push: {result:?}"
    );
    assert_eq!(
        Command::new("git")
            .arg("--git-dir")
            .arg(repo.bare_path())
            .args(["rev-parse", "refs/heads/feature"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .expect("read remote feature"),
        remote_feature,
        "the remote branch must keep its last authored head"
    );
    let gh_calls = fs::read_to_string(gh_log).unwrap_or_default();
    assert!(
        !gh_calls.contains("pr create")
            && !gh_calls.contains("pr edit")
            && !gh_calls.contains("pr ready")
            && !gh_calls.contains("pr merge"),
        "GitHub must not be mutated for an empty replay: {gh_calls}"
    );
}

#[test]
fn land_missing_pr_error_includes_branch_name() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_home(
        &[
            ("gh", gh_no_pr_script()),
            ("codex", &agent_script()),
            ("open", noop_open_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("cached title".to_string()),
            pr_body: Some("cached body".to_string()),
            agent: None,
        },
        &NullProgress,
    );

    let Err(OpsError::Message(message)) = result else {
        panic!("expected missing PR message");
    };
    assert!(message.contains("no open PR found for branch 'feature'"));
}

#[test]
fn land_uses_cached_pr_copy_when_available() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let scratch = repo.path().join("scratch");
    fs::create_dir_all(&scratch).expect("create scratch");
    fs::write(scratch.join("pr-title.txt"), "cached title").expect("write title");
    fs::write(scratch.join("pr-body.md"), "cached body").expect("write body");
    fs::write(scratch.join(".pr-copy-ref"), repo.head_sha()).expect("write ref");

    let log_path = repo.bare_path().join("gh.log");
    let script = gh_land_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str()), ("open", noop_open_script())]);

    land(
        repo.path(),
        &LandOptions {
            strict: false,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect("land with cached copy");

    let log = fs::read_to_string(&log_path).expect("read gh log");
    assert!(log.contains("--title cached title"));
    assert!(log.contains("--body cached body"));
}

#[test]
fn submit_and_land_make_no_presentation_attempt() {
    let marker_dir = tempfile::TempDir::new().expect("marker dir");
    let marker = marker_dir.path().join("present.log");
    let open_script = counting_open_script(&marker);

    // submit prepares the PR for a human to merge — and presents nothing.
    let submit_repo = TestRepo::new();
    submit_repo.create_branch("feature");
    submit_repo.create_file("feature.txt", "feature");
    submit_repo.stage_all();
    submit_repo.commit("feature work");
    push_branch(&submit_repo, "feature");
    let submit_log = submit_repo.bare_path().join("gh.log");
    let submit_script = gh_land_script(submit_log.to_string_lossy().as_ref());
    {
        let _env = EnvGuard::new(&[
            ("gh", submit_script.as_str()),
            ("open", open_script.as_str()),
            ("xdg-open", open_script.as_str()),
        ]);
        submit(
            submit_repo.path(),
            &LandOptions {
                strict: true,
                local: false,
                create_pr: true,
                complete: false,
                next_slug: None,
                worktree: None,
                commit_message: None,
                pr_title: Some("test title".to_string()),
                pr_body: Some("test body".to_string()),
                agent: None,
            },
            &NullProgress,
        )
        .expect("submit");
    }
    assert_eq!(
        presentation_attempts(&marker),
        0,
        "submit must open no review surface"
    );

    // land arms auto-merge and walks away — also presenting nothing.
    let land_repo = TestRepo::new();
    land_repo.create_branch("feature");
    land_repo.create_file("feature.txt", "feature");
    land_repo.stage_all();
    land_repo.commit("feature work");
    push_branch(&land_repo, "feature");
    let land_log = land_repo.bare_path().join("gh.log");
    let land_script = gh_land_script(land_log.to_string_lossy().as_ref());
    {
        let _env = EnvGuard::new(&[
            ("gh", land_script.as_str()),
            ("open", open_script.as_str()),
            ("xdg-open", open_script.as_str()),
        ]);
        land(
            land_repo.path(),
            &LandOptions {
                strict: true,
                local: false,
                create_pr: true,
                complete: false,
                next_slug: None,
                worktree: None,
                commit_message: None,
                pr_title: Some("test title".to_string()),
                pr_body: Some("test body".to_string()),
                agent: None,
            },
            &NullProgress,
        )
        .expect("land");
    }
    assert_eq!(
        presentation_attempts(&marker),
        0,
        "land must open no review surface"
    );
}

#[test]
fn submit_assigns_reviewer_and_skips_auto_merge() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let log_path = repo.bare_path().join("gh.log");
    let script = gh_land_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str()), ("open", noop_open_script())]);

    submit(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("test title".to_string()),
            pr_body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("submit");

    // submit prepares but never merges — that click is the human's.
    let log = fs::read_to_string(&log_path).expect("read gh log");
    // Assigns the PR to the current user for a required, manual merge.
    assert!(log.contains("pr edit --add-assignee @me"));
    // Marks the PR ready, but does NOT arm auto-merge.
    assert!(log.contains("pr ready"));
    assert!(!log.contains("merge --auto"));
}

#[test]
fn submit_surfaces_ready_failure_before_assignment() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-ready-failure";
    repo.create_branch(branch);
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, branch);

    let log_path = repo.bare_path().join("gh.log");
    let script = gh_ready_failure_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );
    let task = register_task(home.path(), repo.path(), branch, &base);

    let result = submit(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("test title".to_string()),
            pr_body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    );

    assert!(matches!(result, Err(OpsError::CommandFailed { .. })));
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert!(
        pr.merge_request().is_none(),
        "failed remote finalization must not leave a false settlement owner"
    );
    let log = fs::read_to_string(&log_path).expect("read gh log");
    assert!(log.contains("pr ready"));
    assert!(!log.contains("pr edit --add-assignee @me"));
}

#[test]
fn land_clears_the_durable_request_when_auto_arm_fails() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-auto-failure";
    repo.create_branch(branch);
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, branch);

    let log_path = home.path().join("gh.log");
    let script = gh_auto_failure_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );
    let task = register_task(home.path(), repo.path(), branch, &base);

    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("test title".to_string()),
            pr_body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    );

    assert!(matches!(result, Err(OpsError::CommandFailed { .. })));
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert!(
        pr.merge_request().is_none(),
        "a failed Auto handoff must not leave GitHub as a false owner"
    );
    let log = fs::read_to_string(log_path).expect("read gh log");
    assert!(log.contains("pr merge 912 --squash --auto --match-head-commit"));
}

#[test]
fn latest_land_disposition_wins_before_merge() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let log_path = home.path().join("gh.log");
    let script = gh_existing_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    repo.push_new_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &base);
    fs::write(format!("{}.auto", log_path.display()), "armed externally")
        .expect("seed external auto-merge state");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: true,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("test title".to_string()),
            pr_body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("land as completing PR");
    let head = repo.head_sha();

    create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("refresh published PR".to_string()),
            body: Some("same head".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("refresh same-head publication");

    let runtime = tokio::runtime::Runtime::new().expect("read task runtime");
    let preserved = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    let preserved = preserved.merge_request().expect("preserved merge request");
    assert_eq!(preserved.after_merge, AfterMerge::CompleteTask);
    assert_eq!(preserved.head_sha, head);

    let hook = repo.bare_path().join("hooks/pre-receive");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\necho git-push >> '{}'\ncat >/dev/null\n",
            log_path.display()
        ),
    )
    .expect("write remote push hook");
    let mut permissions = fs::metadata(&hook)
        .expect("read hook metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&hook, permissions).expect("make push hook executable");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: false,
            next_slug: Some("follow-up-proof".to_string()),
            worktree: None,
            commit_message: None,
            pr_title: Some("test title".to_string()),
            pr_body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("revise land disposition");
    let revised_head = repo.head_sha();

    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert_eq!(pr.phase(), PrPhase::Open);
    let publication = pr.publication.expect("publication");
    let presentation = publication
        .presentation
        .as_ref()
        .expect("Task identity survives refresh and land");
    assert_eq!(presentation.title, "Prove Task PR transitions — test title");
    assert!(presentation.body.starts_with(
        "Linear Task: [INF-123](https://linear.app/loopflow/issue/INF-123/prove-task-pr-transitions)\n\n"
    ));
    assert_eq!(publication.github.map(|pr| pr.number), Some(912));
    let merge = publication.merge.expect("explicit merge request");
    assert_eq!(merge.mode, PrMergeMode::Auto);
    assert_eq!(merge.head_sha, revised_head);
    assert_eq!(merge.after_merge, AfterMerge::ContinueTask);
    assert_eq!(merge.next_slug.as_deref(), Some("follow-up-proof"));
    let log = fs::read_to_string(&log_path).expect("read gh log");
    assert!(log.contains(&format!(
        "pr merge 912 --squash --auto --match-head-commit {revised_head}"
    )));
    let first_disable = log
        .find("pr merge 912 --disable-auto")
        .expect("pre-existing Auto is replaced");
    let first_arm = log
        .find("pr merge 912 --squash --auto --match-head-commit")
        .expect("land arms the exact prepared head");
    assert!(
        first_disable < first_arm,
        "external Auto must be replaced by the exact-head command:\n{log}"
    );
    let disable = log
        .rfind("pr merge 912 --disable-auto")
        .expect("second land revokes prior Auto request");
    if revised_head != head {
        let push = log
            .find("git-push")
            .expect("a changed head is pushed by the second land");
        assert!(
            disable < push,
            "Auto intent must be revoked before the LF-owned head-changing push:\n{log}"
        );
    } else {
        assert!(
            !log.contains("git-push"),
            "an unchanged head needs no replacement push:\n{log}"
        );
    }

    submit(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("test title".to_string()),
            pr_body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("replace auto merge with user merge request");
    let submitted_head = repo.head_sha();

    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    let request = pr.merge_request().expect("user merge request");
    assert_eq!(request.mode, PrMergeMode::User);
    assert_eq!(request.head_sha, submitted_head);
    let log = fs::read_to_string(&log_path).expect("read gh log");
    assert!(log.contains("pr merge 912 --disable-auto"));
    assert!(log.contains("pr edit --add-assignee @me"));
}

#[test]
fn repeated_identical_land_preserves_the_armed_task_request() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let log_path = home.path().join("gh.log");
    let script = gh_existing_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );
    let base = repo.head_sha();
    let branch = "jack/replay-safe-land";
    repo.create_branch(branch);
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    repo.push_new_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let options = LandOptions {
        strict: true,
        local: false,
        create_pr: false,
        complete: true,
        next_slug: None,
        worktree: None,
        commit_message: None,
        pr_title: Some("test title".to_string()),
        pr_body: Some("test body".to_string()),
        agent: None,
    };

    land(repo.path(), &options, &NullProgress).expect("first land arms the request");
    let armed_head = repo.head_sha();
    let first_log = fs::read_to_string(&log_path).expect("read first gh log");
    let first_merge_calls = first_log
        .lines()
        .filter(|line| line.starts_with("pr merge"))
        .count();

    land(repo.path(), &options, &NullProgress).expect("identical land is replay-safe");

    assert_eq!(
        repo.head_sha(),
        armed_head,
        "replay must not rewrite the head"
    );
    let pr = tokio::runtime::Runtime::new()
        .expect("task runtime")
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    let request = pr.merge_request().expect("preserved merge request");
    assert_eq!(request.mode, PrMergeMode::Auto);
    assert_eq!(request.after_merge, AfterMerge::CompleteTask);
    assert_eq!(request.head_sha, armed_head);
    let replay_log = fs::read_to_string(&log_path).expect("read replay gh log");
    assert_eq!(
        replay_log
            .lines()
            .filter(|line| line.starts_with("pr merge"))
            .count(),
        first_merge_calls,
        "replay must neither disable nor arm auto-merge again:\n{replay_log}"
    );
    assert!(!replay_log.contains("--disable-auto"));
}

#[test]
fn non_task_land_leaves_the_merge_queue_before_pushing_a_new_head() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let log_path = home.path().join("gh.log");
    let script = gh_existing_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );
    let branch = "jack/wave-repair";
    repo.create_branch(branch);
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    repo.push_new_branch(branch);
    fs::write(format!("{}.queued", log_path.display()), "queued")
        .expect("seed remote auto-merge state");
    let hook = repo.bare_path().join("hooks/pre-receive");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\necho git-push >> '{}'\ncat >/dev/null\n",
            log_path.display()
        ),
    )
    .expect("write remote push hook");
    let mut permissions = fs::metadata(&hook)
        .expect("read hook metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&hook, permissions).expect("make push hook executable");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("test title".to_string()),
            pr_body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("non-Task land");

    let log = fs::read_to_string(&log_path).expect("read gh log");
    let disable = log
        .find("pr merge 912 --disable-auto")
        .expect("queued Auto is revoked");
    let push = log.find("git-push").expect("prepared head is pushed");
    let arm = log
        .rfind("pr merge 912 --squash --auto --match-head-commit")
        .expect("prepared head is re-armed");
    assert!(
        disable < push && push < arm,
        "unexpected settlement order:\n{log}"
    );
}

#[test]
fn submit_does_not_rotate_worktree() {
    let repo = TestRepo::new();
    let log_path = repo.bare_path().join("gh.log");
    let script = gh_land_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str()), ("open", noop_open_script())]);

    let worktree = create_named_worktree(repo.path(), "sub", None, false).expect("create worktree");
    fs::write(worktree.path.join("feature.txt"), "feature").expect("write feature file");
    let status = Command::new("git")
        .args(["add", "."])
        .current_dir(&worktree.path)
        .status()
        .expect("git add");
    assert!(status.success(), "git add should succeed");
    let status = Command::new("git")
        .args(["commit", "-m", "feature work"])
        .current_dir(&worktree.path)
        .status()
        .expect("git commit");
    assert!(status.success(), "git commit should succeed");
    push_branch(&repo, &worktree.branch);

    submit(
        &worktree.path,
        &LandOptions {
            strict: true,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("test title".to_string()),
            pr_body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("submit from worktree");

    // The worktree stays put — no preserve, no next-item rotation.
    assert!(worktree.path.exists());
}

#[test]
fn land_generates_copy_when_cached_pr_copy_is_stale() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_home(
        &[
            ("gh", gh_no_pr_script()),
            ("codex", &agent_script()),
            ("open", noop_open_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let scratch = repo.path().join("scratch");
    fs::create_dir_all(&scratch).expect("create scratch");
    fs::write(scratch.join("pr-title.txt"), "stale title").expect("write title");
    fs::write(scratch.join("pr-body.md"), "stale body").expect("write body");
    fs::write(
        scratch.join(".pr-copy-ref"),
        "0000000000000000000000000000000000000000",
    )
    .expect("write stale ref");
    let status = Command::new("git")
        .args(["add", "scratch"])
        .current_dir(repo.path())
        .status()
        .expect("git add scratch");
    assert!(status.success(), "git add scratch should succeed");
    let status = Command::new("git")
        .args(["commit", "-m", "add stale gate copy"])
        .current_dir(repo.path())
        .status()
        .expect("git commit scratch");
    assert!(status.success(), "git commit scratch should succeed");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect("land with stale cached copy should regenerate");
}

#[test]
fn lf_pr_land_publishes_without_create_flag_and_leaves_worktree_in_place() {
    let repo = TestRepo::new();
    let log_path = repo.bare_path().join("gh.log");
    let script = gh_land_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str()), ("open", noop_open_script())]);

    let worktree = repo.create_named_worktree("land");
    let branch = "land";

    fs::write(worktree.join("feature.txt"), "feature").expect("write feature file");
    let status = Command::new("git")
        .args(["add", "."])
        .current_dir(&worktree)
        .status()
        .expect("git add");
    assert!(status.success(), "git add should succeed");
    let status = Command::new("git")
        .args(["commit", "-m", "feature work"])
        .current_dir(&worktree)
        .status()
        .expect("git commit");
    assert!(status.success(), "git commit should succeed");
    assert!(
        !remote_branch_exists(&repo, branch),
        "the test must begin before publication"
    );

    let directive_path = repo.path().join("directive.txt");
    let status = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args([
            "pr",
            "land",
            "--strict",
            "--title",
            "test title",
            "--body",
            "test body",
        ])
        .current_dir(&worktree)
        .env("LOOPFLOW_DIRECTIVE_FILE", &directive_path)
        .status()
        .expect("run lf pr land");
    assert!(status.success(), "lf pr land should succeed");
    assert!(
        remote_branch_exists(&repo, branch),
        "lf pr land should push its branch"
    );
    let gh_log = fs::read_to_string(&log_path).expect("read gh log");
    assert!(
        gh_log.lines().any(|line| line.starts_with("pr create ")),
        "lf pr land should create its missing PR, got: {gh_log}"
    );

    // The wave home is permanent: land never rotates the worktree or cds away.
    assert!(
        worktree.exists(),
        "worktree should stay in place after land"
    );
    let directive = fs::read_to_string(&directive_path).unwrap_or_default();
    assert!(
        !directive.contains("cd "),
        "land should not emit a cd directive, got: {directive}"
    );
}
