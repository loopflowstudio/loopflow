use crate::support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

use loopflow::git::worktrees::create_named_worktree;
use loopflow::ops::{
    arm as land, create_or_update_pr, submit, LandOptions, NullProgress, OpsError, PrOptions,
};
use loopflow::work::task::PrMergeMode;
use loopflow_test_support::TestRepo;
use rusqlite::OptionalExtension;
use support::{
    codex_app_server_script, counting_open_script, presentation_attempts, register_task_with_pr,
    EnvGuard,
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
    let armed = support::github_merge_response(1, "fixture-head", "OPEN", "CLEAN", Some("auto"));
    format!(
        r#"#!/bin/sh
auto_state="$(dirname "$0")/auto"
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
  if [ -f "$auto_state" ]; then echo '{armed}'; else echo '{unarmed}'; fi
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

fn gh_finite_land_script(log_path: &str, merge_race: bool) -> String {
    let checks = support::github_checks_page("$head", &[("fixture-check", "FAILURE", true)]);
    let unarmed = support::github_merge_response(1, "$head", "OPEN", "CLEAN", None);
    let armed = support::github_merge_response(1, "$head", "OPEN", "blocked", Some("auto"));
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
    echo "{{\"merged\":false,\"state\":\"open\",\"mergeable_state\":\"blocked\",\"draft\":false,\"number\":1,\"html_url\":\"https://example.com/pr/1\",\"head\":{{\"sha\":\"$head\"}}}}"
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
            wait_and_fix: false,
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
            wait_and_fix: false,
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
            wait_and_fix: false,
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
fn land_cleans_added_scratch_and_retains_existing_edits() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("scratch/research.md", "original research");
    repo.create_file("scratch/untouched.md", "keep this");
    repo.create_file("scratch/renamed.md", "renamed research");
    repo.create_file("scratch/.gitkeep", "");
    repo.stage_all();
    repo.commit("Existing repository research");
    repo.push();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.create_file("scratch/research.md", "updated research");
    fs::rename(
        repo.path().join("scratch/renamed.md"),
        repo.path().join("scratch/new-name.md"),
    )
    .unwrap();
    repo.create_file("scratch/notes.md", "review notes");
    repo.create_file("scratch/nested/new [notes].md", "temporary notes");
    repo.stage_all();
    repo.commit("Feature and notes");
    push_branch(&repo, "feature");
    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            wait_and_fix: false,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(repo.path().join("scratch/research.md")).unwrap(),
        "updated research"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("scratch/untouched.md")).unwrap(),
        "keep this"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("scratch/new-name.md")).unwrap(),
        "renamed research"
    );
    assert!(repo.path().join("scratch/.gitkeep").exists());
    assert!(!repo.path().join("scratch/notes.md").exists());
    assert!(!repo.path().join("scratch/nested/new [notes].md").exists());
}

