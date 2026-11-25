//! TaskSummary: what a Task's run came to, written once when the Task ends.
//! Derived from Graph processes, their steps, the auto mark and Sessions;
//! nothing else records into it.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::graph_process::GraphExecution;
use crate::StoreResult;
use loopflow_model::durable::TaskId;

use super::SqliteStore;

/// How a summarized Task ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskEnd {
    Done,
    Abandoned,
}

impl TaskEnd {
    fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        }
    }
}

/// One ended Task's run. Wire type; no defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskSummary {
    pub ended: TaskEnd,
    /// Unix seconds.
    pub ended_at: i64,
    /// The Graph the Task's last Graph process ran; `null` when none ran.
    #[serde(deserialize_with = "Option::deserialize")]
    pub graph: Option<String>,
    /// Times each loop decider sent its loop around again, by step label.
    pub loop_passes: BTreeMap<String, u32>,
    /// The Task's auto mark when it ended; `null` when nobody set one.
    #[serde(deserialize_with = "Option::deserialize")]
    pub auto: Option<super::AutoState>,
    /// From the first Graph process's start, else the Task's creation.
    pub wall_seconds: i64,
    /// First line the latest `reflect` step said last; `null` without one.
    #[serde(deserialize_with = "Option::deserialize")]
    pub reflect: Option<String>,
}

/// Returns each loop decider took, across every Graph process of one Task. A
/// decider returned when the next step its Graph process started stands at or
/// before it with different loop counts; a retried step keeps its counts.
fn loop_passes(graphs: &[GraphExecution]) -> BTreeMap<String, u32> {
    let mut passes = BTreeMap::new();
    for graph in graphs {
        for pair in graph.steps.windows(2) {
            let (step, next) = (&pair[0], &pair[1]);
            let decides = graph
                .compiled
                .node_at(step.key)
                .is_some_and(|node| node.returns_to.is_some());
            if decides && next.key <= step.key && next.iterations != step.iterations {
                *passes.entry(graph.label(step)).or_default() += 1;
            }
        }
    }
    passes
}

/// Write the summary of a Task that just ended, replacing an earlier ending's.
pub(super) fn write_in(conn: &Connection, task: &TaskId, ended: TaskEnd) -> StoreResult<()> {
    let graphs = super::task_work::graphs_of_task(conn, task)?;
    let ended_at = crate::rows::now_unix();
    let started_at = match graphs.first() {
        Some(graph) => graph.run.started_at,
        None => conn.query_row(
            "SELECT created_at FROM tasks WHERE id=?1",
            [task.as_str()],
            |row| row.get(0),
        )?,
    };
    let auto = super::task_work::auto_in(conn, task)?.map(|mark| mark.state);
    let reflect = conn
        .query_row(
            "SELECT json_extract(a.preview,'$.text') FROM agent_sessions s
             JOIN session_activity a ON a.session_id=s.id
             WHERE s.task_id=?1 AND s.skill='reflect' AND a.preview IS NOT NULL
             ORDER BY s.created_at DESC,s.id LIMIT 1",
            [task.as_str()],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .and_then(|text| {
            text.lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(str::to_owned)
        });
    conn.execute(
        // `ship_events` is leaving (LOO-971); new rows write it empty.
        "INSERT INTO task_summaries(task_id,ended,ended_at,graph,loop_passes,ship_events,auto,wall_seconds,reflect)
         VALUES(?1,?2,?3,?4,?5,'{}',?6,?7,?8)
         ON CONFLICT(task_id) DO UPDATE SET ended=excluded.ended,ended_at=excluded.ended_at,
             graph=excluded.graph,loop_passes=excluded.loop_passes,auto=excluded.auto,
             wall_seconds=excluded.wall_seconds,reflect=excluded.reflect",
        params![
            task.as_str(),
            ended.as_str(),
            ended_at,
            graphs.last().map(|graph| graph.name.as_str()),
            serde_json::to_string(&loop_passes(&graphs))?,
            auto.map(|state| state.as_str()),
            (ended_at - started_at).max(0),
            reflect,
        ],
    )?;
    Ok(())
}

impl SqliteStore {
    /// The summary written when `task` last ended; `None` while it never has.
    pub fn task_summary(&self, task: &TaskId) -> StoreResult<Option<TaskSummary>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let row = conn
            .query_row(
                "SELECT ended,ended_at,graph,loop_passes,auto,wall_seconds,reflect
                 FROM task_summaries WHERE task_id=?1",
                [task.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((ended, ended_at, graph, passes, auto, wall_seconds, reflect)) = row else {
            return Ok(None);
        };
        Ok(Some(TaskSummary {
            ended: serde_json::from_value(ended.into())?,
            ended_at,
            graph,
            loop_passes: serde_json::from_str(&passes)?,
            auto: auto
                .map(|state| super::AutoState::parse(&state))
                .transpose()?,
            wall_seconds,
            reflect,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{TaskEnd, TaskSummary};
    use loopflow_model::durable::WorkRef;
    use loopflow_model::id::LfProcessId;

    #[test]
    fn an_abandoned_task_keeps_one_summary_of_its_run() {
        let (_dir, store, task) = crate::test_fixture::store_with_task();
        let start = crate::graph_process::ExecutionStart::reserved(1);
        // One Graph whose only step sent its loop around twice before going on.
        let graph = LfProcessId::new();
        let compiled = crate::graph::CompiledGraph {
            name: "polish".into(),
            steps: vec![crate::graph::GraphNode {
                key: 0,
                id: None,
                label: "review".into(),
                kind: crate::graph::GraphNodeKind::Skill,
                returns_to: Some(0),
                sources: Vec::new(),
                paths: Vec::new(),
            }],
        };
        store
            .record_graph(&graph, &compiled, Some(&task), &start, None, None, None)
            .unwrap();
        for pass in 0..=2 {
            store
                .record_graph_step(&graph, &LfProcessId::new(), 0, &[vec![pass]], &start)
                .unwrap();
        }
        store
            .take_up_task_graph(&task, r#"{"name":"polish","steps":[]}"#)
            .unwrap();
        let stopped = super::super::Auto {
            state: super::super::AutoState::Off,
            at: 4,
            by: crate::process::Starter::Person { name: None },
        };
        store.set_task_auto(&task, &stopped, false).unwrap();
        assert_eq!(store.task_summary(&task).unwrap(), None);

        let statements = crate::query_plans::statements(&store, || {
            store
                .abandon(&WorkRef::Task(task.clone()), "not needed")
                .unwrap();
        });
        assert_eq!(
            crate::query_plans::growing_scans(&store, &statements),
            Vec::<String>::new()
        );

        let TaskSummary {
            ended,
            graph,
            loop_passes,
            auto,
            wall_seconds,
            reflect,
            ..
        } = store.task_summary(&task).unwrap().unwrap();
        assert_eq!(ended, TaskEnd::Abandoned);
        assert_eq!(graph.as_deref(), Some("polish"));
        assert_eq!(loop_passes, [("review".to_owned(), 2)].into());
        assert_eq!(auto, Some(super::super::AutoState::Off));
        assert!(wall_seconds >= 0);
        assert_eq!(reflect, None);
    }
}
