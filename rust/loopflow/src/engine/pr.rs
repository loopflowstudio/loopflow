//! The `pr` namespace: the pull request of the checkout containing a
//! directory. `pr.publish` and `pr.open` push and create or refresh it,
//! `pr.submit` and `pr.arm` prepare it to land, `pr.land` lands it detached
//! ([`super::launch`]), `pr.abandon` closes it with its branch, `pr.reconcile`
//! checks recorded landings, and `pr.checks`, `pr.reviews` and
//! `pr.request_review` read and ask for its reviews on the provider.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::{launch, Error, ErrorCode, Result};
use crate::ops::pr_landing::DeliveryCheck;
use crate::ops::{LandOptions, PrInfo, Progress};

/// What `pr.publish` or `pr.open` left on the provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Published {
    pub url: String,
    /// The PR was created now; false when an existing one was refreshed.
    pub created: bool,
    /// The PR as the provider reports it afterwards, when it does.
    pub pr: Option<PrInfo>,
}

/// What `pr.submit` prepared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Prepared {
    /// The PR prepared; null when `local` kept everything on this Machine.
    pub pr: Option<PrInfo>,
}

/// What `pr.arm` or `lf pr land` reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Landed {
    /// The PR armed or merged; null when it had already merged or `local`.
    pub pr: Option<PrInfo>,
    /// The PR is merged, by this land or before it.
    pub merged: bool,
}

/// What `pr.abandon` closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Abandoned {
    pub branch: String,
}

/// How the branch's checks stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChecksState {
    Passing,
    Failing,
    Pending,
}

/// `pr.checks`: the provider's check report for the branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Checks {
    pub branch: String,
    pub state: ChecksState,
    /// The report as `gh pr checks` prints it.
    pub report: String,
}

impl Checks {
    /// `gh pr checks`'s exit status for this state, which `lf pr checks` keeps.
    pub fn exit_code(&self) -> u8 {
        match self.state {
            ChecksState::Passing => 0,
            ChecksState::Failing => 1,
            ChecksState::Pending => 8,
        }
    }
}

/// What `pr.request_review` asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewRequested {
    /// The PR named, or null for the current branch's.
    pub pr: Option<String>,
    pub reviewers: Vec<String>,
}

/// `lf pr submit|arm|land`'s arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LandInput {
    pub strict: bool,
    /// Keep everything on this Machine: no push, no PR.
    pub local: bool,
    /// `pr.land` only: wait for the verified merge, repairing failing CI.
    pub wait_and_fix: bool,
    pub worktree: Option<String>,
    pub message: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub agent: Option<String>,
}

impl LandInput {
    fn options(&self) -> LandOptions {
        LandOptions {
            strict: self.strict,
            local: self.local,
            wait_and_fix: self.wait_and_fix,
            worktree: self.worktree.clone(),
            commit_message: self.message.clone(),
            pr_title: self.title.clone(),
            pr_body: self.body.clone(),
            agent: self.agent.clone(),
        }
    }
}

/// `pr.publish`: push and create or refresh a ready PR, as `lf pr publish`
/// does; no review surface opens.
pub fn publish(
    directory: &Path,
    title: Option<&str>,
    body: Option<&str>,
    agent: Option<&str>,
    progress: &impl Progress,
) -> Result<Published> {
    prepare(directory, title, body, agent, false, progress)
}

/// `pr.open`: push and create or refresh a draft PR, as `lf pr open` does;
/// an existing ready PR stays ready. The CLI then presents it for review.
pub fn open(
    directory: &Path,
    title: Option<&str>,
    body: Option<&str>,
    agent: Option<&str>,
    progress: &impl Progress,
) -> Result<Published> {
    prepare(directory, title, body, agent, true, progress)
}

fn prepare(
    directory: &Path,
    title: Option<&str>,
    body: Option<&str>,
    agent: Option<&str>,
    draft: bool,
    progress: &impl Progress,
) -> Result<Published> {
    let repo = repo_root(directory)?;
    let result = crate::ops::create_or_update_pr(
        &repo,
        &crate::ops::PrOptions {
            draft,
            title: title.map(str::to_owned),
            body: body.map(str::to_owned),
            agent: agent.map(str::to_owned),
        },
        progress,
    )
    .map_err(outcome_unknown)?;
    Ok(Published {
        url: result.url,
        created: result.created,
        pr: crate::ops::current_pr(&repo).ok().flatten(),
    })
}

