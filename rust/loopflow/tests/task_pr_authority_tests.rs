//! W2-253 — Task PR operations must fail closed without durable authority.
//!
//! Task-owned publication, stacking, submit, and land must never degrade to
//! generic PR behavior when the shared Task registry is missing, inaccessible,
//! or schema-incompatible. These tests prove the observable contract: every
//! Task entry point refuses **before any push or `gh pr` mutation** with an
//! actionable authority error (zero remote mutation), ordinary non-Task PR
//! flows stay explicit, and valid authority still publishes and records the PR.

mod support;

use std::fs;
use std::process::{Command, Stdio};

use loopflow::ops::task::task_stack;
use loopflow::ops::{
    arm as land, create_or_update_pr, submit, LandOptions, NullProgress, PrOptions,
};
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
        pr_body: Some("authority proof".to_string()),
        agent: None,
    }
}

fn noop_open_script() -> &'static str {
    "#!/bin/sh\nexit 0\n"
}

/// A `gh` that records every non-`--version` invocation to `log_path`. It
/// answers `pr list` with an empty list and `pr create` with a URL so a
/// successful publish can attach a PR; the refuse tests never reach those
/// branches and the log proves no PR mutation was issued.
fn gh_record_script(log_path: &str) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "gh version 1.0.0"
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
  echo 'OPEN'
  exit 0
fi
exit 0
"#
    )
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

/// Assert no GitHub PR mutation was issued. `--version` and `pr list` reads are
/// fine; creates, edits, readiness, merges, and closes are remote mutations.
fn assert_no_gh_pr_mutation(log_path: &std::path::Path) {
    let log = fs::read_to_string(log_path).unwrap_or_default();
    for mutation in ["pr create", "pr edit", "pr ready", "pr merge", "pr close"] {
        assert!(
            !log.contains(mutation),
            "no gh `{mutation}` mutation may be issued before authority refusal, got log:\n{log}"
        );
    }
}

/// RAII guard for an ambient env var so a test never leaks Run context
/// into a concurrently-running test, even on panic.
struct AmbientVarGuard {
    name: &'static str,
}
impl AmbientVarGuard {
    fn set(name: &'static str, value: &str) -> Self {
        std::env::set_var(name, value);
        Self { name }
    }
}
impl Drop for AmbientVarGuard {
    fn drop(&mut self) {
        std::env::remove_var(self.name);
    }
}

/// Make a registry file inaccessible so `open_store` cannot open it — the
/// `RegistryUnavailable::Incompatible` branch (which also covers
/// schema-incompatible and locked stores). Simulating a real registry that
/// became unreadable is more faithful than a garbage file, which SQLite may
/// recreate or recover from via WAL.
#[cfg(unix)]
fn make_registry_inaccessible(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000))
        .expect("make registry inaccessible");
}

/// A Task entry point (ambient id set) whose registry file is gone must refuse
/// before any push or `gh pr` — the missing registry cannot be proven away, so
/// the PR must not degrade to generic behavior. `submit` is the gate.
#[test]
fn submit_refuses_when_registry_missing_before_any_push() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_record_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    let branch = "jack/authority-missing";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");
    // Deliberately NOT pushed: the refusal must precede the first push.

    let _task = register_task(home.path(), repo.path(), branch, &base);

    // The registry vanishes. Ambient Run identity still marks this as an agent entry
    // point, so the missing registry is missing authority — not "no tasks here."
    std::fs::remove_file(home.path().join("loopflow.db")).expect("remove registry");
    let run_id = loopflow::durable::RunId::new();
    let _ambient = AmbientVarGuard::set(loopflow::durable::RUN_ID_ENV, run_id.as_str());

    let err = submit(
        repo.path(),
        &land_options(true, "authority missing"),
        &NullProgress,
    )
    .expect_err("missing registry must refuse before any push");
    let message = err.to_string();
    assert!(
        message.contains("authority refused"),
        "expected an authority refusal, got: {message}"
    );
    assert!(
        message.contains("missing"),
        "refusal must name the missing registry, got: {message}"
    );

    assert!(
        !remote_branch_exists(&repo, branch),
        "the branch must never reach the remote when authority is refused"
    );
    assert_no_gh_pr_mutation(&log_path);
}

