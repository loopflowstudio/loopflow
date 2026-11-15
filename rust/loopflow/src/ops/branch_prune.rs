//! Delete branches Loopflow can tie to finished work, on origin and locally.
//!
//! A branch copy is deleted only with evidence at its exact head: a merged PR,
//! a closed PR of a finished Task, a finished Task whose work reached the
//! default branch, or a release candidate whose tag exists. A closed PR alone
//! is not abandonment: GitHub closes a stacked PR when its base is deleted.
//! Protections come first and override evidence. Missing GitHub facts stop the
//! whole run before anything is deleted, because no answer cannot rule out an
//! open PR.
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::path::Path;
use std::process::Command;

use crate::git::get_default_branch;
use crate::git::worktrees::{
    branch_heads, github_graphql_budget, github_pull_requests, head_pull_request, list_porcelain,
    main_repo_root, remote_heads, remote_tags, GraphqlBudget, PullRequestNode, PullRequestState,
    RemoteFailure,
};
use crate::ops::release::released_candidate_tag;
use crate::ops::wt::{apply_delete, branch_copies_at};
use crate::ops::{NullProgress, OpsError};

/// A run below this GraphQL budget would compete with interactive work.
const MINIMUM_GRAPHQL_BUDGET: u64 = 200;

/// Branches named by Tasks, from every registry this machine reads.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct TaskBranches {
    /// Named by a Task that is not Done or Abandoned.
    pub live: HashSet<String>,
    /// Named by a Done or Abandoned Task.
    pub finished: HashSet<String>,
}

/// Collect `(branch, open)` names; a name an open Task holds anywhere is
/// live, whatever another Task says.
impl FromIterator<(String, bool)> for TaskBranches {
    fn from_iter<I: IntoIterator<Item = (String, bool)>>(named: I) -> Self {
        let mut branches = Self::default();
        for (branch, open) in named.into_iter().filter(|(branch, _)| !branch.is_empty()) {
            if open {
                branches.live.insert(branch);
            } else {
                branches.finished.insert(branch);
            }
        }
        let live = &branches.live;
        branches.finished.retain(|branch| !live.contains(branch));
        branches
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BranchCopy {
    Remote,
    Local,
}

/// Why a branch copy may be deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Evidence {
    PullRequest {
        number: u64,
        state: PullRequestState,
    },
    FinishedTask,
    Released {
        tag: String,
    },
}

/// Why a branch copy is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Retention {
    DefaultBranch,
    Worktree,
    LiveTask,
    OpenPullRequest {
        number: u64,
    },
    /// A finished PR exists for the name, but at another head.
    MovedPastPullRequest {
        number: u64,
        state: PullRequestState,
    },
    /// Closed without merge, and no finished Task names it.
    ClosedPullRequest {
        number: u64,
    },
    /// A finished Task names it, but its head is not in the default branch.
    UnmergedTaskWork,
    NoEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    Delete(Evidence),
    Retain(Retention),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BranchDecision {
    pub copy: BranchCopy,
    pub branch: String,
    pub head: String,
    pub outcome: Outcome,
}

/// Everything the classifier reads, gathered before any decision.
#[derive(Debug, Default)]
pub(crate) struct BranchFacts {
    pub default_branch: String,
    pub remote_heads: HashMap<String, String>,
    pub local_heads: HashMap<String, String>,
    pub worktree_branches: HashSet<String>,
    pub pull_requests: HashMap<String, Vec<PullRequestNode>>,
    pub tasks: TaskBranches,
    pub tags: HashSet<String>,
}

fn pull_request_state_name(state: PullRequestState) -> &'static str {
    match state {
        PullRequestState::Open => "open",
        PullRequestState::Merged => "merged",
        PullRequestState::Closed => "closed",
    }
}

impl fmt::Display for Evidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PullRequest { number, state } => {
                write!(f, "{} PR #{number}", pull_request_state_name(*state))
            }
            Self::FinishedTask => write!(f, "finished Task, contained in the default branch"),
            Self::Released { tag } => write!(f, "released {tag}"),
        }
    }
}

