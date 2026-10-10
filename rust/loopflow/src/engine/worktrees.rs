use crate::engine::error::GitError;
use crate::engine::git::{
    current_branch, fetch, get_default_branch, has_commits_beyond, has_origin, is_clean, rev_parse,
    stash_including_untracked, stash_pop, worktree_add, worktree_add_inheriting, worktree_remove,
    WorktreeBranch,
};
use crate::engine::identity::WorktreeName;
use crate::engine::naming::git_user;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeSegment(String);

impl WorktreeSegment {
    pub fn parse(raw: &str) -> Result<Self, PlacementError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(PlacementError::EmptySegment);
        }
        // Keep the sibling suffix and branch leaf unambiguous and shell-safe.
        if trimmed.contains('.') {
            return Err(PlacementError::DotsReserved(trimmed.to_string()));
        }
        Ok(Self(crate::engine::naming::sanitize_for_branch(trimmed)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementStrategy {
    Create,
    CheckoutExisting,
    UseExistingWorktree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlacementPlan {
    pub base_ref: String,
    pub branch: String,
    pub worktree_path: PathBuf,
    pub strategy: PlacementStrategy,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PlacementError {
    #[error("worktree segment cannot be empty")]
    EmptySegment,
    #[error("\"{0}\" is not a flat worktree name. Use a hyphen instead of a dot.")]
    DotsReserved(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeState {
    pub branch: Option<String>,
    pub path: PathBuf,
    pub base_branch: Option<String>,
    pub merged: bool,
    pub squash_merged: bool,
    /// No commits beyond main and not merged — a brand new worktree.
    pub fresh: bool,
    pub dirty: bool,
    pub remote_gone: bool,
    pub pull_request: Option<PullRequestState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateWorktreeResult {
    pub path: PathBuf,
    pub branch: String,
    pub base_branch: Option<String>,
    pub base_commit: Option<String>,
}

/// One main agent's isolated execution checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWorktree {
    pub path: PathBuf,
    pub branch: String,
}

pub fn git_common_dir(repo: &Path) -> Result<PathBuf, GitError> {
    let output = crate::engine::git::retained_output(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: "git rev-parse --git-common-dir".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    let common_dir = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    if common_dir.is_absolute() {
        Ok(common_dir)
    } else {
        Err(GitError::CommandFailed {
            command: "git rev-parse --git-common-dir".to_string(),
            stderr: format!(
                "Git reported non-absolute common dir {}",
                common_dir.display()
            ),
        })
    }
}

pub fn main_repo_root(repo: &Path) -> Result<PathBuf, GitError> {
    git_common_dir(repo)?
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| GitError::CommandFailed {
            command: "git rev-parse --git-common-dir".to_string(),
            stderr: "unable to resolve common dir parent".to_string(),
        })
}

/// The worktree directory for an identity: `<parent>/<repo>.<dir_component>`.
/// The `/`-scoped branch never reaches disk — only the flat dir component does.
pub fn worktree_dir(repo: &Path, id: &WorktreeName) -> PathBuf {
    dir_for_component(repo, id.dir_component())
}

fn dir_for_component(repo: &Path, component: &str) -> PathBuf {
    let repo_root = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");
    repo_root
        .parent()
        .unwrap_or(repo_root.as_path())
        .join(format!("{repo_name}.{component}"))
}

/// The worktree directory for a branch or local name. Invalid flat names use a
/// neutral fallback; callers that create worktrees validate the segment first.
pub fn worktree_path(repo: &Path, name: &str) -> PathBuf {
    let user = git_user(repo).unwrap_or_else(|_| "user".to_string());
    let component = WorktreeName::parse(name, &user)
        .map(|id| id.dir_component().to_string())
        .unwrap_or_else(|| "worktree".to_string());
    dir_for_component(repo, &component)
}

fn short_hash(value: &str, chars: usize) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut hash = hex::encode(digest);
    hash.truncate(chars);
    hash
}

/// Stable flat placement name for a Wave worktree. Nested locators keep a
/// readable prefix and a hash of the complete slug so sanitization cannot
/// collapse distinct Waves onto one checkout.
pub fn wave_agent_segment(wave: &str) -> Result<WorktreeSegment, PlacementError> {
    let readable = crate::engine::naming::sanitize_for_branch(wave).replace('.', "-");
    let name = if wave.contains('/') {
        format!("wave-{readable}-{}", short_hash(wave, 8))
    } else {
        format!("wave-{readable}")
    };
    WorktreeSegment::parse(&name)
}

/// Extract the suffix from a named sibling worktree directory.
///
/// Given a path like `../loopflow.my-feature`, returns `Some("my-feature")`.
/// Returns `None` if not in a worktree (i.e., in the main repo).
pub fn sibling_worktree_name(repo: &Path) -> Option<String> {
    let main_repo = main_repo_root(repo).ok()?;
    sibling_worktree_name_with_main(repo, &main_repo)
}

/// Extract the sibling suffix using an already-resolved main repo path.
///
/// Use this to avoid repeatedly shelling out to git when iterating many worktrees.
/// Only recognizes sibling worktrees (e.g., `../repo.feature`) — the worktree must
/// share the same parent directory as the main repo. Worktrees elsewhere (e.g.,
/// `.claude/worktrees/`) return `None`.
pub fn sibling_worktree_name_with_main(repo: &Path, main_repo: &Path) -> Option<String> {
    if repo == main_repo {
        return None;
    }

    // Only recognize sibling worktrees: same parent directory as main repo.
    // Canonicalize to resolve symlinks (macOS: /tmp → /private/tmp).
    let repo_parent = repo.parent()?.canonicalize().ok()?;
    let main_parent = main_repo.parent()?.canonicalize().ok()?;
    if repo_parent != main_parent {
        return None;
    }

    let main_name = main_repo.file_name()?.to_str()?;
    let dir_name = repo.file_name()?.to_str()?;
    let prefix = format!("{main_name}.");
    let short_name = dir_name.strip_prefix(&prefix)?;

    (!short_name.is_empty()).then(|| short_name.to_string())
}

pub fn branch_exists(repo: &Path, branch: &str) -> Result<bool, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["show-ref", "--verify", &format!("refs/heads/{branch}")])
        .output()?;
    Ok(output.status.success())
}

pub(crate) fn list_porcelain(repo: &Path) -> Result<Vec<(PathBuf, Option<String>)>, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "list", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: "git worktree list --porcelain".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut items = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(path) = current_path.take() {
                items.push((path, current_branch.take()));
            }
            current_path = Some(PathBuf::from(path.trim()));
            current_branch = None;
        } else if let Some(branch) = line.strip_prefix("branch ") {
            let branch = branch.trim().strip_prefix("refs/heads/").unwrap_or(branch);
            current_branch = Some(branch.to_string());
        } else if line.trim() == "detached" {
            current_branch = None;
        }
    }

    if let Some(path) = current_path.take() {
        items.push((path, current_branch.take()));
    }

    Ok(items)
}

/// Parse GitHub owner/repo from the origin remote URL.
pub(crate) fn github_repo_nwo(repo: &Path) -> Option<(String, String)> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // Handle SSH (git@github.com:owner/repo.git) and HTTPS (https://github.com/owner/repo.git)
    let path = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("https://github.com/"))?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, name) = path.split_once('/')?;
    Some((owner.to_string(), name.to_string()))
}

/// Remote enrichment may be slow or unreachable. It never holds a listing
/// longer than this; an unanswered remote is reported as unknown.
const REMOTE_LIMIT: Duration = Duration::from_secs(10);

