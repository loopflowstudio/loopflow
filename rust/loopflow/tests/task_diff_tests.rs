mod support;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use loopflow_test_support::TestRepo;
use serde_json::Value;
use support::{register_task, EnvGuard};

fn read(repo: &Path, home: &Path, args: &[&str], input: Option<&str>) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_DB_PATH", home.join("loopflow.db"))
        .env("PATH", "/usr/bin:/bin")
        .current_dir(repo)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn diff_files_preserves_comparison_identity_and_draft_editing() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    repo.create_file("original.txt", "original content\n");
    repo.create_file("edit.txt", "before\n");
    repo.stage_all();
    repo.commit("Comparison base");
    let base = repo.head_sha();
    repo.create_branch("file-inspection");
    let task = register_task(home.path(), repo.path(), "file-inspection", &base);
    fs::rename(
        repo.path().join("original.txt"),
        repo.path().join("renamed.txt"),
    )
    .unwrap();
    repo.stage_all();
    repo.commit("Rename file");
    repo.create_file("edit.txt", "staged\n");
    repo.stage_all();
    repo.create_file("edit.txt", "working\n");
    repo.create_file("untracked.txt", "untracked\n");
    let query = |args: &[&str], input| read(repo.path(), home.path(), args, input);

    let files = query(&["task", "diff", "INF-123", "--files", "--json"], None);
    assert_eq!(files["task_id"], task.task.id.as_str());
    assert_eq!(files["base_commit"], base);
    assert_eq!(files["head_commit"], repo.head_sha());
    let paths = files["files"].as_array().unwrap();
    let renamed = paths
        .iter()
        .find(|file| file["path"] == "renamed.txt")
        .unwrap();
    assert_eq!(renamed["old_path"], "original.txt");
    assert_eq!(renamed["committed"], true);
    let edited = paths
        .iter()
        .find(|file| file["path"] == "edit.txt")
        .unwrap();
    assert_eq!(edited["staged"], true);
    assert_eq!(edited["unstaged"], true);
    assert!(paths
        .iter()
        .any(|file| file["path"] == "untracked.txt" && file["untracked"] == true));
    assert_eq!(
        files,
        query(
            &["diff", "INF-123", "--files", "--base", &base, "--json"],
            None
        )
    );
    let head = query(
        &["diff", "INF-123", "--files", "--base", "head", "--json"],
        None,
    );
    assert_eq!(head["base_commit"], repo.head_sha());
    assert!(!head["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["path"] == "renamed.txt"));

    let patch = query(
        &["diff", "INF-123", "renamed.txt", "--base", &base, "--json"],
        None,
    );
    assert_eq!(patch["base_commit"], base);
    assert!(patch["patch"]
        .as_str()
        .unwrap()
        .contains("rename from original.txt"));
    let file = query(&["file", "INF-123", "edit.txt", "--json"], None);
    let draft = query(
        &["diff", "INF-123", "edit.txt", "--draft", "--json"],
        Some("draft\n"),
    );
    assert_eq!(draft["base_commit"], base);
    assert!(draft["patch"].as_str().unwrap().contains("+draft"));
    assert_eq!(
        fs::read_to_string(repo.path().join("edit.txt")).unwrap(),
        "working\n"
    );
    assert_eq!(
        file,
        query(&["file", "INF-123", "edit.txt", "--json"], None)
    );
    let saved = query(
        &[
            "save",
            "INF-123",
            "edit.txt",
            "--revision",
            file["revision"].as_str().unwrap(),
            "--json",
        ],
        Some("draft\n"),
    );
    assert_eq!(saved["published"], true);
    assert_eq!(saved["file"]["content"], "draft\n");
    assert_ne!(saved["file"]["revision"], file["revision"]);
}
