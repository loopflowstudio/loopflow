use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// One lock for every fixture mutating process-global environment or store identity.
pub fn env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Use the runner's prepared CLI, or build it with Cargo's target-directory selection.
pub fn lf_binary() -> &'static str {
    static BINARY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BINARY.get_or_init(|| {
        if let Ok(binary) = std::env::var("LOOPFLOW_TEST_LF") {
            return binary;
        }
        let mut command = Command::new(env!("CARGO"));
        command
            .args([
                "build",
                "--offline",
                "-p",
                "loopflow",
                "--bin",
                "lf",
                "--message-format=json",
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdin(std::process::Stdio::null());
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("LF_") {
                command.env_remove(name);
            }
        }
        let output = command.output().expect("build fixture CLI");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("Cargo JSON output")
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find_map(|entry| {
                (entry["target"]["name"] == "lf")
                    .then(|| entry["executable"].as_str().map(str::to_owned))
                    .flatten()
            })
            .expect("Cargo reports the CLI artifact")
    })
}

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

    pub fn create_named_worktree(&self, name: &str) -> PathBuf {
        let parent = self.repo.parent().expect("repo parent");
        let repo_name = self
            .repo
            .file_name()
            .and_then(|name| name.to_str())
            .expect("repo name utf8");
        let worktree = parent.join(format!("{repo_name}.{name}"));
        let worktree_str = worktree.to_str().expect("worktree path utf8");
        run_git(
            self.repo.as_path(),
            &["worktree", "add", "-b", name, worktree_str, "main"],
        );
        worktree
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
    // Detached receiver maintenance can retain a publisher's inherited lock
    // after push returns. Disposable fixture remotes need no housekeeping.
    run_git(dir, &["config", "receive.autogc", "false"]);
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

/// Every `.jsonl` journal file anywhere under `root`, sorted. The channels-as-
/// wires invariant is a statement about the whole filesystem — "one journal per
/// served mind, zero per channel" — so proving it means looking everywhere, not
/// at the paths we expect.
pub fn journal_files_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "jsonl") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Read retained provider input for replay and preservation assertions in fixtures.
/// This never resolves Work or makes an acquisition/admission decision.
pub fn retained_task_input<T: serde::de::DeserializeOwned>(
    database: &Path,
    repo: &str,
    selector: &str,
) -> T {
    let conn = rusqlite::Connection::open(database).expect("fixture database");
    retained_task_input_in(&conn, repo, selector)
}

pub fn retained_task_input_in<T: serde::de::DeserializeOwned>(
    conn: &rusqlite::Connection,
    repo: &str,
    selector: &str,
) -> T {
    let (item, project, observed_at): (String, Option<String>, i64) = conn
        .query_row(
            "SELECT json_object('id',i.id,'identifier',i.identifier,'project_id',i.project_id,'revision',i.revision,'branch_name',i.branch_name,'url',i.url,'name',i.name,'description',i.description,'rank',i.rank,'completed',json(CASE i.completed WHEN 1 THEN 'true' ELSE 'false' END),'completed_at',i.completed_at,'due_date',i.due_date,'state',i.state,'team_id',i.team_id,'assignee',i.assignee),CASE WHEN p.id IS NOT NULL THEN json_object('id',p.id,'revision',p.revision,'slug',p.slug,'name',p.name,'summary',p.summary,'workflow',p.workflow,'status',p.status,'krs',json(p.krs),'metric_targets',json(p.metric_targets),'initiative_ids',json(p.initiative_ids),'team_ids',json(p.team_ids)) END,i.observed_at FROM pm_items i LEFT JOIN pm_projects p
         ON p.repo=i.repo AND p.provider=i.provider AND p.id=i.project_id
         WHERE i.repo=?1 AND i.provider='linear' AND (i.id=?2 OR i.identifier=?2)",
            rusqlite::params![repo, selector],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("retained fixture input");
    let item: serde_json::Value = serde_json::from_str(&item).expect("issue input");
    let project: serde_json::Value = project
        .map(|body| serde_json::from_str(&body).expect("project input"))
        .unwrap_or(serde_json::Value::Null);
    serde_json::from_value(
        serde_json::json!({"item":item,"project":project,"observed_at":observed_at}),
    )
    .expect("typed fixture input")
}
