use std::process::Command;

use loopflow::engine::git::{is_clean, worktree_move, worktree_remove};
use loopflow::engine::worktrees::{
    create_with_schema, list_worktrees, preserve_worktree, wave_name_from_worktree_and_main,
};
use loopflow_test_support::TestRepo;

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
fn preserve_worktree_uses_human_readable_timestamp() {
    let repo = TestRepo::new();
    let result = create_with_schema(repo.path(), "feature", None, None).expect("create");

    let preserved = preserve_worktree(repo.path(), &result.path).expect("preserve");
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
