//! End-to-end proof for W2-138 and W2-255 — every Task PR contains only that
//! Task's work, and ambiguous ancestry refusal is actionable.
//!
//! The verdict logic lives in `ops::task::verify_task_pr_range` and is unit
//! tested there. These tests drive the *real* `submit`/`land` publication path
//! over a bare-origin fixture to prove the observable acceptance property the
//! design demands: a contaminated range is refused **before any push or
//! `gh pr`**, and a stale base heals so GitHub's range, `lf diff --files`, and the recorded `base_commit` agree. W2-255 extends the proof
//! matrix to divergent ancestry (both sides named), squash-merged parents,
//! and no-remote refusal.

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

use loopflow::ops::{
    arm as land, create_or_update_pr, submit, sync_with_recovery, LandOptions, NullProgress,
    PrOptions, SyncOptions,
};
use loopflow::work::task::{GithubPr, PrMergeMode, PrMergeRequest, PrPresentation, PrPublication};
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard};
use time::OffsetDateTime;

fn land_options(create_pr: bool, pr_title: &str) -> LandOptions {
    LandOptions {
        strict: true,
        local: false,
        create_pr,
        wait_and_fix: false,
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
    let unarmed = support::github_merge_response(925, "fixture-head", "OPEN", "CLEAN", None);
    let armed = support::github_merge_response(925, "fixture-head", "OPEN", "CLEAN", Some("auto"));
    format!(
        r#"#!/bin/sh
auto_state="{log_path}.auto"
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "{log_path}"
if [ "$1 $2" = "pr list" ]; then
  head=$(git rev-parse HEAD)
  printf '[{{"url":"https://example.com/pr/925","state":"OPEN","isDraft":false,"number":925,"mergeCommit":null,"headRefOid":"%s"}}]\n' "$head"
  exit 0
fi
if [ "$1 $2" = "api graphql" ]; then
  if [ -f "$auto_state" ]; then echo '{armed}'; else echo '{unarmed}'; fi
  exit 0
fi
if [ "$1 $2" = "pr view" ]; then
  echo 'https://example.com/pr/925'
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

fn gh_auto_enabled_script(log_path: &str) -> String {
    let armed = support::github_merge_response(912, "fixture-head", "OPEN", "CLEAN", Some("auto"));
    format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "{log_path}"
if [ "$1 $2" = "api graphql" ]; then
  echo '{armed}'
  exit 0
fi
if [ "$1 $2 $3 $4" = "pr merge 912 --disable-auto" ]; then
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

/// A Task PR's recorded base sits behind the
/// current `origin/main` because a sibling landed. `land` syncs, heals the
/// base to the true fork point, and publishes a minimal range — proving the
/// three views (recorded base, `lf diff --files`, GitHub range) agree.
#[test]
fn task_pr_heals_stale_base_and_aligns_the_three_views() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new(); // origin/main = P
    let stale_base = repo.head_sha(); // the base recorded at placement time

    let log_path = home.path().join("gh.log");
    let script = gh_open_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    // The Task PR's own commit, cut from the (soon stale) base and pushed.
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    repo.create_file("task.txt", "Task PR work\n");
    repo.stage_all();
    repo.commit("Task PR commit");
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

    land(repo.path(), &land_options(false, "Task PR"), &NullProgress)
        .expect("stale base heals and lands");

    // The recorded base healed forward to the current origin tip.
    let runtime = tokio::runtime::Runtime::new().expect("read task runtime");
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert_eq!(
        pr.base_commit, advanced,
        "the stale base must heal forward to origin/main"
    );

    // The three views agree. The recorded base is exactly the fork point
    // GitHub would compute for the PR range — proving `lf diff --files`
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
        range_commits.contains("Task PR commit"),
        "the Task's original commit must remain in the branch, got:\n{range_commits}"
    );
    let files = git_out(&repo, &["diff", "--name-only", &range]);
    assert!(
        files.contains("task.txt") && !files.contains("upstream.txt"),
        "the aligned range must show this Task's file and never the upstream file, got:\n{files}"
    );
}

#[test]
fn failed_sync_push_does_not_advance_the_recorded_task_base() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let stale_base = repo.head_sha();
    let _env = EnvGuard::with_lf_home(&[], home.path());

    let branch = "jack/rejected-sync-push";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");
    repo.push_new_branch(branch);

    repo.checkout("main");
    repo.create_file("upstream.txt", "landed upstream\n");
    repo.stage_all();
    repo.commit("upstream advance");
    repo.push();
    let target = repo.head_sha();
    repo.checkout(branch);

    let task = register_task(home.path(), repo.path(), branch, &stale_base);
    let hook = repo.bare_path().join("hooks/pre-receive");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nwhile read old new ref; do\n  if [ \"$ref\" = \"refs/heads/{branch}\" ]; then exit 1; fi\ndone\nexit 0\n"
        ),
    )
    .expect("write rejecting hook");
    let mut permissions = fs::metadata(&hook).expect("hook metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&hook, permissions).expect("make hook executable");

    let error = sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: true,
            fork_base: None,
        },
        &NullProgress,
    )
    .expect_err("the remote must reject the rewritten branch");
    assert!(
        error.to_string().contains("git push --force-with-lease"),
        "expected the push failure, got: {error}"
    );
    assert!(
        loopflow::engine::git::is_ancestor(repo.path(), &target, &repo.head_sha()).unwrap(),
        "the local sync must complete before the rejected push"
    );

    let runtime = tokio::runtime::Runtime::new().expect("read task runtime");
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert_eq!(
        pr.base_commit, stale_base,
        "a failed remote postcondition must not advance durable Task metadata"
    );
}

