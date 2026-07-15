use std::process::Command;
use std::{fs, path::PathBuf};

use loopflow::engine::git::{is_clean, worktree_move, worktree_remove};
use loopflow::engine::worktrees::{
    create_named_worktree, list_worktrees, list_worktrees_local, sibling_worktree_name_with_main,
};
use loopflow_test_support::TestRepo;

fn git_stdout(repo: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn worktree_add_creates_directory() {
    let repo = TestRepo::new();
    let result = create_named_worktree(repo.path(), "feature", None, false).expect("create");
    assert!(result.path.exists());
    assert!(
        result.branch.contains("feature"),
        "branch name should include short name: {}",
        result.branch
    );
}

#[test]
fn worktree_add_is_on_correct_branch() {
    let repo = TestRepo::new();
    let result = create_named_worktree(repo.path(), "feature", None, false).expect("create");

    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&result.path)
        .output()
        .expect("git rev-parse");
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(branch, result.branch);
}

#[test]
fn worktree_remove_deletes_directory() {
    let repo = TestRepo::new();
    let result = create_named_worktree(repo.path(), "feature", None, false).expect("create");
    worktree_remove(repo.path(), &result.path).expect("remove");
    assert!(!result.path.exists());
}

#[test]
fn worktree_list_includes_created() {
    let repo = TestRepo::new();
    let result = create_named_worktree(repo.path(), "feature", None, false).expect("create");
    let worktrees = list_worktrees(repo.path()).expect("list");
    assert!(worktrees
        .iter()
        .any(|wt| wt.branch.as_deref() == Some(result.branch.as_str())));
}

#[test]
fn worktree_list_preserves_namespaced_upstream_branch() {
    let repo = TestRepo::new();
    git_stdout(repo.path(), &["branch", "jack/parent"]);
    git_stdout(repo.path(), &["push", "origin", "jack/parent"]);

    let child_path = repo.path().parent().expect("repo parent").join(format!(
        "{}.child",
        repo.path().file_name().expect("repo dir").to_string_lossy()
    ));
    add_worktree(repo.path(), &child_path, "jack/child");
    git_stdout(
        &child_path,
        &["branch", "--set-upstream-to", "origin/jack/parent"],
    );

    let (_, states) = list_worktrees_local(repo.path()).expect("list");
    let child = states
        .iter()
        .find(|wt| wt.branch.as_deref() == Some("jack/child"))
        .expect("should find child worktree");
    assert_eq!(child.base_branch.as_deref(), Some("jack/parent"));
}

#[test]
fn worktree_state_detects_dirty() {
    let repo = TestRepo::new();
    let result = create_named_worktree(repo.path(), "feature", None, false).expect("create");
    std::fs::write(result.path.join("dirty.txt"), "dirty").expect("write");
    assert!(!is_clean(&result.path).expect("is_clean"));
}

#[test]
fn worktree_move_preserves_content() {
    let repo = TestRepo::new();
    let result = create_named_worktree(repo.path(), "feature", None, false).expect("create");
    let file_path = result.path.join("note.txt");
    std::fs::write(&file_path, "content").expect("write");

    let new_path = result.path.with_extension("moved");
    worktree_move(repo.path(), &result.path, &new_path).expect("move");

    assert!(!result.path.exists());
    assert!(new_path.exists());
    assert!(new_path.join("note.txt").exists());
}

#[test]
fn create_named_worktree_synced_updates_main_before_creation() {
    let repo = TestRepo::new();
    let original_head = git_stdout(repo.path(), &["rev-parse", "HEAD"]);

    let clone_dir = tempfile::TempDir::new().expect("temp clone dir");
    let status = Command::new("git")
        .args([
            "clone",
            repo.bare_path().to_str().expect("remote path"),
            clone_dir.path().to_str().expect("clone path"),
        ])
        .status()
        .expect("clone remote");
    assert!(status.success(), "clone should succeed");
    let status = Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(clone_dir.path())
        .status()
        .expect("set email");
    assert!(status.success(), "git config user.email should succeed");
    let status = Command::new("git")
        .args(["config", "user.name", "test"])
        .current_dir(clone_dir.path())
        .status()
        .expect("set name");
    assert!(status.success(), "git config user.name should succeed");
    std::fs::write(clone_dir.path().join("remote.txt"), "remote update").expect("write file");
    let status = Command::new("git")
        .args(["add", "."])
        .current_dir(clone_dir.path())
        .status()
        .expect("git add");
    assert!(status.success(), "git add should succeed");
    let status = Command::new("git")
        .args(["commit", "-m", "remote update"])
        .current_dir(clone_dir.path())
        .status()
        .expect("git commit");
    assert!(status.success(), "git commit should succeed");
    let status = Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(clone_dir.path())
        .status()
        .expect("git push");
    assert!(status.success(), "git push should succeed");

    let _ = create_named_worktree(repo.path(), "sync-check", None, true).expect("create");

    let updated_head = git_stdout(repo.path(), &["rev-parse", "HEAD"]);
    let origin_head = git_stdout(repo.path(), &["rev-parse", "origin/main"]);
    assert_ne!(
        original_head, updated_head,
        "main head should advance after sync"
    );
    assert_eq!(
        updated_head, origin_head,
        "main should be reset to origin/main before worktree creation"
    );
}

