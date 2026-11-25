//! Task updates: what changed on a Task since its last reading, saved as
//! steers its interactive conversations read, and a coarser notice for the
//! conversations that conduct it. No writer sits at each fact:
//! whoever looks first (a conversation's hook, or the Machine server waking an
//! idle one) compares the Task with its last reading and writes what moved,
//! so an exit nobody recorded is still reported.
use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::durable::{GraphStepProcess, TaskState};
use crate::store::StoreResult;
use crate::task_graph::TaskGraphPosition;
use loopflow_model::comment::TaskCommentAuthor;
use loopflow_model::durable::TaskId;
use loopflow_model::id::LfProcessId;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::task::PrPhase;

/// Run and position changes share one steer; its complete source is on disk.
const UPDATE_BYTES: usize = 4_000;

/// The `task_updates` revision at which a Task's last reading found nothing
/// new, by database and Task. Rebuilding Task history costs tens of
/// milliseconds; until a Task change moves the revision the answer is the
/// same. Per process: the server keeps it, a hook starts without it.
type QuietKey = (String, TaskId);
static QUIET_AT: Mutex<Option<HashMap<QuietKey, i64>>> = Mutex::new(None);

fn quiet_key(store: &SqliteStore, task: &TaskId) -> Option<QuietKey> {
    Some((store.path()?.to_string_lossy().into_owned(), task.clone()))
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct TaskObservation {
    position: Option<TaskGraphPosition>,
    runs: HashMap<LfProcessId, RunObservation>,
    /// Comments that are not direction, by ID.
    comments: BTreeSet<String>,
    /// The phase of the Task's latest PR.
    pr: Option<String>,
    /// `done` or `abandoned` once the Task closed.
    closed: Option<String>,
}

impl TaskObservation {
    fn merged(&self) -> bool {
        self.pr.as_deref() == Some(PrPhase::Merged.as_str())
    }

    /// How the Task closed or reopened since `old`, in a word.
    fn closing(&self, old: &Self) -> Option<&str> {
        (old.closed != self.closed).then_some(match self.closed.as_deref() {
            Some("done") => "completed",
            Some(closed) => closed,
            None => "reopened",
        })
    }

    /// The step the Task stands at or last left.
    fn step(&self) -> Option<&str> {
        self.position.as_ref().map(TaskGraphPosition::step)
    }

    /// How each run that ended without succeeding ended, by run.
    fn failures(&self) -> impl Iterator<Item = (&LfProcessId, String)> {
        self.runs
            .iter()
            .filter(|(_, run)| run.completed && run.outcome.as_deref() != Some("succeeded"))
            .map(|(id, run)| {
                let outcome = run.outcome.as_deref().unwrap_or("stopped");
                let failure = match &run.step {
                    Some(step) => format!("{outcome} at {}", step.position()),
                    None => outcome.to_owned(),
                };
                (id, failure)
            })
    }

    /// How the Task's standing moved since `old`, as a conductor hears it:
    /// it stopped at an interactive step, a run ended without succeeding, its
    /// PR merged, it closed. Steps passing inside a run are not news.
    fn news(&self, old: &Self) -> Vec<String> {
        // A relaunch that fails where a run in `old` did says nothing new
        // while the Task's step and PR are where they were.
        let said = if old.step() == self.step() && old.pr == self.pr {
            old.failures().map(|(_, failure)| failure).collect()
        } else {
            BTreeSet::new()
        };
        // A run and its relaunches that end the same way are one piece of news.
        let failures = self
            .failures()
            .filter(|(id, failure)| {
                !old.runs.get(id).is_some_and(|run| run.completed) && !said.contains(failure)
            })
            .map(|(_, failure)| failure)
            .collect::<BTreeSet<_>>();
        let mut news = failures.into_iter().collect::<Vec<_>>();
        if let Some(TaskGraphPosition::At { step } | TaskGraphPosition::Breakpoint { step, .. }) =
            &self.position
        {
            if old.position != self.position && step != "start" && step != "end" {
                news.push(format!("stopped at {step}"));
            }
        }
        if self.merged() && !old.merged() {
            news.push("merged".into());
        }
        if let Some(closing) = self.closing(old) {
            news.push(format!("was {closing}"));
        }
        news
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RunObservation {
    name: String,
    outcome: Option<String>,
    completed: bool,
    step: Option<GraphStepProcess>,
}

impl RunObservation {
    fn report(&self, id: &LfProcessId, error: Option<&str>) -> String {
        let outcome = self
            .outcome
            .as_deref()
            .expect("only settled runs are reported");
        let mut report = format!("Run {} ({id}) · {outcome}", self.name);
        if let Some(step) = &self.step {
            report.push_str(&format!(
                " · {} ({}): {}",
                step.position(),
                step.lf_process_id,
                step.outcome.as_deref().unwrap_or("outcome unavailable")
            ));
        }
        report.push('.');
        if let Some(error) = error {
            report.push(' ');
            report.push_str(error);
        }
        report
    }
}

/// Write what changed on the Session's Task and on the Tasks it conducts,
/// since their last readings as Task steers. Only an interactive conversation
/// reads them, so only one looks; a worker Run pays nothing.
pub fn write(store: &SqliteStore, session_id: &str) -> StoreResult<()> {
    let Some(session) = store.session(session_id)? else {
        return Ok(());
    };
    if !session.interactive || session.completed_at.is_some() {
        return Ok(());
    }
    let revision = store.task_update_revision()?;
    if let Some(task) = store.session_task(session_id)? {
        let quiet = quiet_key(store, &task);
        if quiet_at(quiet.as_ref()) != Some(revision) {
            write_task(store, &task)?;
            set_quiet_at(quiet, revision);
        }
    }
    for (task, closed) in store.session_conducted_tasks(session_id)? {
        let quiet = quiet_key(store, &task);
        if quiet_at(quiet.as_ref()) == Some(revision) {
            continue;
        }
        // A closed child whose reading says so has no more news; a hook has
        // no quiet cache and would rebuild its history at every boundary.
        if closed.is_none() || reading(store, &task)?.1.and_then(|old| old.closed) != closed {
            write_task(store, &task)?;
        }
        set_quiet_at(quiet, revision);
    }
    Ok(())
}

/// The Task's stored reading and what it says. One written by a build with
/// other facts is no reading: the next one is a baseline.
fn reading(
    store: &SqliteStore,
    task: &TaskId,
) -> StoreResult<(Option<String>, Option<TaskObservation>)> {
    let stored = store.task_update_reading(task)?;
    let old = stored
        .as_deref()
        .and_then(|stored| serde_json::from_str(stored).ok());
    Ok((stored, old))
}

/// The Task's first reading is a baseline: its history is in every prompt
/// already. After that, one steer says how its position and runs moved, and
/// one per comment that is not direction, named for the Session that wrote it
/// so that Session is not told. A Task with a parent also gets one notice for
/// the conversations that conduct it when its standing moved; a child starts
/// from nothing, so its first reading is news to them.
fn write_task(store: &SqliteStore, task: &TaskId) -> StoreResult<()> {
    let Some(row) = store.task(task)? else {
        return Ok(());
    };
    let (stored, old) = reading(store, task)?;
    let (graph_row, graphs) = store.task_progress_records(task)?;
    let mut runs = HashMap::new();
    let mut run_in_progress = false;
    let mut changes = Vec::new();
    for graph in graphs {
        let Some(outcome) = crate::process::os_process::observed_result(&graph.run).outcome()
        else {
            run_in_progress = true;
            continue;
        };
        let step = graph.latest().map(|step| graph.step_detail(step));
        let id = graph.run.id;
        let run = RunObservation {
            name: graph.name,
            outcome: Some(outcome.to_owned()),
            completed: true,
            step,
        };
        // Older readings included running steps with no outcome.
        let previous = old.as_ref().and_then(|old| old.runs.get(&id));
        if old.is_some() && previous.and_then(|run| run.outcome.as_deref()) != Some(outcome) {
            changes.push(run.report(&id, graph.run.error.as_deref()));
        }
        runs.insert(id, run);
    }
    let comments = store.task_undirected_comments(task)?;
    let observation = TaskObservation {
        position: crate::store::task_work::settled_task_position(
            graph_row,
            old.as_ref().and_then(|old| old.position.as_ref()),
            run_in_progress,
        )?,
        runs,
        comments: comments.iter().map(|comment| comment.id.clone()).collect(),
        pr: store
            .task_prs(task)?
            .last()
            .map(|pr| pr.phase().as_str().to_owned()),
        closed: match store.task_state(task)? {
            state @ (TaskState::Done | TaskState::Abandoned) => Some(state.label().to_owned()),
            _ => None,
        },
    };
    if old.as_ref() == Some(&observation) {
        return Ok(());
    }
    let reading = serde_json::to_string(&observation)?;
    let key = format!("{:x}", Sha256::digest(reading.as_bytes()));
    let identifier = &row.identifier;
    let mut updates = Vec::new();
    if row.parent_task_id.is_some() {
        let news = observation.news(old.as_ref().unwrap_or(&TaskObservation::default()));
        if !news.is_empty() {
            updates.push((
                format!("notice:{task}:{key}"),
                format!(
                    "{identifier} {:?}, which this conversation conducts, {}. lf task status {identifier}",
                    row.title,
                    news.join(" and ")
                ),
            ));
        }
    }
    if let Some(old) = &old {
        if observation.merged() && !old.merged() {
            changes.push("PR merged.".into());
        }
        if let Some(closing) = observation.closing(old) {
            changes.push(format!("Task {closing}."));
        }
        let took_up_graph = old.position.is_none()
            && matches!(&observation.position, Some(TaskGraphPosition::At { step }) if step == "start");
        if (old.position != observation.position && !took_up_graph) || !changes.is_empty() {
            let mut text = format!(
                "Task {identifier}: {}.",
                position_text(observation.position.as_ref())
            );
            for change in &changes {
                text.push(' ');
                text.push_str(change);
                if text.len() > UPDATE_BYTES {
                    let more = "… Read `lf task status` for the full update.";
                    text.truncate(text.floor_char_boundary(UPDATE_BYTES - more.len()));
                    text.push_str(more);
                    break;
                }
            }
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
        Some(TaskGraphPosition::Breakpoint { step, .. }) => format!("At breakpoint {step}"),
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
    use loopflow_model::comment::{TaskComment, TaskCommentAuthor};
    use loopflow_model::durable::TaskId;
    use loopflow_model::id::LfProcessId;
    use loopflow_storage::sqlite::SqliteStore;

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
            .finish_run(
                loopflow_storage::graph_process::RunOwner::GraphRun,
                &graph,
                20,
                Some(1),
                Some(&format!("provider refused: {}", "é".repeat(4_000))),
            )
            .unwrap();
        let text = deliver(&store);
        assert!(text.contains("provider refused: "));
        assert!(text.len() <= super::UPDATE_BYTES);
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
    fn a_run_reports_once_after_its_last_step_with_its_error() {
        let (_home, store, _task) = task_session();
        assert!(quiet(&store), "baseline before launch");
        let run = store.test_graph("feature", "/task-updates", &[("implement", None)], None);
        assert!(quiet(&store), "run start");
        finish_step(&store, &run, 0, None);
        assert!(quiet(&store), "between steps");
        let step = next_step(&store, &run, 0);
        assert!(quiet(&store), "next step");
        finish_run(&store, &run, "failed");
        let updates = pending(&store, "reader");
        assert_eq!(updates.len(), 1);
        assert!(updates[0].contains(&format!("Run feature ({run}) · failed")));
        assert!(updates[0].contains(&format!("implement ({step}): failed. it failed")));
        deliver(&store);
        assert!(quiet(&store));
    }

    #[test]
    fn a_graph_that_died_between_steps_reports_its_unrecorded_exit_once() {
        let (_home, store, _task) = task_session();
        assert!(quiet(&store));
        let run = store.test_graph(
            "feature",
            "/task-updates",
            &[("implement", Some("succeeded"))],
            None,
        );
        assert!(quiet(&store));
        // Its reservation dates from before boot and no process recorded an exit.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE graph_processes SET started_at=1 WHERE lf_process_id=?1",
                [&run],
            )
            .unwrap();
        assert!(deliver(&store).contains(&format!("Run feature ({run}) · unknown")));
        assert!(quiet(&store));
    }

    #[test]
    fn a_reading_saved_mid_run_stays_quiet_until_the_run_settles() {
        let (_home, store, task) = task_session();
        let run = store.test_graph("feature", "/task-updates", &[("implement", None)], None);
        let graph = crate::task_graph::CapturedGraph {
            name: "feature".into(),
            steps: Vec::new(),
        };
        crate::store::task_work::take_up_task_graph(&store, &task, &graph).unwrap();
        let start = loopflow_storage::graph_process::ExecutionStart {
            pid: Some(std::process::id()),
            ..loopflow_storage::graph_process::ExecutionStart::reserved(1)
        };
        store
            .start_task_graph_run(
                &task,
                loopflow_storage::sqlite::task_work::TaskGraphClaim {
                    graph: &serde_json::to_string(&graph).unwrap(),
                    step: "start",
                    stopped: None,
                    arrival: None,
                },
                (r#"{"index":0}"#, None),
                (&run, &start),
            )
            .unwrap();
        let saved = serde_json::json!({
            "position": {"kind":"run","step":"start","at":"earlier step","lf_process_id":run,"running":true},
            "runs": {(run.as_str()): {"name":"feature","outcome":null,"completed":false,"step":null}},
            "comments": [], "closed": null, "pr": null
        });
        store
            .save_task_update(&task, None, &saved.to_string(), &[])
            .unwrap();
        comment(
            &store,
            &task,
            "PR: https://example.test/pr/1",
            TaskCommentAuthor::Person {
                name: Some("jack".into()),
            },
        );
        let updates = pending(&store, "reader");
        assert_eq!(updates.len(), 1, "only the new comment");
        assert!(deliver(&store).contains("new comment from jack: PR:"));
        assert!(quiet(&store));
        // A run arrives at its last step before it records its outcome.
        store.arrive_task_graph(&task, &run, "end", None).unwrap();
        assert!(quiet(&store), "arrived but unsettled");
        finish_run(&store, &run, "succeeded");
        let updates = pending(&store, "reader");
        assert_eq!(updates.len(), 1);
        assert!(updates[0].contains("At end (Task completion is separate)"));
        assert!(updates[0].contains(&format!("Run feature ({run}) · succeeded")));
    }

    #[test]
    fn review_positions_are_reported_without_repeating_history() {
        let (_home, store, task) = task_session();
        crate::store::task_work::take_up_task_graph(
            &store,
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
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let task = task(&store, "Updates", "/task-updates", None);
        conversation(&store, "reader", &task);
        (home, store, task)
    }

    /// A Task with its checkout at `worktree`.
    fn task(store: &SqliteStore, title: &str, worktree: &str, parent: Option<&TaskId>) -> TaskId {
        let task = TaskId::new();
        store
            .create_task(&loopflow_storage::work::task::NewTask {
                parent_task_id: parent.cloned(),
                repository_id: crate::repository::location::ensure_repository(
                    store,
                    "/task-updates",
                )
                .unwrap(),
                id: task.clone(),
                project_id: None,
                title: title.into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store.conn.lock().unwrap().execute("INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2) ON CONFLICT(task_id) DO UPDATE SET worktree=excluded.worktree", [task.as_str(), worktree]).unwrap();
        task
    }

    /// An interactive conversation that belongs to `task`.
    fn conversation(store: &SqliteStore, id: &str, task: &TaskId) {
        let session = store.test_session(id, &crate::session_record::new_artifact_key());
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE agent_sessions SET interactive=1 WHERE id=?1", [id])
            .unwrap();
        crate::store::sessions::bind_session(store, id, session.captured, task).unwrap();
    }

    /// The steers `session` has not read, after a reading: notices and the rest.
    fn unread(store: &SqliteStore, session: &str) -> (Vec<String>, Vec<String>) {
        write(store, session).unwrap();
        let (notices, rest): (Vec<_>, Vec<_>) = store
            .session_steers(session, 0, 100)
            .unwrap()
            .items
            .into_iter()
            .filter(|row| row.delivered_at.is_none())
            .map(|row| row.steer)
            .partition(|steer| steer.source.starts_with("notice:"));
        let text = |steers: Vec<loopflow_storage::steer::Steer>| {
            steers.into_iter().map(|steer| steer.text).collect()
        };
        (text(notices), text(rest))
    }

    fn sql(store: &SqliteStore, sql: &str, task: &TaskId) {
        store
            .conn
            .lock()
            .unwrap()
            .execute(sql, [task.as_str()])
            .unwrap();
    }

    #[test]
    fn a_parents_conversation_hears_when_a_child_stops_not_at_each_step() {
        let (_home, store, parent) = task_session();
        let child = task(
            &store,
            "Log each retry",
            "/task-updates-child",
            Some(&parent),
        );
        let identifier = store.task(&child).unwrap().unwrap().identifier;
        conversation(&store, "child-reader", &child);
        assert_eq!(unread(&store, "reader"), (vec![], vec![]), "a child filed");
        assert_eq!(unread(&store, "child-reader"), (vec![], vec![]));

        crate::store::task_work::take_up_task_graph(
            &store,
            &child,
            &crate::task_graph::CapturedGraph {
                name: "feature".into(),
                steps: Vec::new(),
            },
        )
        .unwrap();
        let run = store.test_graph(
            "feature",
            "/task-updates-child",
            &[("implement", None)],
            None,
        );
        finish_step(&store, &run, 0, None);
        next_step(&store, &run, 0);
        assert_eq!(
            unread(&store, "reader"),
            (vec![], vec![]),
            "steps passing inside a child's run are not news to its parent"
        );
        assert_eq!(unread(&store, "child-reader"), (vec![], vec![]));

        finish_run(&store, &run, "succeeded");
        store.set_task_graph_step(&child, "ship-review").unwrap();
        let (notices, updates) = unread(&store, "reader");
        assert_eq!(
            notices,
            [format!(
                "{identifier} \"Log each retry\", which this conversation conducts, stopped at ship-review. lf task status {identifier}"
            )]
        );
        assert!(updates.is_empty(), "a child's updates stay with the child");
        let (notices, updates) = unread(&store, "child-reader");
        assert!(notices.is_empty());
        assert_eq!(updates.len(), 1);
        assert!(updates[0].contains("At ship-review · available for review"));
        assert!(updates[0].contains(&format!("Run feature ({run}) · succeeded")));

        store
            .save_steer(
                &loopflow_model::durable::WorkRef::Task(child.clone()),
                &loopflow_model::durable::Author::Named("person".into()),
                "use the staging bucket",
            )
            .unwrap();
        let (notices, steers) = unread(&store, "reader");
        assert_eq!(notices.len(), 1, "one notice, written once");
        assert!(
            steers.is_empty(),
            "direction for a child stays with the child"
        );
        assert!(unread(&store, "child-reader")
            .1
            .contains(&"use the staging bucket".to_string()));
    }

    #[test]
    fn a_run_and_its_relaunch_failing_alike_are_one_piece_of_news() {
        let (_home, store, parent) = task_session();
        task(&store, "Retry", "/task-updates-child", Some(&parent));
        for _ in 0..2 {
            store.test_graph(
                "feature",
                "/task-updates-child",
                &[("gate", Some("failed"))],
                Some("failed"),
            );
        }
        let (notices, _) = unread(&store, "reader");
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(
            notices[0].contains("conducts, failed at gate. lf task status"),
            "{notices:?}"
        );
    }

    #[test]
    fn a_relaunch_failing_where_the_last_run_did_is_not_told_twice() {
        let (_home, store, parent) = task_session();
        task(&store, "Retry", "/task-updates-child", Some(&parent));
        let fail = |step: &str| {
            store.test_graph(
                "feature",
                "/task-updates-child",
                &[(step, Some("failed"))],
                Some("failed"),
            );
        };
        fail("gate");
        assert_eq!(unread(&store, "reader").0.len(), 1);
        // The relaunch is read later and fails at the same step.
        fail("gate");
        let (notices, _) = unread(&store, "reader");
        assert_eq!(notices.len(), 1, "{notices:?}");
        // A failure somewhere else is news.
        fail("lint");
        let (notices, _) = unread(&store, "reader");
        assert_eq!(notices.len(), 2, "{notices:?}");
        assert!(
            notices[1].contains("conducts, failed at lint."),
            "{notices:?}"
        );
    }

    #[test]
    fn a_failed_run_a_merge_and_a_close_are_news_to_the_parent_and_the_task() {
        let (_home, store, parent) = task_session();
        let child = task(&store, "Retry", "/task-updates-child", Some(&parent));
        conversation(&store, "child-reader", &child);
        assert_eq!(unread(&store, "child-reader"), (vec![], vec![]), "baseline");
        store.test_graph(
            "feature",
            "/task-updates-child",
            &[("gate", Some("failed"))],
            Some("failed"),
        );
        let (notices, _) = unread(&store, "reader");
        assert!(
            notices[0].contains("conducts, failed at gate."),
            "{notices:?}"
        );

        sql(
            &store,
            "UPDATE task_checkouts SET branch='retry',base_commit='base' WHERE task_id=?1",
            &child,
        );
        sql(
            &store,
            "INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,created_at,updated_at)
             SELECT '4f0a3d7e-6c1b-4a52-9d3e-0b8c7a6f5e4d',?1,1,'retry',branch,base_commit,1,1,1
             FROM task_checkouts WHERE task_id=?1",
            &child,
        );
        assert!(
            unread(&store, "reader").0.len() == 1,
            "an open PR is not news"
        );
        sql(
            &store,
            "UPDATE task_prs SET github_number=7,github_url='https://example.test/pr/7',merge_commit='abc123' WHERE task_id=?1",
            &child,
        );
        let (notices, _) = unread(&store, "reader");
        assert!(notices[1].contains("conducts, merged."), "{notices:?}");
        assert!(unread(&store, "child-reader")
            .1
            .iter()
            .any(|update| update.contains("PR merged.")));

        sql(
            &store,
            "UPDATE tasks SET planning_state='completed' WHERE id=?1",
            &child,
        );
        let (notices, _) = unread(&store, "reader");
        assert!(
            notices[2].contains("conducts, was completed."),
            "{notices:?}"
        );
        assert!(unread(&store, "child-reader")
            .1
            .iter()
            .any(|update| update.contains("Task completed.")));
        assert_eq!(
            unread(&store, "reader").0.len(),
            3,
            "a closed child is quiet"
        );
    }

    #[test]
    fn a_child_nobody_read_before_it_stopped_is_still_news() {
        let (_home, store, parent) = task_session();
        let child = task(&store, "Retry", "/task-updates-child", Some(&parent));
        sql(
            &store,
            "UPDATE tasks SET abandoned_at=1 WHERE id=?1",
            &child,
        );
        let (notices, _) = unread(&store, "reader");
        assert!(
            notices[0].contains("conducts, was abandoned."),
            "{notices:?}"
        );
        // A conversation that starts now reads the children's standing itself.
        conversation(&store, "later", &parent);
        store.baseline_task_updates("later").unwrap();
        assert_eq!(unread(&store, "later"), (vec![], vec![]));
    }

    fn finish_run(store: &SqliteStore, run: &LfProcessId, outcome: &str) {
        let (exit, error) = match outcome {
            "succeeded" => (0, None),
            _ => (1, Some("it failed")),
        };
        finish_step(store, run, exit, error);
        store
            .finish_run(
                loopflow_storage::graph_process::RunOwner::GraphRun,
                run,
                20,
                Some(exit),
                error,
            )
            .unwrap();
    }

    fn finish_step(store: &SqliteStore, run: &LfProcessId, exit: i32, error: Option<&str>) {
        let graph = store.graph_process(run.as_str()).unwrap().unwrap().0;
        for step in graph.steps {
            store
                .finish_run(
                    loopflow_storage::graph_process::RunOwner::GraphStep,
                    &step.run.id,
                    20,
                    Some(exit),
                    error,
                )
                .unwrap();
        }
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
        let start = loopflow_storage::graph_process::ExecutionStart::reserved(21);
        store
            .record_graph_step(run, &step, key, &[vec![]], &start)
            .unwrap();
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
        crate::store::sessions::bind_session(&store, "worker", worker.captured, &task).unwrap();
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

    #[test]
    fn a_parents_conversation_hears_when_a_child_parks_at_a_breakpoint() {
        let (_home, store, parent) = task_session();
        let child = task(
            &store,
            "Compare layouts",
            "/task-updates-child",
            Some(&parent),
        );
        let identifier = store.task(&child).unwrap().unwrap().identifier;
        conversation(&store, "child-reader", &child);
        assert_eq!(unread(&store, "reader"), (vec![], vec![]));
        assert_eq!(unread(&store, "child-reader"), (vec![], vec![]));

        crate::store::task_work::take_up_task_graph(
            &store,
            &child,
            &crate::task_graph::CapturedGraph {
                name: "feature".into(),
                steps: Vec::new(),
            },
        )
        .unwrap();
        let carrier = LfProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&carrier],
            )
            .unwrap();
            conn.execute(
                "UPDATE task_graphs SET lf_process_id=?2,run_start='{\"index\":0}' WHERE task_id=?1",
                rusqlite::params![child.as_str(), carrier.as_str()],
            )
            .unwrap();
        }
        let record =
            serde_json::json!({"id": carrier.as_str(), "cursor": {"index": 1}}).to_string();
        store
            .arrive_task_graph(
                &child,
                &carrier,
                "after-explore",
                Some((&record, "Compare layouts")),
            )
            .unwrap();
        let (notices, updates) = unread(&store, "reader");
        assert_eq!(
            notices,
            [format!(
                "{identifier} \"Compare layouts\", which this conversation conducts, stopped at after-explore. lf task status {identifier}"
            )]
        );
        assert!(updates.is_empty());
        let (child_notices, _) = unread(&store, "child-reader");
        assert!(child_notices.is_empty());
    }
}