fn recorded_base(task: &support::RegisteredTask) -> String {
    let runtime = tokio::runtime::Runtime::new().expect("read task runtime");
    runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR")
        .base_commit
}

fn sync_onto(repo: &TestRepo, onto: &str) {
    sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: onto.to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .unwrap_or_else(|error| panic!("sync onto {onto} failed: {error}"));
}

/// A Task branch whose remote holds a commit the checkout lacks, with the
/// base recorded at the fork point from main.
fn task_behind_its_own_remote(repo: &TestRepo, branch: &str) -> String {
    let base = repo.head_sha();
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");
    repo.create_file("remote.txt", "pushed from elsewhere\n");
    repo.stage_all();
    repo.commit("remote-only commit");
    repo.push_new_branch(branch);
    git_out(repo, &["reset", "--hard", "HEAD~1"]);
    repo.create_file("local.txt", "local follow-up\n");
    repo.stage_all();
    repo.commit("local commit");
    base
}

/// Integrating the PR's own remote branch brings its commits in and leaves the
/// range measured from main: those commits are the PR's work, not its base.
#[test]
fn sync_onto_the_prs_own_remote_branch_keeps_the_recorded_base() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let branch = "jack/own-remote-sync";
    let base = task_behind_its_own_remote(&repo, branch);
    let task = register_task(home.path(), repo.path(), branch, &base);

    sync_onto(&repo, &format!("origin/{branch}"));

    assert!(repo.path().join("remote.txt").exists());
    assert_eq!(recorded_base(&task), base);
    let files = git_out(&repo, &["diff", "--name-only", &format!("{base}..HEAD")]);
    assert_eq!(files, "local.txt\nremote.txt\ntask.txt");
}

/// The incident's stored shape: an earlier sync onto the PR's own remote branch
/// recorded that branch's tip as the base. Syncing onto main returns the base
/// to the fork point without rewriting or dropping any commit.
#[test]
fn sync_onto_main_recovers_a_base_recorded_at_the_prs_own_remote_tip() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let branch = "jack/own-tip-base";
    task_behind_its_own_remote(&repo, branch);
    let own_tip = git_out(&repo, &["rev-parse", &format!("origin/{branch}")]);
    git_out(&repo, &["merge", "--no-edit", &own_tip]);
    let head_before = repo.head_sha();

    repo.checkout("main");
    repo.create_file("upstream.txt", "landed upstream\n");
    repo.stage_all();
    repo.commit("upstream advance");
    repo.push();
    let advanced = repo.head_sha();
    repo.checkout(branch);
    let task = register_task(home.path(), repo.path(), branch, &own_tip);

    sync_onto(&repo, "origin/main");

    assert_eq!(recorded_base(&task), advanced);
    assert!(
        loopflow::engine::git::is_ancestor(repo.path(), &head_before, &repo.head_sha()).unwrap(),
        "recovery must keep every commit the branch had"
    );
    let files = git_out(
        &repo,
        &["diff", "--name-only", &format!("{advanced}..HEAD")],
    );
    assert_eq!(files, "local.txt\nremote.txt\ntask.txt");
}