#[test]
fn wt_switch_finds_worktree_by_sibling_name() {
    let repo = TestRepo::new();
    let result = create_named_worktree(repo.path(), "docs", None, false).expect("create");

    let worktrees = list_worktrees(repo.path()).expect("list");
    let name = sibling_worktree_name_with_main(&result.path, repo.path()).expect("sibling name");

    let matches: Vec<_> = worktrees
        .iter()
        .filter(|wt| {
            sibling_worktree_name_with_main(&wt.path, repo.path()).as_deref() == Some(name.as_str())
        })
        .collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].path, result.path);
}

#[test]
fn wt_switch_prefers_exact_branch_match_over_sibling_name() {
    let repo = TestRepo::new();
    let exact_branch = "jack/feature";
    let other_branch = "jack/other";
    let exact_path = repo.path().parent().expect("repo parent").join(format!(
        "{}.feature-exact",
        repo.path().file_name().expect("repo dir").to_string_lossy()
    ));
    let sibling_path = repo.path().parent().expect("repo parent").join(format!(
        "{}.feature",
        repo.path().file_name().expect("repo dir").to_string_lossy()
    ));

    add_worktree(repo.path(), &exact_path, exact_branch);
    add_worktree(repo.path(), &sibling_path, other_branch);

    let directive_path = repo.path().join("directive.txt");
    let status = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["wt", "switch", exact_branch])
        .current_dir(repo.path())
        .env("LOOPFLOW_DIRECTIVE_FILE", &directive_path)
        .status()
        .expect("run lf wt switch");
    assert!(status.success(), "lf wt switch should succeed");

    let directive = fs::read_to_string(&directive_path).expect("read directive");
    let target = PathBuf::from(
        directive
            .trim()
            .strip_prefix("cd ")
            .expect("directive should start with cd "),
    );
    assert_eq!(
        fs::canonicalize(target).expect("canonicalize directive target"),
        fs::canonicalize(exact_path).expect("canonicalize exact path")
    );
}

#[test]
fn wt_switch_finds_exact_branch_match() {
    let repo = TestRepo::new();

    let feature_path = repo.path().parent().expect("repo parent").join(format!(
        "{}.feature",
        repo.path().file_name().expect("repo dir").to_string_lossy()
    ));
    add_worktree(repo.path(), &feature_path, "jack/feature");

    let directive_path = repo.path().join("directive.txt");
    let status = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["wt", "switch", "jack/feature"])
        .current_dir(repo.path())
        .env("LOOPFLOW_DIRECTIVE_FILE", &directive_path)
        .status()
        .expect("run lf wt switch");
    assert!(status.success(), "lf wt switch should succeed");

    let directive = fs::read_to_string(&directive_path).expect("read directive");
    let target = PathBuf::from(
        directive
            .trim()
            .strip_prefix("cd ")
            .expect("directive should start with cd "),
    );
    assert_eq!(
        fs::canonicalize(target).expect("canonicalize directive target"),
        fs::canonicalize(feature_path).expect("canonicalize feature path")
    );
}

#[test]
fn wt_switch_does_not_map_branch_name_to_unrelated_worktree_path() {
    let repo = TestRepo::new();

    let feature_path = repo.path().parent().expect("repo parent").join(format!(
        "{}.feature",
        repo.path().file_name().expect("repo dir").to_string_lossy()
    ));
    add_worktree(repo.path(), &feature_path, "feature-live");

    let status = Command::new("git")
        .args(["branch", "jack/feature"])
        .current_dir(repo.path())
        .status()
        .expect("git branch");
    assert!(status.success(), "git branch should succeed");

    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["wt", "switch", "jack/feature"])
        .current_dir(repo.path())
        .output()
        .expect("run lf wt switch");
    assert!(
        !output.status.success(),
        "lf wt switch should fail for unrelated branch/worktree reuse"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no worktree found for 'jack/feature'"),
        "unexpected stderr: {stderr}"
    );
}