#[test]
fn final_preparation_keeps_published_history_and_merges_main_only_on_conflict() {
    // (manual merge, main advanced, GitHub reports the PR conflicting)
    for (manual_merge, advance_base, conflicting) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, true, true),
        (true, true, true),
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
        if advance_base {
            repo.checkout("main");
            repo.create_file("upstream.txt", "new upstream work");
            repo.stage_all();
            repo.commit("advance main");
            repo.push();
            repo.checkout("feature");
        }
        let gh_log = home.path().join("gh.log");
        let mut script = gh_existing_pr_script(gh_log.to_str().unwrap())
            .replace("git rev-parse HEAD", "git rev-parse @{upstream}");
        if conflicting {
            script = script.replace(
                r#""mergeStateStatus":"CLEAN""#,
                r#""mergeStateStatus":"DIRTY""#,
            );
        }
        let _env = EnvGuard::with_lf_home(&[("gh", script.as_str())], home.path());
        let options = LandOptions {
            strict: true,
            local: false,
            wait_and_fix: false,
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
        if conflicting {
            assert_ne!(repo.head_sha(), published_head);
            assert!(loopflow::git::is_ancestor(repo.path(), &published_head, "HEAD").unwrap());
            assert_eq!(
                fs::read_to_string(repo.path().join("upstream.txt")).unwrap(),
                "new upstream work"
            );
        } else {
            // A repair or re-arm on a mergeable PR keeps the base its checks ran on.
            assert_eq!(repo.head_sha(), published_head);
            assert!(!repo.path().join("upstream.txt").exists());
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
            wait_and_fix: false,
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
    repo.push();
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
            wait_and_fix: false,
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
    repo.push();
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
            wait_and_fix: false,
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
            wait_and_fix: false,
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
fn land_creates_missing_pr_without_an_extra_flag() {
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
            wait_and_fix: false,
            worktree: None,
            commit_message: None,
            pr_title: Some("cached title".to_string()),
            pr_body: Some("cached body".to_string()),
            agent: None,
        },
        &NullProgress,
    );

    assert_eq!(result.unwrap().unwrap().number, 1);
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
            wait_and_fix: false,
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
                wait_and_fix: false,
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
                wait_and_fix: false,
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
fn submit_preserves_assignments_and_skips_auto_merge() {
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
            wait_and_fix: false,
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
    // Preparing a manual merge must not change the team's assignments.
    assert!(!log.contains("--add-assignee"));
    // Marks the PR ready, but does NOT arm auto-merge.
    assert!(log.contains("pr ready"));
    assert!(!log.contains("merge --auto"));
}

#[test]
fn auto_merge_follows_repository_methods_and_queue_policy() {
    for (preference, queue, expected) in [
        (None, false, "--merge"),
        (Some("rebase"), false, "--rebase"),
        (Some("squash"), false, "rejected"),
        (Some("squash"), true, "queue"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("gh.log");
        let mut script = gh_land_script(log.to_str().unwrap()).replace(
            "\"squashMergeAllowed\":true",
            "\"squashMergeAllowed\":false",
        );
        if queue {
            script = script.replace(
                "\"isMergeQueueEnabled\":false",
                "\"isMergeQueueEnabled\":true",
            );
        }
        // The provider accepts only its configured policy and the observed head.
        let accept = if expected == "queue" {
            r#"case "$*" in *--squash*|*--merge*|*--rebase*) exit 7 ;; esac"#.to_string()
        } else {
            format!(r#"case " $* " in *" {expected} "*) ;; *) exit 7 ;; esac"#)
        };
        script = script.replace("  touch \"$auto_state\"", &format!("  {accept}\n  case \"$*\" in *\"--match-head-commit $(git rev-parse HEAD)\"*) ;; *) exit 7 ;; esac\n  touch \"$auto_state\""));
        let _env = EnvGuard::new(&[("gh", &script)]);
        let repo = TestRepo::new();
        if let Some(preference) = preference {
            repo.create_file(".lf/config.yaml", &format!("merge_method: {preference}\n"));
            repo.stage_all();
            repo.commit("repository merge policy");
            repo.push();
        }
        repo.create_branch("feature");
        repo.create_file("feature.txt", "feature");
        repo.stage_all();
        repo.commit("feature");
        push_branch(&repo, "feature");
        let result = land(
            repo.path(),
            &LandOptions {
                strict: true,
                local: false,
                wait_and_fix: false,
                worktree: None,
                commit_message: None,
                pr_title: Some("Feature".into()),
                pr_body: Some("Useful work".into()),
                agent: None,
            },
            &NullProgress,
        );
        let armed = directory.path().join("gh.log.auto").exists();
        if expected == "rejected" {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("disabled on GitHub"));
            assert!(!armed);
        } else {
            result.unwrap();
            assert!(armed, "provider must accept policy {expected}");
        }
    }
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
    let task = register_task_with_pr(home.path(), repo.path(), branch, &base);

    submit(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            wait_and_fix: false,
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
        log.contains("pr ready") && !log.contains("--add-assignee"),
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
    let task = register_task_with_pr(home.path(), repo.path(), branch, &base);

    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            wait_and_fix: false,
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
fn same_head_publication_preserves_the_armed_merge_request() {
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
    let task = register_task_with_pr(home.path(), repo.path(), branch, &base);
    fs::write(format!("{}.auto", log_path.display()), "armed externally")
        .expect("seed external auto-merge state");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            wait_and_fix: false,
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
    assert!(copy.body.contains(
        "Record independent later work as Tasks and comments; complete the delivered outcome."
    ));
    assert!(!copy.body.contains("no Task settlement is requested"));
    let preserved = preserved.merge_request().expect("preserved merge request");
    assert_eq!(preserved.head_sha, head);
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
    let task = register_task_with_pr(home.path(), repo.path(), branch, &base);
    let options = LandOptions {
        strict: true,
        local: false,
        wait_and_fix: false,
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
        wait_and_fix: false,
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
            wait_and_fix: false,
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

    let worktree =
        create_named_worktree(repo.path(), "sub", None, &|_| {}).expect("create worktree");
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
            wait_and_fix: false,
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
            wait_and_fix: false,
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
fn lf_pr_land_returns_before_a_later_check_observes_merge() {
    for (merge_race, graph) in [(true, false), (false, false), (false, true)] {
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
        let script = gh_finite_land_script(log_path.to_string_lossy().as_ref(), merge_race);
        let _env = EnvGuard::new(&[("gh", script.as_str()), ("open", noop_open_script())]);
        let worktree = repo.create_named_worktree("watched-land");
        fs::write(worktree.join("feature.txt"), "feature").unwrap();
        fs::create_dir_all(worktree.join(".lf")).unwrap();
        fs::write(worktree.join(".lf/config.yaml"), "agent: codex\n").unwrap();
        // A ci-fix run is an ordinary skill run, which keeps its prompt log ignored.
        fs::write(worktree.join(".gitignore"), ".lf/prompts/\n").unwrap();
        if graph {
            fs::create_dir_all(worktree.join(".lf/graphs")).unwrap();
            fs::write(
                worktree.join(".lf/graphs/repair-proof.yaml"),
                "- cmd: pr land --strict --title watched-landing --body Observe-GitHub-before-returning.\n",
            )
            .unwrap();
        }
        for args in [vec!["add", "."], vec!["commit", "-m", "feature work"]] {
            let status = Command::new("git")
                .args(args)
                .current_dir(&worktree)
                .status()
                .unwrap();
            assert!(status.success());
        }

        let lf_home = repo.path().join("lf-home");
        let database = lf_home.join("loopflow.db");
        initialize_landing_store(&database);
        let command = || {
            let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
            command
                .current_dir(&worktree)
                .env_remove("LF_TRACE_ID")
                .env_remove("LF_PROCESS_ID")
                .env("LF_HOME", &lf_home);
            command
        };
        let handed_off = if graph {
            command()
                .args(["graph", "repair-proof", "--batch"])
                .output()
                .unwrap()
        } else {
            command()
                .args([
                    "pr",
                    "land",
                    "--strict",
                    "--title",
                    "finite landing",
                    "--body",
                    "Retain delivery for later checks.",
                ])
                .output()
                .unwrap()
        };
        assert_eq!(
            handed_off.status.success(),
            !graph,
            "{}",
            String::from_utf8_lossy(&handed_off.stderr)
        );
        if graph {
            assert!(String::from_utf8_lossy(&handed_off.stderr).contains("still being watched"));
        }
        let conn = rusqlite::Connection::open(&database).unwrap();
        let initial: Option<i64> = conn
            .query_row(
                "SELECT exit_code FROM processes WHERE parent_lf_process_id IS NULL",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap();
        // A Graph stopped short of its end exits 3, which no caller retries.
        // A direct land nothing references is forgotten at exit.
        assert_eq!(initial, graph.then_some(3));
        let state: String = conn
            .query_row("SELECT state FROM pr_landings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(state, "watching");
        // A later check observes delivery; only a ship process repairs.
        let output = command().args(["pr", "reconcile"]).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let repairs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ci_incidents WHERE repair_lf_process_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            repairs, 0,
            "a delivery check records failures without repairing"
        );
        let state: String = conn
            .query_row("SELECT state FROM pr_landings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(state, "merged");
        if graph {
            // The Graph stopped at its watched landing: the step's command
            // handed off and returned, and its Graph process exited without running
            // further steps. The merge resumes nothing.
            let (step, graph_process): (i64, String) = conn
                .query_row(
                    "SELECT step.exit_code,graph_process.outcome FROM processes step
                     JOIN graph_process_steps recorded ON recorded.lf_process_id=step.id
                     JOIN processes graph_process ON graph_process.id=recorded.graph_lf_process_id",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!((step, graph_process.as_str()), (0, "failed"));
        }
        assert!(
            !worktree.exists(),
            "graph={graph}, merge_race={merge_race}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!local_branch_exists(&repo, "watched-land"));
        assert!(!remote_branch_exists(&repo, "watched-land"));
    }
}

#[test]
fn persistent_submit_keeps_scratch_and_post_commit_edits() {
    let gh = gh_no_pr_script();
    let _env = EnvGuard::new(&[("gh", &gh)]);
    let repo = TestRepo::new();
    let persistent = loopflow::git::worktrees::ensure_agent_worktree(
        repo.path(),
        loopflow::git::worktrees::WorktreeSegment::parse("repo").unwrap(),
    )
    .unwrap();
    fs::write(persistent.path.join("memory.md"), "accepted\n").unwrap();
    loopflow::ops::commit_selected(&persistent.path, &["memory.md".into()], Some("Memory"))
        .unwrap();
    fs::create_dir_all(persistent.path.join("scratch/nested")).unwrap();
    fs::write(persistent.path.join("scratch/nested/plan.md"), "private\n").unwrap();
    fs::write(persistent.path.join("memory.md"), "next decision\n").unwrap();
    submit(
        &persistent.path,
        &LandOptions {
            strict: false,
            local: false,
            wait_and_fix: false,
            worktree: None,
            commit_message: None,
            pr_title: Some("Document memory".into()),
            pr_body: Some("Accepted decisions".into()),
            agent: None,
        },
        &NullProgress,
    )
    .unwrap();
    assert!(persistent.path.is_dir());
    assert_eq!(
        fs::read_to_string(persistent.path.join("scratch/nested/plan.md")).unwrap(),
        "private\n"
    );
    assert_eq!(
        fs::read_to_string(persistent.path.join("memory.md")).unwrap(),
        "next decision\n"
    );
    let committed = Command::new("git")
        .current_dir(&persistent.path)
        .args(["show", "HEAD:memory.md"])
        .output()
        .unwrap();
    assert_eq!(committed.stdout, b"accepted\n");
}

#[test]
fn waited_task_landing_repairs_beside_the_open_task_session() {
    waited_task_landing("review", false);
}

#[test]
fn waited_task_landing_completes_at_graph_end_without_another_command() {
    waited_task_landing("end", false);
}

#[test]
fn waited_task_landing_reports_a_repair_run_that_opened_no_session() {
    waited_task_landing("review", true);
}

fn waited_task_landing(destination: &str, failed_startup: bool) {
    use std::time::{Duration, Instant};

    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    push_branch(&repo, "main");
    let base = repo.head_sha();
    let remote = "https://github.com/loopflowstudio/loopflow.git";
    for args in [
        vec![
            "config".into(),
            format!("url.{}.insteadOf", repo.bare_path().display()),
            remote.into(),
        ],
        vec![
            "remote".into(),
            "set-url".into(),
            "origin".into(),
            remote.into(),
        ],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());
    }
    repo.create_branch("waited-repair");
    let worktree = repo.path().canonicalize().unwrap();
    // The graph lands, then reaches an interactive step or its end.
    let mut graph = "- cmd: graph show deliver\n- cmd: pr land --wait-and-fix --strict --title waited-repair --body Repair-without-Desktop\n".to_string();
    if destination != "end" {
        graph.push_str("- interactive: {name: demo, id: review}\n");
    }
    for (path, contents) in [
        ("feature.txt", "keep the feature"),
        (
            ".lf/config.yaml",
            "agent: codex\nplanning:\n  provider: linear\n  team: team-task-pr-tests\n",
        ),
        (".gitignore", ".lf/prompts/\n"),
        (".lf/graphs/delivery-proof.yaml", graph.as_str()),
    ] {
        let path = worktree.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    for args in [
        vec!["add", "."],
        vec!["commit", "-m", "Feature and delivery fixture"],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(&worktree)
            .status()
            .unwrap()
            .success());
    }
    let home = tempfile::tempdir().unwrap();
    let proof = home.path().join("repair-proof");
    let launches = home.path().join("repair-launches");
    let gh = gh_finite_land_script(home.path().join("gh.log").to_str().unwrap(), false)
        .replace("merge-head", "$head")
        .replace("watched-land", "waited-repair");
    let repair = r#"read -r turn_start
export LF_AGENT_CALLER="$(printf '%s' "$thread_start" | python3 -c 'import json,sys; print(json.load(sys.stdin)["params"]["config"]["shell_environment_policy.set"]["LF_AGENT_CALLER"])')"
echo repair >> "$LF_TEST_REPAIR_LAUNCHES"
sleep 12
printf 'repaired feature\n' > feature.txt
git add feature.txt && git commit -m 'Repair CI failure' >/dev/null || exit 1
"$LF_TEST_BIN" arm --strict --title waited-repair --body Repair-without-Desktop >/dev/null 2>&1 || exit 1
git rev-parse HEAD > "$LF_TEST_REPAIR_PROOF"
"#;
    let codex = codex_app_server_script(
        r#"{"status":"published","summary":"Published and armed the repaired head."}"#,
        "",
    )
    .replace("read -r turn_start\n", repair);
    let _env = EnvGuard::with_lf_home(
        &[("gh", &gh), ("codex", &codex), ("open", noop_open_script())],
        home.path(),
    );
    let launcher = home.path().join("lf-failing-repair");
    fs::write(
        &launcher,
        if failed_startup {
            "#!/bin/sh\nif [ \"$4 $5\" = 'skill ci-fix' ]; then exit 17; fi\nexec \"$LF_TEST_BIN\" \"$@\"\n"
        } else {
            "#!/bin/sh\nexec \"$LF_TEST_BIN\" \"$@\"\n"
        },
    )
    .unwrap();
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
    let fixture = register_task_with_pr(home.path(), &worktree, "waited-repair", &base);
    support::managed_codex_account(home.path());
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    // The Task's own Session stays open in the checkout for the whole landing.
    let session = loopflow::id::LfProcessId::new();
    db.execute("INSERT INTO processes(id,trace_id,pid,cwd,command,started_at) VALUES(?1,?2,?3,?4,'task-session',?5)",
        rusqlite::params![session, loopflow::id::TraceId::new(), std::process::id(), worktree.to_str().unwrap(), time::OffsetDateTime::now_utc().unix_timestamp()]).unwrap();
    let command = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lf"));
        for (name, _) in
            std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
        {
            cmd.env_remove(name);
        }
        cmd.current_dir(&worktree)
            .env("LF_HOME", home.path())
            .env("LF_BIN", &launcher)
            .env("LF_TEST_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("LF_TEST_REPAIR_PROOF", &proof)
            .env("LF_TEST_REPAIR_LAUNCHES", &launches);
        cmd
    };
    // The waiter never gives up on its own, and its run is owned by a detached
    // transport server. Reap that run's process group (the Graph and its landing
    // step) when the test returns early or an assertion fails; otherwise it
    // polls the fake `gh` forever.
    struct Waiter(std::process::Child, std::path::PathBuf);
    impl Drop for Waiter {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
            let Ok(db) = rusqlite::Connection::open(&self.1) else {
                return;
            };
            let pids: Vec<i64> = db
                .prepare("SELECT pid FROM processes WHERE completed_at IS NULL AND pid IS NOT NULL")
                .and_then(|mut rows| rows.query_map([], |row| row.get(0))?.collect())
                .unwrap_or_default();
            for pid in pids {
                // Only a process still running this build's `lf`, never a reused pid.
                let ours = Command::new("ps")
                    .args(["-o", "args=", "-p", &pid.to_string()])
                    .output()
                    .is_ok_and(|ps| {
                        String::from_utf8_lossy(&ps.stdout).starts_with(env!("CARGO_BIN_EXE_lf"))
                    });
                if !ours {
                    continue;
                }
                let pid = pid as libc::pid_t;
                // SAFETY: kill has no memory preconditions. The pid runs this
                // build's `lf` for this home, and a run's leader heads its own group.
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                    libc::kill(pid, libc::SIGKILL);
                }
            }
        }
    }
    let out = home.path().join("run.stdout");
    let err = home.path().join("run.stderr");
    // `-b` runs interactive steps headless too, so a run that must stop at
    // review is bare: it may return once its Graph registers and carry on in
    // that Graph's log.
    let mut args = vec!["task", "run", "INF-123", "delivery-proof"];
    if destination == "end" {
        args.insert(0, "-b");
    }
    let mut waiter = Waiter(
        command()
            .args(&args)
            .stdout(fs::File::create(&out).unwrap())
            .stderr(fs::File::create(&err).unwrap())
            .spawn()
            .unwrap(),
        home.path().join("loopflow.db"),
    );
    let printed = |label: &str| {
        let out = fs::read_to_string(&out).unwrap();
        out.split(label)
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .map(str::to_owned)
    };
    let read = || {
        let mut error = fs::read_to_string(&err).unwrap();
        if let Some(log) = printed("log:    ") {
            error.push_str(&fs::read_to_string(log).unwrap_or_default());
        }
        (fs::read_to_string(&out).unwrap(), error)
    };
    // The run's carrier: this process, or the bare run's Graph process's parent.
    let carrier_exit = || -> Option<i64> {
        let graph = printed("running as Graph process ")?;
        db.query_row(
            "SELECT c.exit_code FROM processes g JOIN processes c ON c.id=g.parent_lf_process_id
             WHERE g.id=?1 AND c.completed_at IS NOT NULL",
            [graph],
            |row| row.get(0),
        )
        .ok()
    };
    let wait_for = |reported: &str| {
        let deadline = Instant::now() + Duration::from_secs(60);
        while !read().1.contains(reported) {
            let (output, error) = read();
            assert!(
                Instant::now() < deadline,
                "waited Task never reported {reported:?}: {output}\n{error}"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    };
    let original_head = repo.head_sha();
    if failed_startup {
        wait_for("exited before opening a Session");
        assert!(!launches.exists());
        assert_eq!(repo.head_sha(), original_head);
        // The attempt ended; the next one backs off rather than blocking.
        let deadline = Instant::now() + Duration::from_secs(60);
        let retained = loop {
            let retained: (String, String, Option<String>) = db
                .query_row(
                    "SELECT state,requested_head_sha,merge_commit FROM pr_landings",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            if retained.0 == "repairing" || Instant::now() > deadline {
                break retained;
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        assert_eq!(retained, ("repairing".into(), original_head, None));
        return;
    }
    let deadline = Instant::now() + Duration::from_secs(120);
    let succeeded = loop {
        if let Some(code) = carrier_exit() {
            break code == 0;
        }
        if let Some(status) = waiter.0.try_wait().unwrap() {
            if printed("running as Graph process ").is_none() {
                break status.success();
            }
        }
        let (output, error) = read();
        assert!(
            Instant::now() < deadline,
            "waited Task did not finish: {output}\n{error}"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    let (output, error) = read();
    assert!(succeeded, "{output}\n{error}");
    assert_eq!(fs::read_to_string(&launches).unwrap().lines().count(), 1);
    let session_open: bool = db
        .query_row(
            "SELECT completed_at IS NULL FROM processes WHERE id=?1",
            [&session],
            |row| row.get(0),
        )
        .unwrap();
    assert!(session_open, "the repair ran beside the open Session");
    let repaired_head = fs::read_to_string(&proof).unwrap();
    assert_ne!(repaired_head.trim(), original_head);
    let published = Command::new("git")
        .args(["rev-parse", "refs/heads/waited-repair"])
        .current_dir(repo.bare_path())
        .output()
        .unwrap();
    assert!(published.status.success());
    assert_eq!(
        String::from_utf8(published.stdout).unwrap().trim(),
        repaired_head.trim()
    );
    let retained: (String, String) = db
        .query_row(
            "SELECT requested_head_sha,observed_head_sha FROM pr_landings",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        retained,
        (repaired_head.trim().into(), repaired_head.trim().into())
    );
    assert!(
        format!("{output}{error}").contains("Pull request merged; Task completion can proceed"),
        "{output}{error}"
    );
    let step: String = db
        .query_row(
            "SELECT step FROM task_graphs WHERE task_id=?1",
            [fixture.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(step, destination);
    // Short of `end` the Task waits at an interactive step with nothing running.
    let expected_state = if destination == "end" {
        "done"
    } else {
        "active"
    };
    let state = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(fixture.store.task_state(&fixture.task.id))
        .unwrap();
    assert_eq!(
        state == loopflow::durable::TaskState::Done,
        destination == "end",
        "the Graph must settle completion before returning, without a status command"
    );
    let status = command()
        .args(["task", "status", "INF-123", "--json"])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(
        status["execution"]["status"], expected_state,
        "only a run reaching end authorizes Task completion"
    );
    assert_eq!(support::recorded_graphs(home.path()).len(), 1);
}

#[test]
fn waited_land_retains_intent_on_interrupt_and_finishes_only_after_merge() {
    use std::time::{Duration, Instant};

    // Reap the waiting CLI even if an assertion fails; never leave an unbounded waiter.
    struct Waiter(std::process::Child);
    impl Drop for Waiter {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    push_branch(&repo, "main");
    let base = repo.head_sha();
    let github_remote = "https://github.com/loopflowstudio/loopflow.git";
    let local_remote = repo.bare_path().to_string_lossy().to_string();
    for args in [
        vec![
            "config".to_string(),
            format!("url.{local_remote}.insteadOf"),
            github_remote.into(),
        ],
        vec![
            "remote".into(),
            "set-url".into(),
            "origin".into(),
            github_remote.into(),
        ],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());
    }
    let branch = "waited-land";
    let worktree = repo.create_named_worktree(branch);
    fs::create_dir_all(worktree.join("scratch")).unwrap();
    fs::write(worktree.join("scratch/.gitkeep"), "").unwrap();
    fs::write(worktree.join("feature.txt"), "retain until completion\n").unwrap();
    for args in [
        vec!["add", "."],
        vec!["commit", "-m", "Feature ready to land"],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(&worktree)
            .status()
            .unwrap()
            .success());
    }
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&worktree)
        .output()
        .unwrap();
    assert!(head.status.success());
    let head = String::from_utf8(head.stdout).unwrap().trim().to_string();
    let home = tempfile::tempdir().unwrap();
    let log_path = repo.bare_path().join("wait-gh.log");
    let merged_path = repo.bare_path().join("merged");
    let unarmed = support::github_merge_response(1, &head, "OPEN", "CLEAN", None);
    let armed = support::github_merge_response(1, &head, "OPEN", "CLEAN", Some("auto"));
    let merged = support::github_merge_response(1, &head, "MERGED", "UNKNOWN", None);
    let checks = support::github_checks_page(&head, &[("fixture-check", "SUCCESS", true)]);
    let script = format!(
        r#"#!/bin/sh
if [ "$1" = --version ]; then exit 0; fi
echo "$@" >> '{log}'
case "$1 $2" in
  'pr list')
    if [ -f '{log}.auto' ] && [ ! -f '{merged_path}' ]; then
      echo '[{{"url":"https://example.com/pr/1","state":"OPEN","isDraft":false,"number":1,"mergeCommit":null,"headRefOid":"{head}"}}]'
    else echo '[]'; fi ;;
  'pr create'|'pr view') echo 'https://example.com/pr/1' ;;
  'pr merge') touch '{log}.auto' ;;
  'api graphql')
    case "$*" in
      *LoopflowPrChecks*)
        echo '{checks}'
        exit 0 ;;
    esac
    if [ -f '{merged_path}' ]; then echo '{merged}';
    elif [ -f '{log}.auto' ]; then echo '{armed}';
    else echo '{unarmed}'; fi ;;
  api*)
    if [ -f '{merged_path}' ]; then
      echo '{{"merged":true,"state":"closed","draft":false,"merge_commit_sha":"{head}","merged_at":"2026-09-29T00:00:00Z","number":1,"html_url":"https://example.com/pr/1","head":{{"sha":"{head}"}}}}'
    else
      echo '{{"merged":false,"state":"open","mergeable_state":"clean","draft":false,"number":1,"html_url":"https://example.com/pr/1","head":{{"sha":"{head}"}}}}'
    fi ;;
esac
"#,
        log = log_path.display(),
        merged_path = merged_path.display(),
    );
    let _env = EnvGuard::with_lf_home(
        &[("gh", &script), ("open", noop_open_script())],
        home.path(),
    );
    let fixture = register_task_with_pr(home.path(), &worktree, branch, &base);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let database = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let command = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        for (name, _) in
            std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
        {
            command.env_remove(name);
        }
        command.current_dir(&worktree).env("LF_HOME", home.path());
        command
    };
    let pending = || {
        let output = command()
            .args(["task", "status", "INF-123", "--json"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let status = &status["execution"];
        assert_eq!(status["status"], "idle", "{status}");
        assert_eq!(
            fs::read_to_string(worktree.join("feature.txt")).unwrap(),
            "retain until completion\n"
        );
    };
    let wait_args = [
        "land",
        "--wait-and-fix",
        "--strict",
        "--title",
        "Waited landing",
        "--body",
        "Completion remains separate from merge.",
    ];
    let stdout = home.path().join("wait.stdout");
    let stderr = home.path().join("wait.stderr");
    let spawn = || {
        Waiter(
            command()
                .args(wait_args)
                .stdout(fs::File::create(&stdout).unwrap())
                .stderr(fs::File::create(&stderr).unwrap())
                .spawn()
                .unwrap(),
        )
    };
    let wait_for_open = |waiter: &mut Waiter, generation: i64| {
        let deadline = Instant::now() + Duration::from_secs(30);
        // A completed finite observation advances the generation and releases
        // its claim. Interrupt during the waiting interval, not arm/preflight.
        while !database
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pr_landings WHERE state='watching'
             AND generation>?1 AND supervisor_process_id IS NULL)",
                [generation],
                |row| row.get::<_, bool>(0),
            )
            .unwrap()
        {
            assert!(
                waiter.0.try_wait().unwrap().is_none(),
                "wait exited before observing open PR: {}",
                fs::read_to_string(&stderr).unwrap()
            );
            assert!(
                Instant::now() < deadline,
                "no prepared landing observation: {}\n{}",
                fs::read_to_string(&stdout).unwrap(),
                fs::read_to_string(&stderr).unwrap()
            );
            std::thread::sleep(Duration::from_millis(25));
        }
        // Green checks and an auto-merge request are still not a merge.
        assert!(
            waiter.0.try_wait().unwrap().is_none(),
            "wait returned while PR was open: {}",
            fs::read_to_string(&stderr).unwrap()
        );
    };
    let finish = |waiter: &mut Waiter| {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = waiter.0.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "wait did not return: {}",
                fs::read_to_string(&stderr).unwrap()
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    };

    let mut first = spawn();
    wait_for_open(&mut first, 1);
    let request = runtime
        .block_on(fixture.store.active_task_pr(&fixture.task.id))
        .unwrap()
        .unwrap()
        .merge_request()
        .cloned()
        .unwrap();
    assert_eq!(request.mode, PrMergeMode::Auto);
    assert!(Command::new("kill")
        .args(["-INT", &first.0.id().to_string()])
        .status()
        .unwrap()
        .success());
    let interrupted = finish(&mut first);
    let interrupted_stderr = fs::read_to_string(&stderr).unwrap();
    assert_eq!(
        runtime
            .block_on(fixture.store.active_task_pr(&fixture.task.id))
            .unwrap()
            .unwrap()
            .merge_request(),
        Some(&request)
    );
    pending();

    // A later observer can run after interruption, without arming or completing.
    let reconcile = command().args(["pr", "reconcile"]).output().unwrap();
    assert!(
        reconcile.status.success(),
        "{}",
        String::from_utf8_lossy(&reconcile.stderr)
    );
    pending();
    let generation: i64 = database
        .query_row("SELECT generation FROM pr_landings", [], |row| row.get(0))
        .unwrap();
    let mut retry = spawn();
    wait_for_open(&mut retry, generation);

    // Advance the real Git remote and then publish the authoritative GitHub fact.
    for args in [
        vec!["merge", "--ff-only", branch],
        vec!["push", "origin", "main"],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());
    }
    fs::write(&merged_path, &head).unwrap();
    assert!(
        finish(&mut retry).success(),
        "{}",
        fs::read_to_string(&stderr).unwrap()
    );
    assert!(fs::read_to_string(&stdout)
        .unwrap()
        .contains("merged; Task completion can proceed"));
    pending();
    let mut failures = Vec::new();
    fs::write(
        worktree.join("scratch/retained.md"),
        "keep the finishing notes",
    )
    .unwrap();
    for args in [&["pr", "reconcile"][..], &wait_args, &["land"]] {
        let output = command().args(args).output().unwrap();
        if !output.status.success() {
            failures.push(format!(
                "{args:?} exited {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        pending();
    }
    assert_eq!(
        fs::read_to_string(worktree.join("scratch/retained.md")).unwrap(),
        "keep the finishing notes"
    );
    let selected = command()
        .current_dir(repo.path())
        .args(["land", "--worktree", worktree.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        selected.status.success(),
        "{}",
        String::from_utf8_lossy(&selected.stderr)
    );
    assert!(String::from_utf8_lossy(&selected.stdout).contains("already merged"));
    // Completion is durable even when dirty notes prevent checkout cleanup.
    let completed = command()
        .args(["task", "complete", "INF-123"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&completed.stderr)
        .contains("is complete, but cleanup is incomplete"));
    assert_eq!(
        runtime
            .block_on(fixture.store.task_state(&fixture.task.id))
            .unwrap(),
        loopflow::durable::TaskState::Done
    );
    for args in [&["land"][..], &["land", "--wait-and-fix"]] {
        let output = command().args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("Task INF-123 is complete"),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    let pr = runtime
        .block_on(fixture.store.active_task_pr(&fixture.task.id))
        .unwrap()
        .unwrap();
    assert_eq!(pr.phase(), loopflow::work::task::PrPhase::Merged);
    let log = fs::read_to_string(&log_path).unwrap();
    assert_eq!(
        log.lines()
            .filter(|line| line.starts_with("pr merge "))
            .count(),
        1,
        "retry must retain the original merge request:\n{log}"
    );
    assert_eq!(
        log.lines()
            .filter(|line| line.starts_with("pr create "))
            .count(),
        1,
        "retry must retain the same PR:\n{log}"
    );
    // Check the interruption contract after recovery, so a bad exit code does
    // not hide lost intent, a removed checkout, or duplicate remote requests.
    // SIGINT uses the process-wide handler; the Graph process treats 130 as stopped.
    if interrupted.code() != Some(130) {
        failures.push(format!(
            "SIGINT exited {:?}, expected stopped (130): {interrupted_stderr}",
            interrupted.code()
        ));
    }
    if !interrupted_stderr.contains("merge intent retained") {
        failures.push("SIGINT did not report retained merge intent".into());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