/// Each Git process costs tens of milliseconds to start, so a listing runs its
/// per-worktree and per-branch commands on this many threads.
const GIT_WORKERS: usize = 16;

/// Branches asked of GitHub in one request. One request for 53 branches took
/// 1.1 s; four of this size, side by side, took 0.65 s.
const GITHUB_BRANCHES_PER_REQUEST: usize = 16;

/// Why a remote gave no usable answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteFailure {
    /// It failed, could not start, or answered something unreadable.
    Unavailable,
    /// It was stopped at its limit.
    TimedOut,
}

/// How remote enrichment ended for one listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RemoteOutcome {
    /// No branch needed remote facts.
    NotAsked,
    Answered,
    Unavailable,
    TimedOut,
}

impl From<RemoteFailure> for RemoteOutcome {
    fn from(failure: RemoteFailure) -> Self {
        match failure {
            RemoteFailure::Unavailable => Self::Unavailable,
            RemoteFailure::TimedOut => Self::TimedOut,
        }
    }
}

/// Run a remote command to completion or stop it at the limit.
fn remote_stdout(command: &mut Command, limit: Duration) -> Result<String, RemoteFailure> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| RemoteFailure::Unavailable)?;
    let mut stdout = child.stdout.take().ok_or(RemoteFailure::Unavailable)?;
    let (sender, receiver) = std::sync::mpsc::channel();
    // A transport helper can outlive its stopped parent and keep the pipe
    // open, so the reader is never joined.
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut bytes);
        let _ = sender.send(bytes);
    });
    match receiver.recv_timeout(limit) {
        Ok(bytes) => child
            .wait()
            .ok()
            .filter(|status| status.success())
            .map(|_| String::from_utf8_lossy(&bytes).to_string())
            .ok_or(RemoteFailure::Unavailable),
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(RemoteFailure::TimedOut)
        }
    }
}

/// Apply `work` to every item on a bounded set of threads, keeping input order.
fn concurrently<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let mut indexed = thread::scope(|scope| {
        let workers: Vec<_> = (0..items.len().min(GIT_WORKERS))
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(index) else {
                            return done;
                        };
                        done.push((index, work(item)));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("listing worker panicked"))
            .collect::<Vec<_>>()
    });
    indexed.sort_by_key(|(index, _)| *index);
    indexed.into_iter().map(|(_, result)| result).collect()
}

#[derive(Debug)]
struct LocalBranch {
    head: String,
    upstream: Option<String>,
}

/// Head and configured upstream of every local branch, in one Git process.
fn local_branches(repo: &Path) -> HashMap<String, LocalBranch> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "for-each-ref",
            "--format=%(refname)%00%(objectname)%00%(upstream:short)%00%(upstream:track)",
            "refs/heads",
        ])
        .output();
    let Ok(output) = output else {
        return HashMap::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\0');
            let name = fields.next()?.strip_prefix("refs/heads/")?;
            let head = fields.next()?;
            let upstream = fields.next().unwrap_or_default();
            let gone = fields.next() == Some("[gone]");
            let upstream = (!upstream.is_empty() && !gone).then(|| {
                upstream
                    .strip_prefix("origin/")
                    .unwrap_or(upstream)
                    .to_string()
            });
            Some((
                name.to_string(),
                LocalBranch {
                    head: head.to_string(),
                    upstream,
                },
            ))
        })
        .collect()
}

/// Local branches with no commit beyond `target`, in one Git process.
///
/// An unreadable target leaves every branch's own commits unproven absent, so
/// none is reported as contained.
fn branches_within(repo: &Path, target: &str) -> HashSet<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "for-each-ref",
            "--merged",
            target,
            "--format=%(refname)",
            "refs/heads",
        ])
        .output();
    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.strip_prefix("refs/heads/"))
            .map(str::to_string)
            .collect(),
        _ => HashSet::new(),
    }
}

/// Answers that depend only on a branch commit and the merge target's commit.
///
/// Both are immutable, so an answer never goes stale; it stops being asked
/// when either side moves. The file lives in the Git directory, never in a
/// checkout, and holds only the current target's answers.
#[derive(Debug)]
struct CommitFacts {
    path: Option<PathBuf>,
    target: String,
    target_tree: String,
    known: HashMap<(String, String), String>,
    changed: bool,
}

