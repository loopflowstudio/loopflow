//! Adopt existing work without touching its files, input, or provider attachment.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::durable::TaskId;
use crate::git::worktrees::{PlacementPlan, PlacementStrategy};
use crate::ops::human_session::{checkout_scope, SessionTarget};
use crate::ops::OpsResult;
use crate::session::{LfSession, PrimaryScope};
use crate::store::{PlanningLocks, SharedStore};
use crate::work::task::{NewTask, Task};

use super::{task_error, TaskPlacement};

#[derive(Debug, Clone, Default)]
pub struct TaskAdoptionOptions {
    pub session: Option<String>,
    pub worktree: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub(super) struct Source {
    pub task: Option<Task>,
    checkout: Option<PathBuf>,
    explicit_checkout: bool,
    session: Option<LfSession>,
    // Serialize source resolution and the local commit, including concurrent retries.
    _admission: Vec<Arc<std::fs::File>>,
}

pub(super) async fn resolve_source(
    store: &SharedStore,
    repo: &Path,
    options: &TaskAdoptionOptions,
) -> OpsResult<Source> {
    let selected = match &options.session {
        Some(id) => Some(
            crate::ops::human_session::session_by_id(store, id)
                .await
                .map_err(task_error)?
                .ok_or_else(|| task_error(format!("Session {id} was not found on this Machine")))?,
        ),
        None => None,
    };
    resolve_checkout(store, repo, selected, options.worktree.as_deref()).await
}

async fn resolve_checkout(
    store: &SharedStore,
    repo: &Path,
    mut selected: Option<LfSession>,
    worktree: Option<&Path>,
) -> OpsResult<Source> {
    let Some(cwd) = worktree.or_else(|| selected.as_ref().map(|session| session.cwd.as_path()))
    else {
        // Plain creation never adopts the caller's checkout or conversation.
        return Ok(Source::default());
    };
    let main = crate::repository::CanonicalRepo::discover(repo).map_err(task_error)?;
    let root = crate::git::worktree_root(cwd).map_err(task_error)?;
    if crate::repository::CanonicalRepo::discover(&root).map_err(task_error)? != main {
        return Err(task_error(
            "The selected work belongs to another repository; run promotion there",
        ));
    }
    if let Some(session) = &selected {
        let session_root = crate::git::worktree_root(&session.cwd).map_err(task_error)?;
        if session_root != root {
            return Err(task_error("The Session and worktree are different checkouts; select the Session's existing checkout"));
        }
        if !session.interactive || session.completed_at.is_some() {
            return Err(task_error(
                "Select an unfinished interactive Session to become the Task conversation",
            ));
        }
    }
    let admission = store
        .lock_checkout_roots(vec![root.clone()])
        .await
        .map_err(task_error)?;
    if let Some(session) = &selected {
        // A bind can have completed while checkout admission was being acquired.
        let current = store
            .session(&session.id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("Session disappeared before promotion"))?;
        if current.cwd != session.cwd {
            return Err(task_error(
                "Session workspace changed before promotion; retry",
            ));
        }
        selected = Some(current);
    }
    let scope = checkout_scope(store, &root, SessionTarget::Context)
        .await
        .map_err(task_error)?;
    let (checkout, checkout_task) = match scope {
        Some(PrimaryScope::Task(id)) => (Some(root), Some(id)),
        None => (Some(root), None),
        Some(PrimaryScope::Repository(_) | PrimaryScope::Wave(_)) => {
            if worktree.is_some() {
                return Err(task_error("This checkout is shared by its repository or Wave; select a Session without --worktree, or use an unassociated linked checkout"));
            }
            (None, None)
        }
    };
    let session_task = selected
        .as_ref()
        .and_then(|session| session.task_id.as_ref());
    if let (Some(checkout_task), Some(session_task)) = (&checkout_task, session_task) {
        if checkout_task != session_task {
            return Err(task_error("The Session and checkout belong to different Tasks; permanent bindings are unchanged"));
        }
    }
    let task = match session_task.or(checkout_task.as_ref()) {
        Some(id) => Some(
            store
                .get_task(id)
                .await
                .map_err(task_error)?
                .ok_or_else(|| task_error("The selected work's Task is unavailable"))?,
        ),
        None => None,
    };
    Ok(Source {
        task,
        checkout,
        explicit_checkout: worktree.is_some(),
        session: selected,
        _admission: admission,
    })
}

pub(super) async fn save(
    store: &SharedStore,
    main: &Path,
    id: TaskId,
    creation: Option<NewTask>,
    source: Source,
    acquisition: Option<Arc<PlanningLocks>>,
    continue_in_task: bool,
    receipt: Option<crate::store::sqlite::creation::CreationIdentity>,
) -> OpsResult<Task> {
    if let Some(task) = source.task.as_ref().filter(|task| task.id != id) {
        return Err(task_error(format!(
            "The selected work already belongs to {}; ownership is unchanged",
            task.identifier
        )));
    }
    let existing = store.get_task(&id).await.map_err(task_error)?;
    if let Some(task) = &existing {
        if super::repository_for_task(store, task)? != main {
            return Err(task_error(
                "The Task belongs to another repository; its ownership is unchanged",
            ));
        }
        if source.explicit_checkout
            && task
                .worktree
                .as_ref()
                .is_some_and(|path| Some(path) != source.checkout.as_ref())
        {
            return Err(task_error(format!(
                "{} already has a different checkout; its work is unchanged",
                task.identifier
            )));
        }
    }
    let destination = if continue_in_task {
        existing
            .as_ref()
            .and_then(|task| task.worktree.clone())
            .filter(|root| source.session.is_some() && source.checkout.as_ref() != Some(root))
    } else {
        None
    };
    let placement = match (
        &source.checkout,
        existing.as_ref().and_then(|task| task.worktree.as_ref()),
    ) {
        (Some(root), None)
            if existing.is_none()
                || !store
                    .task_state(&id)
                    .await
                    .map_err(task_error)?
                    .is_terminal() =>
        {
            let prepared = prepare(main, root)?;
            Some((root.clone(), prepared.pr(&id)))
        }
        _ => None,
    };
    // No checkout restoration here: the selected files and branch already exist.
    let task = store
        .promote_task(
            &id,
            creation,
            placement,
            source.session,
            destination.clone(),
            acquisition,
            receipt,
        )
        .await
        .map_err(task_error)?;
    if let Some(cwd) = destination {
        crate::journal::request::diagnostic(format_args!("Conversation directory set to {}. Already-running clients may keep their original directory. Files stay in their original checkout.", cwd.display()));
    }
    drop(source._admission);
    Ok(task)
}

fn prepare(main: &Path, root: &Path) -> OpsResult<TaskPlacement> {
    let branch = crate::git::current_branch(root)?.ok_or_else(|| {
        task_error("Promotion needs a named branch; create a branch at the current commit first")
    })?;
    let plan = PlacementPlan {
        base_ref: crate::git::get_default_branch(main)?,
        branch,
        worktree_path: root.to_path_buf(),
        strategy: PlacementStrategy::UseExistingWorktree,
    };
    let slug = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| task_error("Worktree has no UTF-8 name"))?
        .to_owned();
    super::prepare_placement(
        main,
        plan,
        slug,
        None,
        crate::git::worktrees::github_repo_nwo(main).is_some(),
    )
}

/// Bind and select one existing conversation through the adoption transaction.
pub(crate) async fn bind_session(
    store: &SharedStore,
    session: &LfSession,
    task: &Task,
) -> OpsResult<()> {
    let repo = super::repository_for_task(store, task)?;
    let source = resolve_checkout(store, &repo, Some(session.clone()), None).await?;
    let acquisition = match super::owning_wave(store, task).await? {
        Some(wave) => Some(crate::ops::planning_sync::lock_wave_planning(&wave).await?),
        None => None,
    };
    save(
        store,
        &repo,
        task.id.clone(),
        None,
        source,
        acquisition,
        true,
        None,
    )
    .await?;
    Ok(())
}
