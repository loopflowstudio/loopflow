//! Occurrences observed alongside a reading, from the same store snapshot.
use std::collections::{HashMap, HashSet};

use anyhow::Result;
use serde_json::Value;

use crate::id::LfProcessId;
use crate::server::wire::SubscriptionEvent;
use crate::store::sqlite::SqliteStore;
use crate::work::task::{TaskEventKind, TaskId};

const MAX_EVENTS: usize = 256;

#[derive(Debug, Default)]
pub(crate) struct Observer {
    tasks: HashMap<TaskId, TaskCursor>,
}

#[derive(Debug, Default)]
struct TaskCursor {
    event: i64,
    completed: HashSet<LfProcessId>,
}

impl Observer {
    pub(crate) fn read(
        &mut self,
        store: &SqliteStore,
        body: &Value,
    ) -> Result<Vec<SubscriptionEvent>> {
        let mut members = HashSet::new();
        if !task_ids(body, &mut members)? {
            // An unavailable reading cannot establish that a Task left the scope.
            members.extend(self.tasks.keys().cloned());
        }
        let watched: HashSet<_> = members
            .iter()
            .cloned()
            .chain(self.tasks.keys().cloned())
            .collect();
        let mut events = Vec::new();
        for task in watched {
            let first = !self.tasks.contains_key(&task);
            let cursor = self.tasks.entry(task.clone()).or_default();
            if first {
                cursor.event = store.latest_task_event(&task)?.map_or(0, |event| event.id);
            } else {
                for event in store.subscription_task_events(&task, cursor.event, MAX_EVENTS + 1)? {
                    cursor.event = event.id;
                    match event.kind {
                        TaskEventKind::PrMerged {
                            pr_id,
                            number,
                            url,
                            merge_commit,
                            ..
                        } => {
                            events.push(SubscriptionEvent::TaskShipped {
                                task_id: task.clone(),
                                pr_id,
                                number,
                                url,
                                merge_commit,
                            });
                        }
                        TaskEventKind::Steer { steer_id } => {
                            let (steer, comment) = store.subscription_steer(&task, &steer_id)?;
                            events.push(SubscriptionEvent::TaskSteer {
                                task_id: task.clone(),
                                steer,
                                comment,
                            });
                        }
                        _ => {}
                    }
                }
            }
            let mut completed = HashSet::new();
            for graph in store.task_graphs(&task)? {
                for step in &graph.steps {
                    if step.process.completed_at.is_none() {
                        continue;
                    }
                    completed.insert(step.process.id.clone());
                    if !first && !cursor.completed.contains(&step.process.id) {
                        events.push(SubscriptionEvent::GraphStepFinished {
                            task_id: task.clone(),
                            graph_process_id: graph.id().clone(),
                            step: graph.step_detail(step),
                        });
                    }
                }
            }
            cursor.completed = completed;
            anyhow::ensure!(
                events.len() <= MAX_EVENTS,
                "subscription fell behind its events; subscribe again for current state"
            );
        }
        self.tasks.retain(|task, _| members.contains(task));
        Ok(events)
    }
}

// Explicit membership paths, before field selection. Saved dashboard queries,
// historical references and comments do not expand the subscription's scope.
fn task_ids(value: &Value, tasks: &mut HashSet<TaskId>) -> Result<bool> {
    let mut complete = true;
    if let Some(items) = value.as_array() {
        for item in items {
            complete &= task_ids(item, tasks)?;
        }
        return Ok(complete);
    }
    for path in ["/task/id", "/task/task_id"] {
        if let Some(id) = value.pointer(path).and_then(Value::as_str) {
            tasks.insert(TaskId::parse(id)?);
        }
    }
    if let Some(items) = value.pointer("/tasks/items").and_then(Value::as_array) {
        for item in items {
            complete &= task_ids(item, tasks)?;
        }
    }
    if let Some(members) = value
        .pointer("/resolution/members")
        .and_then(Value::as_array)
    {
        for member in members {
            if let (Some("task"), Some(id)) = (
                member.pointer("/work/kind").and_then(Value::as_str),
                member.pointer("/work/id").and_then(Value::as_str),
            ) {
                tasks.insert(TaskId::parse(id)?);
            }
        }
    }
    complete &= value.pointer("/tasks/state").and_then(Value::as_str) != Some("unavailable");
    complete &= value
        .pointer("/resolution/unavailable")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty);
    Ok(complete)
}

