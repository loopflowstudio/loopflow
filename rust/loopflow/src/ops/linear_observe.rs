//! Turn a Linear issue read into durable Task direction.
//!
//! Someone editing a Linear issue's title or description or adding a comment
//! appends an ordered Steer. This module maps one observation onto that input
//! spine, and [`Store::apply_linear_observation`] persists it atomically.
//! Exactly-once, the baseline, and the monotonic-revision guard all live in the
//! store, so calling [`reconcile_linear_observation`] twice with the same read
//! is safe.

use time::OffsetDateTime;

use super::{OpsError, OpsResult};
use crate::store::SharedStore;

use crate::pm::IssueObservation;
use crate::store::{Store, StoreResult};
use crate::work::task::{
    LinearFollowUp, LinearObservationApply, LinearObservationOutcome, Task, TaskLinearObservation,
};

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
    reconcile_linear_observation(store, &task, observation, OffsetDateTime::now_utc())
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

fn content_steer_text(title: &str, description: &str) -> String {
    format!(
        "The linked Linear task was edited; use this current definition.\n\n\
         Title: {title}\n\n{description}"
    )
}

/// Read one Linear observation into durable, exactly-once Task direction.
pub async fn reconcile_linear_observation(
    store: &Store,
    task: &Task,
    observation: IssueObservation,
    observed_at: OffsetDateTime,
) -> StoreResult<LinearObservationOutcome> {
    let cursor = store.task_linear_observation(&task.id).await?;
    let apply = plan_apply(task, observation, observed_at, cursor.as_ref());
    store.apply_linear_observation(apply).await
}

