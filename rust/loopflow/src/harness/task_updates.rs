//! Task updates: what changed on a Task since its last reading, saved as
//! steers its interactive conversations read. No writer sits at each fact:
//! whoever looks first (a conversation's hook, or the Machine server waking an
//! idle one) compares the Task with its last reading and writes what moved,
//! so an exit nobody recorded is still reported.
use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::durable::{GraphStepProcess, TaskId};
use crate::id::LfProcessId;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};
use crate::task_graph::TaskGraphPosition;
use crate::work::task::comment::TaskCommentAuthor;

/// Run and position changes share one steer; its complete source is on disk.
const UPDATE_BYTES: usize = 4_000;

/// The `task_updates` revision at which a Task's last reading found nothing
/// new, by database and Task. Rebuilding Task history costs tens of
/// milliseconds; until a Task change moves the revision the answer is the
/// same. Per process: the server keeps it, a hook starts without it.
type QuietKey = (String, TaskId);
static QUIET_AT: Mutex<Option<HashMap<QuietKey, i64>>> = Mutex::new(None);

fn quiet_key(store: &SqliteStore, task: &TaskId) -> Option<QuietKey> {
    let conn = store.conn.lock().expect("store mutex poisoned");
    let path = conn.path().filter(|path| !path.is_empty())?.to_string();
    Some((path, task.clone()))
}

fn quiet_at(key: Option<&QuietKey>) -> Option<i64> {
    QUIET_AT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()?
        .get(key?)
        .copied()
}

fn set_quiet_at(key: Option<QuietKey>, revision: i64) {
    let Some(key) = key else {
        return;
    };
    QUIET_AT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert(key, revision);
}

// Keep only reportable facts, not compiled graphs or their full step histories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TaskObservation {
    position: Option<TaskGraphPosition>,
    runs: HashMap<LfProcessId, RunObservation>,
    /// Comments that are not direction, by ID.
    comments: BTreeSet<String>,
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

/// Write what changed on the Session's Tasks since their last readings as
/// Task steers. Only an interactive conversation reads them, so only one
/// looks; a worker Run pays nothing.
pub(crate) fn write(store: &SqliteStore, session_id: &str) -> StoreResult<()> {
    let Some(session) = store.session(session_id)? else {
        return Ok(());
    };
    if !session.interactive || session.completed_at.is_some() {
        return Ok(());
    }
    let revision = store.task_update_revision()?;
    for task in store.session_task_ids(session_id)? {
        let quiet = quiet_key(store, &task);
        if quiet_at(quiet.as_ref()) == Some(revision) {
            continue;
        }
        write_task(store, &task)?;
        set_quiet_at(quiet, revision);
    }
    Ok(())
}

