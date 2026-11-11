//! Task updates for a native Session, read at its turn boundaries. Receipts
//! live in the Session history; they are delivery evidence, never a cursor.
use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::durable::{GraphStepProcess, TaskId};
use crate::id::LfProcessId;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};
use crate::task_graph::TaskGraphPosition;
use crate::work::task::comment::TaskCommentAuthor;

/// An update shares the hook's context budget with steers and the block.
const UPDATE_BYTES: usize = 4_000;

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

/// The Session's Task changes since the update it last received, or `None`.
/// The Machine server reads it at a boundary and to wake an idle conversation;
/// worker Runs never receive updates. The first read records a baseline:
/// history already in the conversation is not news.
pub(crate) fn read(store: &SqliteStore, session_id: &str) -> StoreResult<Option<TaskUpdate>> {
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
        let position_changed =
            !baseline && old.map(|observation| &observation.position) != Some(&position);
        let mut observation = TaskObservation {
            position,
            runs: HashMap::new(),
            comments: None,
        };
        let mut changes = Vec::new();
        for graph in graphs {
            let id = LfProcessId::parse(&graph.entry.summary.id)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            let prior = old.and_then(|observation| observation.runs.get(&id));
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
            if !baseline && prior != Some(&run) {
                changes.push(run.report(&id));
            }
            observation.runs.insert(id, run);
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
                    .map(|comment| format!("New comment from {}: {}", comment.author, comment.body))
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
    if !baseline && lines.is_empty() {
        return Ok(None);
    }
    let observation = serde_json::to_value(&facts)?;
    let key = format!("{:x}", Sha256::digest(serde_json::to_vec(&observation)?));
    if baseline {
        store.record_task_update(session_id, &key, "baseline", &observation)?;
        return Ok(None);
    }
    let mut body = String::new();
    for (shown, line) in lines.iter().enumerate() {
        if body.len() + line.len() + 1 > UPDATE_BYTES {
            body.push_str(&format!(
                "{} more changes; read `lf task status` for them.\n",
                lines.len() - shown
            ));
            break;
        }
        body.push_str(line);
        body.push('\n');
    }
    Ok(Some(TaskUpdate { key, observation, text: format!(
        "<lf:task-update>\n{body}Report this recorded progress briefly. This is observation, not a request or permission to launch work, advance review, publish or merge. Read current Task status before acting. Diffs belong in Files.\n</lf:task-update>") }))
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

#[cfg(test)]
mod tests {
    use super::read;
    use crate::durable::TaskId;
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;
    use crate::work::task::comment::{TaskComment, TaskCommentAuthor};
    use std::sync::{Arc, Barrier};

    fn deliver(store: &SqliteStore) -> String {
        let update = read(store, "reader").unwrap().expect("an update");
        assert!(store
            .record_task_update("reader", &update.key, "dispatched", &update.observation)
            .unwrap());
        update.text
    }

    #[test]
    fn a_finished_run_reaches_the_next_boundary_once() {
        let (_home, store, _task) = task_session();
        assert!(read(&store, "reader").unwrap().is_none(), "baseline");
        let graph = store.test_graph(
            "feature",
            "/task-updates",
            &[("implement", Some("succeeded"))],
            None,
        );
        assert!(
            read(&store, "reader").unwrap().is_none(),
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
        let text = deliver(&store);
        assert!(text.contains(&format!("Run feature ({graph}) · failed")));
        assert!(text.contains("implement ("));
        assert!(text.contains(": succeeded"));
        assert!(text.contains("not a request or permission"));
        assert!(
            read(&store, "reader").unwrap().is_none(),
            "a delivered reading is not repeated"
        );
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
        assert!(read(&store, "reader").unwrap().is_none());
        finish_step(&store, &run);
        assert!(read(&store, "reader").unwrap().is_none());
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
        assert!(read(&store, "reader").unwrap().is_none());
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
        assert!(read(&store, "reader").unwrap().is_none());
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
            read(&store, "reader").unwrap().is_none(),
            "a conversation is not told about its own comment"
        );
        comment(&store, &task, "use the staging bucket", person());
        let update = read(&store, "reader").unwrap().unwrap();
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
        assert!(read(&store, "worker").unwrap().is_none());
        assert_eq!(store.session_history("worker", 0, 0).unwrap(), before);
    }
}
