//! Turn a tracker read into durable Task direction.
//!
//! Someone editing a tracker item's title or description or adding a comment
//! appends an ordered Steer. This module maps one event onto that input
//! spine, and [`Store::apply_tracker_event`] persists it atomically.
//! Exactly-once, the baseline, and the monotonic-revision guard all live in the
//! store; refresh supplies events without proposing a second write.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use super::tracker::{drain::Scope, message};
use super::OpsResult;
use crate::store::{SharedStore, Store};
use loopflow_storage::sqlite::tracker_actions::ActionOwner;
use loopflow_storage::work::task::Task;

pub(crate) async fn refresh_task_comments(store: &SharedStore, task: &Task) -> OpsResult<()> {
    // A command can outlive export or a mapping change. Observe the current link;
    // pending outbound writes do not own or pause this input stream.
    let task = store
        .get_task(&task.id)
        .await
        .map_err(message)?
        .ok_or_else(|| message(format!("Task {} is missing", task.id)))?;
    let Some(issue) = &task.tracker_id else {
        return Ok(());
    };
    let repository = super::task::repository_for_task(store, &task)?;
    let planning_connected = super::tracker::connected(&repository.to_string_lossy());
    if !planning_connected {
        return Ok(());
    }
    // The Team delta carries this issue's new comments; a current one needs no
    // per-issue read.
    if team_current(store, &repository.to_string_lossy()).await {
        return Ok(());
    }
    let client = super::tracker::Tracker::for_items(&repository).await?;
    // A coworker deleted the item: remove the Task from the plan, keeping its work.
    if client.removed(issue.as_str()).await.map_err(message)? {
        store
            .sqlite
            .record_tracker_event(issue.as_str(), None, true)
            .map_err(message)?;
        return Ok(());
    }
    let mut event = client.fetch_issue(issue.as_str()).await.map_err(message)?;
    claim_local_comments(store, &task, &client, &mut event)?;
    // Linear's team feed accepts tracker edits into Work. Asana has
    // none, so an exact Asana read applies the change: a renamed or completed task
    // updates the Task, and an unsent local edit stays as a visible conflict.
    if !matches!(client, super::tracker::Tracker::Linear(_)) {
        if let Some((item, project)) = client
            .issue_ownership(issue.as_str())
            .await
            .map_err(message)?
        {
            super::tracker::ingest_task(
                store,
                &repository,
                &item,
                project.as_ref(),
                OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await?;
        }
    }
    store
        .apply_tracker_event(
            &task.id,
            event,
            OffsetDateTime::now_utc(),
            planning_connected,
        )
        .await
        .map_err(message)?;
    Ok(())
}

/// Asana comments carry no caller identity. A comment showing a
/// Loopflow comment's visible text is that comment: delivery settles and nothing
/// echoes back.
fn claim_local_comments(
    store: &Store,
    task: &Task,
    client: &super::tracker::Tracker,
    event: &mut loopflow_storage::planning::TrackerEvent,
) -> OpsResult<()> {
    if matches!(client, super::tracker::Tracker::Linear(_)) {
        return Ok(());
    }
    let local = store.task_comments(&task.id).map_err(message)?.comments;
    for comment in &mut event.comments {
        if let Some(saved) = local.iter().find(|saved| {
            uuid::Uuid::parse_str(&saved.id).is_ok()
                && client.claimed_comment(&saved.body).as_deref() == Some(comment.body.as_str())
        }) {
            comment.id = saved.id.clone();
            comment.body = saved.body.clone();
        }
    }
    Ok(())
}

/// Bring the repository's planning scope (a Linear Team or Asana project) up
/// to date: items people created or edited there become or
/// update Tasks. Freshness belongs to the scope, even when several repositories
/// or processes share it.
#[cfg(test)]
pub(crate) async fn check_repository(store: &Store, repository: &str) -> OpsResult<bool> {
    check_scope(store, repository, fresh_for(repository)).await
}

/// `check_repository` unless a check started within `fresh_for` seconds.
async fn check_scope(store: &Store, repository: &str, fresh_for: i64) -> OpsResult<bool> {
    let planning_connected = super::tracker::connected(repository);
    if !planning_connected {
        return Ok(false);
    }
    let repo = Path::new(repository);
    let team = super::tracker::repository_scope(repo)?;
    let mut scope_matches = HashMap::from([(repo.to_path_buf(), true)]);
    let mut same_scope = |repo: &Path| {
        *scope_matches.entry(repo.to_path_buf()).or_insert_with(|| {
            super::tracker::repository_scope(repo).is_ok_and(|scope| scope == team)
        })
    };
    let mut live = Vec::new();
    let mut finished = Vec::new();
    let waves: Vec<_> = store
        .list_waves(None)
        .await
        .map_err(message)?
        .into_iter()
        .filter(|wave| same_scope(Path::new(wave.repo())))
        .collect();
    for task in store.list_tasks(None).await.map_err(message)? {
        let task_repo = super::task::repository_for_task(store, &task)?;
        if !same_scope(&task_repo) {
            continue;
        }
        let Some(issue) = task.tracker_id else {
            continue;
        };
        if store
            .task_state(&task.id)
            .await
            .map_err(message)?
            .is_terminal()
        {
            finished.push(issue.as_str().to_owned());
        } else {
            live.push(issue.as_str().to_owned());
        }
    }
    let key = format!("{:x}", Sha256::digest(team.as_bytes()));
    let path = store
        .sqlite
        .home_dir()
        .map_err(message)?
        .join("locks/linear-teams")
        .join(key);
    let Some(_lock) = super::tracker::drain::lock_delivery(&path)? else {
        return Ok(false);
    };
    let now = OffsetDateTime::now_utc();
    let Some(since) = store
        .sqlite
        .begin_linear_team_check(&team, now.unix_timestamp(), fresh_for)
        .map_err(message)?
    else {
        return Ok(false);
    };
    let through = now
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(message)?;
    let client = super::tracker::Tracker::for_items(repo).await?;
    let changes = client
        .changes(&team, since.as_deref(), &through, &live, &finished)
        .await
        .map_err(message)?;
    let changed = !changes.is_empty();
    for (issue, project, event) in changes {
        // Provider filters limit traffic; the current local decision still wins
        // when completion races the request.
        let existing = store.get_task_by_issue(&issue.id).await.map_err(message)?;
        if let Some(task) = &existing {
            if store
                .task_state(&task.id)
                .await
                .map_err(message)?
                .is_terminal()
            {
                continue;
            }
        }
        let owner = if let Some(task) = &existing {
            Some(
                super::task::repository_for_task(store, task)?
                    .display()
                    .to_string(),
            )
        } else {
            // An issue in the Project a Wave links is that Wave's; any other
            // issue is this repository's, and ingestion imports it only when
            // it is in no Project at all.
            let linked = match &project {
                Some(project) => store
                    .project_by_tracker_id(&project.id)
                    .await
                    .map_err(message)?,
                None => None,
            };
            match linked {
                Some(linked) => waves
                    .iter()
                    .find(|wave| wave.id() == &linked.wave_id)
                    .map(|wave| wave.repo().to_owned()),
                None => Some(repository.to_owned()),
            }
        };
        let Some(owner) = owner else { continue };
        super::tracker::ingest_task(
            store,
            std::path::Path::new(&owner),
            &issue,
            project.as_ref(),
            now.unix_timestamp(),
        )
        .await?;
        if let Some(task) = store.get_task_by_issue(&issue.id).await.map_err(message)? {
            let mut event = event;
            claim_local_comments(store, &task, &client, &mut event)?;
            store
                .apply_tracker_event(&task.id, event, now, planning_connected)
                .await
                .map_err(message)?;
        }
    }
    store
        .sqlite
        .finish_linear_team_check(&team, &through)
        .map_err(message)?;
    Ok(changed)
}

/// Seconds a finished scope delta answers reads. Linear's stretches as its
/// shared budget falls; Asana keeps the healthy window.
fn fresh_for(repository: &str) -> i64 {
    if crate::ops::tracker::queued_tracker(std::path::Path::new(repository))
        == Some(loopflow_storage::planning_sync::TrackerKind::Linear)
    {
        crate::linear::delta_fresh_secs()
    } else {
        crate::linear::HEALTHY_FRESH_SECS
    }
}

/// The one freshness check for Linear reads: bring the repository's Team
/// delta up to date (a no-op inside its window), then report whether local
/// state is current enough to answer in place of a per-item read.
pub(crate) async fn team_current(store: &Store, repository: &str) -> bool {
    if crate::ops::tracker::queued_tracker(std::path::Path::new(repository))
        != Some(loopflow_storage::planning_sync::TrackerKind::Linear)
    {
        return false;
    }
    check_repository_bounded(store, repository, fresh_for(repository)).await;
    let Ok(team) = super::tracker::repository_scope(std::path::Path::new(repository)) else {
        return false;
    };
    store
        .sqlite
        .linear_team_current(
            &team,
            OffsetDateTime::now_utc().unix_timestamp(),
            crate::linear::delta_fresh_secs(),
        )
        .unwrap_or(false)
}

/// Bring the repository's scope delta up to date within five seconds; whether
/// it changed local state. Unavailable or slow trackers leave state as it is.
async fn check_repository_bounded(store: &Store, repository: &str, fresh_for: i64) -> bool {
    let check = tokio::time::timeout(
        Duration::from_secs(5),
        check_scope(store, repository, fresh_for),
    )
    .await;
    match check {
        Ok(Ok(changed)) => changed,
        result => {
            tracing::debug!(?result, "Linear team check pending");
            false
        }
    }
}

/// Bound on-demand work and retain local planning when Linear is unavailable.
pub async fn sync_repository(store: &Store, repository: &str) -> bool {
    sync_scope(store, repository, fresh_for(repository), Scope::All).await
}

/// An explicit planning refresh: the scope delta, however recently a check
/// started, then pending deliveries. It runs before the Wave inventory, which
/// would otherwise learn a remote completion first and leave the delta skipping
/// that finished Task's other changes, such as a comment made alongside it.
pub(crate) async fn refresh_repository(
    store: &Store,
    repository: &str,
    first: &ActionOwner,
) -> bool {
    sync_scope(store, repository, 0, Scope::First(first)).await
}

async fn sync_scope(store: &Store, repository: &str, fresh_for: i64, scope: Scope<'_>) -> bool {
    let changed = check_repository_bounded(store, repository, fresh_for).await;
    if let Err(error) = super::tracker::drain::drain_repository(
        store,
        repository,
        scope,
        super::tracker::drain::Bound::PASS,
    )
    .await
    {
        tracing::debug!(%error, "tracker actions pending");
    }
    changed
}
