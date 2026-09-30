//! Captured invocation identity and selected steps shared by durable Flows.

use crate::engine::flow::RepeatPolicy;
use crate::engine::{expand_flow, load_flow, ConcreteStep};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Skill,
    Op,
    // Retained so journals written by retired flow steps remain readable. New
    // flow plans never produce these values.
    And,
    Xor,
    Or,
    Loop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedInvocation {
    pub id: String,
    pub flow: String,
    pub accounts: Option<Box<crate::provider_account::lease::AccountSelection>>,
    #[serde(deserialize_with = "deserialize_steps")]
    pub steps: Vec<ConcreteStep>,
}

impl QueuedInvocation {
    pub fn new(flow: impl Into<String>, steps: Vec<ConcreteStep>) -> Result<Self> {
        let flow = flow.into();
        if steps.is_empty() {
            return Err(anyhow!("flow '{flow}' has no steps"));
        }
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            flow,
            accounts: Some(Box::new(
                crate::provider_account::lease::AccountSelection::from_env()?,
            )),
            steps,
        })
    }

    pub fn load(repo: &Path, flow: &str) -> Result<Self> {
        let definition = load_flow(flow, repo)?;
        let steps = expand_flow(&definition, repo)?;
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

const LEGACY_STEP_PLAN: &str = "__legacy_step_plan__";

#[derive(Deserialize)]
struct LegacyStepPlan {
    name: String,
    kind: StepKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    human: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeat: Option<RepeatPolicy>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredStep {
    Current(ConcreteStep),
    Legacy(LegacyStepPlan),
    Uncaptured(UncapturedStep),
}

// Shipped XOR records held source references rather than branch content. Keep
// their journal readable through the existing explicit recovery disposition.
// The original references remain in the append-only journal, never reloaded.
#[derive(Deserialize)]
enum UncapturedStep {
    Xor { router: Option<String> },
}

pub(crate) fn deserialize_steps<'de, D>(deserializer: D) -> Result<Vec<ConcreteStep>, D::Error>
where
    D: Deserializer<'de>,
{
    Vec::<StoredStep>::deserialize(deserializer).map(|steps| {
        steps
            .into_iter()
            .map(|step| match step {
                StoredStep::Current(step) => step,
                StoredStep::Legacy(step) => legacy_step(step),
                StoredStep::Uncaptured(UncapturedStep::Xor { router }) => {
                    legacy_step(LegacyStepPlan {
                        name: router.unwrap_or_else(|| "xor-route".into()),
                        kind: StepKind::Xor,
                        id: None,
                        human: false,
                        repeat: None,
                    })
                }
            })
            .collect()
    })
}

fn legacy_step(step: LegacyStepPlan) -> ConcreteStep {
    let flow_parents = vec![LEGACY_STEP_PLAN.to_string()];
    match step.kind {
        StepKind::Skill => ConcreteStep::Skill(crate::engine::ConcreteSkill {
            skill: crate::engine::Skill::named(&step.name),
            id: step.id,
            human: step.human,
            repeat: step.repeat,
            flow_parents,
        }),
        StepKind::Op => ConcreteStep::Command(crate::engine::ConcreteCommand {
            item: crate::engine::Command {
                command: step.name,
                args: Vec::new(),
            },
            flow_parents,
        }),
        StepKind::Xor | StepKind::And | StepKind::Or | StepKind::Loop => {
            ConcreteStep::Xor(crate::engine::ConcreteXor {
                router: crate::engine::Skill::named(&step.name),
                paths: Default::default(),
                flow_parents,
            })
        }
    }
}
