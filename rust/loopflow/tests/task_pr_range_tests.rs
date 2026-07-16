//! End-to-end proof for W2-138 and W2-255 — every Task PR contains only that
//! Task's work, and ambiguous ancestry refusal is actionable.
//!
//! The verdict logic lives in `ops::task::verify_task_pr_range` and is unit
//! tested there. These tests drive the *real* `submit`/`land` publication path
//! over a bare-origin fixture to prove the observable acceptance property the
//! design demands: a contaminated range is refused **before any push or
//! `gh pr`**, and a stale serial base heals so GitHub's range, `lf task
//! changes`, and the recorded `base_commit` agree. W2-255 extends the proof
//! matrix to divergent ancestry (both sides named), squash-merged parents,
//! no-remote refusal, and serial rotation.

mod support;

use std::fs;
use std::process::{Command, Stdio};

use loopflow::ops::{create_or_update_pr, land, submit, LandOptions, NullProgress, PrOptions};
use loopflow::task::{AfterMerge, GithubPr, PrPublication};
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard};
use time::OffsetDateTime;

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

/// `lf pr publish` / `lf pr open` rebases behind-base branches onto origin, which
/// advances the fork point. Without healing, the recorded `base_commit` goes
/// stale and `lf task changes` (base..HEAD) balloons to include every commit the
/// rebase pulled in — while GitHub's range stays clean. This regression proves
/// the publish path heals the base so the two views stay aligned.
#[test]
fn publish_heals_the_recorded_base_after_rebasing_onto_origin() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new(); // origin/main = P
    let stale_base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_open_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    // The Task branch, cut from the base and pushed.
    let branch = "jack/publish-heal-proof";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");
    repo.push_new_branch(branch);

    // origin/main advances past the recorded base, so publish must rebase.
    repo.checkout("main");
    repo.create_file("upstream.txt", "landed upstream\n");
    repo.stage_all();
    repo.commit("upstream advance");
    repo.push();
    let advanced = repo.head_sha();
    repo.checkout(branch);

    let task = register_task(home.path(), repo.path(), branch, &stale_base);

    create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("publish heal".to_string()),
            body: Some("proof body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("publish rebases and heals");

    // The rebase advanced the fork point; the recorded base healed to match, so
    // lf task changes (base..HEAD) equals GitHub's range and excludes the pulled
    // upstream commit.
    let runtime = tokio::runtime::Runtime::new().expect("read task runtime");
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.session.id))
        .expect("read active PR")
        .expect("active PR");
    assert_eq!(
        pr.base_commit, advanced,
        "publish must heal the stale base to the rebased fork point"
    );
    let files = git_out(
        &repo,
        &["diff", "--name-only", &format!("{}..HEAD", pr.base_commit)],
    );
    assert!(
        files.contains("task.txt") && !files.contains("upstream.txt"),
        "the healed range must show only this Task's work, got:\n{files}"
    );
}