#[cfg(test)]
mod tests {
    use super::Observer;
    use crate::durable::{Author, WorkRef};
    use crate::server::wire::SubscriptionEvent;
    use crate::store::sqlite::SqliteStore;
    use crate::work::task::{TaskEventKind, TaskId, TaskPrId};
    use serde_json::json;

    fn fixture() -> (tempfile::TempDir, SqliteStore, TaskId) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute_batch(include_str!(
            "../../tests/fixtures/saved_selection_work.sql"
        ))
        .unwrap();
        (
            dir,
            store,
            TaskId::parse("task_11111111111111111111111111111111").unwrap(),
        )
    }

    #[test]
    fn a_reading_baselines_history_then_delivers_the_whole_steer_burst() {
        let (_dir, store, task) = fixture();
        store
            .save_steer(&WorkRef::Task(task.clone()), &Author::User, "old")
            .unwrap();
        let body = json!({"task": {"id": task}});
        let mut observer = Observer::default();
        assert!(observer.read(&store, &body).unwrap().is_empty());
        for text in ["first", "second"] {
            store
                .save_steer(&WorkRef::Task(task.clone()), &Author::User, text)
                .unwrap();
        }
        let events = observer.read(&store, &body).unwrap();
        let texts: Vec<_> = events
            .iter()
            .map(|event| match event {
                SubscriptionEvent::TaskSteer { task_id, steer, .. } => {
                    assert_eq!(task_id, &task);
                    steer.text.as_str()
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(texts, ["first", "second"]);
        assert!(observer.read(&store, &body).unwrap().is_empty());
        assert!(
            Observer::default().read(&store, &body).unwrap().is_empty(),
            "reconnect never replays"
        );
    }

    #[test]
    fn a_comment_steer_carries_the_exact_thread_entry() {
        use crate::work::task::comment::{TaskComment, TaskCommentAuthor};
        let (_dir, store, task) = fixture();
        let body = json!({"task":{"id":task}});
        let mut observer = Observer::default();
        observer.read(&store, &body).unwrap();
        let comment = TaskComment {
            id: uuid::Uuid::new_v4().to_string(),
            body: "Keep the examples.".into(),
            author: TaskCommentAuthor::Person {
                name: Some("Maya".into()),
            },
            created_at: Some("2026-11-09T00:00:00Z".into()),
        };
        store.append_task_comment(&task, &comment).unwrap();
        let events = observer.read(&store, &body).unwrap();
        assert!(
            matches!(&events[..], [SubscriptionEvent::TaskSteer { steer, comment:Some(entry), .. }]
            if entry == &comment && steer.source == format!("comment:{}", comment.id))
        );
    }

    #[test]
    fn a_ship_is_delivered_when_the_task_leaves_the_plan() {
        let (_dir, store, task) = fixture();
        let mut observer = Observer::default();
        observer
            .read(&store, &json!({"tasks":{"items":[{"task":{"id":task}}]}}))
            .unwrap();
        store
            .append_task_event(
                &task,
                &TaskEventKind::PrMerged {
                    pr_id: TaskPrId::new(),
                    sequence: 1,
                    number: 42,
                    url: "https://example.com/pull/42".into(),
                    merge_commit: "abc".into(),
                },
            )
            .unwrap();
        let events = observer
            .read(&store, &json!({"tasks":{"items":[]}}))
            .unwrap();
        assert!(
            matches!(&events[..], [SubscriptionEvent::TaskShipped { task_id, number: 42, .. }] if task_id == &task)
        );
        assert!(observer
            .read(&store, &json!({"tasks":{"items":[]}}))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn unrelated_tasks_and_new_members_do_not_replay_occurrences() {
        let (_dir, store, task) = fixture();
        let sibling = TaskId::parse("task_22222222222222222222222222222222").unwrap();
        let mut observer = Observer::default();
        observer.read(&store, &json!({"task":{"id":task}})).unwrap();
        store
            .save_steer(&WorkRef::Task(sibling.clone()), &Author::User, "elsewhere")
            .unwrap();
        assert!(observer
            .read(&store, &json!({"task":{"id":task}}))
            .unwrap()
            .is_empty());
        assert!(observer
            .read(&store, &json!({"task":{"id":sibling}}))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn an_unavailable_plan_keeps_its_previous_task_membership() {
        let (_dir, store, task) = fixture();
        let body = json!({"tasks":{"state":"ok","items":[{"task":{"id":task}}]}});
        let mut observer = Observer::default();
        observer.read(&store, &body).unwrap();
        observer
            .read(
                &store,
                &json!({"tasks":{"state":"unavailable","reason":"offline"}}),
            )
            .unwrap();
        store
            .save_steer(&WorkRef::Task(task), &Author::User, "while unavailable")
            .unwrap();
        assert!(
            matches!(&observer.read(&store, &body).unwrap()[..], [SubscriptionEvent::TaskSteer { steer, .. }] if steer.text == "while unavailable")
        );
    }

    #[test]
    fn an_overflow_requires_a_fresh_reading_instead_of_silently_losing_events() {
        let (_dir, store, task) = fixture();
        let body = json!({"task":{"id":task}});
        let mut observer = Observer::default();
        observer.read(&store, &body).unwrap();
        for _ in 0..257 {
            store
                .append_task_event(&task, &TaskEventKind::Started)
                .unwrap();
        }
        assert!(observer
            .read(&store, &body)
            .unwrap_err()
            .to_string()
            .contains("fell behind"));
        assert!(Observer::default().read(&store, &body).unwrap().is_empty());
    }

    #[test]
    fn recorded_step_exits_are_emitted_once_including_steps_between_readings() {
        let (dir, store, task) = fixture();
        let conn = rusqlite::Connection::open(dir.path().join("db")).unwrap();
        // Graph membership follows the Task checkout on the local Machine.
        conn.execute("UPDATE tasks SET worktree='/repo/task',checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) WHERE id=?1", [task.as_str()]).unwrap();
        // The graph's session membership also works before Machine registration.
        let old = store.test_graph(
            "code",
            "/repo/task",
            &[("old", Some("succeeded"))],
            Some("succeeded"),
        );
        conn.execute(
            "INSERT INTO relocated_task_processes(process_id,task_id) VALUES(?1,?2)",
            rusqlite::params![old.as_str(), task.as_str()],
        )
        .unwrap();
        let body = json!({"task":{"id":task}});
        let mut observer = Observer::default();
        assert!(observer.read(&store, &body).unwrap().is_empty());
        let graph = store.test_graph(
            "code",
            "/repo/task",
            &[("implement", Some("succeeded")), ("gate", Some("failed"))],
            Some("failed"),
        );
        conn.execute(
            "INSERT INTO relocated_task_processes(process_id,task_id) VALUES(?1,?2)",
            rusqlite::params![graph.as_str(), task.as_str()],
        )
        .unwrap();
        let events = observer.read(&store, &body).unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], SubscriptionEvent::GraphStepFinished { graph_process_id, step, .. } if graph_process_id == &graph && step.label == "implement")
        );
        assert!(
            matches!(&events[1], SubscriptionEvent::GraphStepFinished { step, .. } if step.outcome.as_deref() == Some("failed"))
        );
        assert!(observer.read(&store, &body).unwrap().is_empty());
    }
}