fn add_worktree(repo: &std::path::Path, path: &std::path::Path, branch: &str) {
    let status = Command::new("git")
        .args(["worktree", "add", "-b", branch])
        .arg(path)
        .arg("main")
        .current_dir(repo)
        .status()
        .expect("git worktree add");
    assert!(
        status.success(),
        "git worktree add should succeed for {}",
        branch
    );
}

#[test]
fn nested_worktree_not_recognized_as_wave() {
    // Worktrees under the main repo (e.g., .claude/worktrees/repo.feature)
    // should NOT be recognized — only siblings count.
    let dir = tempfile::TempDir::new().unwrap();
    let main_repo = dir.path().join("repo");
    let nested = main_repo
        .join(".claude")
        .join("worktrees")
        .join("repo.feature");
    std::fs::create_dir_all(&nested).unwrap();

    let result = sibling_worktree_name_with_main(&nested, &main_repo);
    assert_eq!(
        result, None,
        "nested worktree should not produce a sibling name"
    );
}

#[test]
fn branch_at_main_not_detected_as_squash_merged() {
    let repo = TestRepo::new();
    // Create a worktree whose branch points to the same commit as main.
    let result = create_named_worktree(repo.path(), "fresh", None, false).expect("create");
    // Make the worktree dirty (simulates lf ingest writing to scratch/).
    std::fs::write(result.path.join("scratch.txt"), "notes").expect("write");

    let (_, states) = list_worktrees_local(repo.path()).expect("list");
    let wt = states
        .iter()
        .find(|wt| wt.branch.as_deref() == Some(&result.branch))
        .expect("should find worktree");

    assert!(
        !wt.squash_merged,
        "branch at same commit as main should not be squash-merged"
    );
    assert!(!wt.prunable, "fresh worktree should not be prunable");
    assert!(
        wt.fresh,
        "worktree with no commits beyond main should be fresh"
    );
}

#[test]
fn fresh_worktree_is_not_prunable() {
    let repo = TestRepo::new();
    let result = create_named_worktree(repo.path(), "newwave", None, false).expect("create");

    let (_, states) = list_worktrees_local(repo.path()).expect("list");
    let wt = states
        .iter()
        .find(|wt| wt.branch.as_deref() == Some(&result.branch))
        .expect("should find worktree");

    assert!(wt.fresh, "worktree with no commits should be fresh");
    assert!(!wt.prunable, "fresh worktree should not be prunable");
    assert!(!wt.dirty, "clean worktree should not be dirty");
}

#[test]
fn fresh_dirty_worktree_is_not_prunable() {
    let repo = TestRepo::new();
    let result = create_named_worktree(repo.path(), "wip", None, false).expect("create");
    std::fs::write(result.path.join("work.txt"), "in progress").expect("write");

    let (_, states) = list_worktrees_local(repo.path()).expect("list");
    let wt = states
        .iter()
        .find(|wt| wt.branch.as_deref() == Some(&result.branch))
        .expect("should find worktree");

    assert!(wt.fresh, "no commits beyond main means fresh");
    assert!(wt.dirty, "uncommitted changes means dirty");
    assert!(!wt.prunable, "dirty fresh worktree must not be prunable");
}

#[test]
fn worktree_with_commits_is_active_not_fresh() {
    let repo = TestRepo::new();
    let result = create_named_worktree(repo.path(), "active", None, false).expect("create");
    std::fs::write(result.path.join("feature.txt"), "work").expect("write");
    git_stdout(&result.path, &["add", "."]);
    git_stdout(&result.path, &["commit", "-m", "feature work"]);

    let (_, states) = list_worktrees_local(repo.path()).expect("list");
    let wt = states
        .iter()
        .find(|wt| wt.branch.as_deref() == Some(&result.branch))
        .expect("should find worktree");

    assert!(!wt.fresh, "worktree with commits beyond main is not fresh");
    assert!(!wt.prunable, "active worktree should not be prunable");
    assert!(!wt.merged, "active worktree is not merged");
}