impl fmt::Display for Retention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefaultBranch => write!(f, "default branch"),
            Self::Worktree => write!(f, "checked out in a worktree"),
            Self::LiveTask => write!(f, "named by an unfinished Task"),
            Self::OpenPullRequest { number } => write!(f, "open PR #{number}"),
            Self::MovedPastPullRequest { number, state } => write!(
                f,
                "head moved past {} PR #{number}",
                pull_request_state_name(*state)
            ),
            Self::ClosedPullRequest { number } => {
                write!(f, "closed PR #{number} without a finished Task")
            }
            Self::UnmergedTaskWork => {
                write!(f, "finished Task has commits outside the default branch")
            }
            Self::NoEvidence => write!(f, "no finished-work evidence"),
        }
    }
}

impl fmt::Display for BranchCopy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Remote => write!(f, "origin"),
            Self::Local => write!(f, "local "),
        }
    }
}

/// Decide every branch copy, remote then local, in branch order.
///
/// `is_ancestor` answers whether a head is contained in the default branch;
/// it is asked only for finished-Task candidates.
pub(crate) fn classify(
    facts: &BranchFacts,
    is_ancestor: impl Fn(&str) -> bool,
) -> Vec<BranchDecision> {
    let copies = [
        (BranchCopy::Remote, &facts.remote_heads),
        (BranchCopy::Local, &facts.local_heads),
    ];
    let is_ancestor = &is_ancestor;
    let mut decisions: Vec<BranchDecision> = copies
        .into_iter()
        .flat_map(|(copy, heads)| {
            heads.iter().map(move |(branch, head)| BranchDecision {
                copy,
                branch: branch.clone(),
                head: head.clone(),
                outcome: decide(facts, branch, head, is_ancestor),
            })
        })
        .collect();
    decisions.sort_by(|a, b| (a.copy, &a.branch).cmp(&(b.copy, &b.branch)));
    decisions
}

/// The protection that keeps a branch whatever GitHub says, if any.
fn protection(facts: &BranchFacts, branch: &str) -> Option<Retention> {
    if branch == facts.default_branch {
        Some(Retention::DefaultBranch)
    } else if facts.worktree_branches.contains(branch) {
        Some(Retention::Worktree)
    } else if facts.tasks.live.contains(branch) {
        Some(Retention::LiveTask)
    } else {
        None
    }
}

fn decide(
    facts: &BranchFacts,
    branch: &str,
    head: &str,
    is_ancestor: &impl Fn(&str) -> bool,
) -> Outcome {
    if let Some(retention) = protection(facts, branch) {
        return Outcome::Retain(retention);
    }
    let nodes = facts
        .pull_requests
        .get(branch)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if let Some(open) = nodes
        .iter()
        .find(|node| node.state == PullRequestState::Open)
    {
        return Outcome::Retain(Retention::OpenPullRequest {
            number: open.number,
        });
    }
    let finished_task = facts.tasks.finished.contains(branch);
    if let Some(node) = head_pull_request(nodes, head) {
        if node.state == PullRequestState::Closed && !finished_task {
            return Outcome::Retain(Retention::ClosedPullRequest {
                number: node.number,
            });
        }
        return Outcome::Delete(Evidence::PullRequest {
            number: node.number,
            state: node.state,
        });
    }
    if finished_task && is_ancestor(head) {
        return Outcome::Delete(Evidence::FinishedTask);
    }
    if let Some(tag) = released_candidate_tag(branch, head, &facts.tags) {
        return Outcome::Delete(Evidence::Released {
            tag: tag.to_string(),
        });
    }
    if let Some(node) = nodes.first() {
        return Outcome::Retain(Retention::MovedPastPullRequest {
            number: node.number,
            state: node.state,
        });
    }
    Outcome::Retain(if finished_task {
        Retention::UnmergedTaskWork
    } else {
        Retention::NoEvidence
    })
}

/// Why a run deleted nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PruneStop {
    GitHub(RemoteFailure),
    Budget(GraphqlBudget),
    Git(String),
}

impl fmt::Display for PruneStop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitHub(failure) => write!(f, "GitHub {failure}; nothing deleted"),
            Self::Budget(budget) => write!(
                f,
                "GitHub GraphQL budget {} left until {}; skipped",
                budget.remaining,
                clock(budget.reset)
            ),
            Self::Git(detail) => write!(f, "{detail}; nothing deleted"),
        }
    }
}