/// Divergent ancestry: the branch was cut from a contaminated base, then
/// rebased onto the current origin without updating the recorded base. The
/// recorded base and origin diverge from the initial commit — neither is an
/// ancestor of the other. `submit` must refuse, name the commits and files on
/// **both sides**, and touch neither the remote nor `gh pr`.
#[test]
fn submit_refuses_divergent_ancestry_naming_both_sides() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let origin_tip = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_open_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    // A foreign commit advances local main ahead of origin (not pushed).
    repo.create_file("foreign.txt", "not this task's work\n");
    repo.stage_all();
    repo.commit("foreign canonical-main commit");
    let contaminated_base = repo.head_sha();

    // Cut the task branch from the contaminated base.
    let branch = "jack/divergent-proof";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");

    // Undo the foreign commit on main and advance origin with a different
    // commit so the recorded base and origin diverge from the initial commit.
    repo.checkout("main");
    git_out(&repo, &["reset", "--hard", &origin_tip]);
    repo.create_file("upstream.txt", "landed upstream\n");
    repo.stage_all();
    repo.commit("upstream advance");
    repo.push();

    // Rebase the task branch onto the current origin, simulating a manual
    // recovery that forgot to update the recorded base.
    repo.checkout(branch);
    git_out(
        &repo,
        &[
            "rebase",
            "--onto",
            "origin/main",
            &contaminated_base,
            branch,
        ],
    );

    register_task(home.path(), repo.path(), branch, &contaminated_base);

    let err = submit(repo.path(), &land_options(true, "divergent"), &NullProgress)
        .expect_err("divergent ancestry must refuse");
    let message = err.to_string();
    assert!(
        message.contains("diverged"),
        "expected divergence refusal, got: {message}"
    );
    // Base side (M..B): the foreign commit and its file.
    assert!(
        message.contains("foreign canonical-main commit"),
        "refusal must name the base-side foreign commit, got: {message}"
    );
    assert!(
        message.contains("foreign.txt"),
        "refusal must name the base-side file, got: {message}"
    );
    // Upstream side (B..M): the upstream commit and its file.
    assert!(
        message.contains("upstream advance"),
        "refusal must name the upstream-side commit, got: {message}"
    );
    assert!(
        message.contains("upstream.txt"),
        "refusal must name the upstream-side file, got: {message}"
    );
    assert!(
        message.contains("rebase --onto"),
        "refusal must print the recovery action, got: {message}"
    );

    // No GitHub side effect happened before the refusal.
    assert!(
        !remote_branch_exists(&repo, branch),
        "the branch must never reach the remote when ancestry is refused"
    );
    let log = fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        !log.contains("pr create") && !log.contains("pr edit") && !log.contains("pr ready"),
        "no gh PR mutation may be issued before refusal, got log:\n{log}"
    );
}

/// Squash-merged parent: PR1 was squash-merged, so origin/main carries a single
/// squash commit. PR2 was cut from PR1's original tip (not the squash), so its
/// recorded base carries commits origin/main doesn't have — the contaminated
/// case. `submit` must refuse and name the pre-squash commits.
#[test]
fn submit_refuses_contaminated_range_after_squash_merged_parent() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();

    let log_path = home.path().join("gh.log");
    let script = gh_open_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    // PR1: two commits cut from the initial origin tip.
    let first_branch = "jack/pr-one";
    repo.create_branch(first_branch);
    repo.create_file("pr1-a.txt", "a\n");
    repo.stage_all();
    repo.commit("PR1 first commit");
    repo.create_file("pr1-b.txt", "b\n");
    repo.stage_all();
    repo.commit("PR1 second commit");
    let pr1_tip = repo.head_sha();

    // Squash-merge PR1 onto main and push.
    repo.checkout("main");
    git_out(&repo, &["merge", "--squash", first_branch]);
    repo.stage_all();
    repo.commit("squash-merge PR1");
    repo.push();

    // PR2 cut from PR1's original tip — the real-world mistake.
    let second_branch = "jack/pr-two";
    git_out(&repo, &["branch", second_branch, &pr1_tip]);
    repo.checkout(second_branch);
    repo.create_file("pr2.txt", "PR2 work\n");
    repo.stage_all();
    repo.commit("PR2 commit");

    register_task(home.path(), repo.path(), second_branch, &pr1_tip);

    let err = submit(
        repo.path(),
        &land_options(true, "squash-merged parent"),
        &NullProgress,
    )
    .expect_err("squash-merged parent contamination must refuse");
    let message = err.to_string();
    assert!(
        message.contains("contaminated"),
        "expected contamination refusal, got: {message}"
    );
    assert!(
        message.contains("PR1 first commit"),
        "refusal must name the first pre-squash commit, got: {message}"
    );
    assert!(
        message.contains("PR1 second commit"),
        "refusal must name the second pre-squash commit, got: {message}"
    );
    assert!(
        message.contains("rebase --onto"),
        "refusal must print the recovery action, got: {message}"
    );

    assert!(
        !remote_branch_exists(&repo, second_branch),
        "the branch must never reach the remote when the range is refused"
    );
}