/// A Task worktree whose registry exists but cannot be opened (inaccessible)
/// must refuse before any push or `gh pr`. No ambient id is set — this is the
/// human-in-a-Task-worktree shape — and the unreadable registry cannot be
/// inspected to prove otherwise, so publication refuses. The
/// `RegistryUnavailable::Incompatible` branch also covers schema-incompatible
/// and locked stores.
#[cfg(unix)]
#[test]
fn publish_refuses_when_registry_inaccessible_before_any_push() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_record_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    let branch = "jack/authority-inaccessible";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");

    let _task = register_task(home.path(), repo.path(), branch, &base);

    // The registry exists with this Task but is now unreadable.
    make_registry_inaccessible(&home.path().join("loopflow.db"));

    let err = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("inaccessible".to_string()),
            body: Some("authority proof".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect_err("inaccessible registry must refuse before any push");
    let message = err.to_string();
    assert!(
        message.contains("authority refused"),
        "expected an authority refusal, got: {message}"
    );
    assert!(
        message.contains("incompatible"),
        "refusal must name the inaccessible/incompatible registry, got: {message}"
    );

    assert!(
        !remote_branch_exists(&repo, branch),
        "the branch must never reach the remote when authority is refused"
    );
    assert_no_gh_pr_mutation(&log_path);
}

/// `land` arms auto-merge — a GitHub mutation. An inaccessible registry must
/// refuse before that first push, proving the land path fails closed too.
#[cfg(unix)]
#[test]
fn land_refuses_when_registry_inaccessible_before_any_push() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_record_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    let branch = "jack/authority-inaccessible-land";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");

    let _task = register_task(home.path(), repo.path(), branch, &base);

    make_registry_inaccessible(&home.path().join("loopflow.db"));

    let err = land(
        repo.path(),
        &land_options(true, "inaccessible land"),
        &NullProgress,
    )
    .expect_err("inaccessible registry must refuse before any push");
    let message = err.to_string();
    assert!(
        message.contains("authority refused"),
        "expected an authority refusal, got: {message}"
    );

    assert!(
        !remote_branch_exists(&repo, branch),
        "the branch must never reach the remote when authority is refused"
    );
    assert_no_gh_pr_mutation(&log_path);
}

/// Stacking resolution (`task_stack`) shares the authority gate. An
/// inaccessible registry must refuse rather than return `None` and let a
/// stacked rebase silently collapse to generic.
#[cfg(unix)]
#[test]
fn task_stack_refuses_when_registry_inaccessible() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();

    let _env = EnvGuard::with_lf_home(&[("open", noop_open_script())], home.path());

    let branch = "jack/authority-stack";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");

    let _task = register_task(home.path(), repo.path(), branch, &base);

    make_registry_inaccessible(&home.path().join("loopflow.db"));

    let err = task_stack(repo.path()).expect_err("inaccessible registry must refuse stacking");
    let message = err.to_string();
    assert!(
        message.contains("authority refused"),
        "expected an authority refusal, got: {message}"
    );
}

