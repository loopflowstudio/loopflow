//! Native Session updates read existing Task/Graph facts. Receipts live in the
//! Session history; they are delivery evidence, never another workflow cursor.
use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::durable::{GraphStepProcess, TaskId};
use crate::id::LfProcessId;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};
use crate::task_graph::TaskGraphPosition;

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
    revision: Option<crate::store::sqlite::StoreRevisions>,
    read_at: Option<Instant>,
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
        let mut revision = store.revisions()?;
        revision.usage = 0;
        if self.revision == Some(revision)
            && self.candidates.is_empty()
            && self
                .read_at
                .is_some_and(|at| at.elapsed() < Duration::from_secs(2))
        {
            return Ok(None);
        }
        self.revision = Some(revision);
        self.read_at = Some(Instant::now());
        let Some(session) = store.session(session_id)? else {
            return Ok(None);
        };
        if !session.interactive || session.completed_at.is_some() {
            return Ok(None);
        }
        let previous = store.latest_task_update(session_id)?;
        let baseline = previous.is_none();
        let previous: HashMap<TaskId, TaskObservation> = previous
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();
        let mut facts = HashMap::new();
        let mut lines = Vec::new();
        for task in store.session_task_ids(session_id)? {
            let (overview, graphs) = store.task_overview(&task)?;
            let old = previous.get(&task);
            let position = overview.graph.map(|graph| graph.position);
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
            };
            let mut changes = Vec::new();
            for graph in graphs {
                let id = LfProcessId::parse(&graph.entry.summary.id)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
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
                    outcome: store.process(&id)?.and_then(|process| process.outcome),
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
            if position_changed || !changes.is_empty() {
                let identifier = store
                    .task(&task)?
                    .map(|task| task.identifier)
                    .unwrap_or_else(|| task.to_string());
                lines.push(format!(
                    "Task {identifier}: {}. {}",
                    position_text(observation.position.as_ref()),
                    changes.join(" ")
                ));
            }
            facts.insert(task, observation);
        }
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
    let mut updates = TaskUpdates::default();
    loop {
        if crate::journal::process_evidence(&store, &adapter.agent_process_id)
            == crate::journal::ProcessIdentityEvidence::Dead
        {
            return Ok(());
        }
        let pending = store.with_provider_context(&adapter, thread, NativeBoundary::Turn, || {
            updates.pending(&store, &adapter.session)
        });
        let pending = match pending {
            Ok(pending) => pending,
            Err(StoreError::InvalidAuthority(_)) => return Ok(()),
            Err(error) => {
                tracing::warn!(%error, "native Task updates unavailable; polling again");
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
                Err(error) => tracing::warn!(%error, "native Task update delivery unavailable"),
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
    use crate::store::sqlite::SqliteStore;
    use std::sync::{Arc, Barrier};
    use std::time::{Duration, Instant};

    fn settle(updates: &mut TaskUpdates) {
        for (_, since) in updates.candidates.values_mut() {
            *since = Instant::now() - Duration::from_secs(2);
        }
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