/// No-remote refusal: a repo with no remote must still catch a contaminated
/// range. The verification falls back to local main; if the recorded base
/// carries commits local main doesn't have, `submit` refuses before any push.
#[test]
fn submit_refuses_contaminated_range_without_a_remote() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_open_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    // Drop the remote entirely.
    git_out(&repo, &["remote", "remove", "origin"]);

    // Advance local main with a foreign commit, cut the branch from it, then
    // reset main to the original tip — the recorded base is off-local-main.
    repo.create_file("foreign.txt", "not this task's work\n");
    repo.stage_all();
    repo.commit("foreign local-main commit");
    let contaminated_base = repo.head_sha();

    let branch = "jack/no-remote-contaminated";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");

    repo.checkout("main");
    git_out(&repo, &["reset", "--hard", &base]);
    repo.checkout(branch);

    register_task(home.path(), repo.path(), branch, &contaminated_base);

    let err = submit(
        repo.path(),
        &land_options(true, "no-remote contaminated"),
        &NullProgress,
    )
    .expect_err("no-remote contaminated range must refuse");
    let message = err.to_string();
    assert!(
        message.contains("contaminated"),
        "expected contamination refusal, got: {message}"
    );
    assert!(
        message.contains("foreign local-main commit"),
        "refusal must name the foreign commit, got: {message}"
    );
    assert!(
        message.contains("rebase --onto"),
        "refusal must print the recovery action, got: {message}"
    );
}

/// Serial rotation: a continuation PR's recorded base sits behind origin/main
/// because a sibling landed. `land` rebases, heals the base to the true fork
/// point, and publishes — proving the three views agree. This is the serial
/// rotation case from the proof matrix, exercised end-to-end through `land`.
#[test]
fn serial_rotation_heals_stale_base_and_lands_the_continuation() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let stale_base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_open_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    // The serial PR's own commit, cut from the (soon stale) base and pushed.
    let branch = "jack/serial-rotation-proof";
    repo.create_branch(branch);
    repo.create_file("task.txt", "serial rotation work\n");
    repo.stage_all();
    repo.commit("serial rotation commit");
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
        &land_options(false, "serial rotation"),
        &NullProgress,
    )
    .expect("serial rotation heals stale base and lands");

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

    // The three views agree: recorded base == GitHub fork point.
    let github_fork_point = git_out(&repo, &["merge-base", "origin/main", "HEAD"]);
    assert_eq!(
        pr.base_commit, github_fork_point,
        "recorded base must equal GitHub's range fork point"
    );

    // The Task's own commit is in the range; the upstream commit is excluded.
    let range = format!("{}..HEAD", pr.base_commit);
    let range_commits = git_out(&repo, &["log", "--oneline", "--no-decorate", &range]);
    assert!(
        !range_commits.contains("upstream advance"),
        "the merged upstream commit must be excluded, got:\n{range_commits}"
    );
    assert!(
        range_commits.contains("serial rotation commit"),
        "the Task's own commit must be in the range, got:\n{range_commits}"
    );
}

/// The core hole W2-254 closes: an existing PR (already has a GitHub number)
/// that is reset or rebased empty must refuse before any `gh pr` mutation. The
/// old `task_pr_has_changes` guard ran only when `pr.github().is_none()`; once
/// a PR had a number, an empty update sailed through to `gh pr edit`/`ready`/
/// `merge`. The shared verifier is unconditional, so `submit` on an empty
/// branch refuses before any `gh` call.
#[test]
fn submit_refuses_an_empty_range_before_any_gh_call() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_open_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    let branch = "jack/empty-range";
    repo.create_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &base);

    // Simulate a previously-published PR so the old guard's
    // `pr.github().is_none()` condition is false — the exact case it skipped.
    let runtime = tokio::runtime::Runtime::new().expect("update PR runtime");
    runtime.block_on(async {
        let mut pr = task
            .store
            .active_task_pr(&task.session.id)
            .await
            .expect("read active PR")
            .expect("active PR exists");
        pr.publication = Some(PrPublication {
            requested_at: OffsetDateTime::now_utc(),
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 925,
                url: "https://example.com/pr/925".to_string(),
                head_sha: None,
            }),
        });
        pr.updated_at = OffsetDateTime::now_utc();
        task.store
            .update_task_pr(&pr)
            .await
            .expect("set github number");
    });

    let err = submit(
        repo.path(),
        &land_options(true, "empty range"),
        &NullProgress,
    )
    .expect_err("empty range must refuse");
    assert!(
        err.to_string().contains("empty"),
        "expected empty-range refusal, got: {err}"
    );

    // No GitHub side effect happened before the refusal.
    let log = fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        !log.contains("pr create") && !log.contains("pr edit") && !log.contains("pr ready"),
        "no gh PR mutation may be issued for an empty range, got log:\n{log}"
    );
}