/// A registry file that is present but not a valid database (schema-incompatible
/// / corrupt) must refuse before any push or `gh pr`. The WAL sidecars are
/// removed first so SQLite cannot recover the prior valid state from them and
/// `open_store` hits "file is not a database" — the same
/// `RegistryUnavailable::Incompatible` branch as an inaccessible store.
#[test]
fn publish_refuses_when_registry_schema_incompatible_before_any_push() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_record_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    let branch = "jack/authority-schema-incompatible";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");

    let _task = register_task(home.path(), repo.path(), branch, &base);

    let db = home.path().join("loopflow.db");
    // Drop the WAL sidecars so SQLite cannot recover a valid database from them
    // after the main file is corrupted.
    for sidecar in ["loopflow.db-wal", "loopflow.db-shm"] {
        let _ = std::fs::remove_file(home.path().join(sidecar));
    }
    std::fs::write(&db, b"not a sqlite database").expect("corrupt the registry");

    let err = create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("schema-incompatible".to_string()),
            body: Some("authority proof".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect_err("schema-incompatible registry must refuse before any push");
    let message = err.to_string();
    assert!(
        message.contains("authority refused"),
        "expected an authority refusal, got: {message}"
    );
    assert!(
        message.contains("incompatible"),
        "refusal must name the incompatible registry, got: {message}"
    );

    assert!(
        !remote_branch_exists(&repo, branch),
        "the branch must never reach the remote when authority is refused"
    );
    assert_no_gh_pr_mutation(&log_path);
}

/// Valid authority: a healthy registry with a Task that owns this
/// worktree publishes, pushes, and durably records the PR — the publication
/// request and GitHub attachment prove the Task path ran, not a generic PR.
#[test]
fn valid_authority_publishes_and_records_the_pr() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_record_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    let branch = "jack/authority-valid";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");
    repo.push_new_branch(branch);

    let task = register_task(home.path(), repo.path(), branch, &base);

    create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("valid authority".to_string()),
            body: Some("authority proof".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("valid authority publishes");

    assert!(
        remote_branch_exists(&repo, branch),
        "the Task branch must be pushed under valid authority"
    );

    // The Task path ran end to end: the publication request was recorded and the
    // GitHub PR was attached, so this did not degrade to a generic PR.
    let runtime = tokio::runtime::Runtime::new().expect("read task runtime");
    let pr = runtime
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert!(
        pr.publication.is_some(),
        "publication must be recorded under valid authority, not degraded to generic"
    );
    assert!(
        pr.publication.as_ref().unwrap().merge.is_none(),
        "publication alone must not request a merge"
    );
    let github = pr
        .github()
        .expect("GitHub PR must be attached under valid authority");
    assert_eq!(github.number, 1, "the published PR number must be attached");
}

/// Ordinary non-Task PR flows remain explicit: a healthy registry that
/// registers a task for a *different* worktree must not capture this checkout.
/// The publish proceeds as a normal PR (the Task bookkeeping is an explicit
/// no-op), proving the fail-closed gate does not over-block ordinary PRs.
#[test]
fn ordinary_pr_publishes_when_worktree_is_not_a_task_worktree() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_record_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    // A healthy registry exists, but its one Task claims a different
    // worktree — this checkout is provably not a Task worktree.
    let elsewhere = tempfile::TempDir::new().expect("unrelated worktree");
    let _unrelated = register_task(home.path(), elsewhere.path(), "jack/elsewhere", &base);

    let branch = "jack/ordinary-pr";
    repo.create_branch(branch);
    repo.create_file("task.txt", "ordinary work\n");
    repo.stage_all();
    repo.commit("ordinary commit");
    repo.push_new_branch(branch);

    create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("ordinary non-Task PR".to_string()),
            body: Some("not a task".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("ordinary non-Task PR publishes");

    assert!(
        remote_branch_exists(&repo, branch),
        "an ordinary non-Task PR must still push and publish"
    );
}