/// Build the durable apply from a read and the current cursor. A
/// title/description edit becomes a Steer only when a baseline exists and the
/// content changed; every user comment rides as a candidate Steer, and the
/// store drops the ones already seen.
pub(crate) fn plan_apply(
    task: &Task,
    observation: IssueObservation,
    observed_at: OffsetDateTime,
    cursor: Option<&TaskLinearObservation>,
) -> LinearObservationApply {
    let content_steer = match cursor {
        Some(cursor)
            if cursor.last_title != observation.title
                || cursor.last_description != observation.description =>
        {
            Some(content_steer_text(
                &observation.title,
                &observation.description,
            ))
        }
        _ => None,
    };
    let follow_ups = observation
        .comments
        .iter()
        .filter(|comment| is_direction_comment(&comment.body, comment.author_id.as_deref()))
        .map(|comment| LinearFollowUp {
            comment_id: comment_revision_id(&comment.id, comment.revision.as_deref()),
            text: render_comment(
                &comment.id,
                &comment.body,
                comment.author_id.as_deref(),
                comment.author_name.as_deref(),
            ),
        })
        .collect();
    LinearObservationApply {
        task_id: task.id.clone(),
        revision: observation.revision,
        title: observation.title,
        description: observation.description,
        observed_at,
        content_steer,
        follow_ups,
        comments: observation.comments,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{is_direction_comment, plan_apply};
    use crate::planning::{LinearIssueId, TaskPlan};
    use crate::pm::{IssueComment, IssueObservation};
    use crate::work::task::{Task, TaskId, TaskLinearObservation};

    const VIEWER: &str = "user-loopflow";

    #[test]
    fn agent_progress_never_reenters_direction_even_with_a_quoted_steer() {
        let obs = observation(
            "title",
            "body",
            vec![
                comment(
                    "progress",
                    "done <!-- loopflow-progress:run_fixture --> <!-- loopflow-steer:quoted -->",
                    Some(VIEWER),
                ),
                comment("person", "keep the API", Some(VIEWER)),
                comment(
                    "explicit",
                    "new instruction <!-- loopflow-steer:explicit -->",
                    None,
                ),
            ],
        );
        let apply = plan_apply(&task(), obs, time::OffsetDateTime::now_utc(), None);
        assert_eq!(
            apply
                .follow_ups
                .iter()
                .map(|follow| follow.comment_id.as_str())
                .collect::<Vec<_>>(),
            ["person", "explicit"]
        );
    }

    #[test]
    fn request_authors_survive_linear_direction_rendering() {
        let comments = [
            ("one", Some("Jack")),
            ("two", Some("Maya")),
            ("three", None),
        ]
        .into_iter()
        .map(|(id, name)| IssueComment {
            id: id.into(),
            created_at: None,
            revision: None,
            body: "prototype".into(),
            author_id: Some(format!("person-{id}")),
            author_name: name.map(str::to_string),
        })
        .collect();
        let apply = plan_apply(
            &task(),
            observation("title", "body", comments),
            time::OffsetDateTime::now_utc(),
            None,
        );
        assert!(apply.follow_ups[0]
            .text
            .contains("by \"Jack\" (provider user person-one)"));
        assert!(apply.follow_ups[1]
            .text
            .contains("by \"Maya\" (provider user person-two)"));
        assert!(!apply.follow_ups[2].text.contains(" by "));
        let explicit =
            "prototype\n<!-- loopflow-requester:\"Jack\" -->\n<!-- loopflow-steer:one -->";
        let rendered =
            super::render_comment("one", explicit, Some("publisher"), Some("Account Owner"));
        assert!(rendered.contains("by \"Jack\""));
        assert!(!rendered.contains("Account Owner"));
        let legacy = super::render_comment(
            "old",
            "prototype\n<!-- loopflow-steer:old -->",
            Some("publisher"),
            Some("Account Owner"),
        );
        assert!(legacy.contains("by \"Account Owner\""));
    }

    fn comment(id: &str, body: &str, author: Option<&str>) -> IssueComment {
        IssueComment {
            author_name: None,
            id: id.to_string(),
            created_at: None,
            revision: None,
            body: body.to_string(),
            author_id: author.map(str::to_string),
        }
    }

    fn observation(
        title: &str,
        description: &str,
        comments: Vec<IssueComment>,
    ) -> IssueObservation {
        IssueObservation {
            revision: "2026-07-15T18:00:00.000Z".to_string(),
            title: title.to_string(),
            description: description.to_string(),
            comments,
        }
    }

    fn task() -> Task {
        let now = time::OffsetDateTime::now_utc();
        Task {
            id: TaskId::from_raw("ts_plan"),
            plan: TaskPlan {
                revision: 0,
                linear_id: Some(LinearIssueId::new("issue-1").unwrap()),
                identifier: "INF-123".to_string(),
                title: "Old title".to_string(),
                description: "Old body".to_string(),
                pm_snapshot_synced_at: Some(1),
            },
            pm_writeback: crate::work::task::PmWritebackState::Current,
            wave_id: crate::id::WaveId::new(),
            project_id: crate::work::project::ProjectId::new(),
            worktree: Some("/tmp/task".into()),
            workspace_slug: "ship-it".to_string(),
            agent: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::work::task::Observation::NotRequired,
        }
    }

    fn cursor(title: &str, description: &str) -> TaskLinearObservation {
        let now = time::OffsetDateTime::now_utc();
        TaskLinearObservation {
            task_id: TaskId::from_raw("task_plan"),
            last_revision: "2026-07-15T00:00:00.000Z".to_string(),
            last_title: title.to_string(),
            last_description: description.to_string(),
            last_success_at: now,
            degraded_reason: None,
            updated_at: now,
        }
    }

    #[test]
    fn account_identity_does_not_filter_direction() {
        for author in ["user-human", VIEWER] {
            assert!(is_direction_comment("please fix this", Some(author)));
            assert!(!is_direction_comment("PR: x", Some(author)));
        }
        assert!(!is_direction_comment("bot", None));
    }

    #[test]
    fn baseline_emits_no_content_steer_and_keeps_user_comments_as_candidates() {
        // No cursor yet: a title change must not become a Steer, but user
        // comments still ride so the store can deliver them.
        let obs = observation(
            "New title",
            "New body",
            vec![
                comment("c-1", "please prioritize", Some("user-human")),
                comment("c-2", "PR: x", Some(VIEWER)),
            ],
        );
        let apply = plan_apply(&task(), obs, time::OffsetDateTime::now_utc(), None);
        assert!(apply.content_steer.is_none());
        assert_eq!(apply.follow_ups.len(), 1);
        assert_eq!(apply.follow_ups[0].comment_id, "c-1");
    }

    #[test]
    fn a_content_edit_becomes_one_steer() {
        let obs = observation("New title", "New body", vec![]);
        let apply = plan_apply(
            &task(),
            obs,
            time::OffsetDateTime::now_utc(),
            Some(&cursor("Old title", "Old body")),
        );
        let steer = apply.content_steer.expect("Steer for a content edit");
        assert!(steer.contains("New title"));
        assert!(steer.contains("New body"));
    }

    #[test]
    fn an_unchanged_issue_emits_no_steer() {
        let obs = observation("Old title", "Old body", vec![]);
        let apply = plan_apply(
            &task(),
            obs,
            time::OffsetDateTime::now_utc(),
            Some(&cursor("Old title", "Old body")),
        );
        assert!(apply.content_steer.is_none());
    }
}
