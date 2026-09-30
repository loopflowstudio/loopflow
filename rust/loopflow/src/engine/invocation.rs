//! Captured invocation identity and selected steps shared by durable Flows.

use crate::engine::flow::RepeatPolicy;
use crate::engine::{compile_flow, load_flow, ConcreteStep};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Skill,
    Op,
    Xor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedInvocation {
    pub id: String,
    pub flow: String,
    pub steps: Vec<ConcreteStep>,
}

impl QueuedInvocation {
    /// Exact occurrence in this captured graph, shared by agent and operation history.
    pub(crate) fn location(
        &self,
        cursor: &crate::engine::ExecutionCursor,
    ) -> Result<(u32, Vec<Vec<u32>>)> {
        Ok((
            self.node_id(cursor)?,
            crate::engine::flow_graph::flow_iterations(&self.steps, cursor),
        ))
    }

    /// Local preorder index in the captured graph, including every XOR alternative.
    pub(crate) fn node_id(&self, cursor: &crate::engine::ExecutionCursor) -> Result<u32> {
        fn count(steps: &[ConcreteStep]) -> usize {
            steps
                .iter()
                .map(|step| {
                    1 + match step {
                        ConcreteStep::Xor(branch) => {
                            branch.paths.values().map(|path| count(&path.steps)).sum()
                        }
                        _ => 0,
                    }
                })
                .sum()
        }
        fn locate(
            steps: &[ConcreteStep],
            cursor: &crate::engine::ExecutionCursor,
        ) -> Result<usize> {
            let step = steps
                .get(cursor.index)
                .ok_or_else(|| anyhow!("cursor has no captured node"))?;
            let mut index = count(&steps[..cursor.index]);
            if let Some(child) = cursor.child.as_deref() {
                let (
                    ConcreteStep::Xor(branch),
                    crate::engine::NestedCursor::Xor { selected, cursor },
                ) = (step, child)
                else {
                    return Err(anyhow!("cursor child does not belong to a captured XOR"));
                };
                let mut paths: Vec<_> = branch.paths.iter().collect();
                paths.sort_by_key(|(name, _)| *name);
                index += 1;
                for (name, path) in paths {
                    if name == selected {
                        return Ok(index + locate(&path.steps, cursor)?);
                    }
                    index += count(&path.steps);
                }
                return Err(anyhow!("cursor selects an uncaptured XOR alternative"));
            }
            Ok(index)
        }
        Ok(u32::try_from(locate(&self.steps, cursor)?)?)
    }

    /// The Ask key of a loop blocker raised at this cursor:
    /// `flow:<invocation>:<node>:<iterations>`. A Task's decision and a saved
    /// Flow's decision share it, so the deciding Run and its recovery meet the
    /// same unblock Session.
    pub fn blocker_key(&self, cursor: &crate::engine::ExecutionCursor) -> Result<String> {
        Ok(format!(
            "flow:{}:{}:{}",
            self.id,
            self.node_id(cursor)?,
            serde_json::to_string(&crate::engine::flow_graph::flow_iterations(
                &self.steps,
                cursor
            ))?
        ))
    }

    pub fn new(flow: impl Into<String>, steps: Vec<ConcreteStep>) -> Result<Self> {
        let flow = flow.into();
        if steps.is_empty() {
            return Err(anyhow!("flow '{flow}' has no steps"));
        }
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            flow,
            steps,
        })
    }

    pub fn load(repo: &Path, flow: &str) -> Result<Self> {
        let definition = load_flow(flow, repo)?;
        let steps = compile_flow(&definition, repo)?;
        Self::new(definition.name, steps)
    }
}

/// One logical step selected from the journaled flow expansion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRef {
    pub invocation_id: String,
    pub flow: String,
    pub step: String,
    pub kind: StepKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub human: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat: Option<RepeatPolicy>,
    pub index: u32,
    pub total: u32,
    pub iteration: u32,
}
