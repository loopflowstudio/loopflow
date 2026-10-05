//! A Flow is one driver Exec and the step Execs it starts. The driver holds
//! the graph and cursor in memory and tells each step what it is through
//! `--__flow-step`; that argument, on the step Exec's recorded argv, is the
//! only record of the Flow's position. Nothing reads it back to resume.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::engine::flow_output::FlowOutput;
use crate::engine::ExecutionCursor;
use crate::exec::Exec;
use crate::id::ExecId;

pub(crate) const FLOW_STEP_ARG: &str = "--__flow-step";
/// A step's agent sees which Flow it serves: the driver Exec's id. Steps of one
/// Flow can share notes under it. It configures nothing in lf.
pub(crate) const FLOW_ID_ENV: &str = "LF_FLOW_ID";
pub(crate) const FLOW_STEP_COMMAND: &str = "__flow-step";

/// A step that handed its effect to a watcher exits with this status: the
/// Flow stops there, and neither the step nor the Flow failed.
pub(crate) const HANDED_OFF_EXIT: u8 = 75;

/// What a driver tells one step's process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowStep {
    pub flow: String,
    /// This driver's nth step launch, from 1. A repeated or corrected step
    /// takes a new number.
    pub seq: u32,
    /// Skill name, operation or XOR router, as the graph labels it.
    pub label: String,
    /// The driver's position: index, selected XOR paths and per-edge returns.
    pub cursor: ExecutionCursor,
    /// The step's node in the driver's graph, counted in preorder.
    pub key: u32,
    /// Returns taken on each loop edge, per nesting level, outermost first.
    pub iterations: Vec<Vec<u32>>,
    /// The driver's compiled skill, when looking `label` up by name would not
    /// yield it: a step-level override, a built-in router, or a file edited
    /// since the Flow started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<crate::engine::Skill>,
    /// The structured answer a deciding or routing step must return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<FlowOutput>,
    /// Continue this conversation instead of opening one: a correction of the
    /// answer it just gave.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

impl FlowStep {
    pub(crate) fn arg(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        serde_json::from_str(value).context("invalid Flow step")
    }

    /// The step an Exec was started as, read from its recorded argv.
    pub(crate) fn of_exec(exec: &Exec) -> Option<Self> {
        let argv: Vec<String> = serde_json::from_str(exec.command.as_deref()?).ok()?;
        let marker = argv
            .iter()
            .position(|arg| arg == FLOW_STEP_ARG || arg == FLOW_STEP_COMMAND)?;
        Self::parse(argv.get(marker + 1)?).ok()
    }
}

/// One Flow as its Execs record it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowExecs {
    pub driver: Exec,
    /// Step Execs in launch order.
    pub steps: Vec<(FlowStep, Exec)>,
}

impl FlowExecs {
    pub(crate) fn id(&self) -> &ExecId {
        &self.driver.id
    }

    pub(crate) fn name(&self) -> &str {
        &self.steps[0].0.flow
    }

    pub(crate) fn latest(&self) -> &(FlowStep, Exec) {
        self.steps.last().expect("a recorded Flow has a step")
    }
}