/// `pr.submit`: sync, clear committed scratch additions and mark the PR
/// ready for a teammate's merge, as `lf pr submit` does.
pub fn submit(directory: &Path, input: &LandInput, progress: &impl Progress) -> Result<Prepared> {
    let repo = repo_root(directory)?;
    let pr =
        crate::lf::commands::ops::with_sync_retry(&repo, "submit", progress, |repo, integrated| {
            if integrated {
                crate::ops::finish_submit_after_sync(repo, &input.options(), progress)
            } else {
                crate::ops::submit(repo, &input.options(), progress)
            }
        })
        .map_err(outcome_unknown)?;
    Ok(Prepared { pr })
}

/// `pr.arm`: prepare the PR and request exact-head auto-merge, returning
/// without watching, as `lf pr arm` does. A PR that already merged is
/// reported `merged` without another push.
pub fn arm(directory: &Path, input: &LandInput, progress: &impl Progress) -> Result<Landed> {
    let repo = repo_root(directory)?;
    land_in(
        &repo,
        &LandOptions {
            wait_and_fix: false,
            ..input.options()
        },
        progress,
    )
    .map_err(|error| Error::outcome_unknown(&error))
}

/// `pr.land`: start `lf pr land …` detached and return the Process carrying
/// it. Landing syncs first, which may launch an agent to resolve conflicts;
/// with `wait_and_fix` the Process stays until the merge is verified.
pub fn land(directory: &Path, input: &LandInput) -> Result<launch::Launched> {
    let mut args = vec!["pr".to_owned(), "land".to_owned()];
    for (flag, on) in [
        ("--strict", input.strict),
        ("--local", input.local),
        ("--wait-and-fix", input.wait_and_fix),
    ] {
        if on {
            args.push(flag.to_owned());
        }
    }
    for (flag, value) in [
        ("--worktree", &input.worktree),
        ("--message", &input.message),
        ("--title", &input.title),
        ("--body", &input.body),
    ] {
        if let Some(value) = value {
            args.extend([flag.to_owned(), value.clone()]);
        }
    }
    args.push("--json".to_owned());
    let mut argv = Vec::new();
    if let Some(agent) = &input.agent {
        argv.extend(["--agent".to_owned(), agent.clone()]);
    }
    argv.extend(args);
    launch::start(directory, &argv)
}

/// `lf pr land` in this process. Started by [`land`], it reports its Process
/// first and carries on in its own log. Its errors keep their exit status: a
/// held delivery is `GraphHeld`.
pub fn carry_land(
    directory: &Path,
    input: &LandInput,
    progress: &impl Progress,
) -> anyhow::Result<Landed> {
    let repo = crate::repo::require_repo_root(directory, "lf pr land")?;
    launch::carry(
        "pr.land",
        format!(
            "land {}",
            input
                .worktree
                .clone()
                .or_else(|| crate::git::current_branch(&repo).ok().flatten())
                .unwrap_or_else(|| "the checkout".into())
        ),
    )?;
    land_in(&repo, &input.options(), progress)
}

/// Arm the checkout's PR, and with `wait_and_fix` stay until it merges. A PR
/// that already merged with nothing new to push is reported without a push.
fn land_in(repo: &Path, options: &LandOptions, progress: &impl Progress) -> anyhow::Result<Landed> {
    use crate::ops::OpsError;
    let (checkout, _) = crate::ops::land::resolve_repos(repo, options.worktree.as_deref())?;
    if !options.local {
        if let Some((task, pr, state)) = crate::ops::task::reconcile_checkout_pr(&checkout)? {
            let unchanged = pr.phase() == crate::work::task::PrPhase::Merged
                && crate::git::git_stdout(
                    &checkout,
                    &["status", "--porcelain", "--", ".", ":(exclude)scratch"],
                )?
                .trim()
                .is_empty()
                && (pr.head_sha() == Some(crate::git::rev_parse(&checkout, "HEAD")?.as_str())
                    || crate::ops::landed_at(
                        &checkout,
                        &crate::git::integration_base(&checkout)?,
                        "HEAD",
                    )?);
            if unchanged {
                let next = match state {
                    crate::durable::TaskState::Done => {
                        format!("Task {} is complete.", task.identifier)
                    }
                    crate::durable::TaskState::Abandoned => {
                        format!("Task {} is abandoned.", task.identifier)
                    }
                    _ => format!(
                        "Complete the accepted outcome with lf task complete {}.",
                        task.identifier
                    ),
                };
                progress.status(&format!("Pull request already merged. {next}"));
                return Ok(Landed {
                    pr: None,
                    merged: true,
                });
            }
        }
    }
    // The wave home stays put on land.
    let pr =
        crate::lf::commands::ops::with_sync_retry(repo, "land", progress, |repo, integrated| {
            if integrated {
                crate::ops::finish_arm_after_sync(repo, options, progress, &|_| {})
            } else {
                crate::ops::arm(repo, options, progress)
            }
        })?;
    let Some(pr) = pr else {
        return Ok(Landed {
            pr: None,
            merged: false,
        });
    };
    if options.wait_and_fix {
        crate::agent::register_interrupt_cleanup(|| {
            eprintln!("Landing wait interrupted; merge intent retained.");
        });
        crate::ops::pr_landing::wait_for_merge(repo, options, &pr).map_err(
            |error| match error {
                OpsError::DeliveryHeld(reason) => {
                    anyhow::Error::new(crate::process::GraphHeld(reason))
                }
                error => anyhow::Error::new(error),
            },
        )?;
        progress.status("Pull request merged; Task completion can proceed.");
        return Ok(Landed {
            pr: Some(pr),
            merged: true,
        });
    }
    progress.status(&format!(
        "PR #{} handed off; lf pr reconcile checks delivery.",
        pr.number
    ));
    Ok(Landed {
        pr: Some(pr),
        merged: false,
    })
}

