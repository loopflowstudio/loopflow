mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use loopflow::engine::worktrees::create_named_worktree;
use loopflow::ops::{
    arm as land, create_or_update_pr, submit, LandOptions, NullProgress, OpsError, PrOptions,
};
use loopflow::work::task::{AfterMerge, PrMergeMode, PrPhase};
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

fn gh_no_pr_script() -> String {
    let unarmed = support::github_merge_response(1, "fixture-head", "OPEN", "CLEAN", None);
    format!(
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
  echo '{unarmed}'
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
    )
}

fn noop_open_script() -> &'static str {
    "#!/bin/sh\nexit 0\n"
}

fn gh_land_script(log_path: &str) -> String {
    let unarmed = support::github_merge_response(1, "fixture-head", "OPEN", "CLEAN", None);
    let armed = support::github_merge_response(1, "fixture-head", "OPEN", "CLEAN", Some("auto"));
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
  if [ -f "$auto_state" ]; then echo '{armed}'; else echo '{unarmed}'; fi
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

fn gh_watched_land_script(log_path: &str, awaiting_queue: bool, merge_race: bool) -> String {
    let merge_state = if awaiting_queue { "behind" } else { "blocked" };
    let checks = support::github_checks_page("$head", &[("fixture-check", "FAILURE", true)]);
    let unarmed = support::github_merge_response(1, "$head", "OPEN", "CLEAN", None);
    let armed = support::github_merge_response(
        1,
        "$head",
        "OPEN",
        merge_state,
        Some(if awaiting_queue {
            "awaiting_queue"
        } else {
            "auto"
        }),
    );
    let merged = support::github_merge_response(1, "$head", "MERGED", "UNKNOWN", None)
        .replace("\"oid\":\"$head\"", "\"oid\":\"merge-head\"");
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
  echo 'https://example.com/pr/1'
  exit 0
fi
if [ "$1 $2" = "api graphql" ]; then
  case "$*" in
    *LoopflowPrChecks*)
      head="$(git rev-parse HEAD)"
      cat <<JSON
{checks}
JSON
      exit 0 ;;
  esac
  # A request-only projection discards the server's merged state at this boundary.
  case "$*" in *--jq*) echo false; exit 0 ;; esac
  head="$(git rev-parse HEAD)"
  if [ ! -f "$auto_state" ]; then
    cat <<JSON
{unarmed}
JSON
  elif [ "{merge_race}" = true ] || [ -z "$LF_TEST_REPAIR_PROOF" ] || [ -f "$LF_TEST_REPAIR_PROOF" ]; then
    cat <<JSON
{merged}
JSON
  else
    cat <<JSON
{armed}
JSON
  fi
  exit 0
fi
if [ "$1 $2" = "pr view" ]; then
  echo 'https://example.com/pr/1'
  exit 0
fi
if [ "$1 $2" = "pr merge" ]; then
  touch "$auto_state"
  exit 0
fi
if [ "$1" = "api" ]; then
  head="$(git rev-parse HEAD)"
  if [ "{merge_race}" = true ] || {{ [ -n "$LF_TEST_REPAIR_PROOF" ] && [ ! -f "$LF_TEST_REPAIR_PROOF" ]; }}; then
    echo "{{\"merged\":false,\"state\":\"open\",\"mergeable_state\":\"{merge_state}\",\"draft\":false,\"number\":1,\"html_url\":\"https://example.com/pr/1\",\"head\":{{\"sha\":\"$head\"}}}}"
    exit 0
  fi
  echo "{{\"merged\":true,\"state\":\"closed\",\"draft\":false,\"merge_commit_sha\":\"merge-head\",\"merged_at\":\"2026-08-21T00:00:00Z\",\"number\":1,\"html_url\":\"https://example.com/pr/1\",\"head\":{{\"sha\":\"$head\"}}}}"
  exit 0
fi
exit 0
"#
    )
}

fn initialize_landing_store(path: &std::path::Path) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    drop(
        runtime
            .block_on(loopflow::store::open_ephemeral_store(
                &loopflow::store::StorageConfig::sqlite(path.to_path_buf()),
            ))
            .unwrap(),
    );
}

