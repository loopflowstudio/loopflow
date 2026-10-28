//! Publication resolves the current Git range without rewriting branch history.
//! Bare origins and simulated GitHub keep the command proof disposable.

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
use support::{register_task_with_pr, EnvGuard};
use time::OffsetDateTime;

fn land_options(pr_title: &str) -> LandOptions {
    LandOptions {
        strict: true,
        local: false,
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

/// A Task PR's recorded base sits behind the current `origin/main` because a
/// sibling landed and GitHub reports the PR behind. `land` syncs, heals the
/// base to the true fork point, and publishes a minimal range — proving the
/// three views (recorded base, `lf diff --files`, GitHub range) agree.
#[test]
fn task_pr_heals_stale_base_and_aligns_the_three_views() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new(); // origin/main = P
    let stale_base = repo.head_sha(); // the base recorded at placement time

    let log_path = home.path().join("gh.log");
    // GitHub reports the PR behind main, so `land` merges main before arming.
    let script = gh_open_pr_script(log_path.to_string_lossy().as_ref()).replace(
        r#""mergeStateStatus":"CLEAN""#,
        r#""mergeStateStatus":"BEHIND""#,
    );
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

    let task = register_task_with_pr(home.path(), repo.path(), branch, &stale_base);

    land(repo.path(), &land_options("Task PR"), &NullProgress).expect("stale base heals and lands");

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

    let task = register_task_with_pr(home.path(), repo.path(), branch, &stale_base);
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
        error.to_string().contains("push --force-with-lease"),
        "expected the push failure, got: {error}"
    );
    assert!(
        loopflow::git::is_ancestor(repo.path(), &target, &repo.head_sha()).unwrap(),
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
    let task = register_task_with_pr(home.path(), repo.path(), branch, &base);

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
    let task = register_task_with_pr(home.path(), repo.path(), branch, &own_tip);

    sync_onto(&repo, "origin/main");

    assert_eq!(recorded_base(&task), advanced);
    assert!(
        loopflow::git::is_ancestor(repo.path(), &head_before, &repo.head_sha()).unwrap(),
        "recovery must keep every commit the branch had"
    );
    let files = git_out(
        &repo,
        &["diff", "--name-only", &format!("{advanced}..HEAD")],
    );
    assert_eq!(files, "local.txt\nremote.txt\ntask.txt");
}

/// Sync retains inherited dependencies and measures the complete current range.
#[test]
fn sync_onto_main_preserves_inherited_commits() {
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
    let task = register_task_with_pr(home.path(), repo.path(), branch, &foreign);

    let head = repo.head_sha();
    sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .expect("inherited commits remain publishable");
    assert_eq!(repo.head_sha(), head);
    assert_eq!(
        recorded_base(&task),
        git_out(&repo, &["merge-base", "origin/main", "HEAD"])
    );
    assert!(repo.path().join("sibling.txt").exists());
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
    let task = register_task_with_pr(home.path(), repo.path(), branch, &stale_base);
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

    let task = register_task_with_pr(home.path(), repo.path(), branch, &stale_base);
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

/// The core hole W2-254 closes: an existing PR (already has a GitHub number)
/// that is reset or synced empty must refuse before any `gh pr` mutation. The
/// old `task_pr_has_changes` guard ran only when `pr.github().is_none()`; once
/// a PR had a number, an empty update sailed through to `gh pr edit`/`ready`/
/// `merge`. The shared verifier is unconditional, so `submit` on an empty
/// branch refuses before any `gh` call.
#[test]
fn submit_refuses_an_empty_range_before_any_gh_call() {
    for empty_commit in [false, true] {
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
        if empty_commit {
            git_out(&repo, &["commit", "--allow-empty", "-m", "No tree changes"]);
        }
        let task = register_task_with_pr(home.path(), repo.path(), branch, &base);

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

        let err = submit(repo.path(), &land_options("empty range"), &NullProgress)
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
        let task = register_task_with_pr(home.path(), repo.path(), branch, &base);
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
        let options = land_options("completed merge");
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

/// The incident's old prototype base can survive a reset/rebase, or upstream
/// can move while the branch still depends on it. Neither requires rewriting.
#[test]
fn pr_open_resolves_stale_bases_without_rewriting_history() {
    for scenario in [
        "rebased",
        "inherited",
        "rewritten-main",
        "squashed",
        "missing",
    ] {
        let home = tempfile::tempdir().unwrap();
        let repo = TestRepo::new();
        let initial = repo.head_sha();
        let log = home.path().join("gh.log");
        let script = if scenario == "rebased" {
            format!(
                r#"#!/bin/sh
echo "$@" >> "{}"
case "$1 $2" in
  "pr list") echo '[]' ;;
  "pr create") echo 'https://example.com/pr/926' ;;
esac
"#,
                log.display()
            )
        } else {
            gh_open_pr_script(log.to_str().unwrap())
        };
        let _env = EnvGuard::with_lf_home(
            &[
                ("gh", &script),
                ("open", noop_open_script()),
                ("xdg-open", noop_open_script()),
            ],
            home.path(),
        );
        repo.create_branch("prototype");
        repo.create_file("dependency.txt", "prototype dependency\n");
        repo.stage_all();
        repo.commit("prototype dependency");
        let saved = repo.head_sha();
        let branch = "jack/stale-base";
        repo.create_branch(branch);
        repo.create_file("task.txt", "task work\n");
        repo.stage_all();
        repo.commit("task work");
        if scenario == "rewritten-main" {
            git_out(&repo, &["push", "origin", "prototype:main"]);
        }
        repo.checkout("main");
        if scenario == "squashed" {
            git_out(&repo, &["merge", "--squash", "prototype"]);
            repo.commit("squashed prototype");
        } else {
            repo.create_file("upstream.txt", "upstream work\n");
            repo.stage_all();
            repo.commit("upstream work");
        }
        if scenario == "rewritten-main" {
            git_out(&repo, &["push", "--force", "origin", "main"]);
        } else {
            repo.push();
        }
        repo.checkout(branch);
        if scenario == "rebased" {
            git_out(&repo, &["rebase", "--onto", "origin/main", &saved]);
        }
        let saved = if scenario == "missing" {
            "0".repeat(40)
        } else {
            saved
        };
        let task = register_task_with_pr(home.path(), repo.path(), branch, &saved);
        let head = repo.head_sha();
        let tree = git_out(&repo, &["rev-parse", "HEAD^{tree}"]);
        let commits = git_out(&repo, &["rev-list", "HEAD"]);
        let expected_base = git_out(&repo, &["merge-base", "origin/main", "HEAD"]);
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .current_dir(repo.path())
            .args([
                "pr",
                "open",
                "--title",
                "Stale base proof",
                "--body",
                "Preserve branch history",
            ])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let message = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{scenario}: {message}");
        assert!(!message.contains("rebase --onto"), "{message}");
        assert!(message.lines().count() < 30, "{message}");
        assert_eq!(repo.head_sha(), head, "{scenario}");
        assert_eq!(git_out(&repo, &["rev-parse", "HEAD^{tree}"]), tree);
        assert_eq!(git_out(&repo, &["rev-list", "HEAD"]), commits);
        assert_eq!(
            git_out(&repo, &["rev-parse", &format!("origin/{branch}")]),
            head
        );
        assert_eq!(recorded_base(&task), expected_base);
        let log = fs::read_to_string(log).unwrap();
        assert!(
            (log.contains("pr edit") || log.contains("pr create")) && log.contains("--base main"),
            "{log}"
        );
        assert!(!log.contains("pr merge"), "{log}");
        eprintln!("{scenario}: lf pr open succeeded; local/remote HEAD and every commit preserved; comparison matches GitHub");
        if scenario != "rebased" {
            assert!(repo.path().join("dependency.txt").exists());
            assert_eq!(expected_base, initial);
        }
    }
}

#[test]
fn pr_open_without_shared_history_keeps_work_and_reports_a_short_error() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let log = home.path().join("gh.log");
    let script = gh_open_pr_script(log.to_str().unwrap());
    let _env = EnvGuard::with_lf_home(&[("gh", &script)], home.path());
    let branch = "jack/unrelated";
    git_out(&repo, &["checkout", "--orphan", branch]);
    repo.create_file("task.txt", "keep unpublished work\n");
    repo.stage_all();
    repo.commit("unrelated work");
    register_task_with_pr(home.path(), repo.path(), branch, &base);
    let head = repo.head_sha();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .current_dir(repo.path())
        .args(["pr", "open", "--title", "Unrelated", "--body", "proof"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        message.contains("cannot resolve a PR comparison"),
        "{message}"
    );
    assert!(message.lines().count() < 10, "{message}");
    assert!(!message.contains("rebase") && !message.contains("reset"));
    assert_eq!(repo.head_sha(), head);
    assert!(!remote_branch_exists(&repo, branch));
    assert!(fs::read_to_string(log).unwrap_or_default().is_empty());
}

#[test]
fn publication_refreshes_comparison_without_a_remote() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let log = home.path().join("gh.log");
    let script = gh_open_pr_script(log.to_str().unwrap());
    let _env = EnvGuard::with_lf_home(&[("gh", &script)], home.path());
    repo.create_branch("jack/local");
    repo.create_file("dependency.txt", "dependency\n");
    repo.stage_all();
    repo.commit("dependency");
    let stale = repo.head_sha();
    repo.create_file("task.txt", "task\n");
    repo.stage_all();
    repo.commit("task");
    git_out(&repo, &["remote", "remove", "origin"]);
    let task = register_task_with_pr(home.path(), repo.path(), "jack/local", &stale);
    let head = repo.head_sha();
    let error = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("Local proof".into()),
            body: Some("proof".into()),
            draft: true,
            agent: None,
        },
        &NullProgress,
    )
    .expect_err("publication needs a remote");
    assert!(!error.to_string().contains("contaminated"));
    assert_eq!(recorded_base(&task), base);
    assert_eq!(repo.head_sha(), head);
}