#[test]
fn branch_from_squash_merged_parent_stays_fresh() {
    let repo = TestRepo::new();

    let landed =
        create_named_worktree(repo.path(), "rules-old", None, false).expect("create landed");
    std::fs::write(landed.path.join("rules.txt"), "rule").expect("write");
    git_stdout(&landed.path, &["add", "."]);
    git_stdout(&landed.path, &["commit", "-m", "add rule"]);

    // Simulate a squash merge into main.
    git_stdout(repo.path(), &["merge", "--squash", landed.branch.as_str()]);
    git_stdout(repo.path(), &["commit", "-m", "land rules"]);
    git_stdout(repo.path(), &["push", "origin", "main"]);

    // Create the next branch from the landed branch tip (land rotation behavior).
    let fresh = create_named_worktree(
        repo.path(),
        "rules-new",
        Some(landed.branch.as_str()),
        false,
    )
    .expect("create fresh from landed");

    let (_, states) = list_worktrees_local(repo.path()).expect("list");
    let wt = states
        .iter()
        .find(|wt| wt.branch.as_deref() == Some(&fresh.branch))
        .expect("should find fresh worktree");

    assert!(
        wt.squash_merged,
        "branch should be tree-equal to main after squash merge"
    );
    assert!(wt.fresh, "rotated branch should still be treated as fresh");
    assert!(
        !wt.prunable,
        "fresh branch must not be pruned immediately after rotation"
    );
}

// --- Inspection surface is side-effect free (W2-169, R5) ---------------------
//
// `lf wt list` used to call `sync_main`, which fetches origin and hard-resets
// (auto-stashing) whichever worktree has main checked out. An inspection command
// must never rewrite the canonical checkout the Wave/Project control-plane turns
// depend on being clean. These pin the boundary: reads leave main untouched, and
// the fast-forward moves only under the explicit `--sync` opt-in.

/// Read-only snapshot of the mutable git state `wt list` must never touch:
/// HEAD, working tree/index, the stash, and every ref.
fn repo_state(path: &std::path::Path) -> Vec<String> {
    vec![
        git_stdout(path, &["rev-parse", "HEAD"]),
        git_stdout(path, &["status", "--porcelain"]),
        git_stdout(path, &["stash", "list"]),
        git_stdout(path, &["show-ref"]),
    ]
}

fn run_wt_list(repo: &TestRepo, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["wt", "list"];
    args.extend_from_slice(extra);
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(&args)
        .current_dir(repo.path())
        .env("LOOPFLOW_DIRECTIVE_FILE", repo.path().join("directive.txt"))
        .output()
        .expect("run lf wt list")
}

/// Put origin/main one commit ahead of local main, so a stray `sync_main` would
/// visibly fast-forward HEAD. Returns the upstream sha now only on origin.
fn advance_origin_ahead_of_local(repo: &TestRepo) -> String {
    repo.create_file("upstream.txt", "upstream change");
    repo.stage_all();
    repo.commit("upstream commit");
    let upstream = repo.head_sha();
    repo.push();
    git_stdout(repo.path(), &["reset", "--hard", "HEAD~1"]);
    upstream
}

#[test]
fn wt_list_leaves_canonical_main_byte_for_byte_unchanged() {
    let repo = TestRepo::new();
    // Something to enumerate.
    let _sibling = repo.create_named_worktree("feature");

    // The two states sync_main would rewrite: origin/main ahead of local (reset
    // --hard would fast-forward) plus a named uncommitted edit (auto-stash).
    let upstream = advance_origin_ahead_of_local(&repo);
    repo.create_file("local-edit.txt", "uncommitted work");

    let before = repo_state(repo.path());
    let out = run_wt_list(&repo, &[]);
    assert!(
        out.status.success(),
        "lf wt list should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let after = repo_state(repo.path());

    assert_eq!(
        before, after,
        "lf wt list must not fetch, reset, or stash canonical main"
    );
    assert_ne!(
        repo.head_sha(),
        upstream,
        "lf wt list must not advance main to origin/main"
    );
}

#[test]
fn wt_list_sync_flag_owns_the_fast_forward() {
    let repo = TestRepo::new();
    let _sibling = repo.create_named_worktree("feature");
    let upstream = advance_origin_ahead_of_local(&repo);
    assert_ne!(
        repo.head_sha(),
        upstream,
        "precondition: local main is behind origin"
    );

    let out = run_wt_list(&repo, &["--sync"]);
    assert!(
        out.status.success(),
        "lf wt list --sync should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert_eq!(
        repo.head_sha(),
        upstream,
        "--sync explicitly fetches and fast-forwards main to origin/main"
    );
}