fn gh_existing_pr_script(log_path: &str) -> String {
    let unarmed = support::github_merge_response(912, "fixture-head", "OPEN", "CLEAN", None);
    let armed = support::github_merge_response(912, "fixture-head", "OPEN", "CLEAN", Some("auto"));
    let queued = support::github_merge_response(
        912,
        "fixture-head",
        "OPEN",
        "CLEAN",
        Some("queued:PR_fixture"),
    );
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
  case "$*" in
    *dequeuePullRequest*) rm -f "$queue_state"; exit 0 ;;
  esac
  if [ -f "$queue_state" ]; then echo '{queued}';
  elif [ -f "$auto_state" ]; then echo '{armed}'; else echo '{unarmed}'; fi
  exit 0
fi
if [ "$1 $2" = "pr view" ]; then
  echo 'https://example.com/pr/912'
  exit 0
fi
if [ "$1 $2 $3 $4" = "pr merge 912 --disable-auto" ]; then
  rm -f "$auto_state"
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

fn gh_auto_failure_script(log_path: &str) -> String {
    let unarmed = support::github_merge_response(912, "fixture-head", "OPEN", "CLEAN", None);
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
  echo '{unarmed}'
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
        .args(["merge", "--abort"])
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
fn final_preparation_keeps_published_history_when_the_base_changes() {
    for (manual_merge, advance_base) in [(false, false), (true, false), (false, true)] {
        let home = tempfile::TempDir::new().unwrap();
        let repo = TestRepo::new();
        repo.create_file("scratch/.gitkeep", "");
        repo.stage_all();
        repo.commit("track empty scratch");
        repo.push();
        repo.create_branch("feature");
        repo.create_file("feature.txt", "reviewed change");
        repo.stage_all();
        repo.commit("reviewed change");
        repo.push_new_branch("feature");
        let published_head = repo.head_sha();
        if advance_base {
            repo.checkout("main");
            repo.create_file("upstream.txt", "new upstream work");
            repo.stage_all();
            repo.commit("advance main");
            repo.push();
            repo.checkout("feature");
        }
        let gh_log = home.path().join("gh.log");
        let script = gh_existing_pr_script(gh_log.to_str().unwrap())
            .replace("git rev-parse HEAD", "git rev-parse @{upstream}");
        let _env = EnvGuard::with_lf_home(&[("gh", script.as_str())], home.path());
        let options = LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("reviewed change".to_string()),
            pr_body: Some("ready".to_string()),
            agent: None,
        };
        if manual_merge {
            submit(repo.path(), &options, &NullProgress).unwrap();
        } else {
            land(repo.path(), &options, &NullProgress).unwrap();
        }
        if advance_base {
            assert_ne!(repo.head_sha(), published_head);
            assert!(
                loopflow::engine::git::is_ancestor(repo.path(), &published_head, "HEAD").unwrap()
            );
            assert_eq!(
                fs::read_to_string(repo.path().join("upstream.txt")).unwrap(),
                "new upstream work"
            );
        } else {
            assert_eq!(repo.head_sha(), published_head);
        }
        assert_eq!(
            fs::read_to_string(repo.path().join("feature.txt")).unwrap(),
            "reviewed change"
        );
        let remote = Command::new("git")
            .arg("--git-dir")
            .arg(repo.bare_path())
            .args(["rev-parse", "refs/heads/feature"])
            .output()
            .unwrap();
        assert!(remote.status.success());
        assert_eq!(
            String::from_utf8_lossy(&remote.stdout).trim(),
            repo.head_sha()
        );
    }
}