/// Local wall-clock `HH:MM` for a Unix time.
fn clock(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .map(|at| at.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|| unix.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryStatus {
    WouldDelete,
    Deleted,
    /// Another machine deleted it first.
    AlreadyGone,
    /// The lease saw a head other than the one judged.
    Moved,
    Failed(String),
    Retained,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BranchPruneEntry {
    pub decision: BranchDecision,
    pub status: EntryStatus,
}

/// One log line: `<action> <copy> <branch>  <reason>`.
impl fmt::Display for BranchPruneEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let decision = &self.decision;
        let head = decision.head.get(..7).unwrap_or(&decision.head);
        let reason = match &decision.outcome {
            Outcome::Delete(evidence) => evidence.to_string(),
            Outcome::Retain(retention) => retention.to_string(),
        };
        let action = match &self.status {
            EntryStatus::WouldDelete => "would delete",
            EntryStatus::Deleted => "deleted",
            EntryStatus::AlreadyGone => "already gone",
            EntryStatus::Retained | EntryStatus::Moved => "retained",
            EntryStatus::Failed(_) => "failed",
        };
        let reason = match &self.status {
            EntryStatus::Moved => format!("head moved from {head} during deletion"),
            EntryStatus::Failed(error) => format!("{reason}: {error}"),
            _ => format!("{reason} at {head}"),
        };
        write!(
            f,
            "{action} {} {}  {reason}",
            decision.copy, decision.branch
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BranchPruneReport {
    pub entries: Vec<BranchPruneEntry>,
    pub budget: GraphqlBudget,
    pub dry_run: bool,
}

impl BranchPruneReport {
    /// Copies deleted, or that a dry run would delete.
    pub(crate) fn deleted(&self, copy: BranchCopy) -> usize {
        self.entries
            .iter()
            .filter(|entry| {
                entry.decision.copy == copy
                    && matches!(
                        entry.status,
                        EntryStatus::WouldDelete | EntryStatus::Deleted
                    )
            })
            .count()
    }

    pub(crate) fn failed(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| matches!(entry.status, EntryStatus::Failed(_)))
            .count()
    }

    pub(crate) fn retained(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| matches!(entry.status, EntryStatus::Retained | EntryStatus::Moved))
            .count()
    }
}

/// The run's summary line.
impl fmt::Display for BranchPruneReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "branches: {} {} remote, {} local; retained {}; failed {}; GitHub GraphQL {}/{} left, resets {}",
            if self.dry_run { "would delete" } else { "deleted" },
            self.deleted(BranchCopy::Remote),
            self.deleted(BranchCopy::Local),
            self.retained(),
            self.failed(),
            self.budget.remaining,
            self.budget.limit,
            clock(self.budget.reset),
        )
    }
}

