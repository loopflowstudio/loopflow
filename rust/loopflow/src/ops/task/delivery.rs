//! Task PR delivery: managed-Task resolution, PR copy and range proofs,
//! merge requests and GitHub observation of the Task's current PR.

use super::{
    block_on_task, git_output, link_pr_to_planning, reconcile_task_completion, repository_for_task,
    task_error,
};
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use crate::durable::TaskState;
use crate::git::{current_branch, get_default_branch, is_clean, rev_parse};
use crate::ops::error::{OpsError, OpsResult};
use crate::store::{open_registry_for_authority, RegistryUnavailable, SharedStore};
use crate::work::task::{
    CiCheck, CiObservation, CiState, GithubObservation, GithubObservationResult, GithubPr,
    Observation, PrMergeMode, PrMergeRequest, PrPhase, PrPresentation, PrPublication, Task,
    TaskEventKind, TaskPr, TaskPrId,
};
use fs2::FileExt;
use time::format_description::well_known::Rfc3339;

/// A managed Task worktree, or an explicit decision
/// that this worktree is not a Task worktree.
///
/// The PR publication, stacking, submit, and land entry points share this one
/// resolver so they cannot disagree about Task ownership. An absent registry
/// without an explicit declaration permits ordinary PR work. An unreadable
/// registry or unresolved declaration reports the missing authority instead.
#[derive(Debug)]
pub(super) enum ManagedTask {
    /// This checkout has no Task. Task-specific bookkeeping is an
    /// explicit no-op; the ordinary PR flow continues unchanged.
    Unmanaged,
    /// The registry is healthy and the checkout belongs to a Task.
    /// Boxed so the `Unmanaged` no-op variant stays small.
    Managed { store: SharedStore, task: Box<Task> },
}

/// Turn a [`RegistryUnavailable`] into an actionable authority error. The
/// message always names the recovery action so the operator can move.
pub(super) fn task_registry_error(err: RegistryUnavailable) -> OpsError {
    task_error(match err {
        RegistryUnavailable::MissingFile { path } => format!(
            "Task PR authority refused: the shared Loopflow registry {} is missing. \
             Run `lf doctor` to inspect the registry.",
            path.display()
        ),
        RegistryUnavailable::Unresolved { error } => format!(
            "Task PR authority refused: the shared Loopflow registry path is not usable: {error}. \
             Fix LF_HOME or run `lf doctor`."
        ),
        RegistryUnavailable::Incompatible { path, error } => format!(
            "Task PR authority refused: the shared Loopflow registry {} is present but \
             inaccessible or schema-incompatible: {error}. Run `lf doctor`.",
            path.display()
        ),
    })
}

/// Resolve the managed Task for a PR entry point at `repo`.
///
/// - Checkout ownership, then an explicit declaration, resolves a Task → [`ManagedTask::Managed`].
/// - Neither resolves a Task → [`ManagedTask::Unmanaged`].
/// - Registry file missing without a declaration → [`ManagedTask::Unmanaged`].
///   Missing records cannot recover checkout ownership or authorize inherited identity.
/// - Registry present but unopenable → refuse.
pub(super) async fn resolve_managed_task(repo: &Path) -> OpsResult<ManagedTask> {
    let store = match open_registry_for_authority().await {
        Ok(store) => Arc::new(store),
        Err(RegistryUnavailable::MissingFile { .. })
            if std::env::var_os(crate::lf::WORK_DECLARATION_ENV).is_none() =>
        {
            return Ok(ManagedTask::Unmanaged);
        }
        Err(err) => return Err(task_registry_error(err)),
    };
    let binding = crate::ops::resolve_execution_binding(&store, repo).await?;
    let task = match binding.map(|binding| binding.work) {
        Some(crate::durable::WorkRef::Task(id)) => store.get_task(&id).await.map_err(task_error)?,
        _ => None,
    };
    match task {
        Some(task) => Ok(ManagedTask::Managed {
            store,
            task: Box::new(task),
        }),
        None => Ok(ManagedTask::Unmanaged),
    }
}

pub(crate) fn record_task_pr_repair(
    repo: &Path,
    kind: crate::work::task::TaskPrRepairKind,
) -> OpsResult<bool> {
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        let Some(pr) = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        else {
            return Ok(false);
        };
        let occurred_at = time::OffsetDateTime::now_utc();
        store
            .record_task_pr_repair_incident(&pr.id, kind, occurred_at)
            .await
            .map_err(|error| task_error(format!("failed to record Task PR repair: {error}")))
    })
}

pub(super) fn task_pr_for_publication(task: &Task) -> TaskPr {
    TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        sequence: 1,
        slug: task.workspace_slug.clone(),
        branch: task.branch.clone(),
        base_commit: task.base_commit.clone(),
        parent_pr_id: task.parent_pr_id.clone(),
        publication: None,
        merge_commit: None,
        abandoned_at: None,
        ci_observation: None,
        github_observation: None,
        linear_attachment_id: None,
        linear_comment_id: None,
        linear_link_error: None,
        created_at: time::OffsetDateTime::now_utc(),
        updated_at: time::OffsetDateTime::now_utc(),
    }
}

