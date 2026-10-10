pub mod worktree;
pub mod worktree_name;
pub mod worktrees;

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::GitError;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergeResult {
    pub success: bool,
    pub conflicts: Option<Vec<PathBuf>>,
    pub new_head: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BranchInfo {
    pub old_branch: String,
    pub old_head: String,
    pub new_branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum LandStrategy {
    SquashMerge,
    LocalMerge,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LandResult {
    pub merged_commit: String,
    pub branch_deleted: bool,
}

#[derive(Debug)]
pub(crate) struct WorktreeLease {
    path: PathBuf,
    file: File,
}

impl WorktreeLease {
    pub(crate) fn try_clone(&self) -> std::io::Result<Self> {
        Ok(Self {
            path: self.path.clone(),
            file: self.file.try_clone()?,
        })
    }

    /// Keep ordinary checkout removal excluded while this child uses it.
    pub(crate) fn inherit(&self, command: &mut Command) {
        let fd = self.file.as_raw_fd();
        command.env("LF_WORKTREE_LEASE_FD", fd.to_string());
        // SAFETY: the lease outlives child launch; fcntl is async-signal-safe
        // and changes only this owned descriptor's inheritance in the child.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
}

fn run_git(repo: &Path, args: &[&str]) -> Result<Output, GitError> {
    Ok(retained_output(repo, args)?)
}

type Question = (PathBuf, Vec<String>);
type RetainedReads = HashMap<Question, (Instant, Output)>;

/// Set only by a process that never writes through Git.
static RETAINED_READS: Mutex<Option<RetainedReads>> = Mutex::new(None);

/// How long a retained answer stands: repository layout rarely moves, while a
/// checkout's files change under a working agent.
const LAYOUT_RETENTION: Duration = Duration::from_secs(600);
const CONTENT_RETENTION: Duration = Duration::from_secs(60);

fn retained() -> MutexGuard<'static, Option<RetainedReads>> {
    RETAINED_READS.lock().expect("git read cache poisoned")
}

/// Let this process reuse a Git answer for a short time instead of spawning
/// `git` again. For a long-lived reader that asks the same questions on every
/// reading and changes nothing itself; one-shot commands never call this.
pub(crate) fn retain_reads() {
    *retained() = Some(HashMap::new());
}

pub(crate) fn retains_reads() -> bool {
    retained().is_some()
}

fn is_layout(args: &[String]) -> bool {
    let layout = ["--git-common-dir", "--show-toplevel"];
    args.iter().any(|arg| layout.contains(&arg.as_str()))
}

/// Ask Git and retain its answer. A reader must not take the index lock from
/// an agent working in the checkout.
fn ask(question: Question) -> std::io::Result<Output> {
    let output = Command::new("git")
        .current_dir(&question.0)
        .args(&question.1)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()?;
    if let Some(reads) = retained().as_mut() {
        reads.insert(question, (Instant::now(), output.clone()));
    }
    Ok(output)
}

/// `git -C repo args`, reusing a retained answer when this process keeps them.
///
/// The retained answers are never held while Git runs: a listing asks about
/// every checkout side by side.
pub(crate) fn retained_output(repo: &Path, args: &[&str]) -> std::io::Result<Output> {
    if !retains_reads() {
        return Command::new("git").current_dir(repo).args(args).output();
    }
    let question = (
        repo.to_path_buf(),
        args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
    );
    let stands = if is_layout(&question.1) {
        LAYOUT_RETENTION
    } else {
        CONTENT_RETENTION
    };
    let standing = retained()
        .as_ref()
        .and_then(|reads| reads.get(&question))
        .filter(|(read_at, _)| read_at.elapsed() < stands)
        .map(|(_, output)| output.clone());
    match standing {
        Some(output) => Ok(output),
        None => ask(question),
    }
}

/// Ask again what this process retains about `repo`'s contents. True when an
/// answer differs from the retained one; layout answers are left standing.
pub(crate) fn reread_retained(repo: &Path) -> bool {
    let questions = retained()
        .iter()
        .flatten()
        .filter(|((asked, args), _)| asked == repo && !is_layout(args))
        .map(|(question, (_, output))| (question.clone(), output.clone()))
        .collect::<Vec<_>>();
    let mut changed = false;
    // Every one is asked again, not only up to the first that differs.
    for (question, before) in questions {
        if let Ok(output) = ask(question) {
            changed |= output.status != before.status || output.stdout != before.stdout;
        }
    }
    changed
}

fn run_git_inheriting(
    repo: &Path,
    args: &[&str],
    inherit: &impl Fn(&mut Command),
) -> Result<Output, GitError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo).args(args);
    inherit(&mut command);
    Ok(command.output()?)
}

pub(crate) fn git_stdout(repo: &Path, args: &[&str]) -> Result<String, GitError> {
    successful_stdout(args, run_git(repo, args)?)
}

fn successful_stdout(args: &[&str], output: Output) -> Result<String, GitError> {
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_stdout_inheriting(
    repo: &Path,
    args: &[&str],
    inherit: &impl Fn(&mut Command),
) -> Result<String, GitError> {
    successful_stdout(args, run_git_inheriting(repo, args, inherit)?)
}

pub(crate) fn find_worktree_for_branch(
    repo: &Path,
    branch: &str,
) -> Result<Option<PathBuf>, GitError> {
    let output = run_git(repo, &["worktree", "list", "--porcelain"])?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: "git worktree list --porcelain".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_path: Option<PathBuf> = None;

    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = Some(PathBuf::from(path.trim()));
            continue;
        }

        if let Some(worktree_branch) = line.strip_prefix("branch ") {
            let worktree_branch = worktree_branch
                .trim()
                .strip_prefix("refs/heads/")
                .unwrap_or(worktree_branch.trim());
            if worktree_branch == branch {
                return Ok(current_path);
            }
        }
    }

    Ok(None)
}

fn run_gh(repo: &Path, args: &[&str]) -> Result<Output, GitError> {
    Ok(Command::new("gh").args(args).current_dir(repo).output()?)
}

