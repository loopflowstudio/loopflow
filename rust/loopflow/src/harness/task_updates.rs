//! Native Session updates read existing Task/Graph facts. Receipts live in the
//! Session history; they are delivery evidence, never another workflow cursor.
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::durable::{GraphStepProcess, TaskId};
use crate::id::LfProcessId;
use crate::store::sqlite::SqliteStore;
use crate::store::{RecordingGap, StoreError, StoreResult};
use crate::task_graph::TaskGraphPosition;
use crate::work::task::comment::TaskCommentAuthor;

#[derive(Debug, Clone)]
pub(crate) struct TaskUpdate {
    pub key: String,
    pub text: String,
    pub observation: Value,
}

// Keep only reportable facts, not compiled graphs or their full step histories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TaskObservation {
    position: Option<TaskGraphPosition>,
    runs: HashMap<LfProcessId, RunObservation>,
    /// Comment IDs this conversation has heard about. Observations recorded
    /// before comments were reported have none; they baseline on next read.
    comments: Option<BTreeSet<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RunObservation {
    name: String,
    outcome: Option<String>,
    completed: bool,
    step: Option<GraphStepProcess>,
}

impl RunObservation {
    fn report(&self, id: &LfProcessId) -> String {
        let outcome = self.outcome.as_deref().unwrap_or(if self.completed {
            "stopped (outcome unavailable)"
        } else {
            "running"
        });
        let status = self
            .step
            .as_ref()
            .map(|step| {
                format!(
                    "{} ({}): {}",
                    step.position(),
                    step.lf_process_id,
                    step.outcome.as_deref().unwrap_or("running")
                )
            })
            .unwrap_or_else(|| "starting".into());
        format!("Run {} ({id}) · {outcome} · {status}.", self.name)
    }
}

#[derive(Debug, Default)]
pub(crate) struct TaskUpdates {
    candidates: HashMap<String, (Value, Instant)>,
    revision: Option<i64>,
    read_at: Option<Instant>,
    poll_again: bool,
}

impl TaskUpdates {
    // Each run settles independently: another run's churn must not delay it.
    fn settled<T: Serialize + PartialEq>(
        &mut self,
        id: &str,
        current: &T,
        previous: Option<&T>,
    ) -> StoreResult<bool> {
        if previous == Some(current) {
            self.candidates.remove(id);
            return Ok(false);
        }
        let value = serde_json::to_value(current)?;
        match self.candidates.get(id) {
            Some((candidate, since)) if candidate == &value => {
                Ok(since.elapsed() >= Duration::from_secs(1))
            }
            _ => {
                self.candidates
                    .insert(id.to_owned(), (value, Instant::now()));
                Ok(false)
            }
        }
    }

    /// Read on the native owner's clock. Worker Runs never receive updates.
    /// Initial history is a baseline; reconnect compares durable observations.
    pub(crate) fn pending(
        &mut self,
        store: &SqliteStore,
        session_id: &str,
    ) -> StoreResult<Option<TaskUpdate>> {
        let revision = store.task_update_revision()?;
        if self.revision == Some(revision)
            && self.candidates.is_empty()
            && (!self.poll_again
                || self
                    .read_at
                    .is_some_and(|at| at.elapsed() < Duration::from_secs(2)))
        {
            return Ok(None);
        }
        // Failed reads remain retryable even when no writer moves the revision.
        self.revision = None;
        let candidates = self.candidates.clone();
        let result = self.read(store, session_id);
        if let Ok(update) = &result {
            self.revision = Some(revision);
            self.read_at = Some(Instant::now());
            // A busy provider can defer delivery without another store write.
            self.poll_again |= update.is_some();
        } else {
            self.candidates = candidates;
        }
        result
    }