pub(crate) fn request_task_pr_publication(repo: &Path, title: &str, body: &str) -> OpsResult<bool> {
    let title = title.trim();
    let body = body.trim();
    if title.is_empty() || body.is_empty() {
        return Err(task_error(
            "Task PR settlement requires a non-empty reviewer-facing title and body; supply both or let Loopflow generate them",
        ));
    }
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        let context = _task_pr_context_from_store(&store, &task).await?;
        _validate_task_pr_copy(&context, body)?;
        let existing = store.active_task_pr(&task.id).await.map_err(task_error)?;
        let existing = existing.filter(|pr| !pr.is_settled());
        let creating = existing.is_none();
        let mut pr = existing.unwrap_or_else(|| task_pr_for_publication(&task));
        if creating {
            pr.sequence = store
                .task_prs(&task.id)
                .await
                .map_err(task_error)?
                .iter()
                .map(|pr| pr.sequence)
                .max()
                .unwrap_or(0)
                + 1;
        }
        let branch = crate::git::current_branch(repo)?
            .ok_or_else(|| task_error("Task worktree is not on a branch"))?;
        if pr.branch != branch {
            return Err(task_error(format!(
                "Task {} active PR expects branch {:?}, but the worktree is on another branch",
                task.identifier, pr.branch
            )));
        }
        let now = time::OffsetDateTime::now_utc();
        let github = pr.github().cloned();
        let merge = pr
            .publication
            .as_ref()
            .and_then(|publication| publication.merge.as_ref())
            .filter(|request| {
                github
                    .as_ref()
                    .and_then(|github| github.head_sha.as_deref())
                    == Some(request.head_sha.as_str())
            })
            .cloned();
        pr.publication = Some(PrPublication {
            requested_at: pr
                .publication
                .as_ref()
                .map_or(now, |publication| publication.requested_at),
            presentation: pr
                .publication
                .as_ref()
                .and_then(|publication| publication.presentation.clone()),
            github,
            merge,
        });
        pr.updated_at = now;
        if creating {
            store.insert_task_pr(&pr).await
        } else {
            store.update_task_pr(&pr).await
        }
        .map_err(|error| task_error(format!("failed to request PR publication: {error}")))?;
        Ok(true)
    })
}

pub(crate) fn record_task_pr_presentation(repo: &Path, title: &str, body: &str) -> OpsResult<()> {
    let head_sha = rev_parse(repo, "HEAD")?;
    block_on_task(async {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(());
        };
        let mut pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("publication intent disappeared"))?;
        let publication = pr
            .publication
            .as_mut()
            .ok_or_else(|| task_error("publication was not requested"))?;
        publication.presentation = Some(PrPresentation {
            title: title.into(),
            body: body.into(),
            head_sha,
        });
        store.update_task_pr(&pr).await.map_err(task_error)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskPrContext {
    pub(crate) title: String,
    pub(crate) identifier: String,
    pub(crate) url: Option<String>,
    pub(crate) merge_request: Option<PrMergeRequest>,
}

impl TaskPrContext {
    pub(crate) fn task_link(&self) -> String {
        let label = format!(
            "{} · {}",
            _markdown_link_text(self.title.trim()),
            _markdown_link_text(self.identifier.trim())
        );
        match &self.url {
            Some(url) => format!("[{label}]({url})"),
            None => label,
        }
    }
}

fn _markdown_link_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

/// Publication observes a previous PR's settlement itself; callers never need
/// to run reconciliation before continuing work in a checkout.
pub(crate) fn refresh_task_pr_for_publication(repo: &Path) -> OpsResult<()> {
    block_on_task(async {
        let ManagedTask::Managed { store, mut task } = resolve_managed_task(repo).await? else {
            return Ok(());
        };
        let Some(mut pr) = store.active_task_pr(&task.id).await.map_err(task_error)? else {
            return Ok(());
        };
        if let Some(known) = pr.github().filter(|_| pr.phase() != PrPhase::Merged) {
            let open = crate::ops::pr::current_pr(repo)?;
            if pr.phase() == PrPhase::Abandoned
                || open.is_none_or(|open| open.number != u64::from(known.number))
            {
                pr = reconcile_task_pr_observation(
                    &store,
                    &mut task,
                    crate::ops::pr::PrReadFreshness::Fresh,
                )
                .await?
                .ok_or_else(|| task_error("publication PR disappeared"))?;
                if let Observation::Degraded { reason, .. } = task.observation {
                    return Err(task_error(reason));
                }
            }
        }
        if pr.phase() == PrPhase::Merged && task.parent_pr_id.is_some() {
            // Its own merge delivered the dependency too. The old PR retains
            // that stack; the next delivery compares with the default branch.
            store
                .sync_task_placement(
                    &task.id,
                    &task.base_commit,
                    true,
                    time::OffsetDateTime::now_utc(),
                )
                .await
                .map_err(task_error)?;
        }
        Ok(())
    })
}

pub(crate) fn task_pr_context(repo: &Path) -> OpsResult<Option<TaskPrContext>> {
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(None);
        };
        _task_pr_context_from_store(&store, &task).await.map(Some)
    })
}

async fn _task_pr_context_from_store(store: &SharedStore, task: &Task) -> OpsResult<TaskPrContext> {
    let url = if task.linear_id.is_some() {
        Some(
            store
                .sqlite
                .task_issue_url(&task.id)
                .map_err(task_error)?
                .filter(|url| _valid_task_url(url))
                .ok_or_else(|| _missing_task_pr_url(task))?,
        )
    } else {
        None
    };
    let pr = store.active_task_pr(&task.id).await.map_err(task_error)?;
    Ok(TaskPrContext {
        title: task.title.clone(),
        identifier: task.identifier.clone(),
        url,
        merge_request: pr.as_ref().filter(|pr| !pr.is_settled()).and_then(|pr| {
            pr.merge_request()
                .filter(|request| {
                    pr.github().and_then(|github| github.head_sha.as_deref())
                        == Some(request.head_sha.as_str())
                })
                .cloned()
        }),
    })
}