#[test]
fn publishing_after_parent_squash_preserves_the_fork_for_sync() {
    let home = tempfile::tempdir().unwrap();
    let child_dir = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let log = home.path().join("gh.log");
    let script = gh_open_pr_script(log.to_str().unwrap());
    let _env = EnvGuard::with_lf_home(&[("gh", &script)], home.path());
    repo.create_branch("parent");
    repo.create_file("dependency.txt", "parent dependency\n");
    repo.stage_all();
    repo.commit("parent work");
    let fork = repo.head_sha();
    let parent = register_task_with_pr(home.path(), repo.path(), "parent", &fork);
    let child_path = child_dir.path().join("child");
    git_out(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            "child",
            child_path.to_str().unwrap(),
        ],
    );
    let child = support::register_sibling_task(&parent, "INF-124", "child", &child_path);
    // This child has no Linear mapping or provider URL; publication is local-plan work.
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .unwrap()
        .execute(
            "UPDATE tasks SET external_issue_id=NULL WHERE id=?1",
            [child.id.as_str()],
        )
        .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut pr = parent.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: pr.created_at,
        presentation: None,
        github: Some(GithubPr {
            number: 41,
            url: "https://example.com/pr/41".into(),
            head_sha: Some(fork.clone()),
        }),
        merge: None,
    });
    runtime.block_on(parent.store.update_task_pr(&pr)).unwrap();
    runtime
        .block_on(parent.store.stack_task_placement(&child, &parent.pr.id))
        .unwrap();
    fs::write(child_path.join("child.txt"), "child work\n").unwrap();
    for args in [vec!["add", "."], vec!["commit", "-m", "child work"]] {
        assert!(Command::new("git")
            .current_dir(&child_path)
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
    repo.checkout("main");
    git_out(&repo, &["merge", "--squash", "parent"]);
    repo.commit("squash parent");
    pr.merge_commit = Some(repo.head_sha());
    runtime.block_on(parent.store.update_task_pr(&pr)).unwrap();
    git_out(&repo, &["rm", "dependency.txt"]);
    repo.commit("remove obsolete dependency after parent merge");
    repo.push();

    create_or_update_pr(
        &child_path,
        &PrOptions {
            draft: true,
            title: Some("Child".into()),
            body: Some("Preserve child work".into()),
            agent: None,
        },
        &NullProgress,
    )
    .unwrap();
    let placed = runtime
        .block_on(parent.store.get_task(&child.id))
        .unwrap()
        .unwrap();
    assert_eq!(placed.base_commit, fork);
    assert_eq!(placed.parent_pr_id, Some(parent.pr.id));
    sync_with_recovery(
        &child_path,
        &SyncOptions {
            onto: "origin/main".into(),
            push: false,
            fork_base: Some(placed.base_commit),
        },
        &NullProgress,
    )
    .unwrap();
    assert!(
        !child_path.join("dependency.txt").exists(),
        "publication must not make sync restore a dependency main removed"
    );
    assert_eq!(
        fs::read_to_string(child_path.join("child.txt")).unwrap(),
        "child work\n"
    );
}

fn continued_publication(scenario: &str) {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let state = home.path().join("github");
    fs::create_dir(&state).unwrap();
    let unarmed = support::github_merge_response(2, "@HEAD@", "OPEN", "CLEAN", None);
    let armed = support::github_merge_response(2, "@HEAD@", "OPEN", "CLEAN", Some("auto"));
    let script = r#"#!/bin/sh
state='@STATE@'
case "$1 $2" in
  'pr list')
    case "$*" in
      *'--state merged'*)
        if [ -f "$state/merge" ]; then
          printf '[{"state":"MERGED","headRefOid":"%s"}]\n' "$(cat "$state/head")"
        else echo '[]'; fi
        exit 0;;
    esac
    if [ ! -f "$state/open" ]; then echo '[]'; exit 0; fi
    number=$(cat "$state/open")
    printf '[{"url":"https://example.com/pr/%s","number":%s,"state":"OPEN","isDraft":false,"headRefOid":"%s"}]\n' "$number" "$number" "$(git rev-parse HEAD)"
    ;;
  'pr create')
    number=1
    if [ -f "$state/count" ]; then number=$(($(cat "$state/count") + 1)); fi
    echo "$number" > "$state/count"
    echo "$number" > "$state/open"
    echo "https://example.com/pr/$number"
    ;;
  'api graphql')
    if [ -f "$state/armed" ]; then response='@ARMED@'; else response='@UNARMED@'; fi
    printf '%s\n' "$response" | sed "s/@HEAD@/$(git rev-parse HEAD)/g"
    ;;
  'pr merge') touch "$state/armed";;
  'pr view') echo "https://example.com/pr/$(cat "$state/open")";;
  'api -H'|'api --cache')
    printf '{"merged":true,"state":"closed","draft":false,"merge_commit_sha":"%s","merged_at":"2026-10-14T17:00:00Z","number":1,"html_url":"https://example.com/pr/1","head":{"sha":"%s"}}\n' "$(cat "$state/merge")" "$(cat "$state/head")"
    ;;
