//! Turn a Linear issue read into durable Task direction.
//!
//! Someone editing a Linear issue's title or description or adding a comment
//! appends an ordered Steer. This module maps one observation onto that input
//! spine, and [`Store::apply_linear_observation`] persists it atomically.
//! Exactly-once, the baseline, and the monotonic-revision guard all live in the
//! store; acquisition supplies observations without proposing a second write.

use time::OffsetDateTime;

use super::{OpsError, OpsResult};
use crate::store::{SharedStore, Store};
use crate::work::task::Task;

fn connected(repo: &str) -> bool {
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
    pub(crate) fn start(store: SharedStore, task: Task) -> std::io::Result<Self> {
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
                    let inbound = async {
                        loop {
                            let result = tokio::time::timeout(
                                std::time::Duration::from_secs(5),
                                refresh_task_comments(&store, &task),
                            )
                            .await;
                            if let Ok(Err(error)) = result {
                                tracing::debug!(%error, "comment acquisition pending");
                            }
                            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                        }
                    };
                    let outbound = async {
                        loop {
                            if let Err(error) =
                                sync_repository_deliveries(&store, &task, false).await
                            {
                                tracing::debug!(%error, "comment delivery pending");
                            }
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        }
                    };
                    let decisions = async {
                        loop {
                            if let Err(error) =
                                sync_repository_deliveries(&store, &task, true).await
                            {
                                tracing::debug!(%error, "Task state delivery pending");
                            }
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        }
                    };
                    let inventory = async {
                        loop {
                            let result = tokio::time::timeout(
                                std::time::Duration::from_secs(5),
                                refresh_task_planning(&store, &task),
                            )
                            .await;
                            if let Ok(Err(error)) = result {
                                tracing::debug!(%error, "planning acquisition pending");
                            }
                            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                        }
                    };
                    tokio::select! {
                        _ = stopped => {},
                        _ = inbound => {},
                        _ = outbound => {},
                        _ = decisions => {},
                        _ = inventory => {},
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

async fn refresh_task_planning(store: &Store, task: &Task) -> OpsResult<()> {
    let wave = store
        .get_wave(&task.wave_id)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| OpsError::Message("Task Wave is missing".into()))?;
    // An absent mapping is not a reason to disable acquisition in a connected
    // repository. Connection configuration, rather than outbound work, selects it.
    let repo = std::path::Path::new(wave.repo());
    if !connected(wave.repo()) {
        return Ok(());
    }
    let owners = store
        .list_waves(Some(wave.repo()))
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

async fn sync_repository_deliveries(store: &Store, task: &Task, state: bool) -> OpsResult<()> {
    let wave = store
        .get_wave(&task.wave_id)
        .await
        .map_err(|e| OpsError::Message(e.to_string()))?
        .ok_or_else(|| OpsError::Message("Task Wave is missing".into()))?;
    if !connected(wave.repo()) {
        return Ok(());
    }
    let pending = store
        .sqlite
        .pending_planning_tasks(wave.repo())
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
    // Multiple foreground readers may deliver the same decision. Serialize
    // provider effects, independently of local saves and inbound acquisition.
    let directory = store
        .sqlite
        .home_dir()
        .map_err(|e| message(&e))?
        .join("locks/task-state");
    std::fs::create_dir_all(&directory).map_err(|e| message(&e))?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(format!("{}.lock", task.id)))
        .map_err(|e| message(&e))?;
    match fs2::FileExt::try_lock_exclusive(&lock) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
        Err(error) => return Err(message(&error)),
    }
    let Some(delivery) = store
        .sqlite
        .pending_task_state(&task.id)
        .map_err(|e| message(&e))?
    else {
        return Ok(());
    };
    if delivery.conflict.is_some() {
        return Ok(());
    }
    // The existing update protocol cannot preserve concurrent provider edits.
    // Do not extend it to cancellation while its write guarantee is unresolved.
    if delivery.target == "canceled" {
        return store.sqlite.settle_task_state(&delivery, Some(
            "Cancellation saved locally; safe Linear cancellation delivery is not implemented",
        )).map_err(|e| message(&e));
    }
    let attempt = async {
        let task = store
            .get_task(&task.id)
            .await
            .map_err(|e| message(&e))?
            .ok_or_else(|| OpsError::Message("Task is missing".into()))?;
        let Some(issue) = &task.plan.linear_id else {
            return Ok(false);
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
            return Ok(false);
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
        if observed.state.as_deref() == Some(&delivery.target) {
            return Ok(true);
        }
        // A provider clock is never compared to the local decision's clock.
        // An uncertain prior write may have completed and then been reopened.
        if delivery.attempted
            || observed.state != delivery.base_state
            || delivery.base_revision.is_none()
            || observed.revision != delivery.base_revision
        {
            store
                .sqlite
                .conflict_task_state(&delivery, &observed)
                .map_err(|e| message(&e))?;
            return Err(OpsError::Message(format!(
                "State conflict: saved locally as {}; Linear reports {} at revision {}. Delivery {} is retained; use `lf task sync {} --resolve local` or `--resolve linear`",
                delivery.target, observed.state.as_deref().unwrap_or("unknown"),
                observed.revision.as_deref().unwrap_or("unknown"), delivery.id, task.plan.identifier,
            )));
        }
        if !store
            .sqlite
            .attempt_task_state(&delivery)
            .map_err(|e| message(&e))?
        {
            return Ok(false);
        }
        if delivery.target == "completed" {
            client
                .complete_item(issue.as_str())
                .await
                .map_err(|e| message(&e))?;
        } else {
            client
                .reopen_item(issue.as_str())
                .await
                .map_err(|e| message(&e))?;
        }
        let (confirmed, _) = client
            .issue_ownership(issue.as_str())
            .await
            .map_err(|e| message(&e))?
            .ok_or_else(|| OpsError::Message("Linear state write has no readback".into()))?;
        if confirmed.state.as_deref() != Some(&delivery.target) {
            store
                .sqlite
                .conflict_task_state(&delivery, &confirmed)
                .map_err(|e| message(&e))?;
            return Err(OpsError::Message(format!(
                "State conflict: saved locally as {}; Linear readback is {}",
                delivery.target,
                confirmed.state.as_deref().unwrap_or("unknown")
            )));
        }
        Ok::<bool, OpsError>(true)
    };
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), attempt).await;
    let error = match result {
        Ok(Ok(true)) => None,
        Ok(Ok(false)) => return Ok(()),
        Ok(Err(error)) => Some(error.to_string()),
        Err(_) => Some(
            "Task state delivery timed out; saved decision and delivery identity retained".into(),
        ),
    };
    store
        .sqlite
        .settle_task_state(&delivery, error.as_deref())
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