fn _valid_task_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && !value.chars().any(char::is_control)
}

fn _missing_task_pr_url(task: &Task) -> OpsError {
    task_error(format!(
        "Task {} has no valid provider URL in the cached PM snapshot. Run `lf wave status --sync` before publishing this Task PR",
        task.identifier,
    ))
}

fn _validate_task_pr_copy(context: &TaskPrContext, body: &str) -> OpsResult<()> {
    let anchor = format!("**Task:** {}", context.task_link());
    if !body.lines().any(|line| {
        line.trim()
            .strip_prefix('>')
            .map(str::trim)
            .is_some_and(|line| line == anchor)
    }) {
        return Err(task_error(format!(
            "Task PR body must include its owning Task: {anchor}"
        )));
    }
    Ok(())
}

/// The exact clean Task settlement already represented by local HEAD and the
/// stored GitHub head. `land` uses this read before any head mutation so a
/// replay can observe an already-armed request instead of clearing it.
pub(crate) fn matching_task_pr_merge_request(
    repo: &Path,
    mode: PrMergeMode,
) -> OpsResult<Option<(u32, String)>> {
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(None);
        };
        if !is_clean(repo)? {
            return Ok(None);
        }
        let branch = current_branch(repo)?;
        let head = rev_parse(repo, "HEAD")?;
        let pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.identifier)))?;
        if pr.is_settled() || branch.as_deref() != Some(pr.branch.as_str()) {
            return Ok(None);
        }
        let Some(github) = pr.github() else {
            return Ok(None);
        };
        let Some(request) = pr.merge_request() else {
            return Ok(None);
        };
        if pr.presentation().is_none()
            || github.head_sha.as_deref() != Some(head.as_str())
            || request.mode != mode
        {
            return Ok(None);
        }
        Ok(Some((github.number, head)))
    })
}

/// Clear settlement intent before a Loopflow-owned operation can move the PR
/// head. Auto-merge is revoked remotely first; a crash between the two steps is
/// replay-safe because the next attempt observes it already disabled.
pub(crate) fn clear_task_pr_merge_before_head_mutation(
    repo: &Path,
    mutation_is_unconditional: bool,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<bool> {
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        clear_task_pr_merge(&store, &task, repo, mutation_is_unconditional, inherit_pr).await
    })
}

/// Serialize the local operations that may change a Task PR head or its merge
/// request. The file descriptor owns the advisory lock until this guard drops.
#[derive(Debug)]
pub(crate) struct TaskPrMutationGuard {
    _file: File,
}

pub(crate) fn lock_task_pr_mutation(repo: &Path) -> OpsResult<TaskPrMutationGuard> {
    let path = crate::git::absolute_git_dir(repo)?.join("lf-pr-mutation.lock");
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    match FileExt::try_lock_exclusive(&file) {
        Ok(()) => Ok(TaskPrMutationGuard { _file: file }),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Err(OpsError::Message(
            "another PR or branch-head mutation is already running for this worktree".to_string(),
        )),
        Err(error) => Err(error.into()),
    }
}

async fn clear_task_pr_merge(
    store: &SharedStore,
    task: &Task,
    repo: &Path,
    mutation_is_unconditional: bool,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<bool> {
    let Some(mut pr) = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
    else {
        return Ok(false);
    };
    if pr.is_settled() {
        return Ok(false);
    }
    let Some(request) = pr
        .publication
        .as_ref()
        .and_then(|publication| publication.merge.as_ref())
        .cloned()
    else {
        return Ok(false);
    };
    if !mutation_is_unconditional {
        let head = rev_parse(repo, "HEAD")?;
        if is_clean(repo)? && head == request.head_sha {
            return Ok(false);
        }
    }
    if request.mode == PrMergeMode::Auto {
        let number = pr
            .github()
            .expect("merge request validation requires GitHub PR")
            .number;
        crate::ops::pr::disable_auto_merge(repo, number, inherit_pr)?;
    }
    pr.publication
        .as_mut()
        .expect("merge request requires publication")
        .merge = None;
    pr.updated_at = time::OffsetDateTime::now_utc();
    store
        .update_task_pr(&pr)
        .await
        .map_err(|error| task_error(format!("failed to clear stale PR merge request: {error}")))?;
    Ok(true)
}

/// Persist the explicit merge request before `submit` assigns or `land` arms
/// GitHub. Repeating the same mode/head request preserves its first timestamp.
pub(crate) fn request_task_pr_merge(
    repo: &Path,
    mode: PrMergeMode,
    head_sha: Option<&str>,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<bool> {
    let head_sha = head_sha.map(str::to_string);
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        let head_sha = head_sha
            .filter(|head| !head.trim().is_empty())
            .ok_or_else(|| {
                task_error(format!(
                    "GitHub did not report the current head for Task {}; refusing to request a merge without an exact commit",
                    task.identifier
                ))
            })?;
        let mut pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.identifier)))?;
        let publication = pr.publication.as_mut().ok_or_else(|| {
            task_error(format!(
                "Task {} has no durable PR publication request",
                task.identifier
            ))
        })?;
        let github_head = publication
            .github
            .as_ref()
            .and_then(|github| github.head_sha.as_deref());
        if github_head != Some(head_sha.as_str()) {
            return Err(task_error(format!(
                "Task {} stored GitHub head {:?}, not requested merge head {}; refusing an unpinned settlement",
                task.identifier, github_head, head_sha
            )));
        }
        if publication
            .presentation
            .as_ref()
            .is_none_or(|presentation| presentation.head_sha != head_sha)
        {
            return Err(task_error(format!(
                "Task {} has no non-empty reviewer-facing title and body for head {}; refresh PR copy before requesting settlement",
                task.identifier, head_sha
            )));
        }
        if publication
            .merge
            .as_ref()
            .is_some_and(|request| request.mode == PrMergeMode::Auto)
            && mode == PrMergeMode::User
        {
            let number = publication
                .github
                .as_ref()
                .expect("merge request validation requires GitHub PR")
                .number;
            crate::ops::pr::disable_auto_merge(repo, number, inherit_pr)?;
        }
        let now = time::OffsetDateTime::now_utc();
        let requested_at = publication
            .merge
            .as_ref()
            .filter(|request| request.mode == mode && request.head_sha == head_sha)
            .map_or(now, |request| request.requested_at);
        publication.merge = Some(PrMergeRequest {
            mode,
            requested_at,
            head_sha: head_sha.clone(),
        });
        pr.updated_at = now;
        store
            .update_task_pr(&pr)
            .await
            .map_err(|error| task_error(format!("failed to request PR merge: {error}")))?;
        Ok(true)
    })
}

