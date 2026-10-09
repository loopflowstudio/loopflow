//! Turn a Linear issue read into durable Task direction.
//!
//! Someone editing a Linear issue's title or description or adding a comment
//! appends an ordered Steer. This module maps one observation onto that input
//! spine, and [`Store::apply_linear_observation`] persists it atomically.
//! Exactly-once, the baseline, and the monotonic-revision guard all live in the
//! store; acquisition supplies observations without proposing a second write.

use std::future::Future;
use std::time::Duration;

use time::OffsetDateTime;

use super::{OpsError, OpsResult};
use crate::store::{SharedStore, Store};
use crate::work::task::Task;

pub(crate) fn connected(repo: &str) -> bool {
    crate::engine::config::load_config_or_default(Some(std::path::Path::new(repo)))
        .pm
        .and_then(|pm| pm.linear_team)
        .is_some()
}

/// Explicit steering carries its marker, whichever account published it.
pub(crate) fn is_steer(body: &str) -> bool {
    body.contains("<!-- loopflow-steer:")
}

pub(crate) fn is_direction_comment(body: &str, author: Option<&str>) -> bool {
    if body.contains("<!-- loopflow-progress:") {
        return false;
    }
    if is_steer(body) {
        return true;
    }
    author.is_some()
        && !body.contains("<!-- loopflow-")
        && !body.starts_with("PR: ")
        && !body.starts_with("Shipped: ")
        && !body.starts_with("Reteamed by loopflow:")
        && !body.starts_with("[GitHub PR #")
}

pub(crate) async fn refresh_task_comments(store: &SharedStore, task: &Task) -> OpsResult<()> {
    // A command can outlive export or a mapping change. Observe the current link;
    // pending outbound writes do not own or pause this input stream.
    let task = store
        .get_task(&task.id)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| OpsError::Message(format!("Task {} is missing", task.id)))?;
    let Some(issue) = &task.plan.linear_id else {
        return Ok(());
    };
    let wave = store
        .get_wave(&task.wave_id)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| OpsError::Message("Task Wave is missing".into()))?;
    if !connected(wave.repo()) {
        return Ok(());
    }
    let client = super::pm::issue_client(std::path::Path::new(wave.repo())).await?;
    let observation = client
        .observe_issue(issue.as_str())
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    store
        .apply_linear_observation(&task.id, observation, OffsetDateTime::now_utc())
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    Ok(())
}

/// A foreground connection's independent planning acquisition and delivery.
/// Dropping the connection cancels requests; durable identities survive cancellation.
#[derive(Debug)]
pub(crate) struct PlanningSync {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PlanningSync {
    /// Repository planning follows every foreground provider, not Task attribution.
    pub(crate) fn start_for_directory(directory: &std::path::Path) -> OpsResult<Self> {
        super::task::block_on_task(async {
            let store = super::pm::pm_store().await?;
            let repo = crate::repository::CanonicalRepo::discover(directory)
                .map_err(|error| OpsError::Message(error.to_string()))?
                .to_string();
            Self::start(std::sync::Arc::new(store), repo)
                .map_err(|error| OpsError::Message(error.to_string()))
        })
    }