    fn read(&mut self, store: &SqliteStore, session_id: &str) -> StoreResult<Option<TaskUpdate>> {
        self.poll_again = false;
        let Some(session) = store.session(session_id)? else {
            self.candidates.clear();
            return Ok(None);
        };
        if !session.interactive || session.completed_at.is_some() {
            self.candidates.clear();
            return Ok(None);
        }
        let previous = store.latest_task_update(session_id)?;
        let baseline = previous.is_none();
        let previous: HashMap<TaskId, TaskObservation> = previous
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();
        let mut visited = HashSet::new();
        let mut facts = HashMap::new();
        let mut lines = Vec::new();
        for task in store.session_task_ids(session_id)? {
            visited.insert(task.to_string());
            let (overview, graphs) = store.task_overview(&task)?;
            let old = previous.get(&task);
            let position = overview.graph.map(|graph| graph.position);
            // A live process can exit without recording it. Settled history
            // otherwise changes only when a store revision moves.
            self.poll_again |=
                matches!(position, Some(TaskGraphPosition::Run { running: true, .. }));
            let position_changed = self.settled(
                task.as_str(),
                &position,
                old.map(|observation| &observation.position),
            )?;
            let mut observation = TaskObservation {
                position: if baseline || position_changed {
                    position
                } else {
                    old.and_then(|observation| observation.position.clone())
                },
                runs: HashMap::new(),
                comments: None,
            };
            let mut changes = Vec::new();
            for graph in graphs {
                self.poll_again |= graph.entry.ended_at.is_none();
                let id = LfProcessId::parse(&graph.entry.summary.id)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                visited.insert(id.to_string());
                let prior = old.and_then(|observation| observation.runs.get(&id));
                // Exit and next start are separate commits. Preserve this run's
                // last reported facts until its next step or terminal result.
                if graph.entry.ended_at.is_none()
                    && graph
                        .steps
                        .last()
                        .is_some_and(|step| step.completed_at.is_some())
                {
                    self.candidates.remove(id.as_str());
                    if let Some(prior) = prior {
                        observation.runs.insert(id, prior.clone());
                    }
                    continue;
                }
                let run = RunObservation {
                    name: graph.entry.summary.name,
                    outcome: store.lf_process(&id)?.and_then(|process| process.outcome),
                    completed: graph.entry.ended_at.is_some(),
                    step: graph.steps.last().cloned(),
                };
                if baseline || self.settled(id.as_str(), &run, prior)? {
                    if !baseline {
                        changes.push(run.report(&id));
                    }
                    observation.runs.insert(id, run);
                } else if let Some(prior) = prior {
                    observation.runs.insert(id, prior.clone());
                }
            }
            let comments = store.task_comment_content(&task)?;
            let heard = old.and_then(|observation| observation.comments.as_ref());
            // A comment this conversation posted is not news to it.
            let arrived: Vec<String> = heard
                .map(|heard| {
                    comments
                        .iter()
                        .filter(|comment| !heard.contains(&comment.id))
                        .filter(|comment| !written_here(&comment.author, session_id))
                        .map(|comment| {
                            format!("New comment from {}: {}", comment.author, comment.body)
                        })
                        .collect()
                })
                .unwrap_or_default();
            observation.comments = Some(comments.into_iter().map(|comment| comment.id).collect());
            if position_changed || !changes.is_empty() || !arrived.is_empty() {
                let identifier = store
                    .task(&task)?
                    .map(|task| task.identifier)
                    .unwrap_or_else(|| task.to_string());
                lines.push(format!(
                    "Task {identifier}: {}. {}",
                    position_text(observation.position.as_ref()),
                    changes.join(" ")
                ));
                lines.extend(arrived);
            }
            facts.insert(task, observation);
        }
        self.candidates.retain(|id, _| visited.contains(id));
        if !baseline && lines.is_empty() {
            return Ok(None);
        }
        let observation = serde_json::to_value(&facts)?;
        let key = format!("{:x}", Sha256::digest(serde_json::to_vec(&observation)?));
        if baseline {
            store.record_task_update(session_id, &key, "baseline", &observation)?;
            self.candidates.clear();
            return Ok(None);
        }
        Ok(Some(TaskUpdate { key, observation, text: format!(
            "<lf:task-update>\n{}\nReport this recorded progress briefly. This is observation, not a request or permission to launch work, advance review, publish or merge. Read current Task status before acting. Diffs belong in Files.\n</lf:task-update>", lines.join("\n")) }))
    }
}

fn written_here(author: &TaskCommentAuthor, session_id: &str) -> bool {
    matches!(author, TaskCommentAuthor::Session { session_id: id, .. } if id == session_id)
}

fn position_text(position: Option<&TaskGraphPosition>) -> String {
    match position {
        Some(TaskGraphPosition::At { step }) if step == "end" => {
            "At end (Task completion is separate)".into()
        }
        Some(TaskGraphPosition::At { step }) if step == "start" => "At start".into(),
        Some(TaskGraphPosition::At { step }) => format!("At {step} · available for review"),
        Some(TaskGraphPosition::Run { step, running, .. }) => format!(
            "{} from {step}",
            if *running { "Running" } else { "Stopped" }
        ),
        None => "No Task workflow selected".into(),
    }
}