impl CommitFacts {
    /// `None` when `target` does not name a commit in this repository.
    fn load(repo: &Path, target: &str) -> Option<Self> {
        let resolved = Command::new("git")
            .arg("-C")
            .arg(repo)
            .arg("rev-parse")
            .arg(format!("{target}^{{commit}}"))
            .arg(format!("{target}^{{tree}}"))
            .output()
            .ok()?;
        if !resolved.status.success() {
            return None;
        }
        let resolved = String::from_utf8_lossy(&resolved.stdout);
        let mut ids = resolved.lines();
        let target = ids.next()?.to_string();
        let target_tree = ids.next()?.to_string();
        let git_dir = repo.join(".git");
        let path = git_dir.is_dir().then(|| git_dir.join("lf-commit-facts"));
        let known = path
            .as_ref()
            .and_then(|path| fs::read_to_string(path).ok())
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let mut fields = line.splitn(4, '\t');
                let (fact, on, head, answer) = (
                    fields.next()?,
                    fields.next()?,
                    fields.next()?,
                    fields.next()?,
                );
                (on == target).then(|| ((fact.to_string(), head.to_string()), answer.to_string()))
            })
            .collect();
        Some(Self {
            path,
            target,
            target_tree,
            known,
            changed: false,
        })
    }

    /// Answer `fact` for each head, computing only the unknown ones.
    fn answers(
        &mut self,
        fact: &str,
        heads: &[String],
        compute: impl Fn(&Self, &str) -> Option<String> + Sync,
    ) -> HashMap<String, String> {
        let unknown: Vec<&String> = heads
            .iter()
            .filter(|head| {
                !self
                    .known
                    .contains_key(&(fact.to_string(), (*head).clone()))
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let computed = concurrently(&unknown, |head| compute(self, head));
        for (head, answer) in unknown.into_iter().zip(computed) {
            // A failed command is not an answer; ask again next time.
            if let Some(answer) = answer {
                self.known.insert((fact.to_string(), head.clone()), answer);
                self.changed = true;
            }
        }
        heads
            .iter()
            .filter_map(|head| {
                let answer = self.known.get(&(fact.to_string(), head.clone()))?;
                Some((head.clone(), answer.clone()))
            })
            .collect()
    }

    /// Heads whose changes the target already contains: merging the head into
    /// the target would leave the target's tree unchanged.
    fn squash_merged(&mut self, repo: &Path, heads: &[String]) -> HashSet<String> {
        self.answers("squash-merged", heads, |facts, head| {
            let output = Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(["merge-tree", "--write-tree", &facts.target, head])
                .output()
                .ok()?;
            // Conflicts mean it is not cleanly merged.
            let merged = output.status.success()
                && String::from_utf8_lossy(&output.stdout).trim() == facts.target_tree;
            Some(if merged { "1" } else { "0" }.to_string())
        })
        .into_iter()
        .filter_map(|(head, answer)| (answer == "1").then_some(head))
        .collect()
    }

    /// `git diff --shortstat target...head` for each head.
    fn shortstats(&mut self, repo: &Path, heads: &[String]) -> HashMap<String, String> {
        self.answers("shortstat", heads, |facts, head| {
            let output = Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(["diff", "--shortstat", &format!("{}...{head}", facts.target)])
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        })
    }

    /// Keep new answers for the next listing. Losing them only costs time.
    fn save(&self) {
        let Some(path) = self.path.as_ref().filter(|_| self.changed) else {
            return;
        };
        let mut lines: Vec<String> = self
            .known
            .iter()
            .map(|((fact, head), answer)| format!("{fact}\t{}\t{head}\t{answer}\n", self.target))
            .collect();
        lines.sort();
        let staged = path.with_extension(format!("{}.tmp", std::process::id()));
        if fs::write(&staged, lines.concat()).is_ok() && fs::rename(&staged, path).is_err() {
            let _ = fs::remove_file(&staged);
        }
    }
}

/// `git diff --shortstat` of each branch against the default branch's remote
/// head, keyed by branch. A branch whose diff cannot be read is absent.
pub fn diff_shortstats(
    repo: &Path,
    default_branch: &str,
    branches: &[&str],
) -> HashMap<String, String> {
    let Some(mut facts) = CommitFacts::load(repo, &format!("origin/{default_branch}")) else {
        return HashMap::new();
    };
    let heads = branch_heads(repo);
    let wanted: Vec<String> = branches
        .iter()
        .filter_map(|branch| heads.get(*branch).cloned())
        .collect();
    let stats = facts.shortstats(repo, &wanted);
    facts.save();
    branches
        .iter()
        .filter_map(|branch| {
            let stat = stats.get(heads.get(*branch)?)?;
            Some((branch.to_string(), stat.clone()))
        })
        .collect()
}

fn branch_heads(repo: &Path) -> HashMap<String, String> {
    local_branches(repo)
        .into_iter()
        .map(|(name, branch)| (name, branch.head))
        .collect()
}

/// What GitHub knows about each branch: the current head's PR state and
/// whether the branch still exists there.
#[derive(Debug, Default, PartialEq)]
struct GithubBranches {
    pull_requests: HashMap<String, PullRequestState>,
    existing: HashSet<String>,
}

/// Read every branch's PR state and existence from GitHub.
///
/// GitHub's answer time grows with the branches in one query, so the branches
/// are asked `GITHUB_BRANCHES_PER_REQUEST` at a time, side by side. A failure
/// means GitHub was unavailable, so callers must not infer that a stale branch
/// has no open PR.
fn github_branches(
    repo: &Path,
    (owner, name): (String, String),
    branch_heads: &[(String, String)],
) -> Result<GithubBranches, RemoteFailure> {
    let requests: Vec<&[(String, String)]> =
        branch_heads.chunks(GITHUB_BRANCHES_PER_REQUEST).collect();
    whole_github_answer(concurrently(&requests, |branch_heads| {
        github_request(repo, &owner, &name, branch_heads)
    }))
}

/// Every request's answer as one, or the failure that leaves all of it unknown.
///
/// A partial answer would report the unanswered branches as deleted and
/// without PRs. A request stopped at its limit has used the whole limit.
fn whole_github_answer(
    answers: Vec<Result<GithubBranches, RemoteFailure>>,
) -> Result<GithubBranches, RemoteFailure> {
    if answers.contains(&Err(RemoteFailure::TimedOut)) {
        return Err(RemoteFailure::TimedOut);
    }
    let mut whole = GithubBranches::default();
    for answer in answers {
        let answer = answer?;
        whole.pull_requests.extend(answer.pull_requests);
        whole.existing.extend(answer.existing);
    }
    Ok(whole)
}

fn github_request(
    repo: &Path,
    owner: &str,
    name: &str,
    branch_heads: &[(String, String)],
) -> Result<GithubBranches, RemoteFailure> {
    // Build aliased GraphQL query: two fields per branch. Branch names are
    // reusable, so current-head identity decides whether historical PR state
    // applies to this worktree.
    let mut fields = String::new();
    for (i, (branch, _)) in branch_heads.iter().enumerate() {
        let escaped = branch.replace('\\', "\\\\").replace('"', "\\\"");
        fields.push_str(&format!(
            "b{i}: pullRequests(first: 100, headRefName: \"{escaped}\", orderBy: {{ field: UPDATED_AT, direction: DESC }}) {{ nodes {{ headRefOid state }} }}\n\
             r{i}: ref(qualifiedName: \"refs/heads/{escaped}\") {{ id }}\n"
        ));
    }
    let query =
        format!("query {{ repository(owner: \"{owner}\", name: \"{name}\") {{ {fields} }} }}");

    let stdout = remote_stdout(
        Command::new("gh").current_dir(repo).args([
            "api",
            "graphql",
            "-f",
            &format!("query={query}"),
        ]),
        REMOTE_LIMIT,
    )?;

    parse_pull_request_states(&stdout, branch_heads)
        .zip(parse_existing_branches(&stdout, branch_heads))
        .map(|(pull_requests, existing)| GithubBranches {
            pull_requests,
            existing,
        })
        .ok_or(RemoteFailure::Unavailable)
}

fn parse_existing_branches(
    response: &str,
    branch_heads: &[(String, String)],
) -> Option<HashSet<String>> {
    let value = serde_json::from_str::<serde_json::Value>(response).ok()?;
    let repository = value.pointer("/data/repository")?;
    branch_heads
        .iter()
        .enumerate()
        .filter_map(
            |(index, (branch, _))| match repository.get(format!("r{index}")) {
                // An absent answer is unknown, never a deleted branch.
                None => Some(None),
                Some(serde_json::Value::Null) => None,
                Some(_) => Some(Some(branch.clone())),
            },
        )
        .collect()
}

fn parse_pull_request_states(
    response: &str,
    branch_heads: &[(String, String)],
) -> Option<HashMap<String, PullRequestState>> {
    let value = serde_json::from_str::<serde_json::Value>(response).ok()?;
    let repository = value.pointer("/data/repository")?;

    Some(
        branch_heads
            .iter()
            .enumerate()
            .filter_map(|(index, (branch, head))| {
                let nodes = repository
                    .pointer(&format!("/b{index}/nodes"))?
                    .as_array()?;
                let mut states = nodes
                    .iter()
                    .filter(|node| {
                        node.get("headRefOid").and_then(serde_json::Value::as_str)
                            == Some(head.as_str())
                    })
                    .filter_map(|node| node.get("state").and_then(serde_json::Value::as_str));
                let state = if states.clone().any(|state| state == "OPEN") {
                    PullRequestState::Open
                } else if states.clone().any(|state| state == "MERGED") {
                    PullRequestState::Merged
                } else if states.any(|state| state == "CLOSED") {
                    PullRequestState::Closed
                } else {
                    return None;
                };
                Some((branch.clone(), state))
            })
            .collect(),
    )
}

/// List all remote branch names via a single `git ls-remote --heads origin` call.
fn list_remote_branches(repo: &Path) -> Result<HashSet<String>, RemoteFailure> {
    let stdout = remote_stdout(
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .env("GIT_TERMINAL_PROMPT", "0")
            .args(["ls-remote", "--heads", "origin"]),
        REMOTE_LIMIT,
    )?;
    Ok(stdout
        .lines()
        .filter_map(|line| {
            line.split('\t')
                .nth(1)?
                .strip_prefix("refs/heads/")
                .map(|b| b.to_string())
        })
        .collect())
}

/// Worktree states from local Git alone. No network calls.
///
/// `squash_merged` and `fresh` come from local history. `merged` is always
/// `false` here: a branch already contained in the merge target has no commits
/// of its own, which the listing calls fresh, so only PR evidence establishes
/// a merge. `remote_gone` is `false` and `pull_request` is `None` until network
/// enrichment.
fn local_states(
    repo: &Path,
    default_branch: &str,
    items: Vec<(PathBuf, Option<String>)>,
    branches: &HashMap<String, LocalBranch>,
) -> Vec<WorktreeState> {
    let merge_target = format!("origin/{default_branch}");
    let (dirty, (within, squash_merged)) = thread::scope(|scope| {
        // A failed cleanliness check is not evidence that removal is safe.
        let dirty =
            scope.spawn(|| concurrently(&items, |(path, _)| !is_clean(path).unwrap_or(false)));
        let within = branches_within(repo, &merge_target);
        // Only a branch with commits of its own can have been squash-merged.
        let candidates: Vec<String> = items
            .iter()
            .filter_map(|(_, branch)| branch.as_deref())
            .filter(|branch| *branch != default_branch && !within.contains(*branch))
            .filter_map(|branch| branches.get(branch).map(|branch| branch.head.clone()))
            .collect();
        let squash_merged = match CommitFacts::load(repo, &merge_target) {
            Some(mut facts) => {
                let merged = facts.squash_merged(repo, &candidates);
                facts.save();
                merged
            }
            None => HashSet::new(),
        };
        (
            dirty.join().expect("listing worker panicked"),
            (within, squash_merged),
        )
    });

    items
        .into_iter()
        .zip(dirty)
        .map(|((path, branch), dirty)| {
            let known = branch.as_deref().and_then(|branch| branches.get(branch));
            let base_branch = known
                .and_then(|branch| branch.upstream.clone())
                .filter(|upstream| upstream != default_branch);
            let is_default = branch.as_deref() == Some(default_branch);
            let has_commits = is_default || branch.as_deref().is_some_and(|b| !within.contains(b));
            let squash_merged = !is_default
                && has_commits
                && known.is_some_and(|branch| squash_merged.contains(&branch.head));
            // "Fresh" means no net content delta against main yet.
            // This includes newly-rotated branches that were forked from a landed
            // branch (commit graph differs, but tree is identical to main).
            let fresh = !is_default && (!has_commits || squash_merged);
            WorktreeState {
                branch,
                path,
                base_branch,
                merged: false,
                squash_merged,
                fresh,
                dirty,
                remote_gone: false,
                pull_request: None,
            }
        })
        .collect()
}

#[derive(Debug)]
struct RemoteFacts {
    /// `None` when GitHub was applicable but unavailable.
    pull_requests: Option<HashMap<String, PullRequestState>>,
    /// Empty when the remote could not be read.
    branches: HashSet<String>,
    outcome: RemoteOutcome,
}

/// Ask the remote which branches exist and what PR state their heads have.
///
/// GitHub answers both together. Any other remote, or an unavailable
/// GitHub, is asked for its branches directly. Each call ends within
/// `REMOTE_LIMIT`, and a GitHub call that reached it is not followed by another.
fn remote_facts(
    repo: &Path,
    default_branch: &str,
    items: &[(PathBuf, Option<String>)],
    branches: &HashMap<String, LocalBranch>,
) -> RemoteFacts {
    let branch_heads: Vec<(String, String)> = items
        .iter()
        .filter_map(|(_, branch)| branch.as_ref())
        .filter(|b| b.as_str() != default_branch)
        .filter_map(|b| Some((b.clone(), branches.get(b)?.head.clone())))
        .collect();
    if branch_heads.is_empty() {
        return RemoteFacts {
            pull_requests: Some(HashMap::new()),
            branches: HashSet::new(),
            outcome: RemoteOutcome::NotAsked,
        };
    }
    let Some(nwo) = github_repo_nwo(repo) else {
        // A non-GitHub remote has no GitHub PR state: known, and empty.
        let listed = list_remote_branches(repo);
        return RemoteFacts {
            pull_requests: Some(HashMap::new()),
            outcome: listed
                .as_ref()
                .map_or_else(|failure| (*failure).into(), |_| RemoteOutcome::Answered),
            branches: listed.unwrap_or_default(),
        };
    };
    match github_branches(repo, nwo, &branch_heads) {
        Ok(mut github) => {
            // The listing compares against the default branch's remote head,
            // so a set naming no worktree branch still means "all gone".
            github.existing.insert(default_branch.to_string());
            RemoteFacts {
                pull_requests: Some(github.pull_requests),
                branches: github.existing,
                outcome: RemoteOutcome::Answered,
            }
        }
        // PR state stays unknown whatever the fallback learns about branches.
        // A GitHub that used the whole limit leaves none for a second call.
        Err(failure) => RemoteFacts {
            pull_requests: None,
            branches: match failure {
                RemoteFailure::Unavailable => list_remote_branches(repo).unwrap_or_default(),
                RemoteFailure::TimedOut => HashSet::new(),
            },
            outcome: failure.into(),
        },
    }
}

fn apply_network_enrichment(
    states: &mut [WorktreeState],
    default_branch: &str,
    pr_states: &HashMap<String, PullRequestState>,
    remote_branches: &HashSet<String>,
) {
    for state in states.iter_mut() {
        let is_default = state.branch.as_deref() == Some(default_branch);
        if is_default {
            continue;
        }

        state.pull_request = state
            .branch
            .as_deref()
            .and_then(|branch| pr_states.get(branch))
            .copied();

        if !state.merged && state.pull_request == Some(PullRequestState::Merged) {
            state.merged = true;
            state.fresh = false;
        }

        if !remote_branches.is_empty() {
            state.remote_gone = state
                .branch
                .as_deref()
                .is_some_and(|b| !remote_branches.contains(b));
        }
    }
}

/// Local listing plus remote enrichment, read at the same time, with its
/// phase timings.
///
/// A remote that fails or does not answer leaves `remote_gone` false and
/// `pull_request` unknown; it never fails or stalls the local listing.
pub fn list_worktrees_timed(repo: &Path) -> Result<Listing, GitError> {
    let started = Instant::now();
    // The remote cannot be asked before all three are read, so they are read
    // together.
    let (default_branch, branches, items) = thread::scope(|scope| {
        let default_branch = scope.spawn(|| get_default_branch(repo));
        let branches = scope.spawn(|| local_branches(repo));
        let items = list_porcelain(repo);
        (
            default_branch.join().expect("listing worker panicked"),
            branches.join().expect("listing worker panicked"),
            items,
        )
    });
    let (default_branch, items) = (default_branch?, items?);
    let ((remote, remote_time), mut worktrees, local_git) = thread::scope(|scope| {
        let remote = scope.spawn(|| {
            let asked = Instant::now();
            (
                remote_facts(repo, &default_branch, &items, &branches),
                asked.elapsed(),
            )
        });
        let worktrees = local_states(repo, &default_branch, items.clone(), &branches);
        let local_git = started.elapsed();
        (
            remote.join().expect("listing worker panicked"),
            worktrees,
            local_git,
        )
    });
    let pull_requests_known = remote.pull_requests.is_some();
    apply_network_enrichment(
        &mut worktrees,
        &default_branch,
        &remote.pull_requests.unwrap_or_default(),
        &remote.branches,
    );
    Ok(Listing {
        default_branch,
        worktrees,
        pull_requests_known,
        local_git,
        remote: remote_time,
        remote_outcome: remote.outcome,
    })
}

/// Every worktree's state, and what reading it cost.
#[derive(Debug)]
pub struct Listing {
    /// The branch every worktree was compared against.
    pub default_branch: String,
    pub worktrees: Vec<WorktreeState>,
    /// False when GitHub was applicable but unavailable: an absent
    /// `pull_request` then proves nothing about an open PR.
    pub pull_requests_known: bool,
    /// Wall time of the local Git reads, which run beside the remote.
    pub local_git: Duration,
    /// Wall time until the remote answered, failed, or was stopped.
    pub remote: Duration,
    pub remote_outcome: RemoteOutcome,
}

/// Full worktree listing with all checks (local + network).
pub fn list_worktrees(repo: &Path) -> Result<Vec<WorktreeState>, GitError> {
    list_worktrees_timed(repo).map(|listing| listing.worktrees)
}

/// Create a local named sibling worktree; the caller owns publishing its branch.
///
/// Source selection belongs to the caller. Wave and Project runtimes use the canonical
/// main checkout; Task placement uses [`plan_placement`].
pub fn create_named_worktree(
    repo: &Path,
    name: &str,
    base: Option<&str>,
    inherit_creation: &impl Fn(&mut Command),
) -> Result<CreateWorktreeResult, GitError> {
    let user = git_user(repo)?;
    let segment = WorktreeSegment::parse(name).map_err(|error| GitError::CommandFailed {
        command: "git worktree add".to_string(),
        stderr: error.to_string(),
    })?;
    let id = WorktreeName::new(&user, segment).ok_or_else(|| GitError::CommandFailed {
        command: "git worktree add".to_string(),
        stderr: format!("invalid worktree author: {user}"),
    })?;
    let branch = id.branch();
    let worktree_path = worktree_dir(repo, &id);

    if worktree_path.exists() {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("worktree path already exists: {worktree_path:?}"),
        });
    }
    if list_porcelain(repo)?
        .into_iter()
        .filter_map(|(_, existing)| existing)
        .any(|existing| existing == branch)
    {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("branch already checked out: {branch}"),
        });
    }

    let remote_branch = format!("origin/{branch}");
    if rev_parse(repo, &remote_branch).is_ok() {
        let mode = if branch_exists(repo, &branch)? {
            WorktreeBranch::Existing
        } else {
            WorktreeBranch::Track {
                remote: &remote_branch,
            }
        };
        worktree_add_inheriting(repo, &worktree_path, &branch, mode, inherit_creation)?;
        return Ok(CreateWorktreeResult {
            path: worktree_path,
            branch,
            base_branch: None,
            base_commit: None,
        });
    }

    if branch_exists(repo, &branch)? {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("branch exists without worktree: {branch}"),
        });
    }

    let default_branch = get_default_branch(repo)?;
    let base_ref = base.unwrap_or(default_branch.as_str());
    let base_branch = base.and_then(|value| (value != default_branch).then(|| value.to_string()));
    let base_commit = if base_branch.is_some() {
        rev_parse(repo, base_ref).ok()
    } else {
        None
    };

    worktree_add_inheriting(
        repo,
        &worktree_path,
        &branch,
        WorktreeBranch::New {
            start_point: base_ref,
        },
        inherit_creation,
    )?;
    Ok(CreateWorktreeResult {
        path: worktree_path,
        branch,
        base_branch,
        base_commit,
    })
}