/// The Task's first reading is a baseline: its history is in every prompt
/// already. After that, one steer says how its position and runs moved, and
/// one per comment that is not direction, named for the Session that wrote it
/// so that Session is not told.
fn write_task(store: &SqliteStore, task: &TaskId) -> StoreResult<()> {
    let stored = store.task_update_reading(task)?;
    let old: Option<TaskObservation> = stored.as_deref().map(serde_json::from_str).transpose()?;
    let (graph, graphs) = store.task_progress(task)?;
    let mut observation = TaskObservation {
        position: graph.map(|graph| graph.position),
        runs: HashMap::new(),
        comments: BTreeSet::new(),
    };
    let mut changes = Vec::new();
    for graph in graphs {
        let id = LfProcessId::parse(&graph.entry.summary.id)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let prior = old.as_ref().and_then(|old| old.runs.get(&id));
        // Exit and next start are separate commits. Keep this run's last
        // reported facts until its next step or terminal result.
        if graph.entry.ended_at.is_none()
            && graph
                .steps
                .last()
                .is_some_and(|step| step.completed_at.is_some())
        {
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
        if prior != Some(&run) {
            changes.push(run.report(&id));
        }
        observation.runs.insert(id, run);
    }
    let comments = store.task_undirected_comments(task)?;
    observation.comments = comments.iter().map(|comment| comment.id.clone()).collect();
    if old.as_ref() == Some(&observation) {
        return Ok(());
    }
    let reading = serde_json::to_string(&observation)?;
    let mut updates = Vec::new();
    if let Some(old) = &old {
        let identifier = store
            .task(task)?
            .map(|task| task.identifier)
            .unwrap_or_else(|| task.to_string());
        if old.position != observation.position || !changes.is_empty() {
            let mut text = format!(
                "Task {identifier}: {}.",
                position_text(observation.position.as_ref())
            );
            for (shown, change) in changes.iter().enumerate() {
                if text.len() + change.len() + 1 > UPDATE_BYTES {
                    text.push_str(&format!(
                        " {} more changes; read `lf task status` for them.",
                        changes.len() - shown
                    ));
                    break;
                }
                text.push(' ');
                text.push_str(change);
            }
            let key = format!("{:x}", Sha256::digest(reading.as_bytes()));
            updates.push((format!("update:-:{task}:{key}"), text));
        }
        for comment in &comments {
            if old.comments.contains(&comment.id) {
                continue;
            }
            let writer = match &comment.author {
                TaskCommentAuthor::Session { session_id, .. } => session_id,
                _ => "-",
            };
            updates.push((
                format!("update:{writer}:{}", comment.id),
                format!(
                    "Task {identifier}: new comment from {}: {}",
                    comment.author, comment.body
                ),
            ));
        }
    }
    // Another reader that wrote first reported the same change.
    store.save_task_update(task, stored.as_deref(), &reading, &updates)
}

fn position_text(position: Option<&TaskGraphPosition>) -> String {
    match position {
        Some(TaskGraphPosition::At { step }) if step == "end" => {
            "At end (Task completion is separate)".into()
        }
        Some(TaskGraphPosition::At { step }) if step == "start" => "At start".into(),
        Some(TaskGraphPosition::At { step }) => format!("At {step} · available for review"),
        Some(TaskGraphPosition::Run { at, running, .. }) => {
            format!("{} at {at}", if *running { "Running" } else { "Stopped" })
        }
        None => "No Task workflow selected".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::write;
    use crate::durable::TaskId;
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;
    use crate::work::task::comment::{TaskComment, TaskCommentAuthor};

    /// The updates `session` has not read, after a reading.
    fn pending(store: &SqliteStore, session: &str) -> Vec<String> {
        write(store, session).unwrap();
        store
            .session_steers(session, 0, 100)
            .unwrap()
            .items
            .into_iter()
            .filter(|row| row.delivered_at.is_none() && row.steer.source.starts_with("update:"))
            .map(|row| row.steer.text)
            .collect()
    }

    fn quiet(store: &SqliteStore) -> bool {
        pending(store, "reader").is_empty()
    }

    /// Read the reader's pending updates and mark them read.
    fn deliver(store: &SqliteStore) -> String {
        let text = pending(store, "reader").join("\n");
        assert!(!text.is_empty(), "an update");
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO steer_delivered(session_id,steer_id,claimed_at,delivered_at)
                 SELECT 'reader',id,1,1 FROM steers WHERE source LIKE 'update:%'
                 ON CONFLICT DO NOTHING",
                [],
            )
            .unwrap();
        text
    }

    #[test]
    fn a_finished_run_reaches_the_next_boundary_once() {
        let (_home, store, _task) = task_session();
        assert!(quiet(&store), "baseline");
        let graph = store.test_graph(
            "feature",
            "/task-updates",
            &[("implement", Some("succeeded"))],
            None,
        );
        assert!(quiet(&store), "between-step commits are silent");
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE processes SET started_at=1,completed_at=20,outcome='failed' WHERE id=?1",
                [graph.as_str()],
            )
            .unwrap();
        let text = deliver(&store);
        assert!(text.contains(&format!("Run feature ({graph}) · failed")));
        assert!(text.contains("implement ("));
        assert!(text.contains(": succeeded"));
        assert!(quiet(&store), "a delivered reading is not repeated");
        store.test_graph(
            "repair",
            "/task-updates",
            &[("gate", Some("failed"))],
            Some("failed"),
        );
        let next = deliver(&store);
        assert!(next.contains("gate ("));
        assert!(next.contains(": failed"));
        assert!(!next.contains(graph.as_str()), "unchanged runs stay quiet");
    }

    #[test]
    fn a_run_between_steps_keeps_its_last_report() {
        let (_home, store, _task) = task_session();
        let run = store.test_graph("feature", "/task-updates", &[("implement", None)], None);
        assert!(quiet(&store));
        finish_step(&store, &run);
        assert!(quiet(&store));
        // Repeated labels are distinct occurrences, identified by Process id.
        let step = next_step(&store, &run, 0);
        assert!(deliver(&store).contains(&format!("implement ({step}): running")));
        finish_run(&store, &run, "failed");
        assert!(deliver(&store).contains(&format!("Run feature ({run}) · failed")));
    }

    #[test]
    fn review_positions_are_reported_without_repeating_history() {
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
        let run = store.test_graph("feature", "/task-updates", &[("gate", None)], None);
        assert!(quiet(&store));
        finish_run(&store, &run, "succeeded");
        store.set_task_graph_step(&task, "demo").unwrap();
        let available = deliver(&store);
        assert!(available.contains("At demo · available for review"));
        assert!(available.contains(&format!("Run feature ({run}) · succeeded")));
        store.set_task_graph_step(&task, "end").unwrap();
        let at_end = deliver(&store);
        assert!(at_end.contains("At end (Task completion is separate)"));
        assert!(
            !at_end.contains(run.as_str()),
            "unchanged history isn't repeated for position-only changes"
        );
    }

    #[test]
    fn two_readers_of_one_change_write_it_once() {
        let (_home, store, task) = task_session();
        assert!(quiet(&store), "baseline");
        let reading = store.task_update_reading(&task).unwrap();
        let update = [("update:-:fixture".to_string(), "Run failed".to_string())];
        store
            .save_task_update(&task, reading.as_deref(), "{\"first\":1}", &update)
            .unwrap();
        store
            .save_task_update(&task, reading.as_deref(), "{\"second\":1}", &update)
            .unwrap();
        assert_eq!(
            store.task_update_reading(&task).unwrap().as_deref(),
            Some("{\"first\":1}")
        );
    }

    #[test]
    fn a_conversation_that_just_started_skips_updates_written_before_it() {
        let (_home, store, _task) = task_session();
        assert!(quiet(&store), "baseline");
        store.test_graph(
            "feature",
            "/task-updates",
            &[("gate", Some("failed"))],
            Some("failed"),
        );
        assert_eq!(pending(&store, "reader").len(), 1);
        store.baseline_task_updates("reader").unwrap();
        assert!(quiet(&store));
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
        assert!(quiet(&store));
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
            quiet(&store),
            "a conversation is not told about its own comment"
        );
        comment(&store, &task, "PR: https://example.test/pr/1", person());
        let update = deliver(&store);
        assert!(update.contains("new comment from jack: PR: https://example.test/pr/1"));
        assert!(!update.contains("already in the prompt"));
        assert!(!update.contains("own progress"));
    }

    #[test]
    fn direction_comments_arrive_as_steers_not_task_updates() {
        let (_home, store, task) = task_session();
        assert!(quiet(&store), "baseline");
        comment(
            &store,
            &task,
            "use the staging bucket",
            TaskCommentAuthor::Person {
                name: Some("jack".into()),
            },
        );
        assert!(
            quiet(&store),
            "the steer saved for this comment is its only delivery"
        );
    }

    #[test]
    fn worker_runs_neither_write_nor_read_updates() {
        let (_home, store, task) = task_session();
        let worker = store.test_session("worker", &crate::session_record::new_artifact_key());
        store
            .bind_session("worker", worker.captured, &task)
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=0 WHERE id='worker'",
                [],
            )
            .unwrap();
        assert!(pending(&store, "worker").is_empty());
        assert!(
            store.task_update_reading(&task).unwrap().is_none(),
            "a worker's boundary reads no Task history"
        );
        assert!(quiet(&store), "baseline");
        store.test_graph(
            "feature",
            "/task-updates",
            &[("gate", Some("failed"))],
            Some("failed"),
        );
        assert_eq!(pending(&store, "reader").len(), 1);
        assert!(pending(&store, "worker").is_empty());
    }
}