/// Native extension/hook observer. Provider identity fences every read and write;
/// attachment-only handoff leaves the same conversation and observer alive.
#[cfg(unix)]
pub(crate) fn watch(
    home: &std::path::Path,
    input: &serde_json::Value,
    adapter: crate::steer::SteerAdapter,
    thread: &str,
    mut deliver: impl FnMut(
        &SqliteStore,
        &crate::steer::SteerAdapter,
        &str,
        &TaskUpdate,
    ) -> StoreResult<()>,
) -> anyhow::Result<()> {
    use crate::steer::NativeBoundary;
    let store = SqliteStore::new(&home.join("loopflow.db"))?;
    if !store
        .session(&adapter.session)?
        .is_some_and(|session| session.interactive && session.completed_at.is_none())
    {
        return Ok(());
    }
    // Select before locking: a clear/resume can replace the native conversation
    // while an older observer still owns its thread's lock.
    let boundary = if input["hook_event_name"] == "SessionStart" {
        NativeBoundary::Select {
            source: input["source"].as_str().unwrap_or("startup"),
        }
    } else {
        NativeBoundary::Turn
    };
    store.with_provider_context(&adapter, thread, boundary, || Ok(()))?;
    let directory = home.join("task-updates");
    std::fs::create_dir_all(&directory)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join(format!(
            "{}-{:x}.lock",
            adapter.agent_process_id,
            Sha256::digest(thread.as_bytes())
        )))?;
    match fs2::FileExt::try_lock_exclusive(&lock) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    let mut sync_started = false;
    let mut updates = TaskUpdates::default();
    let mut gap = RecordingGap::default();
    loop {
        if crate::journal::agent_evidence(&store, &adapter.agent_process_id)
            == crate::journal::ProcessIdentityEvidence::Dead
        {
            return Ok(());
        }
        let pending = store.with_provider_context(&adapter, thread, NativeBoundary::Turn, || {
            updates.pending(&store, &adapter.session)
        });
        let pending = match pending {
            Ok(pending) => {
                gap.recovered();
                // Baseline local comments before syncing, so a quick tracker
                // response cannot swallow newly fetched comments as old history.
                if !sync_started {
                    for task in store.session_task_ids(&adapter.session)? {
                        if let Some(task) = store.task(&task)? {
                            std::thread::spawn(move || {
                                crate::agent_input_timing::record(
                                    "task_sync_begin",
                                    &task.identifier,
                                );
                                if let Err(error) =
                                    crate::ops::task::sync_for_conversation(&task.identifier)
                                {
                                    tracing::info!(task = %task.identifier, %error, "Task sync after launch pending");
                                }
                                crate::agent_input_timing::record(
                                    "task_sync_end",
                                    &task.identifier,
                                );
                            });
                        }
                    }
                    sync_started = true;
                }
                pending
            }
            Err(StoreError::InvalidAuthority(_)) => return Ok(()),
            Err(error) => {
                gap.note("native Task updates", &error);
                None
            }
        };
        if let Some(update) = pending {
            // Claude's next-priority inbox and Pi's follow-up queue own when
            // this input runs. A native terminal may have no availability
            // observation; waiting for one would prevent idle delivery forever.
            match deliver(&store, &adapter, thread, &update) {
                Ok(()) => {}
                Err(StoreError::InvalidAuthority(_)) => return Ok(()),
                Err(error) => gap.note("native Task update delivery", &error),
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::TaskUpdates;
    use crate::durable::TaskId;
    use crate::id::LfProcessId;
    use crate::performance::testing::{measure, observe};
    use crate::store::sqlite::SqliteStore;
    use crate::work::task::comment::{TaskComment, TaskCommentAuthor};
    use std::sync::{Arc, Barrier};
    use std::time::{Duration, Instant};

    fn settle(updates: &mut TaskUpdates) {
        for (_, since) in updates.candidates.values_mut() {
            *since = Instant::now() - Duration::from_secs(2);
        }
    }

    fn tick(updates: &mut TaskUpdates) {
        updates.read_at = Some(Instant::now() - Duration::from_secs(3));
    }

    #[test]
    fn unrelated_activity_has_constant_cost_with_history_and_observers() {
        let (_home, store, task) = task_session();
        store.test_session("other", &crate::session_record::new_artifact_key());
        store.conn.lock().unwrap().execute("INSERT INTO session_activity(session_id,observed_at,open_tools,pending_input,yielded) VALUES('other',1,1,0,0)", []).unwrap();
        let mut measurements = Vec::new();
        for history in [1, 32] {
            for n in 0..history {
                store.test_graph(
                    "history",
                    "/task-updates",
                    &[("gate", Some("succeeded"))],
                    Some("succeeded"),
                );
                comment(
                    &store,
                    &task,
                    &format!("old {n}"),
                    TaskCommentAuthor::Person { name: None },
                );
                store
                    .create_task(&crate::work::task::NewTask {
                        repository_id: store.ensure_repository("/unrelated").unwrap(),
                        id: TaskId::new(),
                        project_id: None,
                        title: "Other Task".into(),
                        description: String::new(),
                        due_date: None,
                    })
                    .unwrap();
            }
            let mut observers: Vec<_> = (0..16)
                .map(|n| {
                    let id = format!("reader-{history}-{n}");
                    let session =
                        store.test_session(&id, &crate::session_record::new_artifact_key());
                    store
                        .conn
                        .lock()
                        .unwrap()
                        .execute("UPDATE agent_sessions SET interactive=1 WHERE id=?1", [&id])
                        .unwrap();
                    store.bind_session(&id, session.captured, &task).unwrap();
                    (id, TaskUpdates::default())
                })
                .collect();
            for (id, observer) in &mut observers {
                assert!(observer.pending(&store, id).unwrap().is_none());
            }
            // Each conversation has its own receipt; consume all baselines.
            for (id, observer) in &mut observers {
                assert!(observer.pending(&store, id).unwrap().is_none());
            }
            observe(&store.conn.lock().unwrap());
            for kind in ["usage", "observed", "process", "activity", "task-title"] {
                let mut totals = Vec::new();
                for count in [1, 16] {
                    let mut total = crate::performance::testing::SqliteWork::default();
                    for tick_number in 0..10 {
                        {
                            let conn = store.conn.lock().unwrap();
                            if kind == "process" {
                                let id = LfProcessId::new();
                                conn.execute("INSERT INTO processes(id,trace_id,cwd,started_at,completed_at) VALUES(?1,?1,'/unrelated',1,2)", [&id]).unwrap();
                            } else if kind == "activity" {
                                conn.execute("UPDATE session_activity SET observed_at=observed_at+1,open_tools=1-open_tools", []).unwrap();
                            } else if kind == "task-title" {
                                conn.execute(
                                    "UPDATE tasks SET issue_title=issue_title||'.' WHERE id!=?1",
                                    [task.as_str()],
                                )
                                .unwrap();
                            } else {
                                conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('other',?1,?2,1,'{}')", rusqlite::params![kind, format!("{history}:{kind}:{count}:{tick_number}")]).unwrap();
                            }
                        }
                        let (_, work) = measure(|| {
                            for (id, observer) in &mut observers[..count] {
                                tick(observer);
                                assert!(observer.pending(&store, id).unwrap().is_none());
                            }
                        });
                        total.statements += work.statements;
                        total.rows += work.rows;
                        total.vm_steps += work.vm_steps;
                    }
                    eprintln!("history={history}, observers={count}, {kind}: {total:?}");
                    totals.push(total);
                }
                measurements.push((history, kind, totals));
            }
        }
        for (history, kind, totals) in measurements {
            assert!(totals[0].statements <= 10, "{history} {kind}: {totals:?}");
            assert_eq!(totals[0].rows, 10, "{history} {kind}: {totals:?}");
            assert!(totals[0].vm_steps <= 100, "{history} {kind}: {totals:?}");
            assert_eq!(totals[1].statements, totals[0].statements * 16);
            assert_eq!(totals[1].rows, totals[0].rows * 16);
            assert_eq!(totals[1].vm_steps, totals[0].vm_steps * 16);
        }
    }

    #[test]
    fn idle_task_updates_only_read_revisions_and_still_deliver_changes() {
        let (_home, store, task) = task_session();
        store.test_graph(
            "feature",
            "/task-updates",
            &[
                ("implement", Some("succeeded")),
                ("gate", Some("succeeded")),
            ],
            Some("succeeded"),
        );
        let mut updates = TaskUpdates::default();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        // Consume the baseline receipt's own revision.
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        observe(&store.conn.lock().unwrap());
        let (_, work) = measure(|| {
            for _ in 0..60 {
                tick(&mut updates);
                assert!(updates.pending(&store, "reader").unwrap().is_none());
            }
        });
        eprintln!("60 idle TaskUpdates polls: {work:?}");
        assert_eq!(
            work.statements, 60,
            "idle polls read only the revision counter"
        );
        assert_eq!(work.rows, 60);
        assert!(work.vm_steps <= 600);

        comment(
            &store,
            &task,
            "new direction",
            TaskCommentAuthor::Person {
                name: Some("Jack".into()),
            },
        );
        let update = updates.pending(&store, "reader").unwrap().unwrap();
        assert!(update.text.contains("new direction"));
        // A busy provider has not dispatched it yet; no database write is needed.
        tick(&mut updates);
        let (retried, work) = measure(|| updates.pending(&store, "reader").unwrap().unwrap());
        assert_eq!(retried.key, update.key);
        assert!(work.statements <= 20 && work.rows <= 25, "{work:?}");
        updates.read_at = Some(Instant::now() + Duration::from_secs(3600));
        let (_, work) = measure(|| {
            for _ in 0..60 {
                assert!(updates.pending(&store, "reader").unwrap().is_none());
            }
        });
        assert_eq!(
            work.statements, 60,
            "deferred delivery still respects its polling clock"
        );
        store
            .record_task_update("reader", &update.key, "dispatched", &update.observation)
            .unwrap();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        tick(&mut updates);
        let (pending, work) = measure(|| updates.pending(&store, "reader").unwrap());
        assert!(pending.is_none());
        assert_eq!(work.statements, 1);

        store.test_graph(
            "repair",
            "/task-updates",
            &[("gate", Some("failed"))],
            Some("failed"),
        );
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        settle(&mut updates);
        assert!(updates
            .pending(&store, "reader")
            .unwrap()
            .unwrap()
            .text
            .contains(": failed"));
    }

    #[test]
    fn removed_membership_and_graphs_release_settling_candidates() {
        for remove_membership in [false, true] {
            let (_home, store, task) = task_session();
            let retained = TaskId::new();
            store
                .create_task(&crate::work::task::NewTask {
                    repository_id: store.ensure_repository("/task-updates").unwrap(),
                    id: retained.clone(),
                    project_id: None,
                    title: "Retained".into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap();
            // Checkout membership can disappear while the explicit Task binding stays.
            store.conn.lock().unwrap().execute("UPDATE tasks SET worktree='/retained',checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) WHERE id=?1", [retained.as_str()]).unwrap();
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE agent_sessions SET cwd='/retained' WHERE id='reader'",
                    [],
                )
                .unwrap();
            let mut updates = TaskUpdates::default();
            assert!(updates.pending(&store, "reader").unwrap().is_none());
            let graph = store.test_graph(
                "new",
                "/retained",
                &[("gate", Some("succeeded"))],
                Some("succeeded"),
            );
            assert!(updates.pending(&store, "reader").unwrap().is_none());
            assert!(!updates.candidates.is_empty());
            if remove_membership {
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE agent_sessions SET cwd='/elsewhere' WHERE id='reader'",
                        [],
                    )
                    .unwrap();
                assert!(!store
                    .session_task_ids("reader")
                    .unwrap()
                    .contains(&retained));
            } else {
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "DELETE FROM graph_process_steps WHERE graph_lf_process_id=?1",
                        [graph.as_str()],
                    )
                    .unwrap();
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "DELETE FROM graph_processes WHERE lf_process_id=?1",
                        [graph.as_str()],
                    )
                    .unwrap();
            }
            assert!(updates.pending(&store, "reader").unwrap().is_none());
            observe(&store.conn.lock().unwrap());
            let (_, work) = measure(|| {
                for _ in 0..10 {
                    tick(&mut updates);
                    assert!(updates.pending(&store, "reader").unwrap().is_none());
                }
            });
            assert_eq!(
                work.statements, 10,
                "removed demand must recover revision-only polling"
            );
            comment(
                &store,
                &task,
                "still listening",
                TaskCommentAuthor::Person { name: None },
            );
            assert!(updates
                .pending(&store, "reader")
                .unwrap()
                .unwrap()
                .text
                .contains("still listening"));
        }
    }

    #[test]
    fn failed_partial_observation_preserves_candidates_until_retry() {
        let (_home, store, _task) = task_session();
        let mut updates = TaskUpdates::default();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        let graph = store.test_graph(
            "new",
            "/task-updates",
            &[("gate", Some("succeeded"))],
            Some("succeeded"),
        );
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        let candidates = updates.candidates.clone();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE processes SET outcome='failed' WHERE id=?1",
                [graph.as_str()],
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute("ALTER TABLE task_comments RENAME TO held_comments", [])
            .unwrap();
        assert!(updates.pending(&store, "reader").is_err());
        assert_eq!(updates.candidates, candidates);
        store
            .conn
            .lock()
            .unwrap()
            .execute("ALTER TABLE held_comments RENAME TO task_comments", [])
            .unwrap();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        settle(&mut updates);
        assert!(updates
            .pending(&store, "reader")
            .unwrap()
            .unwrap()
            .text
            .contains("failed"));
    }

    #[test]
    fn ineligible_and_missing_sessions_release_candidates() {
        for change in [
            "UPDATE agent_sessions SET interactive=0 WHERE id='reader'",
            "UPDATE agent_sessions SET completed_at=20 WHERE id='reader'",
            "",
        ] {
            let (_home, store, _task) = task_session();
            let mut updates = TaskUpdates::default();
            assert!(updates.pending(&store, "reader").unwrap().is_none());
            store.test_graph(
                "new",
                "/task-updates",
                &[("gate", Some("succeeded"))],
                Some("succeeded"),
            );
            assert!(updates.pending(&store, "reader").unwrap().is_none());
            assert!(!updates.candidates.is_empty());
            if !change.is_empty() {
                store.conn.lock().unwrap().execute(change, []).unwrap();
            }
            let session = if change.is_empty() {
                "missing"
            } else {
                "reader"
            };
            assert!(updates.pending(&store, session).unwrap().is_none());
            observe(&store.conn.lock().unwrap());
            let (_, work) = measure(|| {
                tick(&mut updates);
                assert!(updates.pending(&store, session).unwrap().is_none());
            });
            assert_eq!(work.statements, 1);
        }
    }

    #[test]
    fn live_runs_are_polled_without_store_writes() {
        let (_home, store, _task) = task_session();
        store.test_graph("feature", "/task-updates", &[("gate", None)], None);
        let mut updates = TaskUpdates::default();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        observe(&store.conn.lock().unwrap());
        tick(&mut updates);
        let (pending, work) = measure(|| updates.pending(&store, "reader").unwrap());
        assert!(pending.is_none());
        assert!(work.statements > 1 && work.statements <= 16, "{work:?}");
        updates.read_at = Some(Instant::now() + Duration::from_secs(3600));
        let (_, work) = measure(|| {
            for _ in 0..60 {
                assert!(updates.pending(&store, "reader").unwrap().is_none());
            }
        });
        assert_eq!(
            work.statements, 60,
            "live fallback is throttled between deadlines"
        );
    }

    #[test]
    fn live_task_run_exit_is_observed_without_a_store_write() {
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let (_home, store, task) = task_session();
        let mut child = Child(
            std::process::Command::new("/bin/sleep")
                .arg("60")
                .spawn()
                .unwrap(),
        );
        let run = store.test_graph("feature", "/task-updates", &[("gate", None)], None);
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE processes SET pid=?2,os_started_at=?3 WHERE id=?1",
                rusqlite::params![
                    run,
                    child.0.id(),
                    crate::journal::process_started_at(child.0.id())
                        .unwrap()
                        .unwrap()
                ],
            )
            .unwrap();
        store
            .take_up_task_graph(
                &task,
                &crate::task_graph::CapturedGraph {
                    name: "feature".into(),
                    steps: Vec::new(),
                },
            )
            .unwrap();
        let row = store.task_graph_row(&task).unwrap().unwrap();
        store
            .start_task_graph_run(
                &task,
                &row,
                &crate::graph::runner::ExecutionCursor::default(),
                &run,
                false,
            )
            .unwrap();
        let mut updates = TaskUpdates::default();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        let revision = store.task_update_revision().unwrap();
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        tick(&mut updates);
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        settle(&mut updates);
        let update = updates.pending(&store, "reader").unwrap().unwrap();
        assert!(
            update.text.contains("Stopped from start"),
            "{}",
            update.text
        );
        assert_eq!(store.task_update_revision().unwrap(), revision);
    }

    #[test]
    fn failed_task_update_reads_retry_without_a_revision_change() {
        let (_home, store, _task) = task_session();
        store
            .record_task_update("reader", "bad", "baseline", &serde_json::json!([]))
            .unwrap();
        let mut updates = TaskUpdates::default();
        observe(&store.conn.lock().unwrap());
        let (_, work) = measure(|| {
            for _ in 0..10 {
                assert!(updates.pending(&store, "reader").is_err());
            }
        });
        assert_eq!(
            work.statements, 30,
            "failed reads retry without rebuilding Task history"
        );
        assert_eq!(work.rows, 30);
        // Repair the stored payload without advancing the observation revision:
        // a transient decoding failure must not poison this reader's cache.
        let revision = store.task_update_revision().unwrap();
        store.conn.lock().unwrap().execute("UPDATE session_events SET payload=json_set(payload,'$.observation',json('{}')) WHERE session_id='reader' AND receipt_key LIKE 'task_update:%'", []).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE store_revisions SET revision=?1 WHERE domain='task_updates'",
                [revision],
            )
            .unwrap();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
    }

    #[test]
    fn task_update_revision_follows_reportable_changes_and_rollbacks() {
        let (_home, store, task) = task_session();
        let graph = store.test_graph("feature", "/task-updates", &[("gate", None)], None);
        let quiet = store.task_update_revision().unwrap();
        for sql in [
            "UPDATE agent_sessions SET title='renamed' WHERE id='reader'",
            "UPDATE tasks SET issue_title='renamed'",
            "INSERT INTO session_activity(session_id,observed_at,open_tools,pending_input,yielded) VALUES('reader',1,1,0,0)",
            "UPDATE session_activity SET observed_at=2,open_tools=0",
        ] {
            store.conn.lock().unwrap().execute(sql, []).unwrap();
            assert_eq!(store.task_update_revision().unwrap(), quiet, "{sql}");
        }
        for sql in [
            "UPDATE agent_sessions SET interactive=0 WHERE id='reader'",
            "UPDATE agent_sessions SET interactive=1 WHERE id='reader'",
            "UPDATE agent_sessions SET cwd='/elsewhere' WHERE id='reader'",
            "UPDATE tasks SET worktree='/moved'",
            "UPDATE processes SET outcome='failed',completed_at=20 WHERE id IN (SELECT lf_process_id FROM graph_process_steps)",
            "UPDATE processes SET outcome='failed',completed_at=20 WHERE id IN (SELECT lf_process_id FROM graph_processes)",
            "INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload) SELECT 'reader','observed','graph-link',lf_process_id,1,'{}' FROM graph_processes",
            "DELETE FROM session_events WHERE receipt_key='graph-link'",
        ] {
            let before = store.task_update_revision().unwrap();
            store.conn.lock().unwrap().execute(sql, []).unwrap();
            assert!(store.task_update_revision().unwrap() > before, "{sql}");
        }
        let before = store.task_update_revision().unwrap();
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute(
                "UPDATE processes SET outcome='succeeded' WHERE id=?1",
                [&graph],
            )
            .unwrap();
            tx.rollback().unwrap();
        }
        assert_eq!(store.task_update_revision().unwrap(), before);
        comment(
            &store,
            &task,
            "new",
            TaskCommentAuthor::Person { name: None },
        );
        assert!(store.task_update_revision().unwrap() > before);
        let before = store.task_update_revision().unwrap();
        store
            .record_task_update("reader", "late", "accepted", &serde_json::json!({}))
            .unwrap();
        assert_eq!(
            store.task_update_revision().unwrap(),
            before,
            "acknowledgements cannot change the last dispatched reading"
        );
    }

    #[test]
    fn task_update_receipt_admits_one_concurrent_observer() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("reader", &crate::session_record::new_artifact_key());
        store
            .record_task_update("reader", "initial", "baseline", &serde_json::json!({}))
            .unwrap();
        assert!(!store
            .record_task_update("reader", "initial", "dispatched", &serde_json::json!({}))
            .unwrap());

        let reconnected = SqliteStore::new(&path).unwrap();
        let ready = Arc::new(Barrier::new(2));
        let other_ready = ready.clone();
        let observer = std::thread::spawn(move || {
            other_ready.wait();
            reconnected
                .record_task_update(
                    "reader",
                    "transition",
                    "dispatched",
                    &serde_json::json!({"run":"failed"}),
                )
                .unwrap()
        });
        ready.wait();
        let sent = store
            .record_task_update(
                "reader",
                "transition",
                "dispatched",
                &serde_json::json!({"run":"failed"}),
            )
            .unwrap();
        assert_ne!(
            sent,
            observer.join().unwrap(),
            "exactly one observer may send"
        );
        assert_eq!(
            store.latest_task_update("reader").unwrap(),
            Some(serde_json::json!({"run":"failed"}))
        );
        assert_eq!(
            store
                .session_history("reader", 0, 0)
                .unwrap()
                .iter()
                .filter(|event| event.payload["key"] == "transition")
                .count(),
            1
        );
    }

    #[test]
    fn task_updates_coalesce_exits_and_starts_and_do_not_replay_uncertain_delivery() {
        let (_home, store, _task) = task_session();
        let mut updates = TaskUpdates::default();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        let graph = store.test_graph(
            "feature",
            "/task-updates",
            &[("implement", Some("succeeded"))],
            None,
        );
        assert!(
            updates.pending(&store, "reader").unwrap().is_none(),
            "between-step commits are silent"
        );
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE processes SET started_at=1,completed_at=20,outcome='failed' WHERE id=?1",
                [graph.as_str()],
            )
            .unwrap();
        assert!(
            updates.pending(&store, "reader").unwrap().is_none(),
            "wait for quiet"
        );
        settle(&mut updates);
        let update = updates.pending(&store, "reader").unwrap().unwrap();
        assert!(update.text.contains("implement ("));
        assert!(update.text.contains(": succeeded"));
        assert!(update.text.contains("not a request or permission"));
        store
            .record_task_update("reader", &update.key, "dispatched", &update.observation)
            .unwrap();
        let mut reconnected = TaskUpdates::default();
        assert!(
            reconnected.pending(&store, "reader").unwrap().is_none(),
            "unknown delivery must not replay"
        );
        store.test_graph(
            "repair",
            "/task-updates",
            &[("gate", Some("failed"))],
            Some("failed"),
        );
        assert!(reconnected.pending(&store, "reader").unwrap().is_none());
        settle(&mut reconnected);
        let next = reconnected.pending(&store, "reader").unwrap().unwrap();
        assert!(next.text.contains("gate ("));
        assert!(next.text.contains(": failed"));
        store
            .record_task_update("reader", &next.key, "dispatched", &next.observation)
            .unwrap();
        store
            .record_task_update("reader", &update.key, "accepted", &update.observation)
            .unwrap();
        assert!(
            TaskUpdates::default()
                .pending(&store, "reader")
                .unwrap()
                .is_none(),
            "a late acknowledgement cannot rewind the dispatched reading"
        );
    }

    fn task_session() -> (tempfile::TempDir, SqliteStore, TaskId) {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("reader", &crate::session_record::new_artifact_key());
        let task = TaskId::new();
        store
            .create_task(&crate::work::task::NewTask {
                repository_id: store.ensure_repository("/task-updates").unwrap(),
                id: task.clone(),
                project_id: None,
                title: "Updates".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store.conn.lock().unwrap().execute("UPDATE tasks SET worktree='/task-updates',checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) WHERE id=?1", [task.as_str()]).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=1 WHERE id='reader'",
                [],
            )
            .unwrap();
        store
            .bind_session("reader", session.captured, &task)
            .unwrap();
        (home, store, task)
    }

    fn finish_run(store: &SqliteStore, run: &LfProcessId, outcome: &str) {
        store.conn.lock().unwrap().execute(
            "UPDATE processes SET completed_at=20,outcome=?2 WHERE id=?1 OR parent_lf_process_id=?1",
            rusqlite::params![run, outcome],
        ).unwrap();
    }

    fn finish_step(store: &SqliteStore, run: &LfProcessId) {
        store.conn.lock().unwrap().execute(
            "UPDATE processes SET completed_at=20,outcome='succeeded' WHERE parent_lf_process_id=?1",
            [run],
        ).unwrap();
    }

    fn next_step(store: &SqliteStore, run: &LfProcessId, key: u32) -> LfProcessId {
        let graph = store.graph_process(run.as_str()).unwrap().unwrap().0;
        let label = &graph.compiled.node_at(key).unwrap().label;
        let step = LfProcessId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,parent_lf_process_id,command,cwd,started_at)
             VALUES(?1,?1,?2,?3,'/task-updates',21)",
                rusqlite::params![step, run, serde_json::json!(["lf", label]).to_string()],
            )
            .unwrap();
        store.record_graph_step(run, &step, key, &[vec![]]).unwrap();
        step
    }

    #[test]
    fn task_updates_report_older_failure_independently_of_newer_run() {
        for newer_state in ["unchanged", "between_steps", "unsettled"] {
            let (_home, store, _task) = task_session();
            let older = store.test_graph("feature", "/task-updates", &[("gate", None)], None);
            let newer = store.test_graph("docs", "/task-updates", &[("write", None)], None);
            let mut updates = TaskUpdates::default();
            assert!(updates.pending(&store, "reader").unwrap().is_none());
            finish_run(&store, &older, "failed");
            assert!(updates.pending(&store, "reader").unwrap().is_none());
            settle(&mut updates);
            match newer_state {
                "between_steps" => finish_step(&store, &newer),
                "unsettled" => finish_run(&store, &newer, "succeeded"),
                _ => {}
            }
            let update = updates.pending(&store, "reader").unwrap().unwrap();
            assert!(
                update
                    .text
                    .contains(&format!("Run feature ({older}) · failed")),
                "{newer_state}: {}",
                update.text
            );
            assert!(update.text.contains("gate ("));
            assert!(
                !update.text.contains(newer.as_str()),
                "unchanged or unsettled runs aren't announced"
            );
            store
                .record_task_update("reader", &update.key, "dispatched", &update.observation)
                .unwrap();
            assert!(
                TaskUpdates::default()
                    .pending(&store, "reader")
                    .unwrap()
                    .is_none(),
                "uncertain delivery is suppressed"
            );
        }
    }

    #[test]
    fn task_updates_reconnect_retains_gap_and_reports_older_runs_next_change() {
        let (home, store, _task) = task_session();
        let older = store.test_graph("feature", "/task-updates", &[("implement", None)], None);
        let newer = store.test_graph("docs", "/task-updates", &[("write", None)], None);
        let mut updates = TaskUpdates::default();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        finish_step(&store, &older);
        assert!(
            updates.pending(&store, "reader").unwrap().is_none(),
            "step exit/start gap is silent"
        );
        // Repeated labels are distinct occurrences, identified by Process id.
        let next = next_step(&store, &older, 0);
        finish_step(&store, &newer);
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        settle(&mut updates);
        let update = updates.pending(&store, "reader").unwrap().unwrap();
        assert!(update
            .text
            .contains(&format!("implement ({next}): running")));
        assert!(!update.text.contains(newer.as_str()));
        store
            .record_task_update("reader", &update.key, "dispatched", &update.observation)
            .unwrap();
        drop(store);
        let store = SqliteStore::new(&home.path().join("db")).unwrap();
        let mut reconnected = TaskUpdates::default();
        assert!(reconnected.pending(&store, "reader").unwrap().is_none());
        finish_run(&store, &older, "failed");
        assert!(reconnected.pending(&store, "reader").unwrap().is_none());
        settle(&mut reconnected);
        let failed = reconnected.pending(&store, "reader").unwrap().unwrap();
        assert!(failed
            .text
            .contains(&format!("Run feature ({older}) · failed")));
        assert!(!failed.text.contains(newer.as_str()));
        store
            .record_task_update("reader", &failed.key, "dispatched", &failed.observation)
            .unwrap();
        store
            .record_task_update("reader", &update.key, "accepted", &update.observation)
            .unwrap();
        assert!(
            TaskUpdates::default()
                .pending(&store, "reader")
                .unwrap()
                .is_none(),
            "late acknowledgement cannot rewind observations"
        );
        // B's gap was never consumed by either report about A, even across reopen.
        let step = next_step(&store, &newer, 0);
        assert!(reconnected.pending(&store, "reader").unwrap().is_none());
        settle(&mut reconnected);
        let resumed = reconnected.pending(&store, "reader").unwrap().unwrap();
        assert!(resumed
            .text
            .contains(&format!("Run docs ({newer}) · running")));
        assert!(resumed.text.contains(&format!("write ({step}): running")));
        assert!(!resumed.text.contains(older.as_str()));
    }

    #[test]
    fn task_updates_coalesce_while_busy_and_report_review_position_separately() {
        let (_home, store, task) = task_session();
        store
            .take_up_task_graph(
                &task,
                &crate::task_graph::CapturedGraph {
                    name: "feature".into(),
                    steps: Vec::new(),
                },
            )
            .unwrap();
        let older = store.test_graph("feature", "/task-updates", &[("gate", None)], None);
        let mut updates = TaskUpdates::default();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        finish_step(&store, &older);
        let next = next_step(&store, &older, 0);
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        settle(&mut updates);
        let busy = updates.pending(&store, "reader").unwrap().unwrap();
        assert!(busy.text.contains(&format!("gate ({next}): running")));
        // Native owners do not dispatch while busy. Only the latest reading survives.
        finish_run(&store, &older, "succeeded");
        store.set_task_graph_step(&task, "demo").unwrap();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        settle(&mut updates);
        let available = updates.pending(&store, "reader").unwrap().unwrap();
        assert!(available.text.contains("At demo · available for review"));
        assert!(available
            .text
            .contains(&format!("Run feature ({older}) · succeeded")));
        assert!(!available.text.contains(": running"));
        store
            .record_task_update(
                "reader",
                &available.key,
                "dispatched",
                &available.observation,
            )
            .unwrap();
        store.set_task_graph_step(&task, "end").unwrap();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        settle(&mut updates);
        let at_end = updates.pending(&store, "reader").unwrap().unwrap();
        assert!(at_end.text.contains("At end (Task completion is separate)"));
        assert!(
            !at_end.text.contains(older.as_str()),
            "unchanged history isn't repeated for position-only changes"
        );
    }

    fn comment(store: &SqliteStore, task: &TaskId, body: &str, author: TaskCommentAuthor) {
        store
            .append_task_comment(
                task,
                &TaskComment {
                    id: uuid::Uuid::new_v4().to_string(),
                    body: body.into(),
                    author,
                    created_at: Some("2026-11-07T22:00:00Z".into()),
                },
            )
            .unwrap();
    }

    #[test]
    fn comments_after_launch_reach_the_conversation_but_its_own_do_not() {
        let (_home, store, task) = task_session();
        let person = || TaskCommentAuthor::Person {
            name: Some("jack".into()),
        };
        comment(&store, &task, "already in the prompt", person());
        let mut updates = TaskUpdates::default();
        assert!(updates.pending(&store, "reader").unwrap().is_none());
        comment(
            &store,
            &task,
            "the conversation's own progress\n<!-- loopflow-session: reader -->",
            TaskCommentAuthor::Session {
                session_id: "reader".into(),
                requester: None,
            },
        );
        assert!(
            updates.pending(&store, "reader").unwrap().is_none(),
            "a conversation is not told about its own comment"
        );
        comment(&store, &task, "use the staging bucket", person());
        let update = updates.pending(&store, "reader").unwrap().unwrap();
        assert!(update
            .text
            .contains("New comment from jack: use the staging bucket"));
        assert!(!update.text.contains("already in the prompt"));
        assert!(!update.text.contains("own progress"));
    }

    #[test]
    fn worker_runs_are_never_recipients() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        store.test_session("worker", &crate::session_record::new_artifact_key());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=0 WHERE id='worker'",
                [],
            )
            .unwrap();
        let before = store.session_history("worker", 0, 0).unwrap();
        assert!(TaskUpdates::default()
            .pending(&store, "worker")
            .unwrap()
            .is_none());
        assert_eq!(store.session_history("worker", 0, 0).unwrap(), before);
    }
}