pub fn plan_placement(repo: &Path, segment: WorktreeSegment) -> Result<PlacementPlan, GitError> {
    plan_branch_placement(repo, segment, None)
}

pub(crate) fn plan_branch_placement(
    repo: &Path,
    segment: WorktreeSegment,
    branch: Option<&str>,
) -> Result<PlacementPlan, GitError> {
    let user = git_user(repo)?;
    let id = WorktreeName::new(&user, segment).ok_or_else(|| GitError::CommandFailed {
        command: "git worktree add".to_string(),
        stderr: format!("invalid worktree author: {user}"),
    })?;
    let branch = branch.map(str::to_string).unwrap_or_else(|| id.branch());
    let planned_path = worktree_dir(repo, &id);
    let base_ref = get_default_branch(repo)?;

    let existing_worktree_path =
        list_porcelain(repo)?
            .into_iter()
            .find_map(|(path, existing_branch)| {
                (existing_branch.as_deref() == Some(&branch)).then_some(path)
            });
    let strategy = if existing_worktree_path.is_some() {
        PlacementStrategy::UseExistingWorktree
    } else if branch_exists(repo, &branch)? || rev_parse(repo, &format!("origin/{branch}")).is_ok()
    {
        PlacementStrategy::CheckoutExisting
    } else {
        PlacementStrategy::Create
    };

    Ok(PlacementPlan {
        base_ref,
        branch,
        worktree_path: existing_worktree_path.unwrap_or(planned_path),
        strategy,
    })
}

