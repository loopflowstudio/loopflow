mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use loopflow::git::worktrees::git_common_dir;
use loopflow::work::task::{GithubPr, PrPublication, Task};
use loopflow_test_support::TestRepo;
use serde_json::Value;
use sha2::{Digest, Sha256};
use support::{register_sibling_task, register_task_with_pr, EnvGuard};

fn checkout(home: &Path, caller: &Path, child: &Task, design: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("LF_") {
            command.env_remove(name);
        }
    }
    command
        .env("LF_HOME", home)
        .current_dir(caller)
        .args([
            "checkout",
            &child.plan.identifier,
            "--design",
            design,
            "--json",
        ])
        .output()
        .unwrap()
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
}

fn receipts(child: &Task) -> Vec<Value> {
    fs::read_dir(
        git_common_dir(child.worktree.as_deref().unwrap())
            .unwrap()
            .join("loopflow-design-handoffs")
            .join(child.id.as_str()),
    )
    .unwrap()
    .map(|entry| {
        serde_json::from_slice(&fs::read(entry.unwrap().path().join("receipt.json")).unwrap())
            .unwrap()
    })
    .collect()
}

#[test]
fn checkout_hands_each_child_only_its_selected_design_with_source_receipt() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    repo.create_branch("parent");
    repo.create_file("scratch/parent.md", "Whole parent plan");
    repo.stage_all();
    repo.commit("Parent plan");
    let parent = register_task_with_pr(home.path(), repo.path(), "parent", &repo.head_sha());
    let head = repo.head_sha();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut parent_pr = parent.pr.clone();
    parent_pr.publication = Some(PrPublication {
        requested_at: parent_pr.created_at,
        presentation: None,
        github: Some(GithubPr {
            number: 41,
            url: "https://github.com/fixture/repo/pull/41".into(),
            head_sha: Some(head.clone()),
        }),
        merge: None,
    });
    runtime
        .block_on(parent.store.update_task_pr(&parent_pr))
        .unwrap();
    for (identifier, slug, content) in [
        (
            "INF-124",
            "child-one",
            "# First child\r\nOnly step one.\r\n",
        ),
        ("INF-125", "child-two", "# Second child\nOnly step two.\n"),
    ] {
        let child = register_sibling_task(&parent, identifier, slug, &target.path().join(slug));
        runtime
            .block_on(parent.store.stack_task_placement(&child, &parent_pr.id))
            .unwrap();
        let source = format!("{slug}.md");
        repo.create_file(&format!("scratch/{source}"), content);
        assert_success(checkout(
            home.path(),
            &repo.path().join("scratch"),
            &child,
            &source,
        ));
        assert_eq!(
            fs::read_to_string(
                child
                    .worktree
                    .as_ref()
                    .unwrap()
                    .join("scratch")
                    .join(&source)
            )
            .unwrap(),
            content
        );
        assert!(!child
            .worktree
            .as_ref()
            .unwrap()
            .join("scratch/parent.md")
            .exists());
        let receipts = receipts(&child);
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0]["source_task"], parent.task.id.as_str());
        assert_eq!(receipts[0]["source_issue"], "INF-123");
        assert_eq!(receipts[0]["source_commit"], head);
        assert_eq!(receipts[0]["content"], content);
        assert_eq!(
            receipts[0]["content_sha256"],
            hex::encode(Sha256::digest(content.as_bytes()))
        );
        assert_eq!(
            receipts[0]["source_path"],
            repo.path()
                .join("scratch")
                .join(&source)
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
        );
    }
}

#[test]
fn repeated_handoff_preserves_edited_and_deleted_child_notes_after_parent_commit() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    repo.create_branch("parent");
    let parent = register_task_with_pr(home.path(), repo.path(), "parent", &repo.head_sha());
    let child = register_sibling_task(&parent, "INF-124", "child", &target.path().join("child"));
    repo.create_file("design.md", "Original selected design");
    assert_success(checkout(home.path(), repo.path(), &child, "design.md"));
    let destination = child.worktree.as_ref().unwrap().join("scratch/child.md");
    fs::write(&destination, "Child's revised design").unwrap();
    repo.stage_all();
    repo.commit("Advance parent without changing selected input");
    assert_success(checkout(home.path(), repo.path(), &child, "design.md"));
    assert_eq!(
        fs::read_to_string(&destination).unwrap(),
        "Child's revised design"
    );
    fs::remove_file(&destination).unwrap();
    assert_success(checkout(home.path(), repo.path(), &child, "design.md"));
    assert!(!destination.exists());
    assert_eq!(receipts(&child).len(), 1);
}

#[test]
fn conflicting_design_retains_both_versions_and_repeated_conflict_stays_failed() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    repo.create_branch("parent");
    let parent = register_task_with_pr(home.path(), repo.path(), "parent", &repo.head_sha());
    let child = register_sibling_task(&parent, "INF-124", "child", &target.path().join("child"));
    repo.create_file("design.md", "Original selected design");
    assert_success(checkout(home.path(), repo.path(), &child, "design.md"));
    let destination = child.worktree.as_ref().unwrap().join("scratch/child.md");
    fs::write(&destination, "Child edits").unwrap();
    repo.create_file("design.md", "Different incoming design");
    for _ in 0..2 {
        let output = checkout(home.path(), repo.path(), &child, "design.md");
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("design handoff conflict"), "{error}");
        assert!(error.contains("receipt.json"), "{error}");
        assert_eq!(fs::read_to_string(&destination).unwrap(), "Child edits");
    }
    let receipts = receipts(&child);
    assert_eq!(receipts.len(), 2);
    assert!(receipts
        .iter()
        .any(|receipt| receipt["content"] == "Different incoming design"));
    assert!(receipts
        .iter()
        .any(|receipt| receipt["content"] == "Original selected design"));
}

#[test]
fn unreadable_design_does_not_place_child() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let parent = register_task_with_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let child = register_sibling_task(&parent, "INF-124", "child", &target.path().join("child"));
    assert!(!checkout(home.path(), repo.path(), &child, "missing.md")
        .status
        .success());
    assert!(!child.worktree.as_ref().unwrap().exists());
}