#[test]
fn final_preparation_preserves_published_copy_only_for_the_same_head() {
    for (manual_merge, changed_head, body_override) in [
        (false, false, None),
        (true, false, None),
        (false, false, Some("Updated checks")),
        (false, true, None),
    ] {
        let home = tempfile::TempDir::new().unwrap();
        let repo = TestRepo::new();
        repo.create_file("scratch/.gitkeep", "");
        repo.stage_all();
        repo.commit("track empty scratch");
        repo.push();
        repo.create_branch("feature");
        repo.create_file("feature.txt", "reviewed change");
        repo.stage_all();
        repo.commit("reviewed change");
        repo.push_new_branch("feature");
        let published_head = repo.head_sha();
        if changed_head {
            repo.create_file("feature.txt", "new behavior");
            repo.stage_all();
            repo.commit("change the behavior");
        }
        let log_path = home.path().join("gh.log");
        let title_path = home.path().join("title");
        let body_path = home.path().join("body");
        let edit = format!(
            r#"if [ "$1 $2" = "pr edit" ]; then
  shift 2
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --title) printf '%s' "$2" > '{}'; shift 2 ;;
      --body) printf '%s' "$2" > '{}'; shift 2 ;;
      *) shift ;;
    esac
  done
  exit 0
fi
"#,
            title_path.display(),
            body_path.display()
        );
        let script = gh_existing_pr_script(log_path.to_str().unwrap())
            .replace("git rev-parse HEAD", "git rev-parse @{upstream}")
            .replace(
                r#"\"state\":\"OPEN\""#,
                r#"\"state\":\"OPEN\",\"title\":\"reviewed title\",\"body\":\"Reviewed behavior. 272 tests passed.\""#,
            )
            .replace("if [ \"$1 $2\" = \"pr view\" ]; then", &format!("{edit}if [ \"$1 $2\" = \"pr view\" ]; then"));
        let provider = if changed_head {
            agent_script()
        } else {
            "#!/bin/sh\necho 'published copy needs no provider' >&2\nexit 71\n".to_string()
        };
        let _env = EnvGuard::with_home(
            &[("gh", script.as_str()), ("codex", provider.as_str())],
            Some(home.path()),
        );
        let options = LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: body_override.map(str::to_string),
            agent: Some("codex".to_string()),
        };
        if manual_merge {
            submit(repo.path(), &options, &NullProgress).unwrap();
        } else {
            land(repo.path(), &options, &NullProgress).unwrap();
        }
        assert_eq!(
            fs::read_to_string(title_path).unwrap(),
            if changed_head {
                "generated title"
            } else {
                "reviewed title"
            }
        );
        assert_eq!(
            fs::read_to_string(body_path).unwrap(),
            body_override.unwrap_or(if changed_head {
                "generated body"
            } else {
                "Reviewed behavior. 272 tests passed."
            })
        );
        if !changed_head {
            assert_eq!(repo.head_sha(), published_head);
        }
    }
}

#[test]
fn land_preserves_checkpoint_history_and_pushes_the_final_tree_once() {
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
            pr_title: Some("merged change".to_string()),
            pr_body: Some("proof".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("land preserved history");

    let output = |args: &[&str]| {
        let result = Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .output()
            .expect("run git proof");
        assert!(result.status.success(), "git {:?} failed", args);
        String::from_utf8_lossy(&result.stdout).trim().to_string()
    };
    assert!(
        output(&["log", "--format=%s", "origin/main..HEAD"]).contains("checkpoint: first slice")
    );
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
    // GitHub owns the squash. Exercise that Git operation with the published tree.
    let final_tree = output(&["rev-parse", "HEAD^{tree}"]);
    let original_main = output(&["rev-parse", "main"]);
    output(&["checkout", "main"]);
    output(&["merge", "--squash", "feature"]);
    output(&["commit", "-m", "Squash PR"]);
    assert_eq!(
        output(&["rev-list", "--count", &format!("{original_main}..HEAD")]),
        "1"
    );
    assert_eq!(output(&["rev-parse", "HEAD^{tree}"]), final_tree);
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
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("no authored changes remain") && message.contains("empty")),
        "an empty final integration must fail before push: {result:?}"
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
        "GitHub must not be mutated for an empty integration: {gh_calls}"
    );
}