esac
exit 0
"#.replace("@STATE@", state.to_str().unwrap()).replace("@ARMED@", &armed).replace("@UNARMED@", &unarmed);
    let _env = EnvGuard::with_lf_home(&[("gh", &script)], home.path());
    git_out(
        &repo,
        &[
            "config",
            &format!("url.{}.insteadOf", repo.bare_path().display()),
            "https://github.com/loopflowstudio/loopflow.git",
        ],
    );
    git_out(
        &repo,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/loopflowstudio/loopflow.git",
        ],
    );
    let base = repo.head_sha();
    let branch = "jack/continued-work";
    repo.create_branch(branch);
    let task = (scenario != "ordinary").then(|| {
        // Match Git's canonical checkout path, including macOS temporary-directory aliases.
        register_task_with_pr(
            home.path(),
            &repo.path().canonicalize().unwrap(),
            branch,
            &base,
        )
    });
    repo.create_file("first.txt", "first delivery\n");
    repo.stage_all();
    repo.commit("First delivery");
    let options = PrOptions {
        draft: false,
        title: Some("First delivery".into()),
        body: Some("First delivery".into()),
        agent: None,
    };
    let first = create_or_update_pr(repo.path(), &options, &NullProgress).unwrap();
    assert!(first.created);
    let first_head = repo.head_sha();
    fs::write(state.join("head"), &first_head).unwrap();
    repo.checkout("main");
    git_out(&repo, &["merge", "--squash", branch]);
    repo.commit("Squash first delivery");
    let merge = repo.head_sha();
    repo.push();
    repo.checkout(branch);
    fs::remove_file(state.join("open")).unwrap();
    fs::write(state.join("merge"), &merge).unwrap();
    if scenario == "pruned" {
        git_out(&repo, &["push", "origin", "--delete", branch]);
        git_out(&repo, &["fetch", "--prune", "origin"]);
    }
    let runtime = tokio::runtime::Runtime::new().unwrap();
    if let Some(task) = &task {
        if scenario != "unobserved" {
            let mut pr = runtime
                .block_on(task.store.active_task_pr(&task.task.id))
                .unwrap()
                .unwrap();
            pr.merge_commit = Some(merge.clone());
            runtime.block_on(task.store.settle_task_pr(&pr)).unwrap();
        }
        if scenario == "completed" {
            loopflow::ops::task::task_complete(repo.path(), "INF-123", None).unwrap();
            assert_eq!(
                runtime
                    .block_on(task.store.task_state(&task.task.id))
                    .unwrap(),
                loopflow::durable::TaskState::Done
            );
        }
    }
    repo.create_file("later.txt", "committed follow-up\n");
    repo.stage_all();
    repo.commit("Follow-up already committed");
    let committed_follow_up = repo.head_sha();
    if scenario == "pushed" {
        repo.push();
    }
    let concurrent_head = if scenario == "concurrent" {
        git_out(&repo, &["checkout", "-b", "other-contributor", &first_head]);
        repo.create_file("remote.txt", "another contributor's work\n");
        repo.stage_all();
        repo.commit("Concurrent remote work");
        let head = repo.head_sha();
        git_out(
            &repo,
            &["push", "origin", &format!("HEAD:refs/heads/{branch}")],
        );
        repo.checkout(branch);
        // Model a push that arrived after this checkout's last fetch.
        git_out(
            &repo,
            &[
                "update-ref",
                &format!("refs/remotes/origin/{branch}"),
                &first_head,
            ],
        );
        Some(head)
    } else {
        None
    };
    repo.create_file("local.txt", "local follow-up\n");
    let options = PrOptions {
        title: Some("Follow-up".into()),
        ..options
    };
    if let Some(concurrent_head) = concurrent_head {
        assert!(create_or_update_pr(repo.path(), &options, &NullProgress).is_err());
        assert_eq!(
            git_out(&repo, &["ls-remote", "--heads", "origin", branch])
                .split_whitespace()
                .next(),
            Some(concurrent_head.as_str())
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("later.txt")).unwrap(),
            "committed follow-up\n"
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("local.txt")).unwrap(),
            "local follow-up\n"
        );
        assert_eq!(fs::read_to_string(state.join("count")).unwrap().trim(), "1");
        return;
    }
    let second_url = if matches!(scenario, "pruned" | "land" | "submit") {
        let operation = match scenario {
            "land" => "land",
            "submit" => "submit",
            _ => "publish",
        };
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .current_dir(repo.path())
            .args([
                "pr",
                operation,
                "--title",
                "Follow-up",
                "--body",
                "Follow-up",
            ])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read_to_string(state.join("open")).unwrap().trim(), "2");
        "https://example.com/pr/2".to_string()
    } else {
        let second = create_or_update_pr(repo.path(), &options, &NullProgress)
            .unwrap_or_else(|error| panic!("{scenario}: {error}"));
        assert!(second.created);
        second.url
    };
    assert_ne!(first.url, second_url);
    if scenario == "pushed" {
        git_out(
            &repo,
            &["merge-base", "--is-ancestor", &committed_follow_up, "HEAD"],
        );
    }
    assert_eq!(
        git_out(&repo, &["diff", "--name-only", "origin/main...HEAD"]),
        "later.txt\nlocal.txt"
    );
    assert_eq!(
        git_out(&repo, &["rev-parse", &format!("origin/{branch}")]),
        repo.head_sha()
    );
    let history = task.as_ref().map(|task| {
        let prs = runtime
            .block_on(task.store.task_prs(&task.task.id))
            .unwrap();
        assert_eq!(prs.len(), 2);
        assert_eq!(prs[0].github().unwrap().number, 1);
        assert_eq!(prs[0].merge_commit.as_deref(), Some(merge.as_str()));
        assert_eq!(prs[0].base_commit, base);
        assert_eq!(prs[1].github().unwrap().number, 2);
        assert_eq!(
            prs[1].merge_request().is_some(),
            matches!(scenario, "land" | "submit")
        );
        assert_eq!(
            runtime
                .block_on(task.store.active_task_pr(&task.task.id))
                .unwrap()
                .unwrap()
                .id,
            prs[1].id
        );
        if scenario == "completed" {
            // A new PR respects completion; only `lf task reopen` uncompletes.
            assert_eq!(
                runtime
                    .block_on(task.store.task_state(&task.task.id))
                    .unwrap(),
                loopflow::durable::TaskState::Done
            );
        }
        prs
    });
    let retry = create_or_update_pr(repo.path(), &options, &NullProgress).unwrap();
    assert!(!retry.created);
    assert_eq!(retry.url, second_url);
    assert_eq!(fs::read_to_string(state.join("count")).unwrap().trim(), "2");
    if let (Some(task), Some(history)) = (&task, history) {
        let after = runtime
            .block_on(task.store.task_prs(&task.task.id))
            .unwrap();
        assert_eq!(after.len(), 2);
        assert_eq!(after[0], history[0]);
    }
}

#[test]
fn continued_publication_without_task() {
    continued_publication("ordinary");
}

#[test]
fn continued_publication_in_the_same_task() {
    continued_publication("task");
}

#[test]
fn continued_publication_observes_the_previous_merge() {
    continued_publication("unobserved");
}

#[test]
fn continued_publication_keeps_a_completed_task_complete() {
    continued_publication("completed");
}

#[test]
fn continued_publication_after_remote_branch_pruning() {
    continued_publication("pruned");
}

#[test]
fn continued_publication_through_land() {
    continued_publication("land");
}

#[test]
fn continued_publication_through_submit() {
    continued_publication("submit");
}

#[test]
fn continued_publication_preserves_already_pushed_followup_commits() {
    continued_publication("pushed");
}

#[test]
fn continued_publication_preserves_concurrent_remote_work() {
    continued_publication("concurrent");
}
