//! Turn a Linear issue read into durable Task direction.
//!
//! Someone editing a Linear issue's title or description or adding a comment
//! appends an ordered Steer. This module maps one observation onto that input
//! spine, and [`Store::apply_linear_observation`] persists it atomically.
//! Exactly-once, the baseline, and the monotonic-revision guard all live in the
//! store; acquisition supplies observations without proposing a second write.

use std::time::Duration;

use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use super::{OpsError, OpsResult};
use crate::linear::StateHistory;
use crate::store::sqlite::task_state_delivery::TaskStateDelivery;
use crate::store::{SharedStore, Store};
use crate::work::task::Task;

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
    let Some(issue) = &task.linear_id else {
        return Ok(());
    };
    let repository = super::task::repository_for_task(store, &task)?;
    if !super::planning_client::connected(&repository.to_string_lossy()) {
        return Ok(());
    }
    // The Team delta carries this issue's new comments; a current one needs no
    // per-issue read.
    if team_current(store, &repository.to_string_lossy()).await {
        return Ok(());
    }
    let client = super::planning_client::PlanningClient::for_items(&repository).await?;
    // A coworker deleted the item: remove the Task from the plan, keeping its work.
    let url = store.sqlite.task_issue_url(&task.id).map_err(message)?;
    if client
        .removed(issue.as_str(), url.as_deref())
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
    {
        store
            .sqlite
            .observe_pm_issue_change(issue.as_str(), None, true)
            .map_err(message)?;
        return Ok(());
    }
    let mut observation = client
        .observe_issue(issue.as_str())
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    claim_local_comments(store, &task, &client, &mut observation)?;
    // Linear's team feed accepts provider edits into Work. Asana and Notion have
    // none, so this exact read is their acceptance: a renamed or completed task
    // updates the Task, and an unsent local edit stays as a visible conflict.
    if !matches!(client, super::planning_client::PlanningClient::Linear(_)) {
        if let Some((item, project)) = client
            .issue_ownership(issue.as_str())
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
        {
            super::planning_delivery::ingest_task(
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
        .apply_linear_observation(&task.id, observation, OffsetDateTime::now_utc())
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    Ok(())
}

/// Asana and Notion comments carry no caller identity. A comment showing a
/// Loopflow comment's visible text is that comment: delivery settles and nothing
/// echoes back.
fn claim_local_comments(
    store: &Store,
    task: &Task,
    client: &super::planning_client::PlanningClient,
    observation: &mut crate::linear::LinearIssueObservation,
) -> OpsResult<()> {
    if matches!(client, super::planning_client::PlanningClient::Linear(_)) {
        return Ok(());
    }
    let local = store
        .sqlite
        .task_comments(&task.id)
        .map_err(message)?
        .comments;
    for comment in &mut observation.comments {
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

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

pub(crate) async fn check_before_write(repo: &std::path::Path) -> OpsResult<()> {
    let store = super::planning_sync::planning_store().await?;
    tokio::time::timeout(
        Duration::from_secs(5),
        check_repository(&store, &repo.to_string_lossy()),
    )
    .await
    .map_err(message)??;
    Ok(())
}

/// Bring the repository's planning scope (a Linear Team, Asana project or
/// Notion database) up to date: items people created or edited there become or
/// update Tasks. Freshness belongs to the scope, even when several repositories
/// or processes share it.
pub(crate) async fn check_repository(store: &Store, repository: &str) -> OpsResult<bool> {
    if !super::planning_client::connected(repository) {
        return Ok(false);
    }
    let repo = std::path::Path::new(repository);
    let team = super::planning_client::repository_scope(repo)?;
    let mut live = Vec::new();
    let mut finished = Vec::new();
    let waves: Vec<_> = store
        .list_waves(None)
        .await
        .map_err(message)?
        .into_iter()
        .filter(|wave| {
            super::planning_client::repository_scope(std::path::Path::new(wave.repo()))
                .ok()
                .as_ref()
                == Some(&team)
        })
        .collect();
    for task in store.list_tasks(None).await.map_err(message)? {
        let task_repo = super::task::repository_for_task(store, &task)?;
        if super::planning_client::repository_scope(&task_repo)
            .ok()
            .as_ref()
            != Some(&team)
        {
            continue;
        }
        let Some(issue) = task.linear_id else {
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
    let Some(_lock) = super::planning_delivery::lock_delivery(&path)? else {
        return Ok(false);
    };
    let now = OffsetDateTime::now_utc();
    let Some(since) = store
        .sqlite
        .begin_linear_team_check(&team, now.unix_timestamp(), fresh_for(repository))
        .map_err(message)?
    else {
        return Ok(false);
    };
    let through = now
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(message)?;
    let client = super::planning_client::PlanningClient::for_items(repo).await?;
    let changes = client
        .changes(&team, since.as_deref(), &through, &live, &finished)
        .await
        .map_err(message)?;
    let changed = !changes.is_empty();
    for (issue, project, observation) in changes {
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
            // Ingestion resolves the returned Project against authored Wave mappings.
            waves
                .iter()
                .find(|wave| {
                    super::linear::read_initiative(std::path::Path::new(wave.repo()), wave.slug())
                        .is_some_and(|initiative| {
                            project
                                .as_ref()
                                .is_some_and(|project| project.initiative_ids.contains(&initiative))
                        })
                })
                .map(|wave| wave.repo().to_owned())
                // An item in no Wave Project is a standalone Task of this repository.
                .or_else(|| Some(repository.to_owned()))
        };
        let Some(owner) = owner else { continue };
        super::planning_delivery::ingest_task(
            store,
            std::path::Path::new(&owner),
            &issue,
            project.as_ref(),
            now.unix_timestamp(),
        )
        .await?;
        if let Some(task) = store.get_task_by_issue(&issue.id).await.map_err(message)? {
            let mut observation = observation;
            claim_local_comments(store, &task, &client, &mut observation)?;
            store
                .apply_linear_observation(&task.id, observation, now)
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
/// shared budget falls; Asana and Notion keep the healthy window.
fn fresh_for(repository: &str) -> i64 {
    if super::planning_client::is_linear(repository) {
        crate::linear::delta_fresh_secs()
    } else {
        crate::linear::HEALTHY_FRESH_SECS
    }
}

/// The one freshness check for Linear reads: bring the repository's Team
/// delta up to date (a no-op inside its window), then report whether local
/// state is current enough to answer in place of a per-item read.
pub(crate) async fn team_current(store: &Store, repository: &str) -> bool {
    if !super::planning_client::is_linear(repository) {
        return false;
    }
    check_repository_bounded(store, repository).await;
    let Ok(team) = super::planning_client::repository_scope(std::path::Path::new(repository))
    else {
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
/// it changed local state. Unavailable or slow providers leave state as it is.
async fn check_repository_bounded(store: &Store, repository: &str) -> bool {
    let check =
        tokio::time::timeout(Duration::from_secs(5), check_repository(store, repository)).await;
    match check {
        Ok(Ok(changed)) => changed,
        result => {
            tracing::debug!(?result, "Linear team check pending");
            false
        }
    }
}

/// Bound on-demand work and retain local planning when Linear is unavailable.
pub(crate) async fn sync_repository(store: &Store, repository: &str) -> bool {
    let changed = check_repository_bounded(store, repository).await;
    // Each delivery has its own deadline. A shared deadline repeatedly canceled
    // the first slow export and starved every later saved Task.
    if let Err(error) = sync_repository_deliveries(store, repository).await {
        tracing::debug!(%error, "planning delivery pending");
    }
    if let Err(error) = super::planning_export::sync_repository_exports(store, repository).await {
        tracing::debug!(%error, "planning export pending");
    }
    if let Err(error) = super::planning_delivery::sync_repository_fields(store, repository).await {
        tracing::debug!(%error, "planning field delivery pending");
    }
    changed
}

/// Only an attached watcher owns a Linear clock. Idle agents own no Linear timer.
pub(super) async fn watch_repository(store: &Store, repo: &str) {
    let mut delay = 30;
    loop {
        let changed = sync_repository(store, repo).await;
        delay = if changed { 30 } else { (delay * 2).min(240) };
        tracing::debug!(target: "loopflow::linear_watch", delay, "Linear watcher waiting");
        tokio::time::sleep(Duration::from_secs(delay)).await;
    }
}

pub(super) async fn sync_repository_deliveries(store: &Store, repository: &str) -> OpsResult<()> {
    if !super::planning_client::connected(repository) {
        return Ok(());
    }
    let pending = store
        .sqlite
        .pending_planning_tasks(repository)
        .map_err(message)?;
    // State before comments, as `task sync` orders them: our own comment must
    // not land as an intervening revision while the state write is in flight.
    futures_util::future::join_all(pending.iter().map(|task| async {
        let state = sync_task_state(store, task).await;
        let comments = sync_task_comments(store, task).await;
        for result in [state, comments] {
            if let Err(error) = result {
                tracing::debug!(%error, "planning delivery pending");
            }
        }
    }))
    .await;
    Ok(())
}

pub(crate) async fn sync_task_state(store: &Store, task: &Task) -> OpsResult<()> {
    deliver_task_state(store, task, false).await
}

pub(crate) async fn deliver_task_state(store: &Store, task: &Task, retry: bool) -> OpsResult<()> {
    let path = store
        .sqlite
        .home_dir()
        .map_err(message)?
        .join("locks/task-state")
        .join(format!("{}.lock", task.id));
    let Some(_lock) = super::planning_delivery::lock_delivery(&path)? else {
        return Ok(());
    };
    let Some(delivery) = store.sqlite.pending_task_state(&task.id).map_err(message)? else {
        return Ok(());
    };
    let attempt = async {
        let task = store
            .get_task(&task.id)
            .await
            .map_err(message)?
            .ok_or_else(|| OpsError::Message("Task is missing".into()))?;
        let Some(issue) = &task.linear_id else {
            return Ok(());
        };
        let project = match &task.project_id {
            Some(id) => Some(
                store
                    .get_project(id)
                    .await
                    .map_err(message)?
                    .ok_or_else(|| message("Task Project is missing"))?,
            ),
            None => None,
        };
        let repository = super::task::repository_for_task(store, &task)?;
        let Some(provider) = super::planning_client::remote_name(&repository) else {
            return Ok(());
        };
        let client = super::planning_client::PlanningClient::for_items(&repository).await?;
        let (observed, _) = client
            .issue_ownership(issue.as_str())
            .await
            .map_err(message)?
            .ok_or_else(|| message(format!("Linked {provider} task is unavailable")))?;
        if observed.project_id.as_deref()
            != project
                .as_ref()
                .and_then(|p| p.linear_id.as_ref())
                .map(|id| id.as_str())
        {
            return Err(message(format!(
                "Task membership changed in {provider}; retained local decision"
            )));
        }
        if observed.state.is_none() {
            return Err(message(format!(
                "{provider} state is unknown; saved decision retained"
            )));
        }
        let history = state_history(&client, issue.as_str(), &delivery, &observed).await;
        if !crate::linear::acceptance::observe_task_state(
            &store.sqlite,
            &delivery,
            &observed,
            history,
        )
        .map_err(message)?
        {
            return Ok(());
        }
        let attempted = delivery.attempted && history != StateHistory::Unchanged;
        if attempted && !retry {
            return Err(OpsError::Message(
                "Prior state delivery remains uncertain; saved decision retained".into(),
            ));
        }
        let state_id = client
            .state_id(
                super::planning_client::StateScope::Item(issue.as_str()),
                &delivery.target,
            )
            .await
            .map_err(message)?;
        // Reserve at the send boundary: a slow pre-write check must not leave
        // an unsent request attempted. Its throttle skips the request's own check.
        if matches!(client, super::planning_client::PlanningClient::Linear(_)) {
            if let Err(error) = check_before_write(&repository).await {
                tracing::debug!(%error, "pre-write Linear check pending");
            }
        }
        if !store
            .sqlite
            .attempt_task_state(&delivery, retry)
            .map_err(message)?
        {
            return Ok(());
        }
        // Neither provider has an expected-revision guard here. A reopening between
        // the observation and this mutation can be overwritten; its acknowledgement
        // settles observed state, not the absence of an intervening edit.
        let confirmed = match client.set_item_state(issue.as_str(), &state_id).await {
            Ok(confirmed) => confirmed,
            Err(error) => {
                if !attempted && matches!(error, crate::linear::LinearError::RateLimited { .. }) {
                    store.sqlite.reject_task_state(&delivery).map_err(message)?;
                }
                return Err(message(&error));
            }
        };
        // The reserved row carries the attempt and any base evidence moved forward.
        let history = match store
            .sqlite
            .pending_task_state(&delivery.task_id)
            .map_err(message)?
        {
            Some(reserved) if reserved.id == delivery.id => {
                state_history(&client, issue.as_str(), &reserved, &confirmed).await
            }
            _ => StateHistory::Unknown,
        };
        if crate::linear::acceptance::observe_task_state(
            &store.sqlite,
            &delivery,
            &confirmed,
            history,
        )
        .map_err(message)?
        {
            return Err(message(format!(
                "{provider} state write remains uncertain; saved decision retained"
            )));
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
        .map_err(message)
}

/// Evidence for a write that may have reached the provider while the state
/// still reads as its base. Any other observation needs none.
async fn state_history(
    client: &super::planning_client::PlanningClient,
    issue: &str,
    delivery: &TaskStateDelivery,
    observed: &crate::linear::LinearIssue,
) -> StateHistory {
    let (Some(base_revision), Some(base_state)) = (&delivery.base_revision, &delivery.base_state)
    else {
        return StateHistory::Unknown;
    };
    if !delivery.attempted || observed.state.as_ref() != Some(base_state) {
        return StateHistory::Unknown;
    }
    if observed.revision.as_ref() == Some(base_revision) {
        return StateHistory::Unchanged;
    }
    match client
        .state_history_since(issue, base_revision, base_state)
        .await
    {
        Ok(history) => history,
        Err(error) => {
            tracing::debug!(%error, "Task state history unavailable");
            StateHistory::Unknown
        }
    }
}

pub(crate) async fn sync_task_comments(store: &Store, task: &Task) -> OpsResult<()> {
    let task = store
        .get_task(&task.id)
        .await
        .map_err(message)?
        .ok_or_else(|| OpsError::Message("Task is missing".into()))?;
    let Some(issue) = &task.linear_id else {
        return Ok(());
    };
    let comments = store
        .sqlite
        .pending_task_comments(&task.id)
        .map_err(message)?;
    if comments.is_empty() {
        return Ok(());
    }
    let repository = super::task::repository_for_task(store, &task)?;
    if !super::planning_client::connected(&repository.to_string_lossy()) {
        return Ok(());
    }
    let client = super::planning_client::PlanningClient::for_items(&repository).await?;
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
            .map_err(message)?;
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
        .and_then(crate::config::normalize_user_name)
        .or_else(|| author_name.and_then(crate::config::normalize_user_name))
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
    format!("Planning comment {id}{attribution}{source}:\n\n{body}")
}

#[cfg(test)]
mod tests {
    use super::{is_direction_comment, render_comment};

    #[test]
    fn linear_on_demand_watch_connection_acquires_and_closes() {
        use crate::ops::linear::task_planning_tests::{mark_issue_updated, with_planning_task};

        with_planning_task(|runtime, fixture, repo, task, state| {
            runtime.block_on(async {
                let mut provider = state.lock().await;
                provider.issues[0]["title"] = serde_json::json!("Acquired by watcher");
                mark_issue_updated(&mut provider.issues[0]);
            });
            let watch = crate::ops::planning_sync::PlanningSync::start_watch(
                fixture.store.clone(),
                repo.to_string_lossy().into_owned(),
            )
            .unwrap();
            runtime.block_on(async {
                tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    while fixture
                        .store
                        .get_task(&task.id)
                        .await
                        .unwrap()
                        .unwrap()
                        .title
                        != "Acquired by watcher"
                    {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                })
                .await
                .unwrap();
            });
            let closing = std::time::Instant::now();
            drop(watch);
            assert!(closing.elapsed() < std::time::Duration::from_secs(5));
        });
    }

    #[test]
    fn linear_on_demand_watcher_backs_off_and_resets_on_change() {
        use crate::ops::linear::task_planning_tests::{mark_issue_updated, with_planning_task};
        use tracing_subscriber::layer::SubscriberExt;

        struct Sleeps(std::sync::mpsc::Sender<u64>);
        impl tracing::field::Visit for Sleeps {
            fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
            fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
                if field.name() == "delay" {
                    self.0.send(value).unwrap();
                }
            }
        }
        impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Sleeps {
            fn on_event(
                &self,
                event: &tracing::Event<'_>,
                _: tracing_subscriber::layer::Context<'_, S>,
            ) {
                if event.metadata().target() == "loopflow::linear_watch" {
                    event.record(&mut Sleeps(self.0.clone()));
                }
            }
        }

        with_planning_task(|runtime, fixture, repo, task, state| {
            let clock = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            conn.execute("DELETE FROM linear_team_checks", []).unwrap();
            runtime.block_on(async { state.lock().await.team_changes = 0 });
            let (sender, sleeps) = std::sync::mpsc::channel();
            let subscriber = tracing_subscriber::registry().with(Sleeps(sender));
            let _trace = tracing::subscriber::set_default(subscriber);
            clock.block_on(async {
                tokio::time::pause();
                let mut watch = Box::pin(super::watch_repository(
                    &fixture.store,
                    repo.to_str().unwrap(),
                ));
                let exercise = async {
                    let mut previous = 0;
                    for (index, expected) in [30, 60, 120, 240, 240, 30, 60].into_iter().enumerate()
                    {
                        if previous != 0 {
                            tokio::time::advance(std::time::Duration::from_secs(previous - 1))
                                .await;
                            tokio::task::yield_now().await;
                            assert_eq!(state.lock().await.team_changes, index);
                            // SQLite freshness uses wall time; advance its attempt too.
                            conn.execute("UPDATE linear_team_checks SET attempted_at=0", [])
                                .unwrap();
                            if index == 5 {
                                let mut provider = state.lock().await;
                                provider.issues[0]["title"] =
                                    serde_json::json!("Watcher acquired change");
                                mark_issue_updated(&mut provider.issues[0]);
                            }
                            // Tokio rounds sleep deadlines to millisecond ticks.
                            tokio::time::advance(std::time::Duration::from_millis(1001)).await;
                        }
                        let deadline =
                            std::time::Instant::now() + std::time::Duration::from_secs(5);
                        let delay = loop {
                            if let Ok(delay) = sleeps.try_recv() {
                                break delay;
                            }
                            assert!(
                                std::time::Instant::now() < deadline,
                                "watcher did not finish check {index}"
                            );
                            tokio::task::yield_now().await;
                        };
                        assert_eq!(delay, expected);
                        assert_eq!(state.lock().await.team_changes, index + 1);
                        previous = delay;
                    }
                    assert_eq!(
                        fixture
                            .store
                            .get_task(&task.id)
                            .await
                            .unwrap()
                            .unwrap()
                            .title,
                        "Watcher acquired change"
                    );
                };
                tokio::select! {
                    _ = &mut watch => panic!("watcher unexpectedly stopped"),
                    _ = exercise => {},
                }
                drop(watch);
                tokio::time::advance(std::time::Duration::from_secs(600)).await;
                assert_eq!(state.lock().await.team_changes, 7);
            });
        });
    }

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