#[test]
fn land_missing_pr_error_includes_branch_name() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_home(
        &[
            ("gh", &gh_no_pr_script()),
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

    // submit prepares the PR for the reviewer to merge — and presents nothing.
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

    // submit prepares but never merges — that click belongs to the reviewer.
    let log = fs::read_to_string(&log_path).expect("read gh log");
    // Assigns the PR to the current user for a required, manual merge.
    assert!(log.contains("pr edit --add-assignee @me"));
    // Marks the PR ready, but does NOT arm auto-merge.
    assert!(log.contains("pr ready"));
    assert!(!log.contains("merge --auto"));
}

/// Managed Task delivery is independent of the end-to-end controller. An
/// explicit submit records a user-owned merge request and stops before merge.
#[test]
fn submit_records_user_merge_for_a_managed_task() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-managed-submit";
    repo.create_branch(branch);
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, branch);

    let log_path = repo.bare_path().join("gh.log");
    let script = gh_land_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );
    let task = register_task(home.path(), repo.path(), branch, &base);

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
    .expect("managed Task submits for review");
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    let request = pr.merge_request().expect("user merge request");
    assert_eq!(request.mode, PrMergeMode::User);

    let log = fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        log.contains("pr ready") && log.contains("pr edit --add-assignee @me"),
        "submit must prepare the Task PR for review: {log}"
    );
    assert!(!log.contains("merge --auto"));
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
            draft: false,
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
    let copy = preserved
        .publication
        .as_ref()
        .and_then(|publication| publication.presentation.as_ref())
        .expect("refreshed reviewer copy");
    assert_eq!(copy.title, "refresh published PR");
    assert!(copy.body.starts_with("same head\n\n<!--"));
    assert!(copy.body.contains("Merging PR 1 completes the Task."));
    assert!(!copy.body.contains("no Task settlement is requested"));
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
    assert_eq!(presentation.title, "test title");
    assert!(presentation.body.starts_with(
        "test body\n\n<!-- loopflow:task-pr-context:start -->\n> [!NOTE]\n> **Task:** [Prove Task PR transitions · INF-123](https://linear.app/loopflow/issue/INF-123/prove-task-pr-transitions)"
    ));
    assert!(!presentation.body.contains("Task cycle:"));
    assert!(presentation.body.contains(
        "> **PR lifecycle:** Merging PR 1 leaves the Task open and names `follow-up-proof` as the next serial PR."
    ));
    assert_eq!(publication.github.map(|pr| pr.number), Some(912));
    let merge = publication.merge.expect("explicit merge request");
    assert_eq!(merge.mode, PrMergeMode::Auto);
    assert_eq!(merge.head_sha, revised_head);
    assert_eq!(merge.after_merge, AfterMerge::ContinueTask);
    assert_eq!(merge.next_slug.as_deref(), Some("follow-up-proof"));
    create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
            title: Some(presentation.title.clone()),
            body: Some(presentation.body.clone()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("refresh continuing PR with its existing managed context");
    let refreshed = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .unwrap()
        .unwrap();
    let refreshed_copy = refreshed
        .publication
        .as_ref()
        .and_then(|publication| publication.presentation.as_ref())
        .unwrap();
    assert_eq!(refreshed_copy, presentation);
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
fn non_task_land_preserves_the_queued_head_and_requested_copy() {
    let home = tempfile::TempDir::new().unwrap();
    let repo = TestRepo::new();
    let log_path = home.path().join("gh.log");
    let copy = format!(
        r#"if [ "$1 $2" = "pr edit" ]; then
  shift 2
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --title) printf '%s' "$2" > '{}/title'; shift 2 ;;
      --body) printf '%s' "$2" > '{}/body'; shift 2 ;;
      *) shift ;;
    esac
  done
  exit 0
