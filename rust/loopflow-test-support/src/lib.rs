use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

#[derive(Debug)]
pub struct TestRepo {
    bare: TempDir,
    // Kept alive so the temp directory isn't deleted while TestRepo exists.
    #[allow(dead_code)]
    work: TempDir,
    repo: PathBuf,
}

impl Default for TestRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl TestRepo {
    pub fn new() -> Self {
        let work = TempDir::new().expect("temp work dir");
        let bare = TempDir::new().expect("temp bare dir");

        init_repo(work.path());
        config_user(work.path());
        create_initial_commit(work.path());
        init_bare_remote(bare.path());
        add_remote(work.path(), bare.path());
        push_main(work.path());

        let repo = work.path().to_path_buf();
        Self { bare, work, repo }
    }

    pub fn path(&self) -> &Path {
        &self.repo
    }

    pub fn bare_path(&self) -> &Path {
        self.bare.path()
    }

    pub fn create_file(&self, name: &str, content: &str) {
        let path = self.repo.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(&path, content).expect("write file");
    }

    pub fn stage_all(&self) {
        run_git(self.repo.as_path(), &["add", "."]);
    }

    pub fn commit(&self, message: &str) {
        run_git(self.repo.as_path(), &["commit", "-m", message]);
    }

    pub fn create_branch(&self, name: &str) {
        run_git(self.repo.as_path(), &["checkout", "-b", name]);
    }

    pub fn checkout(&self, name: &str) {
        run_git(self.repo.as_path(), &["checkout", name]);
    }

    pub fn push(&self) {
        run_git(self.repo.as_path(), &["push"]);
    }

    pub fn push_new_branch(&self, branch: &str) {
        run_git(self.repo.as_path(), &["push", "-u", "origin", branch]);
    }

    pub fn head_sha(&self) -> String {
        run_git_output(self.repo.as_path(), &["rev-parse", "HEAD"])
    }

    pub fn bare_head_sha(&self) -> String {
        run_git_output_bare(self.bare.path(), &["rev-parse", "HEAD"])
    }
}

fn init_repo(dir: &Path) {
    run_git(dir, &["init", "-b", "main"]);
}

fn config_user(dir: &Path) {
    run_git(dir, &["config", "user.email", "test@test.com"]);
    run_git(dir, &["config", "user.name", "Jack"]);
}

fn create_initial_commit(dir: &Path) {
    std::fs::write(dir.join("README.md"), "initial").expect("write readme");
    run_git(dir, &["add", "."]);
    run_git(dir, &["commit", "-m", "initial"]);
}

fn init_bare_remote(dir: &Path) {
    run_git(dir, &["init", "--bare", "-b", "main"]);
}

fn add_remote(work: &Path, bare: &Path) {
    let bare_str = bare.to_str().expect("bare path utf8");
    run_git(work, &["remote", "add", "origin", bare_str]);
}

fn push_main(work: &Path) {
    run_git(work, &["push", "-u", "origin", "main"]);
}

fn run_git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    if !output.status.success() {
        panic!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn run_git_output(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    if !output.status.success() {
        panic!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn run_git_output_bare(dir: &Path, args: &[&str]) -> String {
    let dir_str = dir.to_str().expect("bare path utf8");
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(dir_str)
        .args(args)
        .output()
        .expect("run git");
    if !output.status.success() {
        panic!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