fn gh_stdout(repo: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = run_gh(repo, args)?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("gh {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn list_conflicts(repo: &Path) -> Result<Vec<PathBuf>, GitError> {
    let output = git_stdout(repo, &["diff", "--name-only", "--diff-filter=U"])?;
    let conflicts = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    Ok(conflicts)
}

/// Fetch a remote ref (e.g., "origin/main").
pub fn fetch(repo: &Path, remote: &str, refspec: &str) -> Result<(), GitError> {
    fetch_inheriting(repo, remote, refspec, &|_| {})
}

pub(crate) fn fetch_inheriting(
    repo: &Path,
    remote: &str,
    refspec: &str,
    inherit: &impl Fn(&mut Command),
) -> Result<(), GitError> {
    let output = run_git_inheriting(repo, &["fetch", remote, refspec], inherit)?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git fetch {} {}", remote, refspec),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

pub(crate) fn has_origin(repo: &Path) -> Result<bool, GitError> {
    let output = run_git(repo, &["config", "--get", "remote.origin.url"])?;
    Ok(output.status.success() && !String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

/// Check if `commit` is an ancestor of `descendant`.
/// Returns true if commit is fully merged into descendant.
pub fn is_ancestor(repo: &Path, commit: &str, descendant: &str) -> Result<bool, GitError> {
    let output = run_git(repo, &["merge-base", "--is-ancestor", commit, descendant])?;
    if output.status.success() {
        return Ok(true);
    }
    if output.status.code() == Some(1) {
        return Ok(false);
    }
    Err(GitError::CommandFailed {
        command: format!("git merge-base --is-ancestor {commit} {descendant}"),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Whether this repo holds `revision` as a commit object.
pub fn commit_exists(repo: &Path, revision: &str) -> Result<bool, GitError> {
    let object = format!("{revision}^{{commit}}");
    let output = run_git(repo, &["cat-file", "-e", &object])?;
    Ok(output.status.success())
}

/// Commits reachable from `to` but not `from`, oldest first.
pub fn commits_between(
    repo: &Path,
    from: &str,
    to: &str,
) -> Result<Vec<(String, String)>, GitError> {
    let range = format!("{from}..{to}");
    let stdout = git_stdout(repo, &["log", "--reverse", "--format=%H%x00%s", &range])?;
    Ok(stdout
        .lines()
        .filter_map(|line| line.split_once('\0'))
        .map(|(revision, subject)| (revision.to_string(), subject.to_string()))
        .collect())
}

/// Find the merge-base (common ancestor) of two refs.
pub fn merge_base(repo: &Path, a: &str, b: &str) -> Result<String, GitError> {
    git_stdout(repo, &["merge-base", a, b]).map(|s| s.trim().to_string())
}

/// Checkout a ref (branch, tag, or commit).
pub fn checkout(repo: &Path, ref_name: &str) -> Result<(), GitError> {
    git_stdout(repo, &["checkout", ref_name])?;
    Ok(())
}

/// Create and checkout a new branch from current HEAD.
pub fn checkout_new_branch(repo: &Path, branch: &str) -> Result<(), GitError> {
    git_stdout(repo, &["checkout", "-b", branch])?;
    Ok(())
}

/// Create and checkout a new branch from an explicit ref.
pub fn checkout_new_branch_from(
    repo: &Path,
    branch: &str,
    start_point: &str,
) -> Result<(), GitError> {
    git_stdout(repo, &["checkout", "--no-track", "-b", branch, start_point])?;
    Ok(())
}

/// Stash the working tree including untracked files. Returns `true` when
/// something was stashed, `false` when the tree was already clean.
pub fn stash_including_untracked(repo: &Path) -> Result<bool, GitError> {
    if is_clean(repo)? {
        return Ok(false);
    }
    git_stdout(
        repo,
        &[
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "lf-rotate-carry",
        ],
    )?;
    Ok(true)
}

/// Reapply the most recent stash and drop it.
pub fn stash_pop(repo: &Path) -> Result<(), GitError> {
    git_stdout(repo, &["stash", "pop"])?;
    Ok(())
}

pub fn ref_exists(repo: &Path, ref_name: &str) -> Result<bool, GitError> {
    Ok(
        run_git(repo, &["rev-parse", "--verify", "--quiet", ref_name])?
            .status
            .success(),
    )
}

/// Push and set upstream tracking.
pub fn push_with_upstream(
    repo: &Path,
    remote: &str,
    branch: &str,
    inherit: &impl Fn(&mut Command),
) -> Result<(), GitError> {
    let output = run_git_inheriting(repo, &["push", "-u", remote, branch], inherit)?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git push -u {} {}", remote, branch),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

pub fn delete_remote_branch(repo: &Path, remote: &str, branch: &str) -> Result<(), GitError> {
    let output = run_git(repo, &["push", remote, "--delete", branch])?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git push {} --delete {}", remote, branch),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

pub fn delete_local_branch(repo: &Path, branch: &str) -> Result<(), GitError> {
    delete_local_branch_inheriting(repo, branch, &|_| {})
}

pub(crate) fn delete_local_branch_inheriting(
    repo: &Path,
    branch: &str,
    inherit: &impl Fn(&mut Command),
) -> Result<(), GitError> {
    let output = run_git_inheriting(repo, &["branch", "-D", branch], inherit)?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git branch -D {}", branch),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

pub fn branch_rename(repo: &Path, old_name: &str, new_name: &str) -> Result<(), GitError> {
    if let Some(worktree) = find_worktree_for_branch(repo, old_name)? {
        let command = format!("git -C {} branch -m {}", worktree.display(), new_name);
        let output = run_git(&worktree, &["branch", "-m", new_name])?;
        if output.status.success() {
            return Ok(());
        }

        return Err(GitError::CommandFailed {
            command,
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let command = format!("git branch -m {} {}", old_name, new_name);
    for attempt in 0..3 {
        let output = run_git(repo, &["branch", "-m", old_name, new_name])?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.contains(&format!("no branch named '{old_name}'")) {
            if let Some(worktree) = find_worktree_for_branch(repo, old_name)? {
                let fallback_command =
                    format!("git -C {} branch -m {}", worktree.display(), new_name);
                let fallback = run_git(&worktree, &["branch", "-m", new_name])?;
                if fallback.status.success() {
                    return Ok(());
                }

                let fallback_stderr = String::from_utf8_lossy(&fallback.stderr).to_string();
                if attempt < 2 && fallback_stderr.contains(&format!("no branch named '{old_name}'"))
                {
                    thread::sleep(Duration::from_millis(25));
                    continue;
                }

                return Err(GitError::CommandFailed {
                    command: fallback_command,
                    stderr: fallback_stderr,
                });
            }

            if attempt < 2 {
                thread::sleep(Duration::from_millis(25));
                continue;
            }
        }
        let lock_failed = stderr.contains("could not lock config file");
        if lock_failed && attempt < 2 {
            thread::sleep(Duration::from_millis(25));
            continue;
        }
        return Err(GitError::CommandFailed { command, stderr });
    }
    Err(GitError::CommandFailed {
        command,
        stderr: "git branch rename retry exhausted".to_string(),
    })
}

/// Get current branch name. Returns None if in detached HEAD state.
pub fn current_branch(repo: &Path) -> Result<Option<String>, GitError> {
    let output = run_git(repo, &["symbolic-ref", "--short", "HEAD"])?;
    if output.status.success() {
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    } else {
        // Detached HEAD
        Ok(None)
    }
}

/// Get the branch this checkout tracks on `origin`.
pub fn origin_branch(repo: &Path) -> Result<Option<String>, GitError> {
    let output = run_git(
        repo,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )?;
    if !output.status.success() {
        return Ok(None);
    }
    let upstream = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(upstream
        .strip_prefix("origin/")
        .filter(|branch| !branch.is_empty())
        .map(str::to_string))
}

/// Resolve the root of the working tree containing `repo`.
///
/// Reads the filesystem instead of launching Git: Session and Task readers
/// resolve every recorded checkout on each read.
pub fn worktree_root(repo: &Path) -> Result<PathBuf, GitError> {
    let start = repo.canonicalize()?;
    let root = start.ancestors().find(|dir| {
        let marker = dir.join(".git");
        // A linked worktree or submodule has a `.git` file naming its Git dir.
        marker.join("HEAD").is_file()
            || std::fs::read(&marker).is_ok_and(|bytes| bytes.starts_with(b"gitdir:"))
    });
    match root {
        Some(root) if !start.starts_with(root.join(".git")) => Ok(root.to_path_buf()),
        _ => Err(GitError::CommandFailed {
            command: "git rev-parse --show-toplevel".to_string(),
            stderr: format!("not a git work tree: {}", repo.display()),
        }),
    }
}

/// Return default branch name from origin/HEAD. Falls back to "main".
pub fn get_default_branch(repo: &Path) -> Result<String, GitError> {
    let output = run_git(
        repo,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )?;
    if !output.status.success() {
        return Ok("main".to_string());
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return Ok("main".to_string());
    }
    if let Some((_, branch)) = raw.split_once('/') {
        if !branch.trim().is_empty() {
            return Ok(branch.trim().to_string());
        }
    }
    Ok(raw)
}

/// Return true if working tree is clean.
pub fn is_clean(repo: &Path) -> Result<bool, GitError> {
    is_clean_for_pathspec(repo, &[])
}

/// Return true when only gate artifacts under `scratch/` are dirty.
pub fn is_materially_clean(repo: &Path) -> Result<bool, GitError> {
    is_clean_for_pathspec(repo, &[".", ":(exclude)scratch", ":(exclude)scratch/**"])
}

fn is_clean_for_pathspec(repo: &Path, pathspec: &[&str]) -> Result<bool, GitError> {
    let mut args = vec!["status", "--porcelain"];
    args.extend_from_slice(pathspec);
    let output = run_git(repo, &args)?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

/// Repository state relevant to a Task flow's no-progress check.
///
/// The returned text is an opaque comparison value, not a user-facing diff.
/// It includes committed, staged, tracked working-tree, and untracked-file
/// changes without reading ignored files.
pub fn worktree_state(repo: &Path) -> Result<String, GitError> {
    worktree_state_for_pathspec(repo, &[])
}

/// Repository state that ignores gate-only artifacts under `scratch/`.
pub fn material_worktree_state(repo: &Path) -> Result<String, GitError> {
    worktree_state_for_pathspec(repo, &[".", ":(exclude)scratch", ":(exclude)scratch/**"])
}

fn worktree_state_for_pathspec(repo: &Path, pathspec: &[&str]) -> Result<String, GitError> {
    let head = git_stdout(repo, &["rev-parse", "HEAD"])?;
    let mut status_args = vec!["status", "--porcelain"];
    status_args.extend_from_slice(pathspec);
    let status = git_stdout(repo, &status_args)?;
    let mut diff_args = vec!["diff", "--binary", "HEAD"];
    diff_args.extend_from_slice(pathspec);
    let diff = git_stdout(repo, &diff_args)?;
    let mut untracked_args = vec!["ls-files", "--others", "--exclude-standard", "-z"];
    untracked_args.extend_from_slice(pathspec);
    let untracked = git_stdout(repo, &untracked_args)?;
    let mut untracked_state = String::new();
    for relative in untracked.split('\0').filter(|path| !path.is_empty()) {
        let path = repo.join(relative);
        let metadata = std::fs::symlink_metadata(&path)?;
        let contents = if metadata.file_type().is_symlink() {
            std::fs::read_link(path)?
                .as_os_str()
                .as_encoded_bytes()
                .to_vec()
        } else if metadata.is_file() {
            std::fs::read(path)?
        } else {
            Vec::new()
        };
        let digest = hex::encode(Sha256::digest(contents));
        untracked_state.push_str(relative);
        untracked_state.push('\0');
        untracked_state.push_str(&digest);
        untracked_state.push('\0');
    }
    Ok(format!("{head}\0{status}\0{diff}\0{untracked_state}"))
}

/// Stage all changes.
pub fn stage_all(repo: &Path, inherit: &impl Fn(&mut Command)) -> Result<(), GitError> {
    git_stdout_inheriting(repo, &["add", "-A"], inherit)?;
    Ok(())
}

/// Committer identity used when the environment configures none.
///
/// Headless hosts (GitHub Actions runners, the self-hosted cron Mac) have no
/// `user.name`/`user.email`, so `git commit` refuses with "empty ident name".
/// Automated commits are correctly attributed to loopflow rather than a person.
const FALLBACK_COMMITTER_NAME: &str = "loopflow";
const FALLBACK_COMMITTER_EMAIL: &str = "loopflow@users.noreply.github.com";

/// Return true if the repo has both a `user.name` and `user.email` configured.
///
/// A developer's local commits keep their own identity; only a truly absent
/// identity (empty output or non-zero exit) is treated as missing.
fn has_git_identity(repo: &Path) -> bool {
    let configured = |key: &str| -> bool {
        run_git(repo, &["config", key])
            .map(|out| {
                out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty()
            })
            .unwrap_or(false)
    };
    configured("user.name") && configured("user.email")
}

/// Commit with message.
///
/// When the environment has no git identity configured, supplies a fallback
/// committer via `-c user.name/-c user.email` so headless commits (e.g. the
/// release version bump on a CI runner) succeed. A configured identity is
/// never overridden.
pub fn commit(repo: &Path, message: &str, inherit: &impl Fn(&mut Command)) -> Result<(), GitError> {
    if has_git_identity(repo) {
        git_stdout_inheriting(repo, &["commit", "-m", message], inherit)?;
    } else {
        git_stdout_inheriting(
            repo,
            &[
                "-c",
                &format!("user.name={FALLBACK_COMMITTER_NAME}"),
                "-c",
                &format!("user.email={FALLBACK_COMMITTER_EMAIL}"),
                "commit",
                "-m",
                message,
            ],
            inherit,
        )?;
    }
    Ok(())
}

pub fn pr_exists(repo: &Path) -> Result<bool, GitError> {
    let output = run_gh(repo, &["pr", "view", "--json", "state", "-q", ".state"])?;
    if !output.status.success() {
        return Ok(false);
    }
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

pub fn pr_create_draft(repo: &Path) -> Result<String, GitError> {
    let url = gh_stdout(repo, &["pr", "create", "--draft", "--fill"])?;
    Ok(url.trim().to_string())
}

pub fn pr_merge_squash_auto(repo: &Path) -> Result<(), GitError> {
    gh_stdout(repo, &["pr", "merge", "--squash", "--auto"])?;
    Ok(())
}

/// Fetch origin/main_branch and reset the local branch to that exact commit.
///
/// This release/relocation helper discards unpublished branch history. Ordinary
/// checkout refreshes use `ops::checkout::refresh_main` to preserve local work.
///
/// If `main_branch` is checked out in any worktree, that worktree is reset to
/// `origin/main_branch` (dirty state is stashed first and popped afterward).
/// Resetting via `git update-ref` alone would advance the ref while leaving
/// the main worktree's index and working tree at the old commit, which
/// produces a phantom-dirty `git status` until the next checkout.
///
/// Returns true if up-to-date or updated, false if the local branch can't be
/// synced.
pub fn sync_main(repo: &Path, main_branch: &str) -> Result<bool, GitError> {
    let output = run_git(repo, &["fetch", "origin", main_branch])?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git fetch origin {}", main_branch),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let origin_ref = format!("origin/{}", main_branch);
    let local_ref = format!("refs/heads/{}", main_branch);

    // Prefer resetting inside whichever worktree has the branch checked out,
    // so HEAD, index, and working tree stay in lockstep.
    let target = if current_branch(repo)? == Some(main_branch.to_string()) {
        Some(repo.to_path_buf())
    } else {
        find_worktree_for_branch(repo, main_branch)?
    };

    if let Some(main_worktree) = target {
        return reset_worktree_to(&main_worktree, &origin_ref);
    }

    // Branch isn't checked out anywhere — safe to move the ref directly.
    let output = run_git(repo, &["update-ref", &local_ref, &origin_ref])?;
    if !output.status.success() {
        tracing::warn!(
            "Failed to update local {} to match {}",
            main_branch,
            origin_ref
        );
    }
    Ok(output.status.success())
}

/// Reset `worktree` to `target_ref`, preserving genuine local-only edits.
///
/// Dirty state is stashed before the hard reset. The stash is only popped back
/// when the dirty paths are disjoint from the paths the reset itself rewrites
/// (`HEAD..target_ref`). Popping a stash across an *overlapping* reset — e.g.
/// when the default branch just absorbed a merge that rewrote the same files —
/// does a 3-way merge that can silently resurrect stale content and revert the
/// merged work. When the paths overlap we leave the edits in a named stash for
/// deliberate recovery rather than corrupt the freshly-synced tree.
fn reset_worktree_to(worktree: &Path, target_ref: &str) -> Result<bool, GitError> {
    let status = run_git(worktree, &["status", "--porcelain"])?;
    let dirty_paths = if status.status.success() {
        porcelain_paths(&String::from_utf8_lossy(&status.stdout))
    } else {
        Vec::new()
    };

    let mut stashed = false;
    if !dirty_paths.is_empty() {
        let stash = run_git(
            worktree,
            &[
                "stash",
                "push",
                "--include-untracked",
                "-m",
                "sync_main: auto-stash",
            ],
        )?;
        stashed = stash.status.success();
        if !stashed {
            // Stash can fail during a conflicted merge or with odd index state.
            // Log and continue — the reset will clear the bad state.
            tracing::warn!(
                "sync_main: stash failed in {}: {}",
                worktree.display(),
                String::from_utf8_lossy(&stash.stderr).trim()
            );
        }
    }

    // Capture what the reset will rewrite while HEAD still points at the old tip.
    let changed = changed_paths(worktree, target_ref);

    let reset = run_git(worktree, &["reset", "--hard", target_ref])?;
    let ok = reset.status.success();

    if stashed {
        if dirty_paths.iter().any(|p| changed.contains(p)) {
            tracing::warn!(
                "sync_main: {} had local edits to paths the default branch also \
                 rewrote; left them in a stash (\"sync_main: auto-stash\") rather than \
                 popping over the synced tree. Recover with `git stash pop`.",
                worktree.display()
            );
        } else {
            let pop = run_git(worktree, &["stash", "pop"])?;
            if !pop.status.success() {
                tracing::warn!(
                    "sync_main: stash pop failed in {}; stash preserved",
                    worktree.display()
                );
            }
        }
    }

    Ok(ok)
}

/// Paths reported by `git status --porcelain`, including both sides of a rename.
fn porcelain_paths(output: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in output.lines() {
        if line.len() < 4 {
            continue;
        }
        let rest = &line[3..];
        match rest.split_once(" -> ") {
            Some((from, to)) => {
                paths.push(unquote_path(from));
                paths.push(unquote_path(to));
            }
            None => paths.push(unquote_path(rest)),
        }
    }
    paths
}

/// Paths that differ between the worktree's current `HEAD` and `target_ref`.
/// Returns empty on any git failure — callers treat "unknown" as "no overlap".
fn changed_paths(worktree: &Path, target_ref: &str) -> Vec<String> {
    let Ok(output) = run_git(worktree, &["diff", "--name-only", "HEAD", target_ref]) else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn unquote_path(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(trimmed)
        .to_string()
}

fn normalized_worktree_path(repo: &Path, path: &Path) -> PathBuf {
    if let Ok(path) = path.canonicalize() {
        return path;
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    };
    let Some(name) = absolute.file_name() else {
        return absolute;
    };
    absolute
        .parent()
        .and_then(|parent| parent.canonicalize().ok())
        .map(|parent| parent.join(name))
        .unwrap_or(absolute)
}

fn worktree_lease_path(repo: &Path, path: &Path) -> Result<PathBuf, GitError> {
    let common_dir = git_stdout(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let lock_dir = PathBuf::from(common_dir.trim())
        .join("loopflow")
        .join("worktree-locks");
    fs::create_dir_all(&lock_dir)?;
    let path = normalized_worktree_path(repo, path);
    let digest = hex::encode(Sha256::digest(path.as_os_str().as_encoded_bytes()));
    Ok(lock_dir.join(format!("{digest}.lock")))
}

pub(crate) fn acquire_worktree_lease(
    repo: &Path,
    path: &Path,
    owner: &str,
) -> Result<WorktreeLease, GitError> {
    acquire_worktree_lease_wait(repo, path, owner, Duration::ZERO)
}

pub(crate) fn acquire_worktree_lease_wait(
    repo: &Path,
    path: &Path,
    owner: &str,
    timeout: Duration,
) -> Result<WorktreeLease, GitError> {
    let lock_path = worktree_lease_path(repo, path)?;
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;
    let deadline = Instant::now() + timeout;
    while let Err(error) = fs2::FileExt::try_lock_exclusive(&file) {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            if Instant::now() < deadline {
                thread::sleep(Duration::from_millis(25));
                continue;
            }
            let active_owner = fs::read_to_string(&lock_path)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "another active operation".to_string());
            return Err(GitError::CommandFailed {
                command: "acquire worktree lease".to_string(),
                stderr: format!(
                    "{} is owned by {active_owner}",
                    normalized_worktree_path(repo, path).display()
                ),
            });
        }
        return Err(error.into());
    }
    file.set_len(0)?;
    file.write_all(owner.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(WorktreeLease {
        path: normalized_worktree_path(repo, path),
        file,
    })
}

pub fn worktree_remove(repo: &Path, path: &Path) -> Result<(), GitError> {
    let lease = acquire_worktree_lease(repo, path, "worktree removal")?;
    worktree_remove_owned(repo, path, &lease, &|_| {})
}

pub(crate) fn worktree_remove_owned(
    repo: &Path,
    path: &Path,
    lease: &WorktreeLease,
    inherit: &impl Fn(&mut Command),
) -> Result<(), GitError> {
    let path = normalized_worktree_path(repo, path);
    if lease.path != path {
        return Err(GitError::CommandFailed {
            command: "remove owned worktree".to_string(),
            stderr: format!("lease does not own {}", path.display()),
        });
    }
    let path_str = path.to_string_lossy();
    let output = run_git_inheriting(
        repo,
        &["worktree", "remove", "--force", path_str.as_ref()],
        &|command| {
            lease.inherit(command);
            inherit(command);
        },
    )?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git worktree remove --force {}", path.display()),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

/// Move a worktree to a new path.
pub fn worktree_move(repo: &Path, old_path: &Path, new_path: &Path) -> Result<(), GitError> {
    let old_str = old_path.to_string_lossy();
    let new_str = new_path.to_string_lossy();
    let output = run_git(
        repo,
        &["worktree", "move", old_str.as_ref(), new_str.as_ref()],
    )?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git worktree move {} {}", old_str, new_str),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

/// How to set up the branch when creating a worktree.
#[derive(Debug)]
pub enum WorktreeBranch<'a> {
    /// `git worktree add --no-track -b <branch> <path> <start_point>`
    New { start_point: &'a str },
    /// `git worktree add --track -b <branch> <path> <remote>`
    Track { remote: &'a str },
    /// `git worktree add <path> <branch>`
    Existing,
}

/// Create a worktree for a branch.
pub fn worktree_add(
    repo: &Path,
    path: &Path,
    branch: &str,
    mode: WorktreeBranch<'_>,
) -> Result<(), GitError> {
    worktree_add_inheriting(repo, path, branch, mode, &|_| {})
}

pub(crate) fn worktree_add_inheriting(
    repo: &Path,
    path: &Path,
    branch: &str,
    mode: WorktreeBranch<'_>,
    inherit: &impl Fn(&mut Command),
) -> Result<(), GitError> {
    let path_str = path.to_string_lossy();
    let args: Vec<&str> = match mode {
        WorktreeBranch::New { start_point } => {
            vec![
                "worktree",
                "add",
                "--no-track",
                "-b",
                branch,
                path_str.as_ref(),
                start_point,
            ]
        }
        WorktreeBranch::Track { remote } => {
            vec![
                "worktree",
                "add",
                "--track",
                "-b",
                branch,
                path_str.as_ref(),
                remote,
            ]
        }
        WorktreeBranch::Existing => {
            vec!["worktree", "add", path_str.as_ref(), branch]
        }
    };
    let output = run_git_inheriting(repo, &args, inherit)?;
    if !output.status.success() {
        // A failing post-checkout hook causes git to exit non-zero even when
        // the worktree was created successfully. Verify before reporting failure.
        if path.join(".git").exists() {
            return Ok(());
        }
        return Err(GitError::CommandFailed {
            command: format!("git {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

/// Read a file's contents at a given revision.
/// Returns `None` if the path does not exist at that revision.
pub fn show_file(repo: &Path, rev: &str, path: &str) -> Result<Option<String>, GitError> {
    let output = run_git(repo, &["show", &format!("{rev}:{path}")])?;
    if output.status.success() {
        Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
    } else {
        Ok(None)
    }
}

/// List files in a directory at a given revision.
/// Returns file names (not full paths) for blobs directly under the tree.
pub fn list_tree(repo: &Path, rev: &str, dir: &str) -> Result<Vec<String>, GitError> {
    let tree_path = if dir.is_empty() {
        rev.to_string()
    } else {
        format!("{rev}:{dir}")
    };
    let output = run_git(repo, &["ls-tree", "--name-only", &tree_path])?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect())
}

/// Get the SHA for a ref (branch, tag, HEAD, etc.).
pub fn rev_parse(repo: &Path, refspec: &str) -> Result<String, GitError> {
    let sha = git_stdout(repo, &["rev-parse", refspec])?
        .trim()
        .to_string();
    Ok(sha)
}

/// Check if a branch has any commits not in target.
pub fn has_commits_beyond(repo: &Path, branch: &str, target: &str) -> Result<bool, GitError> {
    let branch_sha = rev_parse(repo, branch)?;
    let base_sha = merge_base(repo, branch, target)?;
    Ok(branch_sha != base_sha)
}

/// Check if a branch has been squash-merged into target.
///
/// Simulates merging branch into target and checks if the resulting tree
/// is identical to target's tree (meaning branch adds nothing new).
pub fn is_squash_merged(repo: &Path, branch: &str, target: &str) -> Result<bool, GitError> {
    let output = run_git(repo, &["merge-tree", "--write-tree", target, branch])?;
    if !output.status.success() {
        // Conflicts mean it's not cleanly merged
        return Ok(false);
    }
    let result_tree = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let target_tree = rev_parse(repo, &format!("{target}^{{tree}}"))?;
    Ok(result_tree == target_tree)
}

/// Return true if `branch` is already merged into `target`.
///
/// Covers both shapes a merge can take: a fast-forwardable ancestor (its
/// commits live in `target`) and a squash-merge (its *changes* live in
/// `target` even though its commits do not). A stacked child must re-parent
/// onto the default branch once its parent is merged either way.
pub fn is_merged_into(repo: &Path, branch: &str, target: &str) -> Result<bool, GitError> {
    if is_ancestor(repo, branch, target)? {
        return Ok(true);
    }
    is_squash_merged(repo, branch, target)
}

/// Merge one target tree, retaining branch history. A squash-landed stack uses
/// its recorded fork as the comparison base without adding synthetic ancestry.
pub fn merge(
    worktree: &Path,
    target: &str,
    fork_base: Option<&str>,
) -> Result<MergeResult, GitError> {
    if let Some(state) = intervention_state(worktree)? {
        return Err(GitError::CommandFailed {
            command: "git merge".to_string(),
            stderr: format!("refusing to merge while a {state} operation is in progress"),
        });
    }
    let target_sha = rev_parse(worktree, target)?;
    let target = target_sha.as_str();
    if is_ancestor(worktree, target, "HEAD")? {
        return Ok(MergeResult {
            success: true,
            conflicts: None,
            new_head: Some(rev_parse(worktree, "HEAD")?),
        });
    }
    let comparison = match fork_base {
        Some(base) if !is_ancestor(worktree, base, target)? => {
            let tree = rev_parse(worktree, &format!("{target}^{{tree}}"))?;
            Some(
                git_stdout(
                    worktree,
                    &[
                        "commit-tree",
                        &tree,
                        "-p",
                        base,
                        "-m",
                        "lf sync: stack comparison tree",
                    ],
                )?
                .trim()
                .to_string(),
            )
        }
        _ => None,
    };
    let merge_target = comparison.as_deref().unwrap_or(target);
    let output = run_git(
        worktree,
        &[
            "-c",
            "rerere.enabled=true",
            "-c",
            "rerere.autoupdate=false",
            "merge",
            "--no-ff",
            "--no-commit",
            merge_target,
        ],
    )?;
    if intervention_state(worktree)? == Some("merge") {
        // Git has populated the index/worktree with its normal three-way merge.
        // Record the real target before either committing or handing off recovery.
        write_merge_target(worktree, target)?;
        let preserve_scratch =
            fork_base.is_some() || crate::git::worktrees::is_persistent_worktree(worktree)?;
        if preserve_scratch {
            // Scratch belongs to the child, including deletions and cleanly
            // merged parent additions. Restoring changed scratch paths also resolves
            // modify/delete conflicts which a text merge cannot handle.
            restore_scratch_from_head(worktree)?;
        }
        if output.status.success() || (preserve_scratch && list_conflicts(worktree)?.is_empty()) {
            return continue_merge(worktree, None);
        }
    } else if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: "git merge".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    let conflicts = list_conflicts(worktree)?;
    Ok(MergeResult {
        success: output.status.success(),
        conflicts: (!conflicts.is_empty()).then_some(conflicts),
        new_head: output
            .status
            .success()
            .then(|| rev_parse(worktree, "HEAD"))
            .transpose()?,
    })
}

fn restore_scratch_from_head(worktree: &Path) -> Result<(), GitError> {
    // Restore only paths changed in the merge index. Unrelated working edits
    // and untracked child notes never enter this path list.
    let paths = git_stdout(
        worktree,
        &[
            "diff",
            "--cached",
            "--name-only",
            "--no-renames",
            "-z",
            "HEAD",
            "--",
            "scratch",
        ],
    )?;
    if paths.is_empty() {
        return Ok(());
    }
    let child_paths = git_stdout(
        worktree,
        &[
            "ls-tree",
            "-r",
            "--name-only",
            "-z",
            "HEAD",
            "--",
            "scratch",
        ],
    )?;
    let (restore, remove): (Vec<_>, Vec<_>) = paths
        .split('\0')
        .filter(|path| !path.is_empty())
        .partition(|path| child_paths.split('\0').any(|child| child == *path));
    // restore cannot resolve an unmerged path absent from the source tree.
    for (mut args, paths) in [
        (vec!["--literal-pathspecs", "rm", "-f", "--"], remove),
        (
            vec![
                "--literal-pathspecs",
                "restore",
                "--source=HEAD",
                "--staged",
                "--worktree",
                "--",
            ],
            restore,
        ),
    ] {
        if !paths.is_empty() {
            args.extend(paths);
            git_stdout(worktree, &args)?;
        }
    }
    Ok(())
}

fn write_merge_target(worktree: &Path, target: &str) -> Result<(), GitError> {
    let directory = absolute_git_dir(worktree)?;
    fs::write(directory.join("MERGE_HEAD"), format!("{target}\n"))?;
    fs::write(directory.join("MERGE_MSG"), format!("Merge {target}\n"))?;
    Ok(())
}

/// Stage the resolved conflict paths and complete an in-progress merge.
pub fn continue_merge(worktree: &Path, target: Option<&str>) -> Result<MergeResult, GitError> {
    if let Some(target) = target {
        // Also runs after owner death between Git's merge and target replacement.
        write_merge_target(worktree, target)?;
    }
    let conflicts = list_conflicts(worktree)?;
    if !conflicts.is_empty() {
        let output = Command::new("git")
            .arg("-C")
            .arg(worktree)
            .args(["add", "--"])
            .args(&conflicts)
            .output()?;
        if !output.status.success() {
            return Err(GitError::CommandFailed {
                command: "git add -- <resolved conflicts>".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
    }

    let output = run_git(
        worktree,
        &[
            "-c",
            "rerere.enabled=true",
            "-c",
            "rerere.autoupdate=false",
            "-c",
            "core.editor=true",
            "commit",
            "--no-edit",
        ],
    )?;
    if output.status.success() {
        return Ok(MergeResult {
            success: true,
            conflicts: None,
            new_head: Some(rev_parse(worktree, "HEAD")?),
        });
    }

    let conflicts = list_conflicts(worktree)?;
    Ok(MergeResult {
        success: false,
        conflicts: (!conflicts.is_empty()).then_some(conflicts),
        new_head: None,
    })
}

/// Conflict paths whose current resolution was not supplied by rerere.
///
/// Loopflow keeps rerere auto-staging disabled. An empty result while Git still
/// reports unmerged paths means every conflict path was populated from a
/// previously reviewed resolution and may be staged explicitly for continue.
pub fn rerere_remaining(worktree: &Path) -> Result<Vec<PathBuf>, GitError> {
    let output = run_git(
        worktree,
        &[
            "-c",
            "rerere.enabled=true",
            "-c",
            "rerere.autoupdate=false",
            "rerere",
            "remaining",
        ],
    )?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: "git rerere remaining".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect())
}

/// Abort an in-progress merge and restore its original state.
pub fn abort_merge(worktree: &Path) -> Result<(), GitError> {
    git_stdout(worktree, &["merge", "--abort"])?;
    Ok(())
}

/// `Some(kind)` when the worktree is mid-rebase, merge, cherry-pick, revert, or
/// bisect — a crash boundary a Task recovery must not adopt. Reads the
/// worktree's own git dir (`git rev-parse --absolute-git-dir`) so linked
/// worktrees inspect their private state, not the shared main repo.
pub fn intervention_state(worktree: &Path) -> Result<Option<&'static str>, GitError> {
    let git_dir = absolute_git_dir(worktree)?;
    let present = |name: &str| std::fs::metadata(git_dir.join(name)).is_ok();
    if present("rebase-merge") || present("rebase-apply") {
        return Ok(Some("rebase"));
    }
    if present("MERGE_HEAD") {
        return Ok(Some("merge"));
    }
    if present("CHERRY_PICK_HEAD") {
        return Ok(Some("cherry-pick"));
    }
    if present("REVERT_HEAD") {
        return Ok(Some("revert"));
    }
    if present("BISECT_LOG") {
        return Ok(Some("bisect"));
    }
    Ok(None)
}

pub fn absolute_git_dir(worktree: &Path) -> Result<PathBuf, GitError> {
    let raw = git_stdout(worktree, &["rev-parse", "--absolute-git-dir"])?;
    Ok(PathBuf::from(raw.trim()))
}

pub fn create_branch(worktree: &Path, name: &str) -> Result<BranchInfo, GitError> {
    let old_branch = git_stdout(worktree, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string();
    let old_head = git_stdout(worktree, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    git_stdout(worktree, &["checkout", "-b", name])?;

    Ok(BranchInfo {
        old_branch,
        old_head,
        new_branch: name.to_string(),
    })
}

pub fn push(
    worktree: &Path,
    force_with_lease: bool,
    inherit: &impl Fn(&mut Command),
) -> Result<(), GitError> {
    let mut args = vec!["push"];
    if force_with_lease {
        args.push("--force-with-lease");
    }
    git_stdout_inheriting(worktree, &args, inherit)?;
    Ok(())
}

pub fn land(
    worktree: &Path,
    strategy: LandStrategy,
    main_branch: &str,
) -> Result<LandResult, GitError> {
    let feature_branch = git_stdout(worktree, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string();
    git_stdout(worktree, &["checkout", main_branch])?;
    match strategy {
        LandStrategy::SquashMerge => {
            git_stdout(worktree, &["merge", "--squash", &feature_branch])?;
            git_stdout(
                worktree,
                &["commit", "-m", &format!("Squash merge {}", feature_branch)],
            )?;
        }
        LandStrategy::LocalMerge => {
            git_stdout(
                worktree,
                &[
                    "merge",
                    "--no-ff",
                    &feature_branch,
                    "-m",
                    &format!("Merge {}", feature_branch),
                ],
            )?;
        }
    }
    let merged_commit = git_stdout(worktree, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    let delete_output = run_git(worktree, &["branch", "-d", &feature_branch])?;
    let branch_deleted = delete_output.status.success();
    Ok(LandResult {
        merged_commit,
        branch_deleted,
    })
}

/// List file paths changed between two commits.
pub fn diff_names(repo: &Path, old: &str, new: &str) -> Result<Vec<PathBuf>, GitError> {
    let output = git_stdout(repo, &["diff", "--name-only", old, new])?;
    Ok(output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect())
}

/// Like `diff_names` but scoped to paths under `prefix`.
pub fn diff_names_under(
    repo: &Path,
    old: &str,
    new: &str,
    prefix: &str,
) -> Result<Vec<PathBuf>, GitError> {
    let output = git_stdout(repo, &["diff", "--name-only", old, new, "--", prefix])?;
    Ok(output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect())
}

/// Find the most recent commit on the current branch whose message contains `pattern`.
/// Returns the commit SHA, or `None` if no match.
pub fn log_grep(repo: &Path, pattern: &str) -> Result<Option<String>, GitError> {
    let output = run_git(
        repo,
        &["log", "--grep", pattern, "--format=%H", "-1", "HEAD"],
    )?;
    if !output.status.success() {
        return Ok(None);
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() {
        Ok(None)
    } else {
        Ok(Some(sha))
    }
}

/// Hash the contents of areas in a repo using git ls-tree.
///
/// Returns a hex-encoded SHA-256 digest of the `git ls-tree` output for the
/// given paths. Changes to any file in any area will produce a different hash.
/// Fast: reads from the git index, no file I/O.
pub fn hash_areas(repo: &Path, areas: &[String]) -> Result<String, GitError> {
    use sha2::{Digest, Sha256};

    let mut args = vec!["ls-tree", "-r", "HEAD", "--"];
    for area in areas {
        args.push(area);
    }
    let output = git_stdout(repo, &args)?;
    let digest = Sha256::digest(output.as_bytes());
    Ok(hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn git_reads_from_several_threads_run_side_by_side() {
        let dir = tempfile::tempdir().expect("create temp dir");
        // Each read leaves its mark, then succeeds only once the other's is
        // there too: reads taken in turn would both give up.
        let meet = "alias.meet=!f() { touch \"$1\"; n=0; \
                    while [ ! -e \"$2\" ]; do n=$((n+1)); [ $n -gt 300 ] && exit 1; sleep 0.1; done; }; f";
        let (first, second) = (dir.path().join("first"), dir.path().join("second"));
        let read = |mine: &Path, theirs: &Path| {
            let (mine, theirs) = (mine.to_string_lossy(), theirs.to_string_lossy());
            run_git(dir.path(), &["-c", meet, "meet", &mine, &theirs])
                .expect("run git")
                .status
                .success()
        };

        let (one, other) = thread::scope(|scope| {
            let one = scope.spawn(|| read(&first, &second));
            (read(&second, &first), one.join().expect("reader panicked"))
        });

        assert!(one && other);
    }

    fn init_repo() -> TempDir {
        let dir = tempfile::tempdir().expect("create temp dir");
        git_stdout(dir.path(), &["init", "-b", "main"]).expect("git init");
        git_stdout(
            dir.path(),
            &["config", "user.email", "loopflow@example.com"],
        )
        .expect("set user.email");
        git_stdout(dir.path(), &["config", "user.name", "Loopflow"]).expect("set user.name");
        dir
    }

    fn commit_file(repo: &Path, name: &str, content: &str) {
        let path = repo.join(name);
        fs::write(&path, content).expect("write file");
        git_stdout(repo, &["add", name]).expect("git add");
        git_stdout(repo, &["commit", "-m", &format!("add {}", name)]).expect("git commit");
    }

    #[test]
    fn worktree_root_agrees_with_git_without_launching_it() {
        let repo = init_repo();
        commit_file(repo.path(), "a.txt", "a");
        let nested = repo.path().join("src/deep");
        fs::create_dir_all(&nested).expect("create nested dir");
        let linked = tempfile::tempdir().expect("create temp dir");
        let linked_root = linked.path().join("linked");
        git_stdout(
            repo.path(),
            &[
                "worktree",
                "add",
                "-b",
                "side",
                linked_root.to_str().unwrap(),
            ],
        )
        .expect("git worktree add");
        let alias = linked.path().join("alias");
        std::os::unix::fs::symlink(&nested, &alias).expect("symlink");

        for path in [
            repo.path(),
            nested.as_path(),
            linked_root.as_path(),
            alias.as_path(),
        ] {
            let expected = git_stdout(path, &["rev-parse", "--show-toplevel"]).expect("git root");
            assert_eq!(
                worktree_root(path).expect("root"),
                PathBuf::from(expected.trim())
            );
        }
        // Git's own directory, a plain directory and a retired checkout are not work trees.
        assert!(worktree_root(&repo.path().join(".git/refs")).is_err());
        assert!(worktree_root(linked.path()).is_err());
        assert!(worktree_root(&linked.path().join("retired")).is_err());
    }

    #[test]
    fn waiting_for_worktree_lease_never_displaces_its_owner() {
        let repo = init_repo();
        let lease = acquire_worktree_lease(repo.path(), repo.path(), "active repair").unwrap();
        let error = acquire_worktree_lease_wait(
            repo.path(),
            repo.path(),
            "cleanup",
            Duration::from_millis(50),
        )
        .unwrap_err();
        assert!(error.to_string().contains("owned by active repair"));
        assert!(repo.path().exists());
        drop(lease);
        assert!(acquire_worktree_lease(repo.path(), repo.path(), "cleanup").is_ok());
    }

    #[test]
    fn commit_supplies_fallback_identity_when_none_configured() {
        // Isolate from the developer's global/system git identity so the
        // no-identity path (as on a CI runner) is exercised deterministically.
        std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        std::env::set_var("GIT_CONFIG_SYSTEM", "/dev/null");

        let dir = tempfile::tempdir().expect("create temp dir");
        git_stdout(dir.path(), &["init", "-b", "main"]).expect("git init");
        assert!(
            !has_git_identity(dir.path()),
            "test repo should start with no identity"
        );

        fs::write(dir.path().join("README.md"), "hello").expect("write file");
        stage_all(dir.path(), &|_| {}).expect("stage");
        commit(dir.path(), "add readme", &|_| {})
            .expect("commit should succeed without configured identity");

        let email =
            git_stdout(dir.path(), &["log", "-1", "--format=%ae"]).expect("read author email");
        assert_eq!(email.trim(), FALLBACK_COMMITTER_EMAIL);
        let name =
            git_stdout(dir.path(), &["log", "-1", "--format=%an"]).expect("read author name");
        assert_eq!(name.trim(), FALLBACK_COMMITTER_NAME);
    }

    #[test]
    fn git_create_branch_records_old_state() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        let info = create_branch(repo.path(), "feature").expect("create branch");
        assert_eq!(info.old_branch, "main");
        assert_eq!(info.new_branch, "feature");
        assert!(!info.old_head.is_empty());
    }

    #[test]
    fn git_merge_succeeds_on_linear_history() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");
        create_branch(repo.path(), "feature").expect("create branch");
        commit_file(repo.path(), "feature.txt", "feature");

        git_stdout(repo.path(), &["checkout", "main"]).expect("checkout main");
        commit_file(repo.path(), "main.txt", "main");

        git_stdout(repo.path(), &["checkout", "feature"]).expect("checkout feature");
        let result = merge(repo.path(), "main", None).expect("merge");
        assert!(result.success);
        assert!(result.new_head.is_some());
    }

    #[test]
    fn git_merge_can_preserve_and_continue_conflicts() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "base\n");
        create_branch(repo.path(), "feature").expect("create branch");
        fs::write(repo.path().join("README.md"), "feature\n").expect("write feature");
        git_stdout(repo.path(), &["add", "README.md"]).expect("stage feature");
        git_stdout(repo.path(), &["commit", "-m", "feature change"]).expect("commit feature");

        git_stdout(repo.path(), &["checkout", "main"]).expect("checkout main");
        fs::write(repo.path().join("README.md"), "main\n").expect("write main");
        git_stdout(repo.path(), &["add", "README.md"]).expect("stage main");
        git_stdout(repo.path(), &["commit", "-m", "main change"]).expect("commit main");
        git_stdout(repo.path(), &["checkout", "feature"]).expect("checkout feature");

        let conflicted = merge(repo.path(), "main", None).expect("start manual merge");
        assert!(!conflicted.success);
        assert_eq!(conflicted.conflicts, Some(vec![PathBuf::from("README.md")]));
        assert_eq!(list_conflicts(repo.path()).unwrap().len(), 1);

        fs::write(repo.path().join("README.md"), "main\nfeature\n").expect("resolve conflict");
        let completed = continue_merge(repo.path(), None).expect("continue merge");
        assert!(completed.success);
        assert_eq!(
            current_branch(repo.path()).unwrap(),
            Some("feature".to_string())
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("README.md")).unwrap(),
            "main\nfeature\n"
        );
    }

    #[test]
    fn git_push_force_with_lease() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        let remote_dir = tempfile::tempdir().expect("remote dir");
        git_stdout(remote_dir.path(), &["init", "--bare"]).expect("init bare");

        git_stdout(
            repo.path(),
            &[
                "remote",
                "add",
                "origin",
                remote_dir.path().to_str().expect("remote path"),
            ],
        )
        .expect("add remote");

        // Initial push to set upstream
        push_with_upstream(repo.path(), "origin", "main", &|_| {}).expect("initial push");

        // Now test force-with-lease push
        commit_file(repo.path(), "second.txt", "second commit");
        push(repo.path(), true, &|_| {}).expect("push force-with-lease");
    }

    #[test]
    fn git_is_ancestor_returns_true_when_merged() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");
        let base_commit = git_stdout(repo.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .to_string();

        create_branch(repo.path(), "feature").expect("create branch");
        commit_file(repo.path(), "feature.txt", "feature");

        // feature is ahead of base_commit, so base_commit is ancestor of feature
        assert!(is_ancestor(repo.path(), &base_commit, "feature").unwrap());

        // feature is not an ancestor of base_commit
        assert!(!is_ancestor(repo.path(), "feature", &base_commit).unwrap());
    }

    #[test]
    fn git_is_ancestor_after_merge() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        create_branch(repo.path(), "feature").expect("create branch");
        commit_file(repo.path(), "feature.txt", "feature");
        let feature_head = git_stdout(repo.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .to_string();

        // Merge feature into main
        git_stdout(repo.path(), &["checkout", "main"]).expect("checkout main");
        git_stdout(repo.path(), &["merge", "feature", "--no-ff", "-m", "merge"]).expect("merge");

        // Now feature is an ancestor of main
        assert!(is_ancestor(repo.path(), &feature_head, "main").unwrap());
    }

    #[test]
    fn git_checkout_and_checkout_new_branch() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        checkout_new_branch(repo.path(), "feature").expect("create feature");
        assert_eq!(
            current_branch(repo.path()).unwrap(),
            Some("feature".to_string())
        );

        checkout(repo.path(), "main").expect("checkout main");
        assert_eq!(
            current_branch(repo.path()).unwrap(),
            Some("main".to_string())
        );
    }

    #[test]
    fn git_checkout_new_branch_from_ignores_current_head() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");
        let main = rev_parse(repo.path(), "main").unwrap();
        checkout_new_branch(repo.path(), "first-delivery").unwrap();
        commit_file(repo.path(), "delivery.txt", "shipped");

        checkout_new_branch_from(repo.path(), "second-delivery", "main").unwrap();

        assert_eq!(rev_parse(repo.path(), "HEAD").unwrap(), main);
        assert!(!repo.path().join("delivery.txt").exists());
    }

    #[test]
    fn git_current_branch_detached_head() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");
        let commit = git_stdout(repo.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .to_string();

        // Detach HEAD
        git_stdout(repo.path(), &["checkout", &commit]).expect("detach");
        assert_eq!(current_branch(repo.path()).unwrap(), None);
    }

    #[test]
    fn git_get_default_branch_falls_back_to_main() {
        let repo = init_repo();
        let branch = get_default_branch(repo.path()).expect("default branch");
        assert_eq!(branch, "main");
    }

    #[test]
    fn git_is_clean_tracks_changes() {
        let repo = init_repo();
        assert!(is_clean(repo.path()).expect("clean repo"));

        let path = repo.path().join("dirty.txt");
        fs::write(&path, "dirty").expect("write file");
        assert!(!is_clean(repo.path()).expect("dirty repo"));
    }

    #[test]
    fn worktree_state_changes_with_committed_and_uncommitted_work() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "one");
        let initial = worktree_state(repo.path()).expect("initial state");

        fs::write(repo.path().join("README.md"), "two").expect("edit tracked file");
        let dirty = worktree_state(repo.path()).expect("dirty state");
        assert_ne!(dirty, initial);

        stage_all(repo.path(), &|_| {}).expect("stage all");
        commit(repo.path(), "update readme", &|_| {}).expect("commit");
        let committed = worktree_state(repo.path()).expect("committed state");
        assert_ne!(committed, initial);
        assert_ne!(committed, dirty);
    }

    #[test]
    fn worktree_state_tracks_untracked_file_contents() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "tracked");

        fs::write(repo.path().join("new.txt"), "one").expect("write untracked file");
        let initial = worktree_state(repo.path()).expect("initial untracked state");
        fs::write(repo.path().join("new.txt"), "two").expect("update untracked file");
        let updated = worktree_state(repo.path()).expect("updated untracked state");

        assert_ne!(initial, updated);
    }

    #[test]
    fn material_worktree_state_ignores_gate_scratch_artifacts() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "tracked");
        let initial = material_worktree_state(repo.path()).expect("initial state");
        assert!(is_materially_clean(repo.path()).expect("initially clean"));

        fs::create_dir(repo.path().join("scratch")).expect("create scratch");
        fs::write(repo.path().join("scratch/review.md"), "gate evidence")
            .expect("write gate artifact");
        assert_eq!(
            material_worktree_state(repo.path()).expect("scratch-only state"),
            initial
        );
        assert!(is_materially_clean(repo.path()).expect("scratch-only clean"));

        fs::write(repo.path().join("src.txt"), "material repair").expect("write repair");
        assert_ne!(
            material_worktree_state(repo.path()).expect("material state"),
            initial
        );
        assert!(!is_materially_clean(repo.path()).expect("materially dirty"));
    }

    #[test]
    fn git_stage_all_and_commit() {
        let repo = init_repo();
        let path = repo.path().join("stage.txt");
        fs::write(&path, "staged").expect("write file");

        stage_all(repo.path(), &|_| {}).expect("stage all");
        commit(repo.path(), "add staged", &|_| {}).expect("commit");
        assert!(is_clean(repo.path()).expect("clean after commit"));
    }

    #[test]
    fn git_push_with_upstream() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        let remote_dir = tempfile::tempdir().expect("remote dir");
        git_stdout(remote_dir.path(), &["init", "--bare"]).expect("init bare");
        git_stdout(
            repo.path(),
            &[
                "remote",
                "add",
                "origin",
                remote_dir.path().to_str().expect("remote path"),
            ],
        )
        .expect("add remote");

        checkout_new_branch(repo.path(), "feature").expect("create feature");
        commit_file(repo.path(), "feature.txt", "feature");
        push_with_upstream(repo.path(), "origin", "feature", &|_| {}).expect("push with upstream");

        // Verify upstream is set
        let tracking = git_stdout(
            repo.path(),
            &["rev-parse", "--abbrev-ref", "feature@{upstream}"],
        )
        .expect("get upstream");
        assert_eq!(tracking.trim(), "origin/feature");
        assert_eq!(
            origin_branch(repo.path()).unwrap().as_deref(),
            Some("feature")
        );
    }

    #[test]
    fn git_delete_local_branch() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        checkout_new_branch(repo.path(), "feature").expect("create feature");
        commit_file(repo.path(), "feature.txt", "feature");
        checkout(repo.path(), "main").expect("checkout main");

        delete_local_branch(repo.path(), "feature").expect("delete branch");
    }

    #[test]
    fn git_branch_rename() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");
        checkout_new_branch(repo.path(), "old-name").expect("create branch");
        branch_rename(repo.path(), "old-name", "new-name").expect("rename branch");
        assert_eq!(
            current_branch(repo.path()).unwrap(),
            Some("new-name".to_string())
        );
    }

    #[test]
    fn git_branch_rename_after_worktree_move() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        checkout_new_branch(repo.path(), "feature-old").expect("create feature branch");
        checkout(repo.path(), "main").expect("back to main");

        let wt_path = repo.path().parent().unwrap().join("feature-old-worktree");
        git_stdout(
            repo.path(),
            &["worktree", "add", wt_path.to_str().unwrap(), "feature-old"],
        )
        .expect("create worktree");

        let moved_wt = repo.path().parent().unwrap().join("feature-new-worktree");
        worktree_move(repo.path(), &wt_path, &moved_wt).expect("move worktree");

        branch_rename(repo.path(), "feature-old", "feature-new").expect("rename branch");
        assert_eq!(
            current_branch(&moved_wt).unwrap(),
            Some("feature-new".to_string())
        );

        worktree_remove(repo.path(), &moved_wt).expect("remove worktree");
    }

    #[test]
    fn git_rev_parse_returns_sha() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        let sha = rev_parse(repo.path(), "HEAD").expect("rev-parse HEAD");
        assert!(!sha.is_empty());
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn git_worktree_move_and_add() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        // Create a worktree
        let wt_path = repo.path().parent().unwrap().join("test-worktree");
        checkout_new_branch(repo.path(), "feature").expect("create feature");
        checkout(repo.path(), "main").expect("back to main");

        git_stdout(
            repo.path(),
            &["worktree", "add", wt_path.to_str().unwrap(), "feature"],
        )
        .expect("create worktree");

        // Move the worktree
        let new_path = repo.path().parent().unwrap().join("moved-worktree");
        worktree_move(repo.path(), &wt_path, &new_path).expect("move worktree");

        // Verify old path doesn't exist, new path does
        assert!(!wt_path.exists());
        assert!(new_path.exists());

        // Clean up
        worktree_remove(repo.path(), &new_path).expect("remove worktree");
    }

    #[test]
    fn git_worktree_add_creates_branch() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        let wt_path = repo.path().parent().unwrap().join("new-worktree");
        worktree_add(
            repo.path(),
            &wt_path,
            "new-feature",
            WorktreeBranch::New {
                start_point: "HEAD",
            },
        )
        .expect("worktree add");

        // Verify worktree exists
        assert!(wt_path.exists());

        // Verify branch was created
        let branch = current_branch(&wt_path).expect("get branch");
        assert_eq!(branch, Some("new-feature".to_string()));

        // Clean up
        worktree_remove(repo.path(), &wt_path).expect("remove worktree");
    }

    #[test]
    fn hash_areas_returns_stable_digest() {
        let repo = init_repo();
        let src = repo.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("lib.rs"), "fn main() {}").unwrap();
        git_stdout(repo.path(), &["add", "."]).unwrap();
        git_stdout(repo.path(), &["commit", "-m", "init"]).unwrap();

        let h1 = hash_areas(repo.path(), &["src/".to_string()]).unwrap();
        let h2 = hash_areas(repo.path(), &["src/".to_string()]).unwrap();
        assert_eq!(h1, h2, "same content should produce same hash");
        assert_eq!(h1.len(), 64, "SHA-256 hex digest should be 64 chars");
    }

    #[test]
    fn hash_areas_changes_when_file_changes() {
        let repo = init_repo();
        let src = repo.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("lib.rs"), "v1").unwrap();
        git_stdout(repo.path(), &["add", "."]).unwrap();
        git_stdout(repo.path(), &["commit", "-m", "v1"]).unwrap();
        let h1 = hash_areas(repo.path(), &["src/".to_string()]).unwrap();

        fs::write(src.join("lib.rs"), "v2").unwrap();
        git_stdout(repo.path(), &["add", "."]).unwrap();
        git_stdout(repo.path(), &["commit", "-m", "v2"]).unwrap();
        let h2 = hash_areas(repo.path(), &["src/".to_string()]).unwrap();

        assert_ne!(h1, h2, "different content should produce different hash");
    }
}
