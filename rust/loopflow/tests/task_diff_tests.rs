use crate::support;

use std::fs;
use std::os::fd::FromRawFd;
use std::process::{Command, Stdio};

use crate::isolated_lf::isolated_read as read;
use loopflow_test_support::TestRepo;
use serde_json::Value;
use support::{register_task_with_pr, EnvGuard};

#[test]
fn diff_displays_the_task_comparison_through_git_tools_or_a_pipe() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    repo.create_file("edit.txt", "before\n");
    repo.stage_all();
    repo.commit("Comparison base");
    let base = repo.head_sha();
    repo.create_branch("diff-viewer");
    register_task_with_pr(home.path(), repo.path(), "diff-viewer", &base);
    repo.create_file("edit.txt", "committed\n");
    repo.stage_all();
    repo.commit("Comparison head");
    let head = repo.head_sha();
    repo.create_file("edit.txt", "working\n");
    for (key, value) in [
        ("diff.tool", "fixture"),
        ("difftool.prompt", "false"),
        ("difftool.fixture.cmd", "pwd -P; cat \"$LOCAL\" \"$REMOTE\""),
        ("core.pager", "exit 99"),
    ] {
        assert!(Command::new("git")
            .current_dir(repo.path())
            .args(["config", key, value])
            .status()
            .unwrap()
            .success());
    }
    let patch = read(repo.path(), home.path(), &["diff", "INF-123"], None);
    assert!(patch.starts_with("diff --git"), "{patch}");
    assert!(patch.contains("-before\n+working"), "{patch}");
    let view = |extra: &[&str]| {
        let mut args = vec!["diff", "INF-123", "edit.txt", "--difftool"];
        args.extend_from_slice(extra);
        read(repo.path(), home.path(), &args, None)
    };
    let cwd = repo.path().canonicalize().unwrap();
    assert_eq!(view(&[]), format!("{}\nbefore\nworking\n", cwd.display()));
    assert_eq!(
        view(&["--head", &head]),
        format!("{}\nbefore\ncommitted\n", cwd.display())
    );

    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["config", "core.pager", "cat > .git/paged.patch"])
        .status()
        .unwrap()
        .success());
    let mut master = -1;
    let mut slave = -1;
    // SAFETY: valid output pointers; null selects default terminal settings.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    // SAFETY: openpty returned two fresh, independently owned descriptors.
    let (_master, slave) = unsafe { (fs::File::from_raw_fd(master), fs::File::from_raw_fd(slave)) };
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home.path())
        .env("LF_HOME", home.path())
        .env("PATH", "/usr/bin:/bin")
        .current_dir(repo.path())
        .args(["diff", "INF-123"])
        .stdin(Stdio::null())
        .stdout(slave)
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(
        fs::read_to_string(repo.path().join(".git/paged.patch")).unwrap(),
        patch
    );
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
    let task = register_task_with_pr(home.path(), repo.path(), "file-inspection", &base);
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
    let query = |args: &[&str], input| -> Value {
        serde_json::from_str(&read(repo.path(), home.path(), args, input)).unwrap()
    };

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