    pub(crate) fn start(store: SharedStore, repo: String) -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let (stop, stopped) = tokio::sync::oneshot::channel();
        #[cfg(test)]
        let context = super::pm::PM_TEST_CONTEXT.try_with(Clone::clone).ok();
        let thread = std::thread::Builder::new()
            .name("planning-sync".into())
            .spawn(move || {
                let drive = async {
                    tokio::select! {
                        _ = stopped => {},
                        _ = repeat_sync("Git planning acquisition", Duration::from_secs(5), || {
                            super::planning_peer::acquire_repository(&store, &repo)
                        }) => {},
                        _ = repeat_sync("Git planning publication", Duration::from_secs(1), || {
                            super::planning_peer::publish_repository(&store, &repo)
                        }) => {},
                        _ = repeat_sync("comment acquisition", Duration::from_secs(15), || {
                            refresh_repository_comments(&store, &repo)
                        }) => {},
                        _ = repeat_sync("planning acquisition", Duration::from_secs(15), || async {
                            tokio::time::timeout(Duration::from_secs(5), refresh_repository_planning(&store, &repo))
                                .await.map_err(|error| OpsError::Message(error.to_string()))?
                        }) => {},
                        _ = repeat_sync("comment delivery", Duration::from_secs(1), || {
                            sync_repository_deliveries(&store, &repo, false)
                        }) => {},
                        _ = repeat_sync("planning export", Duration::from_secs(1), || {
                            super::planning_export::sync_repository_exports(&store, &repo)
                        }) => {},
                        _ = repeat_sync("field delivery", Duration::from_secs(1), || {
                            super::planning_delivery::sync_repository_fields(&store, &repo)
                        }) => {},
                        _ = repeat_sync("Task state delivery", Duration::from_secs(1), || {
                            sync_repository_deliveries(&store, &repo, true)
                        }) => {},
                    }
                };
                #[cfg(test)]
                if let Some(context) = context {
                    runtime.block_on(super::pm::PM_TEST_CONTEXT.scope(context, drive));
                    return;
                }
                runtime.block_on(drive);
            })?;
        Ok(Self {
            stop: Some(stop),
            thread: Some(thread),
        })
    }
}

// Each branch has its own delay after an attempt. A slow delivery cannot
// postpone acquisition or cause missed ticks to burst when the request returns.
async fn repeat_sync<F: Future<Output = OpsResult<()>>>(
    operation: &str,
    delay: Duration,
    mut attempt: impl FnMut() -> F,
) {
    loop {
        if let Err(error) = attempt().await {
            tracing::debug!(%error, operation, "planning synchronization pending");
        }
        tokio::time::sleep(delay).await;
    }
}