pub fn create_from_placement_plan(
    repo: &Path,
    plan: &PlacementPlan,
) -> Result<CreateWorktreeResult, GitError> {
    apply_placement_plan(repo, plan, true)
}

fn apply_placement_plan(
    repo: &Path,
    plan: &PlacementPlan,
    publish_upstream: bool,
) -> Result<CreateWorktreeResult, GitError> {
    if plan.strategy != PlacementStrategy::UseExistingWorktree && plan.worktree_path.exists() {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("worktree path already exists: {:?}", plan.worktree_path),
        });
    }
    match plan.strategy {
        PlacementStrategy::UseExistingWorktree => {}
        PlacementStrategy::CheckoutExisting => {
            let remote_branch = format!("origin/{}", plan.branch);
            let mode = if branch_exists(repo, &plan.branch)? {
                WorktreeBranch::Existing
            } else {
                WorktreeBranch::Track {
                    remote: &remote_branch,
                }
            };
            worktree_add(repo, &plan.worktree_path, &plan.branch, mode)?;
        }
        PlacementStrategy::Create => {
            if branch_exists(repo, &plan.branch)? {
                return Err(GitError::CommandFailed {
                    command: "git worktree add".to_string(),
                    stderr: format!("branch exists without worktree: {}", plan.branch),
                });
            }
            worktree_add(
                repo,
                &plan.worktree_path,
                &plan.branch,
                WorktreeBranch::New {
                    start_point: &plan.base_ref,
                },
            )?;
            if publish_upstream {
                schedule_upstream_sync(plan.worktree_path.clone(), plan.branch.clone());
            }
        }
    }
    Ok(CreateWorktreeResult {
        path: plan.worktree_path.clone(),
        branch: plan.branch.clone(),
        base_branch: None,
        base_commit: None,
    })
}

/// Resolve or create a persistent worktree for an agent scope.
///
/// New branches use the fetched default branch, falling back to cached or local
/// state offline. Existing placements are reused at their current path.
pub fn ensure_agent_worktree(
    main_repo: &Path,
    segment: WorktreeSegment,
) -> Result<AgentWorktree, GitError> {
    let main_repo = main_repo_root(main_repo)?;
    let lock_path =
        git_common_dir(&main_repo)?.join(format!("persistent-{}.lock", segment.as_str()));
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)?;
    fs2::FileExt::lock_exclusive(&lock)?;
    let mut plan = plan_placement(&main_repo, segment)?;
    if plan.strategy == PlacementStrategy::UseExistingWorktree && !plan.worktree_path.exists() {
        worktree_remove(&main_repo, &plan.worktree_path)?;
        plan.strategy = PlacementStrategy::CheckoutExisting;
    }
    if plan.strategy == PlacementStrategy::Create {
        plan.base_ref = agent_base_ref(&main_repo)?;
    }
    let worktree = create_agent_worktree(&main_repo, &plan)?;
    let output = Command::new("git")
        .arg("-C")
        .arg(&main_repo)
        .args([
            "config",
            &format!("branch.{}.loopflow-persistent", worktree.branch),
            "true",
        ])
        .output()?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: "mark persistent worktree".into(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(worktree)
}