fn git_succeeds(repo: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Gather facts, classify, and (unless `dry_run`) delete the qualifying copies.
pub(crate) fn prune_branches(
    repo: &Path,
    tasks: TaskBranches,
    dry_run: bool,
) -> Result<BranchPruneReport, PruneStop> {
    let git_stop = |error: crate::error::GitError| PruneStop::Git(error.to_string());
    let repo = main_repo_root(repo).map_err(git_stop)?;
    let budget = github_graphql_budget(&repo).map_err(PruneStop::GitHub)?;
    if budget.remaining < MINIMUM_GRAPHQL_BUDGET {
        return Err(PruneStop::Budget(budget));
    }
    let default_branch = get_default_branch(&repo).map_err(git_stop)?;
    let worktree_branches = list_porcelain(&repo)
        .map_err(git_stop)?
        .into_iter()
        .filter_map(|(_, branch)| branch)
        .collect();
    let remote_heads = remote_heads(&repo).map_err(PruneStop::GitHub)?;
    let tags = remote_tags(&repo).map_err(PruneStop::GitHub)?;
    let local_heads = branch_heads(&repo);
    let mut facts = BranchFacts {
        default_branch,
        remote_heads,
        local_heads,
        worktree_branches,
        pull_requests: HashMap::new(),
        tasks,
        tags,
    };
    // Protected names need no GitHub answer; every other name needs one.
    let asked: Vec<String> = facts
        .remote_heads
        .keys()
        .chain(facts.local_heads.keys())
        .filter(|branch| protection(&facts, branch).is_none())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !asked.is_empty() {
        facts.pull_requests = github_pull_requests(&repo, &asked).map_err(PruneStop::GitHub)?;
    }
    let target = format!("origin/{}", facts.default_branch);
    let decisions = classify(&facts, |head| {
        git_succeeds(&repo, &["merge-base", "--is-ancestor", head, &target])
    });
    let entries = execute(&repo, decisions, dry_run);
    if !dry_run {
        // Remote-tracking refs for branches deleted here or elsewhere.
        git_succeeds(&repo, &["remote", "prune", "origin"]);
    }
    Ok(BranchPruneReport {
        entries,
        budget,
        dry_run,
    })
}

/// Delete each qualifying copy with a lease on its judged head, remote first.
fn execute(repo: &Path, decisions: Vec<BranchDecision>, dry_run: bool) -> Vec<BranchPruneEntry> {
    decisions
        .into_iter()
        .map(|decision| {
            let status = match &decision.outcome {
                Outcome::Retain(_) => EntryStatus::Retained,
                Outcome::Delete(_) if dry_run => EntryStatus::WouldDelete,
                Outcome::Delete(_) => {
                    let head = Some(decision.head.clone());
                    let deletion = match decision.copy {
                        BranchCopy::Remote => branch_copies_at(repo, &decision.branch, head, None),
                        BranchCopy::Local => branch_copies_at(repo, &decision.branch, None, head),
                    };
                    match apply_delete(deletion, &NullProgress) {
                        Ok(()) => EntryStatus::Deleted,
                        Err(error) => failed_status(repo, &decision, &error),
                    }
                }
            };
            BranchPruneEntry { decision, status }
        })
        .collect()
}

/// Read a failed deletion again: a lease that lost to another machine's
/// deletion, or to a new push, is not a failure.
fn failed_status(repo: &Path, decision: &BranchDecision, error: &OpsError) -> EntryStatus {
    let current = match decision.copy {
        BranchCopy::Remote => match remote_heads(repo) {
            Ok(heads) => heads.get(&decision.branch).cloned(),
            Err(_) => return EntryStatus::Failed(error.to_string()),
        },
        BranchCopy::Local => branch_heads(repo).get(&decision.branch).cloned(),
    };
    match current {
        None => EntryStatus::AlreadyGone,
        Some(head) if head != decision.head => EntryStatus::Moved,
        Some(_) => EntryStatus::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify, BranchCopy, BranchDecision, BranchFacts, Evidence, Outcome, Retention,
        TaskBranches,
    };
    use crate::git::worktrees::{PullRequestNode, PullRequestState};
    use std::collections::{HashMap, HashSet};

    const CANDIDATE: &str =
        "release-candidate/default/v0-14-1/a005a005a005a005a005a005a005a005a005a005";

    fn pr(number: u64, head: &str, state: PullRequestState) -> PullRequestNode {
        PullRequestNode {
            number,
            head: head.to_string(),
            state,
        }
    }

    fn facts(remote: &[(&str, &str)]) -> BranchFacts {
        BranchFacts {
            default_branch: "main".to_string(),
            remote_heads: remote
                .iter()
                .map(|(branch, head)| (branch.to_string(), head.to_string()))
                .collect(),
            tags: HashSet::from(["v0.14.1".to_string()]),
            ..BranchFacts::default()
        }
    }

    fn outcome(facts: &BranchFacts, branch: &str) -> Outcome {
        classify(facts, |head| head == "in-main")
            .into_iter()
            .find(|decision| decision.branch == branch && decision.copy == BranchCopy::Remote)
            .unwrap()
            .outcome
    }

    #[test]
    fn finished_pull_requests_at_the_head_are_evidence() {
        let mut facts = facts(&[
            ("merged", "m"),
            ("closed", "c"),
            ("abandoned", "a"),
            ("moved", "new"),
        ]);
        facts.tasks.finished = HashSet::from(["abandoned".to_string()]);
        facts.pull_requests = HashMap::from([
            (
                "merged".to_string(),
                vec![pr(1, "m", PullRequestState::Merged)],
            ),
            (
                "closed".to_string(),
                vec![pr(2, "c", PullRequestState::Closed)],
            ),
            (
                "abandoned".to_string(),
                vec![pr(8, "a", PullRequestState::Closed)],
            ),
            (
                "moved".to_string(),
                vec![pr(3, "old", PullRequestState::Merged)],
            ),
        ]);

        assert_eq!(
            outcome(&facts, "merged"),
            Outcome::Delete(Evidence::PullRequest {
                number: 1,
                state: PullRequestState::Merged
            })
        );
        // GitHub closes a stacked PR whose base is deleted; that is not abandonment.
        assert_eq!(
            outcome(&facts, "closed"),
            Outcome::Retain(Retention::ClosedPullRequest { number: 2 })
        );
        assert_eq!(
            outcome(&facts, "abandoned"),
            Outcome::Delete(Evidence::PullRequest {
                number: 8,
                state: PullRequestState::Closed
            })
        );
        assert_eq!(
            outcome(&facts, "moved"),
            Outcome::Retain(Retention::MovedPastPullRequest {
                number: 3,
                state: PullRequestState::Merged
            })
        );
    }

    #[test]
    fn an_open_pull_request_at_any_head_protects_the_branch() {
        let mut facts = facts(&[("reopened", "h")]);
        facts.pull_requests = HashMap::from([(
            "reopened".to_string(),
            vec![
                pr(4, "h", PullRequestState::Closed),
                pr(5, "elsewhere", PullRequestState::Open),
            ],
        )]);

        assert_eq!(
            outcome(&facts, "reopened"),
            Outcome::Retain(Retention::OpenPullRequest { number: 5 })
        );
    }

    #[test]
    fn protections_override_evidence() {
        let mut facts = facts(&[("main", "m"), ("checked-out", "c"), ("live", "l")]);
        facts.worktree_branches = HashSet::from(["checked-out".to_string()]);
        facts.tasks.live = HashSet::from(["live".to_string()]);
        for (branch, head) in [("main", "m"), ("checked-out", "c"), ("live", "l")] {
            facts.pull_requests.insert(
                branch.to_string(),
                vec![pr(6, head, PullRequestState::Merged)],
            );
        }

        assert_eq!(
            outcome(&facts, "main"),
            Outcome::Retain(Retention::DefaultBranch)
        );
        assert_eq!(
            outcome(&facts, "checked-out"),
            Outcome::Retain(Retention::Worktree)
        );
        assert_eq!(
            outcome(&facts, "live"),
            Outcome::Retain(Retention::LiveTask)
        );
    }

    #[test]
    fn a_finished_task_needs_its_head_in_the_default_branch() {
        let mut facts = facts(&[("landed", "in-main"), ("extra", "beyond")]);
        facts.tasks = TaskBranches {
            live: HashSet::new(),
            finished: HashSet::from(["landed".to_string(), "extra".to_string()]),
        };

        assert_eq!(
            outcome(&facts, "landed"),
            Outcome::Delete(Evidence::FinishedTask)
        );
        assert_eq!(
            outcome(&facts, "extra"),
            Outcome::Retain(Retention::UnmergedTaskWork)
        );
    }

    #[test]
    fn a_release_candidate_needs_its_commit_and_tag() {
        let moved = "release-candidate/default/v0-14-1/b006b006b006b006b006b006b006b006b006b006";
        let untagged = "release-candidate/default/v0-99-0/a005a005a005a005a005a005a005a005a005a005";
        let facts = facts(&[
            (CANDIDATE, "a005a005a005a005a005a005a005a005a005a005"),
            (moved, "c007"),
            (untagged, "a005a005a005a005a005a005a005a005a005a005"),
            ("release-candidate/v0-14-1", "a005"),
            ("jack-heart/publish-default-v0-13-0", "p"),
        ]);

        assert_eq!(
            outcome(&facts, CANDIDATE),
            Outcome::Delete(Evidence::Released {
                tag: "v0.14.1".to_string()
            })
        );
        for retained in [
            moved,
            untagged,
            "release-candidate/v0-14-1",
            "jack-heart/publish-default-v0-13-0",
        ] {
            assert_eq!(
                outcome(&facts, retained),
                Outcome::Retain(Retention::NoEvidence),
                "{retained}"
            );
        }
    }

    #[test]
    fn each_copy_is_judged_at_its_own_head() {
        let mut facts = facts(&[("feature", "pushed")]);
        facts.local_heads = HashMap::from([("feature".to_string(), "local-only".to_string())]);
        facts.pull_requests = HashMap::from([(
            "feature".to_string(),
            vec![pr(7, "pushed", PullRequestState::Merged)],
        )]);

        let decisions = classify(&facts, |_| false);

        assert_eq!(
            decisions,
            vec![
                BranchDecision {
                    copy: BranchCopy::Remote,
                    branch: "feature".to_string(),
                    head: "pushed".to_string(),
                    outcome: Outcome::Delete(Evidence::PullRequest {
                        number: 7,
                        state: PullRequestState::Merged
                    }),
                },
                BranchDecision {
                    copy: BranchCopy::Local,
                    branch: "feature".to_string(),
                    head: "local-only".to_string(),
                    outcome: Outcome::Retain(Retention::MovedPastPullRequest {
                        number: 7,
                        state: PullRequestState::Merged
                    }),
                },
            ]
        );
    }
}