/// `pr.abandon`: close the branch's PR and delete its worktree and branches,
/// as `lf pr abandon [<branch>] [--force]` does. Without `force`, the
/// deletion of unpublished work asks `progress` first.
pub fn abandon(
    directory: &Path,
    branch: Option<&str>,
    force: bool,
    progress: &impl Progress,
) -> Result<Abandoned> {
    let repo = repo_root(directory)?;
    let branch = match branch {
        Some(branch) => branch.to_owned(),
        None => crate::git::current_branch(&repo)
            .map_err(outcome_unknown)?
            .ok_or_else(|| Error::new(ErrorCode::Invalid, "not on a branch"))?,
    };
    crate::ops::abandon_branch(
        &repo,
        &crate::ops::AbandonOptions {
            branch: Some(branch.clone()),
            force,
        },
        progress,
    )
    .map_err(outcome_unknown)?;
    Ok(Abandoned { branch })
}

/// `pr.reconcile`: check recorded landings once, record CI failures and
/// settle verified merges, as `lf pr reconcile` does. `errors` lists what
/// did not settle; the CLI exits 1 on any.
pub fn reconcile(directory: &Path) -> Result<DeliveryCheck> {
    let repo = repo_root(directory)?;
    crate::ops::task::reconcile_checkout_pr(&repo).map_err(outcome_unknown)?;
    crate::ops::pr_landing::reconcile_repository(&repo).map_err(outcome_unknown)
}

/// `pr.checks`: the branch's check report, as `gh pr checks` prints it;
/// `required` keeps only the checks repository protection requires. A `gh`
/// failure that is neither failing nor pending checks keeps its exit status
/// in the error's `data.exit`.
pub fn checks(directory: &Path, required: bool) -> Result<Checks> {
    let repo = repo_root(directory)?;
    let branch = crate::git::current_branch(&repo)
        .map_err(outcome_unknown)?
        .ok_or_else(|| Error::new(ErrorCode::Invalid, "not on a branch"))?;
    let mut args = vec!["pr", "checks", branch.as_str()];
    if required {
        args.push("--required");
    }
    let output = gh(&repo, &args)?;
    let state = match output.status.code() {
        Some(0) => ChecksState::Passing,
        Some(1) => ChecksState::Failing,
        Some(8) => ChecksState::Pending,
        _ => return Err(gh_failed(&output)),
    };
    Ok(Checks {
        branch,
        state,
        report: String::from_utf8_lossy(&output.stdout).into_owned(),
    })
}

/// What `pr.reviews` answers, decoded from `gh pr view` and named as this API
/// names things. A review keeps the commit it reviewed, so an old approval
/// never reads as one of the current head.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Reviews {
    pub number: u64,
    pub url: String,
    #[serde(rename(deserialize = "headRefOid"))]
    pub head: Option<String>,
    #[serde(rename(deserialize = "isDraft"))]
    pub draft: bool,
    /// The provider's decision, such as `APPROVED`; empty or null when it reports none.
    #[serde(rename(deserialize = "reviewDecision"))]
    pub decision: Option<String>,
    #[serde(rename(deserialize = "reviewRequests"))]
    pub requested: Vec<ReviewRequest>,
    pub reviews: Vec<Review>,
}