/// Whether the repository has at least one configured git remote.
fn has_remote(repo: &Path) -> OpsResult<bool> {
    Ok(!git_output(repo, &["remote"])?.trim().is_empty())
}

pub(crate) fn fetch_task_refs(repo: &Path) -> OpsResult<()> {
    if crate::git::has_origin(repo)? {
        git_output(
            repo,
            &[
                "fetch",
                "--prune",
                "origin",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
        )
        .map_err(|error| task_error(format!("failed to fetch task base and branches: {error}")))?;
    }
    Ok(())
}

/// Resolve a new Task's base through the default branch's existing Git tracking.
pub(super) fn resolve_upstream_base(
    repo: &Path,
    default_branch: &str,
) -> OpsResult<(String, String)> {
    let base = if default_branch == get_default_branch(repo)? {
        crate::git::integration_base(repo)?
    } else if has_remote(repo)? {
        format!("origin/{default_branch}")
    } else {
        default_branch.to_string()
    };
    crate::git::fetch_tracking_ref(repo, &base)
        .map_err(|error| task_error(format!("failed to fetch task base: {error}")))?;
    let base_ref = if base == default_branch {
        format!("refs/heads/{default_branch}")
    } else {
        base
    };
    let base_commit = rev_parse(repo, &base_ref)
        .map_err(|error| task_error(format!("failed to resolve task base {base_ref}: {error}")))?;
    Ok((base_ref, base_commit))
}

/// Reuse the Task's recorded diff base, including a stacked parent.
pub(crate) fn task_diff_base(repo: &Path) -> OpsResult<Option<String>> {
    block_on_task(async move {
        match resolve_managed_task(repo).await? {
            ManagedTask::Managed { task, .. } => Ok(Some(task.base_commit.clone())),
            ManagedTask::Unmanaged => Ok(None),
        }
    })
}

/// Refresh the Task's comparison base from current Git ancestry before push.
/// Saved placement never classifies branch commits as foreign: publication
/// preserves HEAD and uses the same fork point as GitHub. Managed Task and
/// branch ownership still apply; emptiness is checked after committing.
pub(crate) fn refresh_task_pr_base(repo: &Path) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(&repo).await? else {
            return Ok(());
        };
        resolve_task_pr_base(&store, &task, &repo, BaseUpdate::Record, None)
            .await
            .map(|_| ())
    })
}

/// Prove the post-sync Task range against the operation's immutable target
/// without advancing durable metadata before a requested push is verified.
pub(crate) fn validate_task_pr_range_for_integration(
    repo: &Path,
    target_ref: &str,
    target_sha: &str,
) -> OpsResult<()> {
    verify_task_pr_range_for_integration(repo, target_ref, target_sha, BaseUpdate::Preserve)
}

/// Record the immutable base only after every requested Git postcondition,
/// including remote-head equality, has passed.
pub(crate) fn record_task_pr_range_after_integration(
    repo: &Path,
    target_ref: &str,
    target_sha: &str,
) -> OpsResult<()> {
    verify_task_pr_range_for_integration(repo, target_ref, target_sha, BaseUpdate::Record)
}

fn verify_task_pr_range_for_integration(
    repo: &Path,
    target_ref: &str,
    target_sha: &str,
    base_update: BaseUpdate,
) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    let target_ref = target_ref.to_string();
    let target_sha = target_sha.to_string();
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(&repo).await? else {
            return Ok(());
        };
        resolve_task_pr_base(
            &store,
            &task,
            &repo,
            base_update,
            Some((target_ref, target_sha)),
        )
        .await
        .map(|_| ())
    })
}

/// Refuse empty Task PRs before GitHub mutations, including existing PR updates.
/// Non-Task worktrees are unaffected; an unreadable managed registry is an error.
pub(crate) fn require_task_pr_range_nonempty(repo: &Path) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(&repo).await? else {
            return Ok(());
        };
        require_task_pr_range_nonempty_in(&store, &task, &repo).await
    })
}