/// A base carrying another branch's commits was never this PR's own remote
/// tip, so syncing onto main still refuses it and leaves the record alone.
#[test]
fn sync_onto_main_refuses_a_base_carrying_another_branchs_commits() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let _env = EnvGuard::with_lf_home(&[], home.path());

    repo.create_branch("jack/sibling");
    repo.create_file("sibling.txt", "sibling work\n");
    repo.stage_all();
    repo.commit("sibling commit");
    repo.push_new_branch("jack/sibling");
    let foreign = repo.head_sha();

    let branch = "jack/cut-from-sibling";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");
    repo.push_new_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &foreign);

    let error = sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .expect_err("a foreign base must refuse");
    let message = error.to_string();
    assert!(
        message.contains("contaminated") && message.contains("sibling commit"),
        "expected the foreign commit named, got: {message}"
    );
    assert_eq!(recorded_base(&task), foreign);
}

#[test]
fn sync_revokes_auto_before_force_pushing_a_new_task_head() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let stale_base = repo.head_sha();
    let log_path = home.path().join("sync.log");
    let script = gh_auto_enabled_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(&[("gh", script.as_str())], home.path());

    let branch = "jack/sync-revokes-auto";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");
    repo.push_new_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &stale_base);
    let old_head = repo.head_sha();
    let now = OffsetDateTime::now_utc();
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: now,
        presentation: Some(PrPresentation {
            title: "Sync proof".to_string(),
            body: "Reviewer context".to_string(),
            head_sha: old_head.clone(),
        }),
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: Some(old_head.clone()),
        }),
        merge: Some(PrMergeRequest {
            mode: PrMergeMode::Auto,
            requested_at: now,
            head_sha: old_head,
        }),
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("store Auto request");

    repo.checkout("main");
    repo.create_file("upstream.txt", "upstream advance\n");
    repo.stage_all();
    repo.commit("upstream advance");
    repo.push();
    repo.checkout(branch);

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

    sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: true,
            fork_base: None,
        },
        &NullProgress,
    )
    .expect("sync and push Task head");

    let persisted = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert!(persisted.merge_request().is_none());
    let log = fs::read_to_string(log_path).expect("read operation log");
    let disable = log
        .find("pr merge 912 --disable-auto")
        .expect("Auto is revoked");
    let push = log.find("git-push").expect("synced head is pushed");
    assert!(
        disable < push,
        "Auto must be revoked before sync push:\n{log}"
    );
}

