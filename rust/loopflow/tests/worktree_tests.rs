use std::process::Command;

use loopflow::engine::git::{is_clean, worktree_move, worktree_remove};
use loopflow::engine::worktrees::{
    create_with_schema, create_with_schema_synced, list_worktrees, list_worktrees_local,
    preserve_worktree, wave_name_from_worktree_and_main,
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
    let result = create_with_schema(repo.path(), "feature", None, None).expect("create");
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
    let result = create_with_schema(repo.path(), "feature", None, None).expect("create");

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
    let result = create_with_schema(repo.path(), "feature", None, None).expect("create");
    worktree_remove(repo.path(), &result.path).expect("remove");
    assert!(!result.path.exists());
}

#[test]
fn worktree_list_includes_created() {
    let repo = TestRepo::new();
    let result = create_with_schema(repo.path(), "feature", None, None).expect("create");
    let worktrees = list_worktrees(repo.path()).expect("list");
    assert!(worktrees
        .iter()
        .any(|wt| wt.branch.as_deref() == Some(result.branch.as_str())));
}

#[test]
fn worktree_state_detects_dirty() {
    let repo = TestRepo::new();
    let result = create_with_schema(repo.path(), "feature", None, None).expect("create");
    std::fs::write(result.path.join("dirty.txt"), "dirty").expect("write");
    assert!(!is_clean(&result.path).expect("is_clean"));
}

#[test]
fn worktree_move_preserves_content() {
    let repo = TestRepo::new();
    let result = create_with_schema(repo.path(), "feature", None, None).expect("create");
    let file_path = result.path.join("note.txt");
    std::fs::write(&file_path, "content").expect("write");

    let new_path = result.path.with_extension("moved");
    worktree_move(repo.path(), &result.path, &new_path).expect("move");

    assert!(!result.path.exists());
    assert!(new_path.exists());
    assert!(new_path.join("note.txt").exists());
}

#[test]
fn create_with_schema_synced_updates_main_before_creation() {
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

    let _ = create_with_schema_synced(repo.path(), "sync-check", None, None).expect("create");

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
fn wt_switch_finds_worktree_by_wave_name() {
    let repo = TestRepo::new();
    let result = create_with_schema(repo.path(), "docs", None, None).expect("create");

    let worktrees = list_worktrees(repo.path()).expect("list");
    let name =
        wave_name_from_worktree_and_main(&result.path, repo.path()).expect("should have wave name");

    let matches: Vec<_> = worktrees
        .iter()
        .filter(|wt| {
            wave_name_from_worktree_and_main(&wt.path, repo.path()).as_deref()
                == Some(name.as_str())
        })
        .collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].path, result.path);
}

#[test]
fn wt_switch_prefix_matching_is_dot_delimited() {
    // Simulate a worktree whose directory has a timestamp suffix (e.g. repo.waves.1772406404)
    // by creating a worktree with a dotted name.
    let repo = TestRepo::new();
    let result = create_with_schema(repo.path(), "waves", None, None).expect("create");
    let wave_name =
        wave_name_from_worktree_and_main(&result.path, repo.path()).expect("should have wave name");

    // The wave name should be "waves". Searching for "wav" must NOT match.
    let worktrees = list_worktrees(repo.path()).expect("list");
    let prefix = "wav";
    let matches: Vec<_> = worktrees
        .iter()
        .filter(|wt| {
            let wt_name = wave_name_from_worktree_and_main(&wt.path, repo.path());
            wt_name.as_deref() == Some(prefix)
                || wt_name
                    .as_ref()
                    .map(|n| n.starts_with(&format!("{prefix}.")))
                    .unwrap_or(false)
        })
        .collect();
    // "wav" doesn't start with "wav." — it should NOT match "waves" via prefix
    // (prefix matching is on dot boundaries, not arbitrary substrings)
    assert!(
        matches.is_empty(),
        "prefix matching is dot-delimited, not substring: got {:?}",
        wave_name
    );
}