impl FlowExecs {
    /// How far the Flow got, drawn on the authored graph while every recorded
    /// step still fits it, else on the sequence its steps recorded.
    pub(crate) fn detail(
        &self,
        entry: crate::durable::FlowInventoryEntry,
    ) -> crate::durable::FlowDetail {
        use crate::engine::flow_graph::{location, project_cursor, FlowGraph};
        use crate::engine::{ConcreteSkill, ConcreteStep, Skill};

        let cwd = self.driver.cwd.as_ref().map(std::path::PathBuf::from);
        let finished = entry.summary.state == crate::session::FlowSummaryState::Completed;
        let (latest, _) = self.latest();
        let authored = cwd.as_deref().and_then(|cwd| {
            let flow = crate::engine::load_flow(self.name(), cwd).ok()?;
            let steps = crate::engine::compile_flow(&flow, cwd).ok()?;
            let graph = FlowGraph::new(self.name(), &steps);
            self.steps
                .iter()
                .all(|(step, _)| {
                    location(&steps, &step.cursor).is_ok_and(|(key, _)| {
                        key == step.key
                            && graph
                                .node_at(key)
                                .is_some_and(|node| node.label == step.label)
                    })
                })
                .then_some((steps, graph))
        });
        let (graph, cursor) = match authored {
            Some((steps, graph)) => {
                let mut cursor = latest.cursor.clone();
                if finished {
                    cursor.index = steps.len();
                    cursor.child = None;
                }
                (graph, cursor)
            }
            None => {
                let mut occurrences: Vec<(String, &str)> = Vec::new();
                for (step, _) in &self.steps {
                    let path = step.cursor.node_key();
                    if !occurrences.iter().any(|(seen, _)| *seen == path) {
                        occurrences.push((path, &step.label));
                    }
                }
                let position = occurrences
                    .iter()
                    .position(|(path, _)| *path == latest.cursor.node_key())
                    .expect("the latest step is recorded");
                let steps: Vec<_> = occurrences
                    .iter()
                    .map(|(_, label)| {
                        ConcreteStep::Skill(ConcreteSkill {
                            skill: Skill::named(label),
                            id: None,
                            human: false,
                            repeat: None,
                            sources: Vec::new(),
                        })
                    })
                    .collect();
                let cursor = ExecutionCursor {
                    index: if finished { steps.len() } else { position },
                    ..Default::default()
                };
                (FlowGraph::new(self.name(), &steps), cursor)
            }
        };
        let projection = project_cursor(&graph, &cursor);
        crate::durable::FlowDetail {
            entry,
            graph,
            current: projection.current,
            completed: projection.completed,
            returns: projection.returns,
            iterations: latest.iterations.clone(),
            cwd,
            steps: self
                .steps
                .iter()
                .map(|(step, exec)| crate::durable::FlowStepExec {
                    exec_id: exec.id.clone(),
                    label: step.label.clone(),
                    key: step.key,
                    iterations: step.iterations.clone(),
                    started_at: exec.started_at,
                    completed_at: exec.completed_at,
                    outcome: exec.outcome.clone(),
                    exit_code: exec.exit_code,
                })
                .collect(),
        }
    }
}

/// The step this process runs, when its driver named one.
pub(crate) fn current_step(cli: &crate::lf::Cli) -> Result<Option<FlowStep>> {
    cli.flow_step.as_deref().map(FlowStep::parse).transpose()
}

#[cfg(test)]
mod tests {
    use super::{FlowStep, FLOW_STEP_ARG};
    use crate::engine::flow_output::FlowOutput;
    use crate::engine::ExecutionCursor;

    #[test]
    fn a_step_reads_back_from_its_execs_argv() {
        let step = FlowStep {
            flow: "feature".into(),
            seq: 3,
            label: "loop-decide".into(),
            cursor: ExecutionCursor {
                index: 2,
                iteration: 1,
                ..Default::default()
            },
            key: 2,
            iterations: vec![vec![1]],
            skill: None,
            output: Some(FlowOutput::Decision),
            session: None,
        };
        let argv = vec![
            "lf".to_string(),
            "--batch".into(),
            FLOW_STEP_ARG.into(),
            step.arg().unwrap(),
            "skill".into(),
            "loop-decide".into(),
        ];
        let mut exec = crate::exec::Exec {
            id: crate::id::ExecId::new(),
            trace_id: crate::id::TraceId::new(),
            parent_exec_id: None,
            via_agent: None,
            caller_session_id: None,
            caller_provider_generation: None,
            command: Some(serde_json::to_string(&argv).unwrap()),
            repo: None,
            cwd: None,
            started_at: 1,
            completed_at: None,
            outcome: None,
            exit_code: None,
            signal: None,
            error: None,
        };
        assert_eq!(FlowStep::of_exec(&exec), Some(step));
        exec.command = Some(r#"["lf","task","status"]"#.into());
        assert_eq!(FlowStep::of_exec(&exec), None);
    }
}
