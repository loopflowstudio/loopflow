use std::process::Command;

use loopflow::engine::git::{is_clean, worktree_move, worktree_remove};
use loopflow::engine::worktrees::{create_with_schema, list_worktrees};
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
