//! Native Session updates read existing Task/Graph facts. Receipts live in the
//! Session history; they are delivery evidence, never another workflow cursor.
use std::time::{Duration, Instant};

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};
use crate::task_graph::TaskGraphPosition;

#[derive(Debug, Clone)]
pub(crate) struct TaskUpdate {
    pub key: String,
    pub text: String,
}

#[derive(Debug, Default)]
pub(crate) struct TaskUpdates {
    candidate: Option<(String, Instant)>,
    revision: Option<crate::store::sqlite::StoreRevisions>,
    read_at: Option<Instant>,
}

impl TaskUpdates {
    /// Read on the native owner's clock. Worker Runs never receive updates.
    /// Initial history is a baseline; subsequent opens recover the same cursor.
    pub(crate) fn pending(
        &mut self,
        store: &SqliteStore,
        session_id: &str,
    ) -> StoreResult<Option<TaskUpdate>> {
        let mut revision = store.revisions()?;
        revision.usage = 0;
        if self.revision == Some(revision)
            && self.candidate.is_none()
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
        let mut facts = Vec::new();
        let mut lines = Vec::new();
        for task in store.session_task_ids(session_id)? {
            let (overview, graphs) = store.task_overview(&task)?;
            let Some(latest) = graphs.last() else {
                continue;
            };
            // Exit and next start are separate commits. Do not announce their gap.
            if latest.entry.ended_at.is_none()
                && latest
                    .steps
                    .last()
                    .is_some_and(|step| step.completed_at.is_some())
            {
                return Ok(None);
            }
            facts.push(json!({"task": task, "graph": overview.graph, "runs": graphs}));
            let position = overview.graph.as_ref().map(|graph| match &graph.position {
                TaskGraphPosition::At { step } if step == "end" => {
                    "At end (Task completion is separate)".to_owned()
                }
                TaskGraphPosition::At { step } if step == "start" => "At start".to_owned(),
                TaskGraphPosition::At { step } => format!("At {step} · available for review"),
                TaskGraphPosition::Run { step, running, .. } => format!(
                    "{} from {step}",
                    if *running { "Running" } else { "Stopped" }
                ),
            });
            let identifier = store
                .task(&task)?
                .map(|task| task.identifier)
                .unwrap_or_else(|| task.to_string());
            let outcome = store
                .process(
                    &crate::id::LfProcessId::parse(&latest.entry.summary.id)
                        .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                )?
                .and_then(|process| process.outcome)
                .unwrap_or_else(|| "running".into());
            let last = latest.steps.last();
            let status = last
                .map(|step| {
                    format!(
                        "{}: {}",
                        step.label,
                        step.outcome.as_deref().unwrap_or("running")
                    )
                })
                .unwrap_or_else(|| "starting".into());
            lines.push(format!(
                "Task {identifier}: {}. Latest run {} ({}) · {outcome} · {status}.",
                position.unwrap_or_else(|| "No Task workflow selected".into()),
                latest.entry.summary.name,
                latest.entry.summary.id
            ));
        }
        let key = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&facts).map_err(|e| StoreError::InvalidData(e.to_string()))?
            )
        );
        let previous = store.latest_task_update(session_id)?;
        if previous.is_none() {
            store.record_task_update(session_id, &key, "baseline")?;
            return Ok(None);
        }
        if previous.as_ref() == Some(&key) {
            self.candidate = None;
            return Ok(None);
        }
        match &self.candidate {
            Some((candidate, since)) if candidate == &key => {
                if since.elapsed() < Duration::from_secs(1) {
                    return Ok(None);
                }
            }
            _ => {
                self.candidate = Some((key, Instant::now()));
                return Ok(None);
            }
        }
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(TaskUpdate { key, text: format!(
            "<lf:task-update>\n{}\nReport this recorded progress briefly. This is observation, not a request or permission to launch work, advance review, publish or merge. Read current Task status before acting. Diffs belong in Files.\n</lf:task-update>", lines.join("\n")) }))
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
            Err(error) => return Err(error.into()),
        };
        if let Some(update) = pending {
            // Busy Sessions coalesce changes into their latest reading. The
            // provider's next-priority queue handles a turn starting after this read.
            let available = store
                .session_summary(
                    &adapter.session,
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                )?
                .is_some_and(|session| session.available);
            if available {
                match deliver(&store, &adapter, thread, &update) {
                    Ok(()) => {}
                    Err(StoreError::InvalidAuthority(_)) => return Ok(()),
                    Err(error) => tracing::warn!(%error, "native Task update delivery unavailable"),
                }
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::TaskUpdates;
    use crate::durable::TaskId;
    use crate::store::sqlite::SqliteStore;
    use std::sync::{Arc, Barrier};
    use std::time::{Duration, Instant};

    #[test]
    fn task_update_receipt_admits_one_concurrent_observer() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("reader", &crate::session_record::new_artifact_key());
        store
            .record_task_update("reader", "initial", "baseline")
            .unwrap();
        assert!(!store
            .record_task_update("reader", "initial", "dispatched")
            .unwrap());

        let reconnected = SqliteStore::new(&path).unwrap();
        let ready = Arc::new(Barrier::new(2));
        let other_ready = ready.clone();
        let observer = std::thread::spawn(move || {
            other_ready.wait();
            reconnected
                .record_task_update("reader", "transition", "dispatched")
                .unwrap()
        });
        ready.wait();
        let sent = store
            .record_task_update("reader", "transition", "dispatched")
            .unwrap();
        assert_ne!(
            sent,
            observer.join().unwrap(),
            "exactly one observer may send"
        );
        assert_eq!(
            store.latest_task_update("reader").unwrap().as_deref(),
            Some("transition")
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
        store.conn.lock().unwrap().execute("UPDATE tasks SET worktree='/task-updates',checkout_machine_id=(SELECT id FROM machines WHERE route='local') WHERE id=?1", [task.as_str()]).unwrap();
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
        updates.candidate.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(2);
        let update = updates.pending(&store, "reader").unwrap().unwrap();
        assert!(update.text.contains("implement: succeeded"));
        assert!(update.text.contains("not a request or permission"));
        store
            .record_task_update("reader", &update.key, "dispatched")
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
        reconnected.candidate.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(2);
        let next = reconnected.pending(&store, "reader").unwrap().unwrap();
        assert!(next.text.contains("gate: failed"));
        store
            .record_task_update("reader", &next.key, "dispatched")
            .unwrap();
        store
            .record_task_update("reader", &update.key, "accepted")
            .unwrap();
        assert!(
            TaskUpdates::default()
                .pending(&store, "reader")
                .unwrap()
                .is_none(),
            "a late acknowledgement cannot rewind the dispatched reading"
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