/// One requested reviewer: a person's `login` or a team's `name`.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReviewRequest {
    pub login: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Review {
    pub author: Option<ReviewAuthor>,
    pub state: String,
    pub body: String,
    pub commit: Option<ReviewCommit>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReviewAuthor {
    pub login: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReviewCommit {
    pub oid: String,
}

/// `pr.reviews`: the requested reviews and review submissions of `pr` (the
/// current branch's PR with none).
pub fn reviews(directory: &Path, pr: Option<&str>) -> Result<Reviews> {
    let repo = repo_root(directory)?;
    let mut args = vec!["pr", "view"];
    args.extend(pr);
    args.extend([
        "--json",
        "number,url,headRefOid,isDraft,reviewDecision,reviewRequests,reviews",
    ]);
    let output = gh(&repo, &args)?;
    if !output.status.success() {
        return Err(gh_failed(&output));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| {
        Error::new(
            ErrorCode::Unavailable,
            format!("gh pr view printed no JSON: {error}"),
        )
    })
}

/// `pr.request_review`: ask `reviewers` (logins or `org/team`) to review
/// `pr`, the current branch's PR with none, as `lf pr request-review` does.
pub fn request_review(
    directory: &Path,
    pr: Option<&str>,
    reviewers: &[String],
) -> Result<ReviewRequested> {
    let repo = repo_root(directory)?;
    let mut args = vec!["pr", "edit"];
    args.extend(pr);
    for reviewer in reviewers {
        args.extend(["--add-reviewer", reviewer]);
    }
    let output = gh(&repo, &args)?;
    if !output.status.success() {
        return Err(gh_failed(&output));
    }
    Ok(ReviewRequested {
        pr: pr.map(str::to_owned),
        reviewers: reviewers.to_vec(),
    })
}

fn gh(repo: &Path, args: &[&str]) -> Result<std::process::Output> {
    Command::new("gh")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|error| Error::new(ErrorCode::Unavailable, format!("gh: {error}")))
}

/// A failed `gh` call: its stderr, with the exit status in `data.exit` so the
/// CLI can keep it.
fn gh_failed(output: &std::process::Output) -> Error {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Error::new(
        ErrorCode::Unavailable,
        if stderr.is_empty() {
            format!("gh exited with status {}", output.status)
        } else {
            stderr
        },
    )
    .with(serde_json::json!({ "exit": output.status.code().unwrap_or(1) }))
}

/// The exit status a refused engine call carried from `gh`, as a CLI error.
pub fn exit_of(error: Error) -> anyhow::Error {
    let exit = error
        .data
        .as_ref()
        .and_then(|data| data.get("exit"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|code| u8::try_from(code).ok());
    match exit {
        Some(code) => {
            eprintln!("{}", error.message);
            crate::process::CommandExit(code).into()
        }
        None => error.into(),
    }
}

fn repo_root(directory: &Path) -> Result<PathBuf> {
    crate::repo::require_repo_root(directory, "this command")
        .map_err(|error| Error::refused(&error, ErrorCode::Invalid))
}

fn outcome_unknown(error: impl Into<anyhow::Error>) -> Error {
    Error::outcome_unknown(&error.into())
}

#[cfg(test)]
mod tests {
    use super::{Checks, Published, ReviewRequested};

    fn result<T: serde::de::DeserializeOwned + serde::Serialize>(raw: &str) -> T {
        let message: serde_json::Value = serde_json::from_str(raw).unwrap();
        let typed: T = serde_json::from_value(message["result"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&typed).unwrap(), message["result"]);
        typed
    }

    #[test]
    fn receipts_match_the_shared_fixtures_without_defaults() {
        let published: Published = result(include_str!(
            "../../../../tests/fixtures/dto/server/pr_published.json"
        ));
        assert_eq!(published.pr.as_ref().map(|pr| pr.number), Some(1813));
        let checks: Checks = result(include_str!(
            "../../../../tests/fixtures/dto/server/pr_checks_report.json"
        ));
        assert_eq!(checks.exit_code(), 8);
        let requested: ReviewRequested = result(include_str!(
            "../../../../tests/fixtures/dto/server/pr_review_requested.json"
        ));
        assert_eq!(requested.reviewers, ["alice", "org/platform"]);
        assert!(
            serde_json::from_str::<Published>(r#"{"url":"https://example.com/pr/1"}"#).is_err()
        );
    }
}