/// Persistent branches keep their checkout and local plans after delivery.
pub fn is_persistent_worktree(repo: &Path) -> Result<bool, GitError> {
    let Some(branch) = current_branch(repo)? else {
        return Ok(false);
    };
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "config",
            "--bool",
            "--get",
            &format!("branch.{branch}.loopflow-persistent"),
        ])
        .output()?;
    match output.status.code() {
        Some(0) => Ok(output.stdout == b"true\n"),
        Some(1) => Ok(false),
        _ => Err(GitError::CommandFailed {
            command: "read persistent worktree configuration".into(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }),
    }
}

/// Move a bare/default `lf` session off canonical main while preserving both
/// local commits and uncommitted files. A deliberate non-default checkout and
/// an existing linked worktree are left alone.
pub fn move_default_agent_to_worktree(repo: &Path) -> Result<Option<AgentWorktree>, GitError> {
    let main_repo = main_repo_root(repo)?;
    let checkout = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    let canonical = std::fs::canonicalize(&main_repo).unwrap_or_else(|_| main_repo.clone());
    if checkout != canonical {
        return Ok(None);
    }

    let default_branch = get_default_branch(&main_repo)?;
    if current_branch(&main_repo)?.as_deref() != Some(default_branch.as_str()) {
        return Ok(None);
    }

    let base_ref = agent_base_ref(&main_repo)?;
    let head = rev_parse(&main_repo, "HEAD")?;
    let dirty = !is_clean(&main_repo)?;
    let carries_state = dirty || has_commits_beyond(&main_repo, &default_branch, &base_ref)?;
    let stable = WorktreeSegment::parse("agent").map_err(placement_git_error)?;
    let expected_stable = worktree_path(&main_repo, stable.as_str());
    let stable_plan = plan_placement(&main_repo, stable)?;
    let stable_collision = match stable_plan.strategy {
        PlacementStrategy::UseExistingWorktree => {
            normalized_path(&stable_plan.worktree_path) != normalized_path(&expected_stable)
        }
        PlacementStrategy::Create | PlacementStrategy::CheckoutExisting => expected_stable.exists(),
    };
    let mut plan = if stable_collision
        || (carries_state && stable_plan.strategy != PlacementStrategy::Create)
    {
        unique_agent_plan(&main_repo, &head)?
    } else {
        stable_plan
    };
    if plan.strategy == PlacementStrategy::Create {
        plan.base_ref = if carries_state { "HEAD" } else { &base_ref }.to_string();
    }

    let stashed = stash_including_untracked(&main_repo)?;
    let worktree = match create_agent_worktree(&main_repo, &plan) {
        Ok(worktree) => worktree,
        Err(error) => {
            if stashed {
                let _ = stash_pop(&main_repo);
            }
            return Err(error);
        }
    };

    if let Err(error) = reset_checkout_to(&main_repo, &base_ref) {
        if stashed {
            let _ = stash_pop(&main_repo);
        }
        return Err(error);
    }
    if stashed {
        stash_pop(&worktree.path).map_err(|error| GitError::CommandFailed {
            command: "move default lf state".to_string(),
            stderr: format!(
                "canonical main is clean and the moved state remains recoverable in the latest stash, but applying it in {} failed: {error}",
                worktree.path.display()
            ),
        })?;
    }

    Ok(Some(worktree))
}

fn create_agent_worktree(
    main_repo: &Path,
    plan: &PlacementPlan,
) -> Result<AgentWorktree, GitError> {
    let created = apply_placement_plan(main_repo, plan, false)?;
    Ok(AgentWorktree {
        path: created.path,
        branch: created.branch,
    })
}

fn agent_base_ref(main_repo: &Path) -> Result<String, GitError> {
    let default_branch = get_default_branch(main_repo)?;
    if !has_origin(main_repo)? {
        return Ok(default_branch);
    }
    match fetch(main_repo, "origin", &default_branch) {
        Ok(()) => Ok(format!("origin/{default_branch}")),
        Err(_) if rev_parse(main_repo, &format!("origin/{default_branch}")).is_ok() => {
            Ok(format!("origin/{default_branch}"))
        }
        Err(_) => Ok(default_branch),
    }
}