/// Ordinary non-Task PR flows remain explicit when no registry exists at all:
/// a machine with no registry file has no tasks, so a plain PR publishes.
#[test]
fn ordinary_pr_publishes_when_no_registry_exists() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();

    let log_path = home.path().join("gh.log");
    let script = gh_record_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );
    // No register_task, no ambient id, no registry file.

    let branch = "jack/ordinary-no-registry";
    repo.create_branch(branch);
    repo.create_file("task.txt", "ordinary work\n");
    repo.stage_all();
    repo.commit("ordinary commit");
    repo.push_new_branch(branch);

    create_or_update_pr(
        repo.path(),
        &PrOptions {
            title: Some("ordinary PR no registry".to_string()),
            body: Some("not a task".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("ordinary non-Task PR publishes without any registry");

    assert!(
        remote_branch_exists(&repo, branch),
        "an ordinary non-Task PR must still push and publish with no registry"
    );
}

/// LOO-247 — a managed Task has exactly one shipping decision: its `finally`
/// review, declared with `lf pr land`. `lf pr submit` would add a human merge
/// click, so it must refuse before local integration, push, or `gh pr` mutation.
#[test]
fn managed_task_submit_refuses_before_any_mutation() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_record_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    // Advance main before cutting the Task branch, while deliberately keeping
    // the older recorded base. The shared range verifier would heal this
    // metadata if submit reached it, so the assertion below proves the refusal
    // really precedes every durable mutation rather than only PR writes.
    repo.create_file("upstream.txt", "upstream work\n");
    repo.stage_all();
    repo.commit("advance main");
    repo.push();

    let branch = "jack/managed-submit";
    repo.create_branch(branch);
    repo.create_file("task.txt", "task work\n");
    repo.stage_all();
    repo.commit("task commit");
    let task = register_task(home.path(), repo.path(), branch, &base);
    let head_before = repo.head_sha();
    repo.create_file("uncommitted.txt", "preserve this work\n");

    let err = submit(
        repo.path(),
        &land_options(true, "managed submit"),
        &NullProgress,
    )
    .expect_err("managed Task submit must refuse before mutation");
    let message = err.to_string();
    assert!(
        message.contains("second shipping gate") && message.contains("lf pr land"),
        "refusal must explain the competing gate and name land: {message}"
    );

    assert!(
        !remote_branch_exists(&repo, branch),
        "a refused submit must not publish the Task branch"
    );
    assert_eq!(
        repo.head_sha(),
        head_before,
        "a refused submit must not commit or rebase local work"
    );
    let status = Command::new("git")
        .args(["status", "--short"])
        .current_dir(repo.path())
        .output()
        .expect("read worktree status");
    let status = String::from_utf8(status.stdout).expect("utf-8 worktree status");
    assert!(
        status.contains("?? uncommitted.txt"),
        "the caller's dirty worktree must remain untouched: {status}"
    );
    let pr = tokio::runtime::Runtime::new()
        .expect("task runtime")
        .block_on(task.store.active_task_pr(&task.task.id))
        .expect("read active PR")
        .expect("active PR");
    assert_eq!(
        pr.base_commit, base,
        "a refused submit must not heal the recorded Task PR base"
    );
    assert!(
        pr.merge_request().is_none(),
        "a refused submit must not leave a settlement owner"
    );
    assert_no_gh_pr_mutation(&log_path);
}

#[test]
fn ordinary_submit_still_assigns_for_review() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let base = repo.head_sha();

    let log_path = home.path().join("gh.log");
    let script = gh_record_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );

    // A healthy registry exists, but its one Task claims a different worktree —
    // this checkout is provably not a Task worktree, so submit is unaffected.
    let elsewhere = tempfile::TempDir::new().expect("unrelated worktree");
    let _unrelated = register_task(home.path(), elsewhere.path(), "jack/elsewhere", &base);

    let branch = "jack/ordinary-submit";
    repo.create_branch(branch);
    repo.create_file("task.txt", "ordinary work\n");
    repo.stage_all();
    repo.commit("ordinary commit");
    repo.push_new_branch(branch);

    submit(
        repo.path(),
        &land_options(true, "ordinary submit"),
        &NullProgress,
    )
    .expect("ordinary non-Task submit still assigns for review");

    let log = fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        log.contains("pr edit --add-assignee @me"),
        "ordinary submit must assign the PR for a human merge, got log:\n{log}"
    );
    assert!(
        !log.contains("merge --auto"),
        "submit must never arm auto-merge, got log:\n{log}"
    );
}