/// Publication is not an integration boundary. A branch behind origin is
/// pushed unchanged, its recorded fork remains authoritative, and a later
/// explicit sync owns the integration.
#[test]
fn publish_uses_managed_worktree_even_with_unknown_ambient_run() {
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

    // origin/main advances past the recorded base. Publication must not sync.
    repo.checkout("main");
    repo.create_file("upstream.txt", "landed upstream\n");
    repo.stage_all();
    repo.commit("upstream advance");
    repo.push();
    repo.checkout(branch);
    let before_publish = repo.head_sha();

    let task = register_task(home.path(), repo.path(), branch, &stale_base);
    std::env::set_var("LF_CAPTURE_KEY", "run_00000000000000000000000000000000");

    create_or_update_pr(
        repo.path(),
        &PrOptions {
            draft: false,
            title: Some("publish heal".to_string()),
            body: Some("proof body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("publish without integration");

    assert_eq!(
        repo.head_sha(),
        before_publish,
        "a clean publication must not rewrite local HEAD"
    );
    let runtime = tokio::runtime::Runtime::new().expect("read task runtime");
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert_eq!(
        pr.base_commit, stale_base,
        "publication must not advance the recorded integration base"
    );
    let publication = pr.publication.as_ref().expect("adopted publication");
    assert_eq!(publication.github.as_ref().unwrap().number, 925);
    assert_eq!(
        publication.presentation.as_ref().unwrap().head_sha,
        before_publish
    );
    assert!(publication.merge.is_none());
    let files = git_out(
        &repo,
        &["diff", "--name-only", &format!("{}..HEAD", pr.base_commit)],
    );
    assert!(
        files.contains("task.txt") && !files.contains("upstream.txt"),
        "the original authored range must remain intact, got:\n{files}"
    );
    assert!(
        !std::path::Path::new(&git_out(&repo, &["rev-parse", "--absolute-git-dir"]))
            .join("loopflow/rebase-owner.json")
            .exists(),
        "publication must not create sync ownership state"
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

/// The core hole W2-254 closes: an existing PR (already has a GitHub number)
/// that is reset or synced empty must refuse before any `gh pr` mutation. The
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
            .active_task_pr(&task.task.id)
            .await
            .expect("read active PR")
            .expect("active PR exists");
        pr.publication = Some(PrPublication {
            requested_at: OffsetDateTime::now_utc(),
            presentation: None,
            github: Some(GithubPr {
                number: 925,
                url: "https://example.com/pr/925".to_string(),
                head_sha: None,
            }),
            merge: None,
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

#[test]
fn completed_merge_updates_recorded_base_for_publish_submit_and_land() {
    for operation in ["publish", "submit", "land"] {
        let home = tempfile::tempdir().unwrap();
        let repo = TestRepo::new();
        repo.create_file("scratch/.gitkeep", "");
        repo.stage_all();
        repo.commit("Track empty scratch");
        repo.push();
        let base = repo.head_sha();
        let log = home.path().join("gh.log");
        let script = gh_open_pr_script(log.to_str().unwrap());
        let _env = EnvGuard::with_lf_home(
            &[("gh", &script), ("open", noop_open_script())],
            home.path(),
        );
        let branch = "jack/completed-merge";
        repo.create_branch(branch);
        repo.create_file("task.txt", "Task work\n");
        repo.stage_all();
        repo.commit("Task work");
        let authored = repo.head_sha();
        let task = register_task(home.path(), repo.path(), branch, &base);
        repo.checkout("main");
        repo.create_file("upstream.txt", "Main advances\n");
        repo.stage_all();
        repo.commit("Upstream work");
        repo.push();
        let advanced = repo.head_sha();
        repo.checkout(branch);
        git_out(
            &repo,
            &["merge", "--no-ff", "-m", "Completed merge", "origin/main"],
        );
        let merged = repo.head_sha();
        let options = land_options(false, "completed merge");
        match operation {
            "publish" => create_or_update_pr(
                repo.path(),
                &PrOptions {
                    draft: false,
                    title: options.pr_title.clone(),
                    body: options.pr_body.clone(),
                    agent: None,
                },
                &NullProgress,
            )
            .map(|_| ()),
            "submit" => submit(repo.path(), &options, &NullProgress).map(|_| ()),
            "land" => land(repo.path(), &options, &NullProgress).map(|_| ()),
            _ => unreachable!(),
        }
        .unwrap_or_else(|error| panic!("{operation} rejected a completed merge: {error}"));
        assert_eq!(
            repo.head_sha(),
            merged,
            "{operation} changed the completed merge"
        );
        assert_eq!(
            git_out(&repo, &["show", "-s", "--format=%P", "HEAD"]),
            format!("{authored} {advanced}")
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let pr = runtime
            .block_on(task.store.active_task_pr(&task.task.id))
            .unwrap()
            .unwrap();
        assert_eq!(
            pr.base_commit, advanced,
            "{operation} left the recorded base stale"
        );
        assert_eq!(
            git_out(
                &repo,
                &["diff", "--name-only", &format!("{}..HEAD", pr.base_commit)]
            ),
            "task.txt"
        );
        assert_eq!(
            git_out(&repo, &["rev-parse", &format!("origin/{branch}")]),
            merged
        );
    }
}