fn unique_agent_plan(repo: &Path, head: &str) -> Result<PlacementPlan, GitError> {
    for attempt in 0_u32..100 {
        let nonce = format!(
            "{head}-{}-{}-{attempt}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let name = format!("agent-{}", short_hash(&nonce, 8));
        let segment = WorktreeSegment::parse(&name).map_err(placement_git_error)?;
        let plan = plan_placement(repo, segment)?;
        if plan.strategy == PlacementStrategy::Create && !plan.worktree_path.exists() {
            return Ok(plan);
        }
    }
    Err(GitError::CommandFailed {
        command: "resolve agent worktree".to_string(),
        stderr: "could not allocate a unique default-agent worktree".to_string(),
    })
}

fn reset_checkout_to(repo: &Path, target: &str) -> Result<(), GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["reset", "--hard", target])
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(GitError::CommandFailed {
        command: format!("git reset --hard {target}"),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn normalized_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn placement_git_error(error: PlacementError) -> GitError {
    GitError::CommandFailed {
        command: "resolve agent worktree".to_string(),
        stderr: error.to_string(),
    }
}

pub fn schedule_upstream_sync(worktree: PathBuf, branch: String) {
    thread::spawn(move || {
        // Don't block the caller on network/auth issues. Retry in the background.
        for backoff_secs in [0_u64, 2, 5, 15, 30, 60] {
            if backoff_secs > 0 {
                thread::sleep(Duration::from_secs(backoff_secs));
            }
            if crate::engine::git::origin_branch(&worktree)
                .ok()
                .flatten()
                .as_deref()
                == Some(branch.as_str())
            {
                return;
            }
            if push_branch_with_upstream(&worktree, &branch).is_ok() {
                return;
            }
        }
    });
}

pub fn push_branch_with_upstream(worktree: &Path, branch: &str) -> Result<(), GitError> {
    let status = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .args(["push", "-u", "origin", branch])
        // This push can outlive the CLI. Capture pipes would close on exit,
        // killing Git with SIGPIPE before it records the upstream locally.
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git push -u origin {branch}"),
            stderr: format!("background push exited with {status}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_network_enrichment, diff_shortstats, ensure_agent_worktree, git_common_dir,
        list_worktrees, move_default_agent_to_worktree, parse_existing_branches,
        parse_pull_request_states, plan_placement, remote_stdout, wave_agent_segment,
        whole_github_answer, worktree_path, GithubBranches, PlacementError, PlacementStrategy,
        PullRequestState, RemoteFailure, WorktreeSegment, WorktreeState,
    };
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create temp dir");
        let output = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["init", "-b", "main"])
            .output()
            .expect("git init");
        assert!(output.status.success());
        // Deterministic author so branch projections don't depend on the host.
        for (key, value) in [("user.name", "tester"), ("user.email", "t@example.com")] {
            Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(["config", key, value])
                .output()
                .expect("git config");
        }
        dir
    }

    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn common_directory_preserves_git_layouts_and_missing_checkouts() {
        let repo = init_repo();
        git(repo.path(), &["commit", "--allow-empty", "-m", "initial"]);
        let common = repo.path().join(".git").canonicalize().unwrap();
        let nested = repo.path().join("nested directory");
        fs::create_dir(&nested).unwrap();
        assert_eq!(git_common_dir(&nested).unwrap(), common);

        let other = tempfile::tempdir().unwrap();
        let checkout = other.path().join("linked checkout");
        git(
            repo.path(),
            &["worktree", "add", "--detach", checkout.to_str().unwrap()],
        );
        let alias = other.path().join("alias");
        std::os::unix::fs::symlink(&checkout, &alias).unwrap();
        assert_eq!(git_common_dir(&checkout).unwrap(), common);
        assert_eq!(git_common_dir(&alias).unwrap(), common);
        fs::remove_dir_all(&checkout).unwrap();
        assert!(git_common_dir(&checkout).is_err());
        assert!(git_common_dir(&alias).is_err());
        assert!(git_common_dir(other.path()).is_err());

        // Absence was not cached: a new repository at the same path is fresh.
        fs::create_dir(&checkout).unwrap();
        git(&checkout, &["init", "-b", "main"]);
        assert_eq!(
            git_common_dir(&alias).unwrap(),
            checkout.join(".git").canonicalize().unwrap()
        );
    }

    fn repo_with_origin() -> (tempfile::TempDir, std::path::PathBuf) {
        let root = tempfile::tempdir().expect("temp root");
        let repo = root.path().join("repo");
        let origin = root.path().join("origin.git");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&origin).unwrap();
        git(&repo, &["init", "-b", "main"]);
        git(&repo, &["config", "user.name", "tester"]);
        git(&repo, &["config", "user.email", "t@example.com"]);
        std::fs::write(repo.join("tracked.txt"), "base\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "base"]);
        git(&origin, &["init", "--bare"]);
        git(
            &repo,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(&repo, &["push", "-u", "origin", "main"]);
        git(&origin, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        (root, repo)
    }

    #[test]
    fn worktree_path_sanitizes_segment_for_filesystem() {
        let path = worktree_path(Path::new("/tmp/repo"), "new*wave");
        assert_eq!(path, Path::new("/tmp/repo.new-wave"));
    }

    #[test]
    fn worktree_path_uses_neutral_fallback_for_invalid_flat_name() {
        let path = worktree_path(Path::new("/tmp/repo"), "../..");
        assert_eq!(path, Path::new("/tmp/repo.worktree"));
    }

    #[test]
    fn network_enrichment_keeps_squash_fresh_branch_unprunable() {
        let mut states = vec![
            WorktreeState {
                branch: Some("old".to_string()),
                path: Path::new("/tmp/repo.old").to_path_buf(),
                base_branch: None,
                merged: false,
                squash_merged: true,
                fresh: true,
                dirty: false,
                remote_gone: false,
                pull_request: None,
            },
            WorktreeState {
                branch: Some("new".to_string()),
                path: Path::new("/tmp/repo.new").to_path_buf(),
                base_branch: None,
                merged: false,
                squash_merged: true,
                fresh: true,
                dirty: false,
                remote_gone: false,
                pull_request: None,
            },
        ];

        let pr_states = HashMap::from([("old".to_string(), PullRequestState::Merged)]);
        let remote_branches = HashSet::from(["old".to_string(), "new".to_string()]);
        apply_network_enrichment(&mut states, "main", &pr_states, &remote_branches);

        let old = states
            .iter()
            .find(|state| state.branch.as_deref() == Some("old"))
            .unwrap();
        assert!(old.merged, "merged PR branch should be marked merged");
        assert_eq!(old.pull_request, Some(PullRequestState::Merged));
        assert!(!old.fresh, "merged PR branch should not stay fresh");

        let new = states
            .iter()
            .find(|state| state.branch.as_deref() == Some("new"))
            .unwrap();
        assert_eq!(new.pull_request, None);
        assert!(new.fresh, "new branch should remain fresh");
    }

    #[test]
    fn pull_request_state_follows_current_heads() {
        let response = serde_json::json!({
            "data": {
                "repository": {
                    "b0": {
                        "nodes": [
                            {"headRefOid": "current", "state": "OPEN"},
                            {"headRefOid": "previous", "state": "MERGED"}
                        ]
                    },
                    "b1": {
                        "nodes": [
                            {"headRefOid": "landed", "state": "MERGED"}
                        ]
                    },
                    "b2": {
                        "nodes": [
                            {"headRefOid": "closed", "state": "CLOSED"}
                        ]
                    }
                }
            }
        });
        let branches = vec![
            ("jack-heart/product".to_string(), "current".to_string()),
            ("jack-heart/landed".to_string(), "landed".to_string()),
            ("jack-heart/closed".to_string(), "closed".to_string()),
        ];

        assert_eq!(
            parse_pull_request_states(&response.to_string(), &branches),
            Some(HashMap::from([
                ("jack-heart/product".to_string(), PullRequestState::Open),
                ("jack-heart/landed".to_string(), PullRequestState::Merged),
                ("jack-heart/closed".to_string(), PullRequestState::Closed),
            ]))
        );
    }

    #[test]
    fn github_reports_deleted_branches_without_guessing_unanswered_ones() {
        let response = serde_json::json!({
            "data": {"repository": {"r0": {"id": "ref"}, "r1": null}}
        })
        .to_string();
        let heads = |names: &[&str]| -> Vec<(String, String)> {
            names
                .iter()
                .map(|name| (name.to_string(), "head".to_string()))
                .collect()
        };

        assert_eq!(
            parse_existing_branches(&response, &heads(&["kept", "deleted"])),
            Some(HashSet::from(["kept".to_string()]))
        );
        assert_eq!(
            parse_existing_branches(&response, &heads(&["kept", "deleted", "unanswered"])),
            None
        );
    }

    #[test]
    fn a_github_request_without_an_answer_leaves_every_branch_unknown() {
        let answered = |branch: &str| {
            Ok(GithubBranches {
                pull_requests: HashMap::from([(branch.to_string(), PullRequestState::Open)]),
                existing: HashSet::from([branch.to_string()]),
            })
        };

        assert_eq!(
            whole_github_answer(vec![answered("first"), answered("second")]),
            Ok(GithubBranches {
                pull_requests: HashMap::from([
                    ("first".to_string(), PullRequestState::Open),
                    ("second".to_string(), PullRequestState::Open),
                ]),
                existing: HashSet::from(["first".to_string(), "second".to_string()]),
            })
        );
        assert_eq!(
            whole_github_answer(vec![answered("first"), Err(RemoteFailure::Unavailable)]),
            Err(RemoteFailure::Unavailable)
        );
        assert_eq!(
            whole_github_answer(vec![
                Err(RemoteFailure::Unavailable),
                answered("second"),
                Err(RemoteFailure::TimedOut),
            ]),
            Err(RemoteFailure::TimedOut)
        );
    }

    #[test]
    fn unanswered_remote_stops_at_its_limit() {
        let started = std::time::Instant::now();
        let answer = remote_stdout(
            Command::new("sh").args(["-c", "sleep 30; echo late"]),
            Duration::from_millis(200),
        );
        assert_eq!(answer, Err(RemoteFailure::TimedOut));
        assert!(started.elapsed() < Duration::from_secs(10));
        assert_eq!(
            remote_stdout(Command::new("echo").arg("answer"), Duration::from_secs(10)),
            Ok("answer\n".to_string())
        );
        assert_eq!(
            remote_stdout(&mut Command::new("false"), Duration::from_secs(10)),
            Err(RemoteFailure::Unavailable)
        );
    }

    #[test]
    fn listing_reports_each_worktree_and_leaves_checkouts_untouched() {
        let (root, repo) = repo_with_origin();
        let sibling = |name: &str| root.path().join(name);
        let add = |name: &str| {
            let path = sibling(name);
            git(
                &repo,
                &[
                    "worktree",
                    "add",
                    "-b",
                    name,
                    path.to_str().unwrap(),
                    "origin/main",
                ],
            );
            path
        };

        let untouched = add("untouched");
        fs::write(untouched.join("note.txt"), "uncommitted\n").unwrap();

        let active = add("active");
        fs::write(active.join("active.txt"), "work\n").unwrap();
        git(&active, &["add", "."]);
        git(&active, &["commit", "-m", "active work"]);
        git(&active, &["push", "-u", "origin", "active"]);

        // The same change reaches main as a different commit, as a squash does.
        let landed = add("landed");
        fs::write(landed.join("landed.txt"), "landed\n").unwrap();
        git(&landed, &["add", "."]);
        git(&landed, &["commit", "-m", "branch commit"]);
        fs::write(repo.join("landed.txt"), "landed\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "squashed"]);
        git(&repo, &["push", "origin", "main"]);

        let detached = sibling("detached");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "--detach",
                detached.to_str().unwrap(),
                "origin/main",
            ],
        );

        let listed = list_worktrees(&repo).unwrap();
        let state = |branch: Option<&str>| {
            listed
                .iter()
                .find(|state| state.branch.as_deref() == branch)
                .unwrap()
        };
        let flags = |state: &WorktreeState| {
            (
                state.squash_merged,
                state.fresh,
                state.dirty,
                state.remote_gone,
            )
        };
        assert_eq!(flags(state(Some("main"))), (false, false, false, false));
        assert_eq!(flags(state(Some("untouched"))), (false, true, true, true));
        assert_eq!(flags(state(Some("active"))), (false, false, false, false));
        assert_eq!(flags(state(Some("landed"))), (true, true, false, true));
        assert_eq!(flags(state(None)), (false, true, false, false));
        assert_eq!(state(Some("active")).base_branch.as_deref(), Some("active"));
        assert_eq!(state(Some("untouched")).base_branch, None);
        assert!(listed
            .iter()
            .all(|state| !state.merged && state.pull_request.is_none()));

        assert_eq!(
            diff_shortstats(&repo, "main", &["active", "landed"])["active"],
            "1 file changed, 1 insertion(+)"
        );

        // Remembered answers change nothing a second listing reports, and
        // live outside every checkout.
        assert_eq!(
            serde_json::to_value(list_worktrees(&repo).unwrap()).unwrap(),
            serde_json::to_value(&listed).unwrap()
        );
        assert!(repo.join(".git/lf-commit-facts").is_file());
        assert!(crate::engine::git::is_clean(&repo).unwrap());
        assert!(crate::engine::git::is_clean(&active).unwrap());
    }

    #[test]
    fn worktree_segment_rejects_dots() {
        let err = WorktreeSegment::parse("api.v2").unwrap_err();
        assert_eq!(err, PlacementError::DotsReserved("api.v2".to_string()));
    }

    #[test]
    fn main_placement_creates_flat_branch() {
        let repo = init_repo();
        let segment = WorktreeSegment::parse("child").unwrap();
        let plan = plan_placement(repo.path(), segment).expect("plan task wt");

        assert_eq!(plan.branch, "tester/child");
        assert_eq!(plan.base_ref, "main");
        assert_eq!(plan.strategy, PlacementStrategy::Create);
    }

    #[test]
    fn wave_agent_worktree_is_fetched_isolated_and_reused() {
        let (_root, repo) = repo_with_origin();
        std::fs::write(repo.join("upstream.txt"), "new upstream\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "upstream"]);
        git(&repo, &["push", "origin", "main"]);
        git(&repo, &["reset", "--hard", "HEAD^"]);

        let segment = wave_agent_segment("ship").unwrap();
        let first = ensure_agent_worktree(&repo, segment.clone()).unwrap();
        let second = ensure_agent_worktree(&repo, segment).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.branch, "tester/wave-ship");
        assert_eq!(
            super::rev_parse(&first.path, "HEAD").unwrap(),
            super::rev_parse(&repo, "origin/main").unwrap()
        );
        assert!(super::is_clean(&repo).unwrap());
        assert!(!super::has_commits_beyond(&repo, "main", "origin/main").unwrap());
    }

    #[test]
    fn wave_agent_worktree_reuses_a_moved_branch() {
        let (root, repo) = repo_with_origin();
        let segment = wave_agent_segment("ship").unwrap();
        let persistent = ensure_agent_worktree(&repo, segment.clone()).unwrap();
        let displaced = root.path().join("displaced");
        git(
            &repo,
            &[
                "worktree",
                "move",
                persistent.path.to_str().unwrap(),
                displaced.to_str().unwrap(),
            ],
        );

        let recovered = ensure_agent_worktree(&repo, segment).unwrap();
        assert_eq!(
            recovered.path.canonicalize().unwrap(),
            displaced.canonicalize().unwrap()
        );
        assert_eq!(recovered.branch, persistent.branch);
        assert!(super::is_clean(&repo).unwrap());
    }

    #[test]
    fn missing_persistent_checkout_recovers_commits() {
        let (_root, repo) = repo_with_origin();
        let segment = wave_agent_segment("ship").unwrap();
        let persistent = ensure_agent_worktree(&repo, segment.clone()).unwrap();
        fs::write(persistent.path.join("memory.md"), "unpublished").unwrap();
        git(&persistent.path, &["add", "memory.md"]);
        git(&persistent.path, &["commit", "-m", "memory"]);
        fs::remove_dir_all(&persistent.path).unwrap();
        let recovered = ensure_agent_worktree(&repo, segment).unwrap();
        assert_eq!(
            fs::read_to_string(recovered.path.join("memory.md")).unwrap(),
            "unpublished"
        );
        assert!(recovered.path.exists());
    }

    #[test]
    fn nested_wave_segments_cannot_collapse_into_flat_slugs() {
        let nested = wave_agent_segment("platform/security").unwrap();
        let flat = wave_agent_segment("platform-security").unwrap();

        assert_ne!(nested, flat);
        assert!(nested.as_str().starts_with("wave-platform-security-"));
    }

    #[test]
    fn default_agent_carries_commits_and_uncommitted_files_off_main() {
        let (_root, repo) = repo_with_origin();
        std::fs::write(repo.join("ahead.txt"), "committed ahead\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "ahead"]);
        std::fs::write(repo.join("tracked.txt"), "edited\n").unwrap();
        std::fs::write(repo.join("untracked.txt"), "untracked\n").unwrap();

        let moved = move_default_agent_to_worktree(&repo)
            .unwrap()
            .expect("canonical main moves");

        assert!(super::is_clean(&repo).unwrap());
        assert_eq!(
            super::rev_parse(&repo, "HEAD").unwrap(),
            super::rev_parse(&repo, "origin/main").unwrap()
        );
        assert_eq!(
            std::fs::read_to_string(moved.path.join("tracked.txt")).unwrap(),
            "edited\n"
        );
        assert_eq!(
            std::fs::read_to_string(moved.path.join("untracked.txt")).unwrap(),
            "untracked\n"
        );
        assert!(moved.path.join("ahead.txt").is_file());
        assert!(move_default_agent_to_worktree(&moved.path)
            .unwrap()
            .is_none());
    }

    #[test]
    fn default_agent_starts_from_origin_when_clean_main_is_behind() {
        let (_root, repo) = repo_with_origin();
        std::fs::write(repo.join("upstream.txt"), "new upstream\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "upstream"]);
        git(&repo, &["push", "origin", "main"]);
        git(&repo, &["reset", "--hard", "HEAD^"]);

        let moved = move_default_agent_to_worktree(&repo)
            .unwrap()
            .expect("canonical main moves");

        let upstream = super::rev_parse(&repo, "origin/main").unwrap();
        assert_eq!(super::rev_parse(&repo, "HEAD").unwrap(), upstream);
        assert_eq!(super::rev_parse(&moved.path, "HEAD").unwrap(), upstream);
        assert!(moved.path.join("upstream.txt").is_file());
    }
}