fi
"#,
        home.path().display(),
        home.path().display(),
    );
    let script = gh_existing_pr_script(log_path.to_string_lossy().as_ref())
        .replace("git rev-parse HEAD", "git rev-parse @{upstream}")
        .replacen(
            "if [ \"$1 $2\" = \"pr list\" ]; then",
            &format!("{copy}if [ \"$1 $2\" = \"pr list\" ]; then"),
            1,
        );
    let _env = EnvGuard::with_lf_home(&[("gh", script.as_str())], home.path());
    repo.create_branch("jack/queued-replay");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    repo.push_new_branch("jack/queued-replay");
    let head = repo.head_sha();
    let queue = log_path.with_extension("log.queued");
    fs::write(&queue, "queued").unwrap();
    fs::write(home.path().join("title"), "existing title").unwrap();
    fs::write(home.path().join("body"), "existing body").unwrap();
    let hook = repo.bare_path().join("hooks/pre-receive");
    fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();

    let mut options = LandOptions {
        strict: true,
        local: false,
        create_pr: false,
        complete: false,
        next_slug: None,
        worktree: None,
        commit_message: None,
        pr_title: Some("revised title".to_string()),
        pr_body: None,
        agent: None,
    };
    land(repo.path(), &options, &NullProgress).unwrap();
    assert_eq!(
        fs::read_to_string(home.path().join("title")).unwrap(),
        "revised title"
    );
    assert_eq!(
        fs::read_to_string(home.path().join("body")).unwrap(),
        "existing body"
    );

    options.pr_title = None;
    options.pr_body = Some("revised body".to_string());
    land(repo.path(), &options, &NullProgress).unwrap();
    options.pr_body = None;
    land(repo.path(), &options, &NullProgress).unwrap();
    assert_eq!(repo.head_sha(), head);
    assert!(queue.exists(), "replay must preserve queue membership");
    assert_eq!(
        fs::read_to_string(home.path().join("title")).unwrap(),
        "revised title"
    );
    assert_eq!(
        fs::read_to_string(home.path().join("body")).unwrap(),
        "revised body"
    );
}

#[test]
fn non_task_land_leaves_the_merge_queue_before_pushing_a_new_head() {
    assert_non_task_land_publishes_changed_source(true);
}

#[test]
fn non_task_land_publishes_dirty_source_instead_of_resuming_the_old_head() {
    assert_non_task_land_publishes_changed_source(false);
}