#[test]
fn wt_switch_finds_timestamped_worktree_by_short_name() {
    let repo = TestRepo::new();
    // Create worktree — the directory will be <repo>.waves
    let result = create_with_schema(repo.path(), "waves", None, None).expect("create");

    // Manually rename it to simulate a timestamped directory: <repo>.waves.1772406404
    let timestamped = result.path.with_file_name(format!(
        "{}.1772406404",
        result.path.file_name().unwrap().to_string_lossy()
    ));
    worktree_move(repo.path(), &result.path, &timestamped).expect("move");

    let worktrees = list_worktrees(repo.path()).expect("list");
    let wave_name =
        wave_name_from_worktree_and_main(&timestamped, repo.path()).expect("should have wave name");
    assert_eq!(wave_name, "waves.1772406404");

    // Exact match: "waves.1772406404" should find it
    let exact: Vec<_> = worktrees
        .iter()
        .filter(|wt| {
            wave_name_from_worktree_and_main(&wt.path, repo.path()).as_deref()
                == Some("waves.1772406404")
        })
        .collect();
    assert_eq!(exact.len(), 1);

    // Prefix match: "waves" should find it via starts_with("waves.")
    let prefix: Vec<_> = worktrees
        .iter()
        .filter(|wt| {
            let n = wave_name_from_worktree_and_main(&wt.path, repo.path());
            n.as_ref().map(|n| n.starts_with("waves.")).unwrap_or(false)
        })
        .collect();
    assert_eq!(prefix.len(), 1);
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

    let result = wave_name_from_worktree_and_main(&nested, &main_repo);
    assert_eq!(
        result, None,
        "nested worktree should not produce a wave name"
    );
}

#[test]
fn preserve_worktree_uses_human_readable_timestamp() {
    let repo = TestRepo::new();
    let result = create_with_schema(repo.path(), "feature", None, None).expect("create");

    let preserved = preserve_worktree(repo.path(), &result.path, None).expect("preserve");
    let dir_name = preserved.file_name().unwrap().to_string_lossy().to_string();

    // Should end with .YYYYMMDD_HHMM, not a unix epoch
    let suffix = dir_name.rsplit_once('.').expect("should have dot").1;
    assert_eq!(
        suffix.len(),
        13,
        "timestamp should be YYYYMMDD_HHMM (13 chars), got: {suffix}"
    );
    let (date, time) = suffix.split_once('_').expect("should have underscore");
    assert_eq!(date.len(), 8, "date part should be 8 digits: {date}");
    assert_eq!(time.len(), 4, "time part should be 4 digits: {time}");
    assert!(
        date.chars().all(|c| c.is_ascii_digit()),
        "date should be all digits: {date}"
    );
    assert!(
        time.chars().all(|c| c.is_ascii_digit()),
        "time should be all digits: {time}"
    );
}

#[test]
fn preserve_worktree_uses_explicit_suffix_when_provided() {
    let repo = TestRepo::new();
    let result = create_with_schema(repo.path(), "feature", None, None).expect("create");

    let preserved =
        preserve_worktree(repo.path(), &result.path, Some("20260304_1442")).expect("preserve");
    let dir_name = preserved.file_name().unwrap().to_string_lossy().to_string();

    assert!(
        dir_name.ends_with(".20260304_1442"),
        "should use the provided suffix, got: {dir_name}"
    );
}

#[test]
fn branch_at_main_not_detected_as_squash_merged() {
    let repo = TestRepo::new();
    // Create a worktree whose branch points to the same commit as main.
    let result = create_with_schema(repo.path(), "fresh", None, None).expect("create");
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
    let result = create_with_schema(repo.path(), "newwave", None, None).expect("create");

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
    let result = create_with_schema(repo.path(), "wip", None, None).expect("create");
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
    let result = create_with_schema(repo.path(), "active", None, None).expect("create");
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

    let landed = create_with_schema(repo.path(), "rules-old", None, None).expect("create landed");
    std::fs::write(landed.path.join("rules.txt"), "rule").expect("write");
    git_stdout(&landed.path, &["add", "."]);
    git_stdout(&landed.path, &["commit", "-m", "add rule"]);

    // Simulate a squash merge into main.
    git_stdout(repo.path(), &["merge", "--squash", landed.branch.as_str()]);
    git_stdout(repo.path(), &["commit", "-m", "land rules"]);
    git_stdout(repo.path(), &["push", "origin", "main"]);

    // Create the next branch from the landed branch tip (land rotation behavior).
    let fresh = create_with_schema(repo.path(), "rules-new", Some(landed.branch.as_str()), None)
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
