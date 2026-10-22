use crate::support;

use std::process::Command;

use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn git(repo: &TestRepo, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn clear_uses_the_diff_and_preserves_unrelated_index_and_working_edits() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    repo.create_file("scratch/research.md", "existing\n");
    repo.stage_all();
    repo.commit("Existing research");
    repo.push();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature\n");
    repo.create_file("scratch/new [notes].md", "temporary\n");
    repo.stage_all();
    repo.commit("Work and notes");
    repo.create_file("scratch/research.md", "staged edit\n");
    git(&repo, &["add", "scratch/research.md"]);
    repo.create_file("scratch/research.md", "later edit\n");
    repo.create_file("scratch/untracked.md", "outside the checkpoint\n");
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["scratch", "clear"])
        .current_dir(repo.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Cleared 1 added scratch files."));
    assert!(!repo.path().join("scratch/new [notes].md").exists());
    assert_eq!(
        git(&repo, &["show", "HEAD:scratch/research.md"]),
        "existing\n"
    );
    assert_eq!(
        git(&repo, &["show", ":scratch/research.md"]),
        "staged edit\n"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("scratch/research.md")).unwrap(),
        "later edit\n"
    );
    assert!(repo.path().join("scratch/untracked.md").exists());
    let head = repo.head_sha();
    assert_eq!(loopflow::ops::scratch::clear(repo.path()).unwrap(), 0);
    assert_eq!(repo.head_sha(), head);
}

#[test]
fn clear_preserves_existing_files_moved_into_scratch() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    repo.create_file("research.md", "existing research\n");
    repo.create_file("scratch/old.md", "existing plan\n");
    repo.stage_all();
    repo.commit("Existing documents");
    repo.push();
    repo.create_branch("feature");
    git(&repo, &["mv", "research.md", "scratch/research.md"]);
    git(&repo, &["mv", "scratch/old.md", "scratch/renamed.md"]);
    repo.create_file("scratch/new.md", "disposable notes\n");
    repo.stage_all();
    repo.commit("Move existing documents and add notes");

    assert_eq!(loopflow::ops::scratch::clear(repo.path()).unwrap(), 1);
    assert_eq!(
        git(&repo, &["show", "HEAD:scratch/research.md"]),
        "existing research\n"
    );
    assert_eq!(
        git(&repo, &["show", "HEAD:scratch/renamed.md"]),
        "existing plan\n"
    );
    assert!(!repo.path().join("scratch/new.md").exists());
}