/// Compare with the live stack parent, or the default branch after it settles.
/// Publication retains the stack fork separately for explicit integration.
async fn resolve_task_pr_upstream(
    store: &SharedStore,
    task: &Task,
    repo: &Path,
    default_branch: &str,
) -> OpsResult<(String, String)> {
    if let Some(parent_id) = task.parent_pr_id.as_ref() {
        let parent = store
            .get_task_pr(parent_id)
            .await
            .map_err(|error| task_error(format!("failed to read stack parent: {error}")))?
            .ok_or_else(|| task_error(format!("stack parent {parent_id} is missing")))?;
        let parent_live = parent.merge_commit.is_none() && parent.abandoned_at.is_none();
        if parent_live {
            return resolve_upstream_base(repo, &parent.branch);
        }
    }
    resolve_upstream_base(repo, default_branch)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaseUpdate {
    Preserve,
    Record,
}

async fn resolve_task_pr_base(
    store: &SharedStore,
    task: &Task,
    repo: &Path,
    base_update: BaseUpdate,
    upstream_override: Option<(String, String)>,
) -> OpsResult<String> {
    let branch =
        current_branch(repo)?.ok_or_else(|| task_error("Task worktree is not on a branch"))?;
    if task.branch != branch {
        return Err(task_error(format!(
            "Task {} expects branch {:?}, but the worktree is on {:?}",
            task.identifier, task.branch, branch
        )));
    }

    let default_branch = get_default_branch(repo)?;
    // A sync can integrate any ref, including this PR's own remote branch. Only
    // a pinned default-branch target is the PR's upstream; every other target
    // is measured against the real one, so integration never moves the base.
    let pinned = upstream_override.filter(|(target_ref, _)| {
        let name = target_ref
            .strip_prefix("origin/")
            .or_else(|| target_ref.strip_prefix("refs/heads/"))
            .unwrap_or(target_ref);
        name == default_branch
    });
    let (base_ref, upstream) = match pinned {
        Some(target) => target,
        None => resolve_task_pr_upstream(store, task, repo, &default_branch).await?,
    };
    let head = rev_parse(repo, "HEAD")
        .map_err(|error| task_error(format!("failed to resolve Task HEAD: {error}")))?;
    let identifier = &task.identifier;

    let merge_base = crate::git::merge_base(repo, &upstream, &head).map_err(|error| {
        task_error(format!(
            "Task {identifier} cannot resolve a PR comparison between {branch:?} and {base_ref}: {error}. \
             Keep this branch; inspect the upstream and its shared history before publishing"
        ))
    })?;

    // A stored fork may predate a reset, rebase, upstream rewrite or expired
    // reflog. A stack still needs its saved fork for the squash-parent comparison
    // merge; record_stack_sync owns that write after integration. Root Tasks
    // need only the current PR comparison.
    if base_update == BaseUpdate::Record
        && task.parent_pr_id.is_none()
        && merge_base != task.base_commit
    {
        store
            .sync_task_placement(
                &task.id,
                &merge_base,
                false,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .map_err(|error| task_error(format!("failed to record Task PR base: {error}")))?;
    }
    Ok(merge_base)
}

/// Judge emptiness against the current PR range, independently of a stack's
/// retained integration fork.
async fn require_task_pr_range_nonempty_in(
    store: &SharedStore,
    task: &Task,
    repo: &Path,
) -> OpsResult<()> {
    let base = resolve_task_pr_base(store, task, repo, BaseUpdate::Record, None).await?;
    let identifier = &task.identifier;
    let short = base.chars().take(12).collect::<String>();
    let base_tree = rev_parse(repo, &format!("{base}^{{tree}}"))?;
    let head_tree = rev_parse(repo, "HEAD^{tree}")?;
    if head_tree == base_tree {
        return Err(task_error(format!(
            "Task {identifier} PR range is empty: the tree at HEAD matches the comparison base \
             {short}, so the PR has no changes to publish. Commit the Task's work, or complete the \
             Task directly if the work is done. Refused before any GitHub side effect."
        )));
    }
    Ok(())
}

pub(crate) fn attach_task_github_pr(
    repo: &Path,
    github_pr: Option<&crate::ops::pr::PrInfo>,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<bool> {
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        let mut pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.identifier)))?;
        let github_pr = github_pr.ok_or_else(|| {
            task_error(format!(
                "GitHub PR for Task {} could not be read after creation or update",
                task.identifier
            ))
        })?;
        if github_pr.branch != pr.branch {
            return Err(task_error(format!(
                "Task {} active PR expects branch {:?}, but GitHub reported {:?}",
                task.identifier, pr.branch, github_pr.branch
            )));
        }
        let number = u32::try_from(github_pr.number).map_err(|_| {
            task_error(format!(
                "pull request #{} exceeds supported range",
                github_pr.number
            ))
        })?;
        let url = github_pr.url.clone();
        if pr
            .github()
            .is_some_and(|known| known.number != number || known.url != url)
        {
            return Err(task_error(
                "Task already owns a different pull request; new delivery requires a new Task",
            ));
        }
        let opened = pr
            .github()
            .is_none_or(|github| github.number != number || github.url != url);
        // Linked as Open with nothing new to say: another push or refresh
        // rewrites neither the attachment nor the comment.
        let linked_open = !opened
            && pr.phase() == PrPhase::Open
            && pr.linear_attachment_id.is_some()
            && pr.linear_comment_id.is_some()
            && pr.linear_link_error.is_none();
        // An existing GitHub PR may predate local publication. Retain its
        // identity now; reviewer copy is recorded only after promotion succeeds.
        let publication = pr.publication.get_or_insert_with(|| PrPublication {
            requested_at: time::OffsetDateTime::now_utc(),
            presentation: None,
            github: None,
            merge: None,
        });
        // A known identity can carry a head-pinned merge request. Revoke it
        // before replacing its head; the previously acknowledged identity
        // remains stored if that reconciliation fails. First attachment has
        // no merge request and can be saved immediately.
        invalidate_stale_merge_request(repo, publication, github_pr, inherit_pr)?;
        publication.github = Some(GithubPr {
            number,
            url: url.clone(),
            head_sha: github_pr.head_sha.clone(),
        });
        // Persist acknowledged identity before any further provider operation.
        pr.updated_at = time::OffsetDateTime::now_utc();
        store
            .update_task_pr(&pr)
            .await
            .map_err(|error| task_error(format!("failed to attach GitHub PR: {error}")))?;
        if opened {
            let event = TaskEventKind::PrOpened {
                pr_id: pr.id.clone(),
                sequence: pr.sequence,
                number,
                url,
            };
            store
                .append_task_event(&task.id, &event)
                .await
                .map_err(task_error)?;
        }
        // Linear linkage is idempotent. A failed or interrupted writeback cannot
        // erase the GitHub identity already committed above.
        if !(linked_open && pr.phase() == PrPhase::Open) {
            link_pr_to_planning(&store, &task, &mut pr).await;
        }
        pr.updated_at = time::OffsetDateTime::now_utc();
        store
            .update_task_pr(&pr)
            .await
            .map_err(|error| task_error(format!("failed to record PR linkage: {error}")))?;
        Ok(true)
    })
}

