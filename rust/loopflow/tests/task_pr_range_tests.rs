//! End-to-end proof for W2-138 — every Task PR contains only that Task's work.
//!
//! The verdict logic lives in `ops::task::verify_task_pr_range` and is unit
//! tested there. These tests drive the *real* `submit`/`land` publication path
//! over a bare-origin fixture to prove the observable acceptance property the
//! design demands: a contaminated range is refused **before any push or
//! `gh pr`**, and a stale serial base heals so GitHub's range, `lf task
//! changes`, and the recorded `base_commit` agree.

mod support;

use std::fs;
use std::process::{Command, Stdio};

use loopflow::ops::{land, submit, LandOptions, NullProgress};
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard};

fn land_options(create_pr: bool, pr_title: &str) -> LandOptions {
    LandOptions {
        strict: true,
        local: false,
        create_pr,
        complete: false,
        next_slug: None,
        worktree: None,
        commit_message: None,
        pr_title: Some(pr_title.to_string()),
        pr_body: Some("proof body".to_string()),
        agent: None,
    }
}

/// A `gh` that records every non-`--version` invocation to `log_path` and
/// reports an already-open PR, so `land` finds a PR to finalize without
/// creating one.
fn gh_open_pr_script(log_path: &str) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "{log_path}"
if [ "$1 $2" = "pr list" ]; then
  echo '[{{"url":"https://example.com/pr/925","state":"OPEN","isDraft":false,"number":925,"mergeCommit":null}}]'
  exit 0
fi
if [ "$1 $2" = "pr view" ]; then
  echo 'https://example.com/pr/925'
  exit 0
fi
exit 0
"#
    )
}

fn noop_open_script() -> &'static str {
    "#!/bin/sh\nexit 0\n"
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

fn git_out(repo: &TestRepo, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo.path())
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// The #877/#882 acceptance case: the recorded base carries a foreign,
/// unpushed canonical-main commit (M < B). `submit` must refuse, name the
/// foreign commit and the recovery, and touch neither the remote nor `gh pr`.
#[test]
fn submit_refuses_a_contaminated_range_before_any_push() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new(); // origin/main = P (pushed)

    let log_path = home.path().join("gh.log");
    let script = gh_open_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    // A foreign commit advances local main ahead of origin and is never pushed —
    // the branch cut from it inherits off-origin ancestry.
    repo.create_file("foreign.txt", "not this task's work\n");
    repo.stage_all();
    repo.commit("foreign canonical-main commit");
    let contaminated_base = repo.head_sha();

    let branch = "jack/contaminated";
    repo.create_branch(branch); // cut from the contaminated base
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");
    // Deliberately NOT pushed: the refusal must precede the first push.

    register_task(home.path(), repo.path(), branch, &contaminated_base);

    let err = submit(
        repo.path(),
        &land_options(true, "contaminated"),
        &NullProgress,
    )
    .expect_err("contaminated range must refuse");
    let message = err.to_string();
    assert!(
        message.contains("contaminated"),
        "expected contamination refusal, got: {message}"
    );
    assert!(
        message.contains("foreign canonical-main commit"),
        "refusal must name the foreign commit, got: {message}"
    );
    assert!(
        message.contains("rebase --onto"),
        "refusal must print the recovery action, got: {message}"
    );

    // The proof: no GitHub side effect happened before the refusal.
    assert!(
        !remote_branch_exists(&repo, branch),
        "the branch must never reach the remote when the range is refused"
    );
    let log = fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        !log.contains("pr create") && !log.contains("pr edit") && !log.contains("pr ready"),
        "no gh PR mutation may be issued before refusal, got log:\n{log}"
    );
}

/// The serial / dogfood shape: a continuation PR's recorded base sits behind the
/// current `origin/main` because a sibling landed. `land` rebases, heals the
/// base to the true fork point, and publishes a minimal range — proving the
/// three views (recorded base, `lf task changes`, GitHub range) agree.
#[test]
fn serial_pr_heals_stale_base_and_aligns_the_three_views() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new(); // origin/main = P
    let stale_base = repo.head_sha(); // the base recorded at placement time

    let log_path = home.path().join("gh.log");
    let script = gh_open_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    // The serial PR's own commit, cut from the (soon stale) base and pushed.
    let branch = "jack/serial-pr-proof";
    repo.create_branch(branch);
    repo.create_file("task.txt", "serial PR work\n");
    repo.stage_all();
    repo.commit("serial PR commit");
    repo.push_new_branch(branch);

    // A sibling lands: origin/main advances past the recorded base.
    repo.checkout("main");
    repo.create_file("upstream.txt", "landed upstream\n");
    repo.stage_all();
    repo.commit("upstream advance");
    repo.push();
    let advanced = repo.head_sha();
    repo.checkout(branch);

    let task = register_task(home.path(), repo.path(), branch, &stale_base);

    land(
        repo.path(),
        &land_options(false, "serial pr"),
        &NullProgress,
    )
    .expect("stale serial base heals and lands");

    // The recorded base healed forward to the current origin tip.
    let runtime = tokio::runtime::Runtime::new().expect("read task runtime");
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.session.id))
        .expect("read active PR")
        .expect("active PR");
    assert_eq!(
        pr.base_commit, advanced,
        "the stale serial base must heal forward to origin/main"
    );

    // The three views agree. The recorded base is exactly the fork point
    // GitHub would compute for the PR range — proving `lf task changes`
    // (base..HEAD), the GitHub range (merge-base(origin/main, HEAD)..HEAD), and
    // the recorded base all describe the same commits.
    let github_fork_point = git_out(&repo, &["merge-base", "origin/main", "HEAD"]);
    assert_eq!(
        pr.base_commit, github_fork_point,
        "recorded base must equal GitHub's range fork point"
    );

    // The already-merged upstream commit is dropped from the range, while the
    // Task's own work is included — no manual commit dropping.
    let range = format!("{}..HEAD", pr.base_commit);
    let range_commits = git_out(&repo, &["log", "--oneline", "--no-decorate", &range]);
    assert!(
        !range_commits.contains("upstream advance"),
        "the merged upstream commit must be excluded from the range, got:\n{range_commits}"
    );
    assert!(
        range_commits.contains("serial PR commit"),
        "the Task's own commit must be in the range, got:\n{range_commits}"
    );
    let files = git_out(&repo, &["diff", "--name-only", &range]);
    assert!(
        files.contains("task.txt") && !files.contains("upstream.txt"),
        "the aligned range must show this Task's file and never the upstream file, got:\n{files}"
    );
}