fn assert_non_task_land_publishes_changed_source(committed: bool) {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let log_path = home.path().join("gh.log");
    let script = gh_existing_pr_script(log_path.to_string_lossy().as_ref())
        .replace("git rev-parse HEAD", "git rev-parse @{upstream}");
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
    repo.create_file("repair.txt", "new source after the queued head");
    if committed {
        repo.stage_all();
        repo.commit("repair the queued head");
    }
    fs::write(format!("{}.queued", log_path.display()), "queued")
        .expect("seed remote auto-merge state");
    let hook = repo.bare_path().join("hooks/pre-receive");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nif [ -f '{}.queued' ]; then echo 'branch is queued' >&2; exit 1; fi\necho git-push >> '{}'\ncat >/dev/null\n",
            log_path.display(), log_path.display()
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
            strict: committed,
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

    assert!(!log_path.with_extension("log.queued").exists());
    let published = Command::new("git")
        .arg("--git-dir")
        .arg(repo.bare_path())
        .args(["rev-parse", &format!("refs/heads/{branch}")])
        .output()
        .unwrap();
    assert!(published.status.success());
    assert_eq!(
        String::from_utf8_lossy(&published.stdout).trim(),
        repo.head_sha()
    );
    assert!(log_path.with_extension("log.auto").exists());
    assert_eq!(
        fs::read_to_string(repo.path().join("repair.txt")).unwrap(),
        "new source after the queued head"
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
            ("gh", &gh_no_pr_script()),
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
fn pr_arm_publishes_without_create_flag_and_leaves_worktree_in_place() {
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
            "arm",
            "--strict",
            "--title",
            "test title",
            "--body",
            "test body",
        ])
        .current_dir(&worktree)
        .env_remove("LF_GIT_OPERATION_ID")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_PROCESS_ID")
        .env("LOOPFLOW_DIRECTIVE_FILE", &directive_path)
        .status()
        .expect("run lf pr arm");
    assert!(status.success(), "lf pr arm should succeed");
    assert!(
        remote_branch_exists(&repo, branch),
        "lf pr arm should push its branch"
    );
    let gh_log = fs::read_to_string(&log_path).expect("read gh log");
    assert!(
        gh_log.lines().any(|line| line.starts_with("pr create ")),
        "lf pr arm should create its missing PR, got: {gh_log}"
    );

    // The wave home is permanent: arm never rotates the worktree or cds away.
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

#[test]
fn lf_pr_land_waits_for_authoritative_merged_observation() {
    for (repair, blocked, awaiting_queue, merge_race, flow) in [
        (false, false, false, true, false),
        (false, false, false, false, false),
        (true, false, false, false, false),
        (true, true, false, false, false),
        (true, false, true, false, false),
        (true, false, false, false, true),
    ] {
        let repo = TestRepo::new();
        let github_remote = "https://github.com/loopflowstudio/loopflow.git";
        let local_remote = repo.bare_path().to_string_lossy().to_string();
        let status = Command::new("git")
            .args([
                "config",
                &format!("url.{local_remote}.insteadOf"),
                github_remote,
            ])
            .current_dir(repo.path())
            .status()
            .unwrap();
        assert!(status.success());
        let status = Command::new("git")
            .args(["remote", "set-url", "origin", github_remote])
            .current_dir(repo.path())
            .status()
            .unwrap();
        assert!(status.success());
        let log_path = repo.bare_path().join("watched-gh.log");
        let script = gh_watched_land_script(
            log_path.to_string_lossy().as_ref(),
            awaiting_queue,
            merge_race,
        );
        let repair_commands = r#"if [ -n "$LF_TEST_REPAIR_PROOF" ]; then
  export LF_AGENT_CALLER="$(printf '%s' "$thread_start" | python3 -c 'import json,sys; print(json.load(sys.stdin)["params"]["config"]["shell_environment_policy.set"]["LF_AGENT_CALLER"])')"
  echo repair >>"$LF_TEST_REPAIR_LAUNCHES"
  if [ "$(wc -l <"$LF_TEST_REPAIR_LAUNCHES")" -gt 1 ]; then exit 1; fi
  "$LF_TEST_BIN" sync --manual >"$LF_TEST_REBASE_LOG" 2>&1 || exit 1
  if [ "$LF_TEST_REPAIR_BLOCKED" != "1" ]; then
    git rev-parse HEAD >"$LF_TEST_REPAIR_PROOF"
  fi
fi"#;
        let codex = codex_app_server_script(
            if blocked {
                r#"{"status":"blocked","summary":"GitHub credential revoked; reconnect it before retrying."}"#
            } else {
                r#"{"status":"published","summary":"Synced the linked worktree; the same head can now merge."}"#
            },
            "",
        )
        .replace(
            "read -r turn_start\n",
            &format!("read -r turn_start\n{repair_commands}\n"),
        );
        let _env = EnvGuard::new(&[
            ("gh", script.as_str()),
            ("codex", codex.as_str()),
            ("open", noop_open_script()),
        ]);
        let worktree = repo.create_named_worktree("watched-land");
        fs::write(worktree.join("feature.txt"), "feature").unwrap();
        fs::create_dir_all(worktree.join(".lf")).unwrap();
        fs::write(worktree.join(".lf/config.yaml"), "agent: codex\n").unwrap();
        if flow {
            fs::create_dir_all(worktree.join(".lf/flows")).unwrap();
            fs::write(
                worktree.join(".lf/flows/repair-proof.yaml"),
                "- cmd: pr land --strict --title watched-landing --body Observe-GitHub-before-returning.\n",
            )
            .unwrap();
        }
        let status = Command::new("git")
            .args(["add", "."])
            .current_dir(&worktree)
            .status()
            .unwrap();
        assert!(status.success());
        let status = Command::new("git")
            .args(["commit", "-m", "feature work"])
            .current_dir(&worktree)
            .status()
            .unwrap();
        assert!(status.success());

        let lf_home = repo.path().join("lf-home");
        let database = lf_home.join("loopflow.db");
        initialize_landing_store(&database);
        let repair_proof = repo.path().join(".git/landing-repair-proof");
        let sync_log = repo.bare_path().join("repair-sync.log");
        let repair_launches = repo.bare_path().join("repair-launches.log");
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        if flow {
            command.args(["flow", "repair-proof", "-b", "--no-loopflow"]);
        } else {
            command.args([
                "pr",
                "land",
                "--strict",
                "--title",
                "watched landing",
                "--body",
                "Observe GitHub before returning.",
            ]);
        }
        let output = command
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    Path::new(env!("CARGO_BIN_EXE_lf"))
                        .parent()
                        .unwrap()
                        .display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .current_dir(&worktree)
            .env_remove("LF_GIT_OPERATION_ID")
            .env_remove("LF_TRACE_ID")
            .env_remove("LF_PROCESS_ID")
            .env("LF_HOME", &lf_home)
            .env("LF_DB_PATH", &database)
            .env("LF_TEST_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("LF_TEST_REBASE_LOG", &sync_log)
            .env("LF_TEST_REPAIR_LAUNCHES", &repair_launches)
            .env("LF_TEST_REPAIR_BLOCKED", if blocked { "1" } else { "0" })
            .env(
                "LF_TEST_REPAIR_PROOF",
                if repair {
                    repair_proof.as_os_str()
                } else {
                    std::ffi::OsStr::new("")
                },
            )
            .output()
            .unwrap();
        if repair {
            let db = rusqlite::Connection::open(&database).unwrap();
            let parent: (String, i64) = db
                .query_row(
                    "SELECT id,exit_code FROM execs WHERE parent_exec_id IS NULL",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(
                parent.1,
                i64::from(blocked),
                "repair={repair} blocked={blocked} flow={flow}: {}\nNested rebase: {}",
                String::from_utf8_lossy(&output.stderr),
                fs::read_to_string(&rebase_log).unwrap_or_default(),
            );
            let owners: Vec<String> = db
                .prepare("SELECT DISTINCT exec_id FROM session_events WHERE kind='started'")
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            let caller = if flow {
                let (id, via_agent): (String, bool) = db
                    .query_row(
                        "SELECT id,via_agent FROM execs WHERE parent_exec_id=?1",
                        [&parent.0],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .unwrap();
                assert!(!via_agent, "the Flow driver directly executes its step");
                id
            } else {
                parent.0.clone()
            };
            assert_eq!(
                owners.as_slice(),
                std::slice::from_ref(&caller),
                "repair uses the actual calling lf process"
            );
            assert_eq!(
                db.query_row("SELECT count(*) FROM execs", [], |row| row.get::<_, i64>(0))
                    .unwrap(),
                if flow { 3 } else { 2 },
                "only actual lf processes are Execs"
            );
            let via_agent: bool = db
                .query_row(
                    "SELECT via_agent FROM execs WHERE parent_exec_id=?1",
                    [&caller],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(via_agent, "the repair agent invokes the nested rebase");
            let completions: i64 = db
                .query_row(
                    "SELECT count(*) FROM execs WHERE id=?1 AND completed_at IS NOT NULL",
                    [&parent.0],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                completions, 1,
                "the command completes once across worker threads"
            );
            if flow {
                let merged: (String, String) = db
                    .query_row("SELECT state,merge_commit FROM pr_landings", [], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })
                    .unwrap();
                assert_eq!(merged, ("merged".into(), "merge-head".into()));
                let operations: Vec<(String, String)> = db
                    .prepare("SELECT kind,exec_id FROM flow_events ORDER BY seq")
                    .unwrap()
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap();
                assert_eq!(
                    operations,
                    [
                        ("operation_started".into(), caller.clone()),
                        ("operation_completed".into(), caller),
                    ]
                );
            }
        }
        if blocked {
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr)
                .contains("GitHub credential revoked; reconnect it before retrying."));
            assert!(!repair_proof.exists());
            assert_eq!(
                fs::read_to_string(&repair_launches)
                    .unwrap()
                    .lines()
                    .count(),
                1
            );
            continue;
        }
        assert!(
            output.status.success(),
            "lf pr land failed: {}\nNested sync: {}",
            String::from_utf8_lossy(&output.stderr),
            fs::read_to_string(&sync_log).unwrap_or_default(),
        );
        assert!(
            flow || String::from_utf8_lossy(&output.stdout).contains("merged as merge-head"),
            "land returned without merged evidence: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        if repair {
            let repaired_head =
                fs::read_to_string(&repair_proof).expect("repair wrote shared Git metadata");
            let head = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&worktree)
                .output()
                .unwrap();
            assert_eq!(
                repaired_head.trim(),
                String::from_utf8_lossy(&head.stdout).trim()
            );
            assert!(String::from_utf8_lossy(&output.stderr).contains("Synced the linked worktree"));
        }
    }
}