impl Drop for PlanningSync {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

async fn refresh_repository_comments(store: &SharedStore, repo: &str) -> OpsResult<()> {
    if !connected(repo) {
        return Ok(());
    }
    let waves = store
        .list_waves(Some(repo))
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    futures_util::future::join_all(waves.iter().map(|wave| async {
        let tasks = store
            .list_tasks(Some(wave.id()))
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?;
        futures_util::future::join_all(
            tasks
                .iter()
                .filter(|task| task.plan.linear_id.is_some())
                .map(|task| async {
                    match tokio::time::timeout(
                        Duration::from_secs(5),
                        refresh_task_comments(store, task),
                    )
                    .await
                    {
                        Ok(Ok(())) => {}
                        result => {
                            tracing::debug!(?result, task = %task.id, "comment acquisition pending")
                        }
                    }
                }),
        )
        .await;
        Ok::<(), OpsError>(())
    }))
    .await
    .into_iter()
    .collect::<OpsResult<Vec<_>>>()?;
    Ok(())
}

async fn refresh_repository_planning(store: &Store, repository: &str) -> OpsResult<()> {
    // An absent mapping is not a reason to disable acquisition in a connected
    // repository. Connection configuration, rather than outbound work, selects it.
    let repo = std::path::Path::new(repository);
    if !connected(repository) {
        return Ok(());
    }
    let owners = store
        .list_waves(Some(repository))
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let results = futures_util::future::join_all(owners.iter().map(|owner| async {
        let ctx = super::pm::resolve_context(repo, owner.slug()).await?;
        let guard = super::pm::lock_wave_planning(owner).await?;
        super::pm::refresh_pm_snapshot_locked(repo, owner, &ctx, store, guard).await
    }))
    .await;
    for result in results {
        if let Err(error) = result {
            tracing::debug!(%error, "Wave acquisition pending");
        }
    }
    Ok(())
}

async fn sync_repository_deliveries(store: &Store, repository: &str, state: bool) -> OpsResult<()> {
    if !connected(repository) {
        return Ok(());
    }
    let pending = store
        .sqlite
        .pending_planning_tasks(repository)
        .map_err(|e| OpsError::Message(e.to_string()))?;
    let results = futures_util::future::join_all(pending.iter().map(|task| async {
        if state {
            sync_task_state(store, task).await
        } else {
            sync_task_comments(store, task).await
        }
    }))
    .await;
    for result in results {
        if let Err(error) = result {
            tracing::debug!(%error, "planning delivery pending");
        }
    }
    Ok(())
}

pub(crate) async fn sync_task_state(store: &Store, task: &Task) -> OpsResult<()> {
    let message = |error: &dyn std::fmt::Display| OpsError::Message(error.to_string());
    let path = store
        .sqlite
        .home_dir()
        .map_err(|e| message(&e))?
        .join("locks/task-state")
        .join(format!("{}.lock", task.id));
    let Some(_lock) = super::planning_delivery::lock_delivery(&path)? else {
        return Ok(());
    };
    let Some(delivery) = store
        .sqlite
        .pending_task_state(&task.id)
        .map_err(|e| message(&e))?
    else {
        return Ok(());
    };
    let attempt = async {
        let task = store
            .get_task(&task.id)
            .await
            .map_err(|e| message(&e))?
            .ok_or_else(|| OpsError::Message("Task is missing".into()))?;
        let Some(issue) = &task.plan.linear_id else {
            return Ok(());
        };
        let project = store
            .get_project(&task.project_id)
            .await
            .map_err(|e| message(&e))?
            .ok_or_else(|| OpsError::Message("Task Project is missing".into()))?;
        let wave = store
            .get_wave(&task.wave_id)
            .await
            .map_err(|e| message(&e))?
            .ok_or_else(|| OpsError::Message("Task Wave is missing".into()))?;
        if !connected(wave.repo()) {
            return Ok(());
        }
        let client = super::pm::issue_client(std::path::Path::new(wave.repo())).await?;
        let (observed, _) = client
            .issue_ownership(issue.as_str())
            .await
            .map_err(|e| message(&e))?
            .ok_or_else(|| OpsError::Message("Linked Linear issue is unavailable".into()))?;
        if observed.project_id.as_deref() != project.plan.linear_id.as_ref().map(|id| id.as_str()) {
            return Err(OpsError::Message(
                "Task membership changed in Linear; retained local decision".into(),
            ));
        }
        if observed.state.is_none() {
            return Err(OpsError::Message(
                "Linear state is unknown; saved decision retained".into(),
            ));
        }
        if !store
            .sqlite
            .observe_task_state(&delivery, &observed)
            .map_err(|e| message(&e))?
        {
            return Ok(());
        }
        if delivery.attempted {
            return Err(OpsError::Message(
                "Prior state delivery remains uncertain; saved decision retained".into(),
            ));
        }
        let state_id = client
            .item_state_id(issue.as_str(), &delivery.target)
            .await
            .map_err(|e| message(&e))?;
        if !store
            .sqlite
            .attempt_task_state(&delivery)
            .map_err(|e| message(&e))?
        {
            return Ok(());
        }
        // Linear has no expected-revision guard here. A reopening between the
        // observation and this mutation can be overwritten; matching readback
        // settles observed state, not the absence of an intervening edit.
        client
            .set_item_state(issue.as_str(), &state_id)
            .await
            .map_err(|e| message(&e))?;
        let (confirmed, _) = client
            .issue_ownership(issue.as_str())
            .await
            .map_err(|e| message(&e))?
            .ok_or_else(|| OpsError::Message("Linear state write has no readback".into()))?;
        if store
            .sqlite
            .observe_task_state(&delivery, &confirmed)
            .map_err(|e| message(&e))?
        {
            return Err(OpsError::Message(
                "Linear state write remains uncertain; saved decision retained".into(),
            ));
        }
        Ok::<(), OpsError>(())
    };
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), attempt).await;
    let error = match result {
        Ok(Ok(())) => return Ok(()),
        Ok(Err(error)) => error.to_string(),
        Err(_) => {
            "Task state delivery timed out; saved decision and delivery identity retained".into()
        }
    };
    store
        .sqlite
        .task_state_error(&delivery, &error)
        .map_err(|e| message(&e))
}

