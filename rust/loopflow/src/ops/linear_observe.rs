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
use crate::store::{SharedStore, Store};
use crate::work::task::Task;

pub(crate) fn connected(repo: &str) -> bool {
    crate::config::load_config(Some(std::path::Path::new(repo))).is_ok_and(|config| {
        matches!(
            config.unwrap_or_default().planning_transport(),
            crate::config::PlanningConfig::Linear {}
        )
    })
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
    let Some(issue) = &task.linear_id else {
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
    let client = super::linear::issue_client()
        .await?
        .for_repository(std::path::Path::new(wave.repo()));
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

/// Acquire only when a caller can consume the result. Freshness belongs to the
/// team, even when several repositories or processes share it.
pub(crate) async fn check_repository(store: &Store, repository: &str) -> OpsResult<bool> {
    if !connected(repository) {
        return Ok(false);
    }
    let repo = std::path::Path::new(repository);
    let team = super::linear::repository_team_id(repo)?;
    let mut live = Vec::new();
    let mut finished = Vec::new();
    let waves: Vec<_> = store
        .list_waves(None)
        .await
        .map_err(message)?
        .into_iter()
        .filter(|wave| {
            super::linear::repository_team_id(std::path::Path::new(wave.repo()))
                .ok()
                .as_ref()
                == Some(&team)
        })
        .collect();
    for wave in &waves {
        for task in store.list_tasks(Some(wave.id())).await.map_err(message)? {
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
    }
    if live.is_empty() {
        return Ok(false);
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
        .begin_linear_team_check(&team, now.unix_timestamp())
        .map_err(message)?
    else {
        return Ok(false);
    };
    let through = now
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(message)?;
    let client = super::linear::issue_client().await?;
    let changes = client
        .team_changes(&team, since.as_deref(), &through, &live, &finished)
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
            store
                .get_wave(&task.wave_id)
                .await
                .map_err(message)?
                .map(|wave| wave.repo().to_owned())
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

/// Bound on-demand work and retain local planning when Linear is unavailable.
pub(crate) async fn sync_repository(store: &Store, repository: &str) -> bool {
    let check =
        tokio::time::timeout(Duration::from_secs(5), check_repository(store, repository)).await;
    let changed = match check {
        Ok(Ok(changed)) => changed,
        result => {
            tracing::debug!(?result, "Linear team check pending");
            false
        }
    };
    if tokio::time::timeout(Duration::from_secs(5), async {
        if let Err(error) = sync_repository_deliveries(store, repository).await {
            tracing::debug!(%error, "planning delivery pending");
        }
        if let Err(error) = super::planning_export::sync_repository_exports(store, repository).await
        {
            tracing::debug!(%error, "planning export pending");
        }
        if let Err(error) =
            super::planning_delivery::sync_repository_fields(store, repository).await
        {
            tracing::debug!(%error, "planning field delivery pending");
        }
    })
    .await
    .is_err()
    {
        tracing::debug!("Linear delivery deadline elapsed; pending receipts retained");
    }
    changed
}

/// Only an attached watcher owns a Linear clock. Idle agents own no Linear timer.
#[derive(Debug)]
pub(crate) struct PlanningWatch {
    _git: super::planning_sync::PlanningSync,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PlanningWatch {
    pub(crate) fn start(store: SharedStore, repo: String) -> OpsResult<Self> {
        let git = super::planning_sync::PlanningSync::start(store.clone(), repo.clone())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let (stop, stopped) = tokio::sync::oneshot::channel();
        #[cfg(test)]
        let context = super::linear::LINEAR_TEST_CONTEXT
            .try_with(Clone::clone)
            .ok();
        let thread = std::thread::Builder::new()
            .name("planning-watch".into())
            .spawn(move || {
                let drive = async {
                    tokio::select! {
                        _ = stopped => {},
                        _ = watch_repository(&store, &repo) => {},
                    }
                };
                #[cfg(test)]
                if let Some(context) = context {
                    runtime.block_on(super::linear::LINEAR_TEST_CONTEXT.scope(context, drive));
                    return;
                }
                runtime.block_on(drive);
            })?;
        Ok(Self {
            _git: git,
            stop: Some(stop),
            thread: Some(thread),
        })
    }
}

async fn watch_repository(store: &Store, repo: &str) {
    let mut delay = 30;
    loop {
        let changed = sync_repository(store, repo).await;
        delay = if changed { 30 } else { (delay * 2).min(240) };
        tracing::debug!(target: "loopflow::linear_watch", delay, "Linear watcher waiting");
        tokio::time::sleep(Duration::from_secs(delay)).await;
    }
}

impl Drop for PlanningWatch {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) async fn sync_repository_deliveries(store: &Store, repository: &str) -> OpsResult<()> {
    if !connected(repository) {
        return Ok(());
    }
    let pending = store
        .sqlite
        .pending_planning_tasks(repository)
        .map_err(message)?;
    futures_util::future::join_all(pending.iter().map(|task| async {
        let (comments, state) = tokio::join!(
            sync_task_comments(store, task),
            sync_task_state(store, task)
        );
        for result in [comments, state] {
            if let Err(error) = result {
                tracing::debug!(%error, "planning delivery pending");
            }
        }
    }))
    .await;
    Ok(())
}

pub(crate) async fn sync_task_state(store: &Store, task: &Task) -> OpsResult<()> {
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
        let project = store
            .get_project(&task.project_id)
            .await
            .map_err(message)?
            .ok_or_else(|| OpsError::Message("Task Project is missing".into()))?;
        let wave = store
            .get_wave(&task.wave_id)
            .await
            .map_err(message)?
            .ok_or_else(|| OpsError::Message("Task Wave is missing".into()))?;
        if !connected(wave.repo()) {
            return Ok(());
        }
        let client = super::linear::issue_client()
            .await?
            .for_repository(std::path::Path::new(wave.repo()));
        let (observed, _) = client
            .issue_ownership(issue.as_str())
            .await
            .map_err(message)?
            .ok_or_else(|| OpsError::Message("Linked Linear issue is unavailable".into()))?;
        if observed.project_id.as_deref() != project.linear_id.as_ref().map(|id| id.as_str()) {
            return Err(OpsError::Message(
                "Task membership changed in Linear; retained local decision".into(),
            ));
        }
        if observed.state.is_none() {
            return Err(OpsError::Message(
                "Linear state is unknown; saved decision retained".into(),
            ));
        }
        if !crate::linear::acceptance::observe_task_state(&store.sqlite, &delivery, &observed)
            .map_err(message)?
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
            .map_err(message)?;
        if !store
            .sqlite
            .attempt_task_state(&delivery)
            .map_err(message)?
        {
            return Ok(());
        }
        // Linear has no expected-revision guard here. A reopening between the
        // observation and this mutation can be overwritten; its acknowledgement
        // settles observed state, not the absence of an intervening edit.
        let confirmed = match client.set_item_state(issue.as_str(), &state_id).await {
            Ok(confirmed) => confirmed,
            Err(error) => {
                if matches!(error, crate::linear::LinearError::RateLimited { .. }) {
                    store.sqlite.reject_task_state(&delivery).map_err(message)?;
                }
                return Err(message(&error));
            }
        };
        if crate::linear::acceptance::observe_task_state(&store.sqlite, &delivery, &confirmed)
            .map_err(message)?
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
        .map_err(message)
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
    let wave = store
        .get_wave(&task.wave_id)
        .await
        .map_err(message)?
        .ok_or_else(|| OpsError::Message("Task Wave is missing".into()))?;
    if !connected(wave.repo()) {
        return Ok(());
    }
    let client = super::linear::issue_client()
        .await?
        .for_repository(std::path::Path::new(wave.repo()));
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
    format!("Linear comment {id}{attribution}{source}:\n\n{body}")
}

#[cfg(test)]
mod tests {
    use super::{is_direction_comment, render_comment};

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
            runtime.block_on(async { state.lock().await.requests = 0 });
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
                            assert_eq!(state.lock().await.requests, index);
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
                        assert_eq!(state.lock().await.requests, index + 1);
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
                assert_eq!(state.lock().await.requests, 7);
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