/// A merge request belongs to one exact head. Revoke an armed auto-merge before
/// forgetting a stale request so a later push cannot inherit settlement intent.
fn invalidate_stale_merge_request(
    repo: &Path,
    publication: &mut PrPublication,
    github_pr: &crate::ops::pr::PrInfo,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<()> {
    let Some(request) = publication.merge.as_ref() else {
        return Ok(());
    };
    let observed_head = github_pr.head_sha.as_deref().ok_or_else(|| {
        task_error(format!(
            "GitHub did not report the current head for pull request #{}; refusing to change its head-pinned merge request",
            github_pr.number
        ))
    })?;
    if observed_head == request.head_sha {
        return Ok(());
    }
    if request.mode == PrMergeMode::Auto && matches!(github_pr.state.as_str(), "open" | "draft") {
        let number = u32::try_from(github_pr.number).map_err(|_| {
            task_error(format!(
                "pull request #{} exceeds supported range",
                github_pr.number
            ))
        })?;
        crate::ops::pr::disable_auto_merge(repo, number, inherit_pr)?;
    }
    publication.merge = None;
    Ok(())
}

pub(crate) async fn reconcile_task_pr(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_observation(store, task, crate::ops::pr::PrReadFreshness::Cached).await
}

pub(crate) fn reconcile_checkout_pr(repo: &Path) -> OpsResult<Option<(Task, TaskPr, TaskState)>> {
    block_on_task(async {
        let ManagedTask::Managed { store, mut task } = resolve_managed_task(repo).await? else {
            return Ok(None);
        };
        let pr = reconcile_task_pr_observation(
            &store,
            &mut task,
            crate::ops::pr::PrReadFreshness::Fresh,
        )
        .await?;
        if let Observation::Degraded { reason, .. } = task.observation {
            return Err(task_error(reason));
        }
        let state = store.sqlite.task_state(&task.id).map_err(task_error)?;
        Ok(pr.map(|pr| (*task, pr, state)))
    })
}

/// Apply a watched landing's authoritative merged observation to its Task.
/// The landing owns Auto settlement; Task and Project runners do not poll or
/// infer it from process liveness.
pub(crate) async fn settle_task_landing(
    store: &SharedStore,
    landing: &crate::pr_landing::PrLanding,
) -> OpsResult<()> {
    let task_id = landing
        .task_id
        .as_ref()
        .ok_or_else(|| task_error("direct landing has no Task to settle"))?;
    let mut task = store
        .get_task(task_id)
        .await
        .map_err(|error| task_error(format!("failed to read landing Task: {error}")))?
        .ok_or_else(|| task_error(format!("landing Task {task_id} disappeared")))?;
    // Old landing rows remain history after migration; they cannot settle a
    // different current delivery or a PR-less Task.
    let Some(current) = store.active_task_pr(task_id).await.map_err(task_error)? else {
        return Ok(());
    };
    if current.github().map(|github| github.number) != Some(landing.pr_number) {
        return Ok(());
    }
    let pr = if current.phase() == PrPhase::Merged {
        current
    } else {
        reconcile_task_pr_observation(store, &mut task, crate::ops::pr::PrReadFreshness::Fresh)
            .await?
            .ok_or_else(|| task_error("landing Task PR disappeared during merge settlement"))?
    };
    apply_merged_task_landing(store, &mut task, &pr, landing).await?;
    Ok(())
}

pub(super) async fn apply_merged_task_landing(
    store: &SharedStore,
    task: &mut Task,
    pr: &TaskPr,
    landing: &crate::pr_landing::PrLanding,
) -> OpsResult<()> {
    if pr.phase() != PrPhase::Merged
        || pr.github().map(|github| github.number) != Some(landing.pr_number)
        || pr.head_sha() != Some(landing.observed_head_sha.as_str())
    {
        return Err(task_error(format!(
            "GitHub did not confirm landing pull request #{} merged for Task {}",
            landing.pr_number, task.identifier
        )));
    }
    reconcile_task_completion(store, task).await
}

/// Read the open PR's required checks and classify them for `head_sha`. Returns
/// `None` — no current-head CI owner can be derived — when GitHub reports no
/// head, there are no required checks, or gh is unavailable.
/// Failure dominates: any failing required check makes the head `Failing` even
/// while others are still pending.
fn observe_required_checks(
    worktree: &Path,
    pr_number: u64,
    head_sha: Option<&str>,
    now: time::OffsetDateTime,
) -> Option<CiObservation> {
    let head_sha = head_sha?.to_string();
    let checks = crate::ops::pr::merge_gate_state(worktree, pr_number, &head_sha)
        .ok()
        .flatten()?;
    let state = if checks.failing {
        CiState::Failing
    } else if checks.pending {
        CiState::Pending
    } else {
        CiState::Passing
    };
    Some(CiObservation {
        head_sha,
        state,
        // Keep actionable leaf failures, never the required aggregate, so the
        // landing supervisor can report the broken jobs precisely.
        failing_checks: checks
            .failing_leaves
            .into_iter()
            .map(|check| CiCheck {
                name: check.name,
                url: check.url,
            })
            .collect(),
        observed_at: now,
    })
}

// Local control commands often arrive in a burst (`status`, then `complete`,
// then another `status`). One minute keeps merge/CI state responsive while
// bounding those bursts to one GitHub read. A failed read opens a longer circuit:
// a quota or outage should not be hammered by every short-lived `lf` process.
const PR_OBSERVATION_TTL: time::Duration = time::Duration::seconds(60);
const PR_OBSERVATION_DEGRADED_BACKOFF: time::Duration = time::Duration::minutes(5);

fn cached_github_observation(pr: &TaskPr, now: time::OffsetDateTime) -> Option<Observation> {
    let observation = pr.github_observation.as_ref()?;
    let retry_at = observation.checked_at
        + match observation.result {
            GithubObservationResult::Fresh | GithubObservationResult::Partial { .. } => {
                PR_OBSERVATION_TTL
            }
            GithubObservationResult::Degraded { .. } => PR_OBSERVATION_DEGRADED_BACKOFF,
        };
    if retry_at <= now {
        return None;
    }
    Some(match &observation.result {
        GithubObservationResult::Fresh | GithubObservationResult::Partial { .. } => {
            Observation::Cached {
                observed_at: observation.checked_at,
            }
        }
        GithubObservationResult::Degraded { reason } => Observation::Degraded {
            reason: reason.clone(),
            cached_as_of: pr.updated_at,
            retry_at,
        },
    })
}

/// `abandoned_at` on a published PR caches GitHub's closed state rather than
/// deciding it — `lf pr abandon` runs `gh pr close` before stamping it — so a
/// reopen must be able to clear it. A merge is terminal: GitHub cannot unmerge.
pub(super) async fn reconcile_task_pr_observation(
    store: &SharedStore,
    task: &mut Task,
    freshness: crate::ops::pr::PrReadFreshness,
) -> OpsResult<Option<TaskPr>> {
    // Reconciliation updates the same projection as publication/finalization.
    // Refuse overlap so a remote read begun before a push cannot overwrite the
    // request or head recorded by the command that completed after it. A
    // removed checkout (a landed stack parent) hosts no mutation to overlap.
    let checkout = task.worktree.clone().filter(|path| path.exists());
    let _mutation = checkout.as_deref().map(lock_task_pr_mutation).transpose()?;
    let Some(mut pr) = store.active_task_pr(&task.id).await.map_err(task_error)? else {
        return Ok(None);
    };
    if pr.phase() == PrPhase::Merged {
        task.observation = Observation::NotRequired;
        return Ok(Some(pr));
    }
    // Ordinary status does not enumerate unpublished branches. Explicit fresh
    // reconciliation can recover a publication whose GitHub identity was lost.
    let number = pr.github().map(|github| github.number);
    if number.is_none() && matches!(freshness, crate::ops::pr::PrReadFreshness::Cached) {
        task.observation = Observation::NotRequired;
        return Ok(Some(pr));
    }
    let now = time::OffsetDateTime::now_utc();
    // A fresh caller must never receive the store's warm head observation.
    if matches!(freshness, crate::ops::pr::PrReadFreshness::Cached) {
        if let Some(observation) = cached_github_observation(&pr, now) {
            task.observation = observation;
            return Ok(Some(pr));
        }
    }
    let previous = pr.clone();
    let repo = match checkout {
        Some(path) => path,
        None => repository_for_task(store, task)?,
    };
    let observation = match number {
        Some(number) => crate::ops::pr::observe_pr_by_number(&repo, number, &pr.branch, freshness),
        None => crate::ops::pr::observe_pr_by_branch(&repo, &pr.branch),
    };
    let github_pr = match observation {
        crate::ops::pr::PrObservation::Fresh(info) => {
            pr.github_observation = Some(GithubObservation {
                checked_at: now,
                result: GithubObservationResult::Fresh,
            });
            task.observation = Observation::Fresh { observed_at: now };
            info
        }
        crate::ops::pr::PrObservation::NotFound => {
            // The PR ref was deleted remotely; a merge (if any) is already
            // persisted. Cache the successful absence briefly and keep the
            // settled/working state.
            pr.github_observation = Some(GithubObservation {
                checked_at: now,
                result: GithubObservationResult::Fresh,
            });
            pr.updated_at = now;
            store.update_task_pr(&pr).await.map_err(task_error)?;
            task.observation = Observation::Fresh { observed_at: now };
            return Ok(Some(pr));
        }
        crate::ops::pr::PrObservation::Degraded { reason } => {
            let retry_at = now + PR_OBSERVATION_DEGRADED_BACKOFF;
            pr.github_observation = Some(GithubObservation {
                checked_at: now,
                result: GithubObservationResult::Degraded {
                    reason: reason.clone(),
                },
            });
            // `updated_at` remains the time of the cached PR data, not the
            // failed attempt. Only the observation metadata changes.
            store.update_task_pr(&pr).await.map_err(task_error)?;
            task.observation = Observation::Degraded {
                reason,
                cached_as_of: pr.updated_at,
                retry_at,
            };
            return Ok(Some(pr));
        }
    };
    let number = u32::try_from(github_pr.number).map_err(|_| {
        task_error(format!(
            "pull request #{} exceeds supported range",
            github_pr.number
        ))
    })?;
    let url = github_pr.url.clone();
    let previous_phase = previous.phase();
    let previous_github = previous.github().cloned();
    let publication = pr.publication.get_or_insert(PrPublication {
        requested_at: now,
        presentation: None,
        github: None,
        merge: None,
    });
    invalidate_stale_merge_request(&repo, publication, &github_pr, &|_| {})?;
    publication.github = Some(GithubPr {
        number,
        url: url.clone(),
        head_sha: github_pr.head_sha.clone(),
    });

    let mut authoritative_merged_at = None;
    let pr_event = match github_pr.state.as_str() {
        "merged" => {
            let merge_commit = github_pr.merge_commit.clone().ok_or_else(|| {
                task_error(format!(
                    "GitHub reports pull request #{} merged without a merge commit",
                    github_pr.number
                ))
            })?;
            pr.merge_commit = Some(merge_commit.clone());
            pr.ci_observation = None;
            match github_pr.merged_at.as_deref() {
                Some(value) => match time::OffsetDateTime::parse(value, &Rfc3339) {
                    Ok(value) => authoritative_merged_at = Some(value),
                    Err(error) => {
                        let reason = format!(
                            "GitHub returned malformed merged_at for pull request #{}: {error}",
                            github_pr.number
                        );
                        pr.github_observation = Some(GithubObservation {
                            checked_at: now,
                            result: GithubObservationResult::Partial { reason },
                        });
                    }
                },
                None => {
                    let reason = format!(
                        "GitHub returned no merged_at for merged pull request #{}",
                        github_pr.number
                    );
                    pr.github_observation = Some(GithubObservation {
                        checked_at: now,
                        result: GithubObservationResult::Partial { reason },
                    });
                }
            }
            Some(TaskEventKind::PrMerged {
                pr_id: pr.id.clone(),
                sequence: pr.sequence,
                number,
                url: url.clone(),
                merge_commit,
            })
        }
        "closed" => {
            // A PR the flow already abandoned settles again when GitHub's
            // `closed` is observed. Re-stamping the time makes the second
            // settle differ from the first and wedges the task on
            // "already settled differently" — the first abandonment is the
            // fact; observation only confirms it.
            pr.abandoned_at = pr.abandoned_at.or(Some(now));
            pr.ci_observation = None;
            None
        }
        _ => {
            // GitHub has it open, so any `abandoned_at` here is a stale claim that
            // it was closed. Clearing it returns the same row to `Open`.
            pr.abandoned_at = None;
            if let Some(ci_observation) =
                observe_required_checks(&repo, github_pr.number, github_pr.head_sha.as_deref(), now)
            {
                pr.ci_observation = Some(ci_observation);
            }
            Some(TaskEventKind::PrOpened {
                pr_id: pr.id.clone(),
                sequence: pr.sequence,
                number,
                url: url.clone(),
            })
        }
    };

    let pr_changed = pr != previous;
    if pr_changed {
        pr.updated_at = now;
        if pr.phase() == PrPhase::Merged {
            let outcome = store
                .settle_task_pr_merged(&pr, authoritative_merged_at)
                .await
                .map_err(task_error)?;
            if let crate::store::TaskPrMergeEvidenceOutcome::Conflict { accepted_at } = outcome {
                let reason =
                    format!("GitHub merged_at conflicts with first accepted value {accepted_at}");
                pr.github_observation = Some(GithubObservation {
                    checked_at: now,
                    result: GithubObservationResult::Partial { reason },
                });
            }
        } else if pr.is_settled() {
            store.settle_task_pr(&pr).await.map_err(task_error)?;
        } else {
            store.update_task_pr(&pr).await.map_err(task_error)?;
        }
    }
    if pr_changed {
        if let Some(event) = pr_event {
            let should_append = match &event {
                TaskEventKind::PrOpened { .. } => previous_github.as_ref() != pr.github(),
                TaskEventKind::PrMerged { .. } => previous_phase != PrPhase::Merged,
                _ => true,
            };
            if should_append {
                store
                    .append_task_event(&task.id, &event)
                    .await
                    .map_err(task_error)?;
            }
        }
    }
    Ok(Some(pr))
}