pub(crate) async fn sync_task_comments(store: &Store, task: &Task) -> OpsResult<()> {
    let message = |error: &dyn std::fmt::Display| OpsError::Message(error.to_string());
    let task = store
        .get_task(&task.id)
        .await
        .map_err(|error| message(&error))?
        .ok_or_else(|| OpsError::Message("Task is missing".into()))?;
    let Some(issue) = &task.plan.linear_id else {
        return Ok(());
    };
    let comments = store
        .sqlite
        .pending_task_comments(&task.id)
        .map_err(|error| message(&error))?;
    if comments.is_empty() {
        return Ok(());
    }
    let wave = store
        .get_wave(&task.wave_id)
        .await
        .map_err(|error| message(&error))?
        .ok_or_else(|| OpsError::Message("Task Wave is missing".into()))?;
    if !connected(wave.repo()) {
        return Ok(());
    }
    let client = super::pm::issue_client(std::path::Path::new(wave.repo())).await?;
    for comment in comments {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            client.sync_comment(&comment.id, issue.as_str(), &comment.body),
        )
        .await;
        let error = match result {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error.to_string()),
            Err(_) => Some("Comment delivery timed out; its identity is retained".into()),
        };
        store
            .sqlite
            .record_comment_delivery(&comment.id, error.as_deref())
            .map_err(|error| message(&error))?;
    }
    Ok(())
}

pub(crate) fn comment_revision_id(id: &str, revision: Option<&str>) -> String {
    match revision {
        Some(revision) => format!("{id}@{revision}"),
        None => id.to_string(),
    }
}

/// The person a comment speaks for. Explicit steering can be published through
/// an integration account: its recorded requester wins over the Linear author.
pub(crate) fn comment_requester(body: &str, author_name: Option<&str>) -> Option<String> {
    let requester = if is_steer(body) {
        body.split_once("<!-- loopflow-requester:")
            .and_then(|(_, rest)| rest.split_once(" -->"))
            .and_then(|(name, _)| serde_json::from_str::<String>(name).ok())
    } else {
        author_name.map(str::to_string)
    };
    requester
        .as_deref()
        .and_then(crate::engine::config::normalize_user_name)
        .or_else(|| author_name.and_then(crate::engine::config::normalize_user_name))
}

pub(crate) fn render_comment(
    id: &str,
    body: &str,
    author_id: Option<&str>,
    author_name: Option<&str>,
) -> String {
    let attribution = comment_requester(body, author_name)
        .map(|name| {
            format!(
                " by {}",
                serde_json::to_string(&name).expect("name is serializable")
            )
        })
        .unwrap_or_default();
    let source = author_id
        .map(|id| format!(" (provider user {id})"))
        .unwrap_or_default();
    format!("Linear comment {id}{attribution}{source}:\n\n{body}")
}

#[cfg(test)]
mod tests {
    use super::{is_direction_comment, render_comment};

    #[test]
    fn agent_progress_never_reenters_direction_even_with_a_quoted_steer() {
        assert!(!is_direction_comment(
            "done <!-- loopflow-progress:run_fixture --> <!-- loopflow-steer:quoted -->",
            Some("publisher"),
        ));
        assert!(is_direction_comment("keep the API", Some("publisher")));
        assert!(is_direction_comment(
            "new instruction <!-- loopflow-steer:explicit -->",
            None
        ));
    }

    #[test]
    fn request_authors_survive_linear_direction_rendering() {
        for name in ["Jack", "Maya"] {
            let rendered = render_comment("one", "prototype", Some("person"), Some(name));
            assert!(rendered.contains(&format!("by \"{name}\" (provider user person)")));
        }
        assert!(!render_comment("one", "prototype", Some("person"), None).contains(" by "));
        let explicit =
            "prototype\n<!-- loopflow-requester:\"Jack\" -->\n<!-- loopflow-steer:one -->";
        let rendered = render_comment("one", explicit, Some("publisher"), Some("Account Owner"));
        assert!(rendered.contains("by \"Jack\""));
        assert!(!rendered.contains("Account Owner"));
        let legacy = render_comment(
            "old",
            "prototype\n<!-- loopflow-steer:old -->",
            Some("publisher"),
            Some("Account Owner"),
        );
        assert!(legacy.contains("by \"Account Owner\""));
    }

    #[test]
    fn account_identity_does_not_filter_direction() {
        for author in ["person", "publisher"] {
            assert!(is_direction_comment("please fix this", Some(author)));
            assert!(!is_direction_comment("PR: x", Some(author)));
        }
        assert!(!is_direction_comment("bot", None));
    }
}
