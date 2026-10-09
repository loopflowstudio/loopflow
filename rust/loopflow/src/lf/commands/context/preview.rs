//! Effect-free invocation input, assembled through the ordinary prompt and graph owners.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::engine::flow_graph::{FlowGraph, FlowNode, FlowNodeKind};
use crate::engine::flow_output::FlowOutput;
use crate::engine::skill_invocation::SkillInvocation;
use crate::engine::target::{resolve_definition, DefinitionKind, Target};
use crate::engine::{compile_flow, ExecutionCursor, FlowDefinition};
use crate::lf::commands::{flow, run};
use crate::lf::Cli;
use crate::ops::WorkBinding;

/// Assemble the launch's ordinary input from existing local evidence only.
pub fn assemble(
    repo: &Path,
    skill: Option<&str>,
    kind: Option<DefinitionKind>,
    message: Option<&str>,
    cli: &Cli,
) -> Result<ContextPreview> {
    let runtime = tokio::runtime::Runtime::new()?;
    let mut binding = runtime.block_on(async {
        let Some(store) = crate::store::read_existing_registry()? else {
            anyhow::ensure!(
                cli.task.is_none() && cli.wave.is_none(),
                "Work input unavailable: no local registry"
            );
            return Ok::<_, anyhow::Error>(None);
        };
        let store = std::sync::Arc::new(store);
        let binding = if let Some(task) = &cli.task {
            Some(
                crate::ops::resolve_work_selection(
                    &store,
                    repo,
                    crate::ops::WorkSelection {
                        task: Some(task),
                        wave: cli.wave.as_deref(),
                    },
                )
                .await?,
            )
        } else {
            crate::ops::resolve_execution_binding(&store, repo).await?
        };
        if let Some(wave) = cli.wave.as_deref().filter(|_| cli.task.is_none()) {
            let selected = crate::ops::resolve_work_selection(
                &store,
                repo,
                crate::ops::WorkSelection {
                    task: cli.task.as_deref(),
                    wave: Some(wave),
                },
            )
            .await?;
            if let Some(binding) = &binding {
                anyhow::ensure!(
                    binding.wave_id == selected.wave_id,
                    "selected Wave conflicts with checkout Work"
                );
            }
        };

        if let Some(WorkBinding {
            work: crate::durable::WorkRef::Task(id),
            ..
        }) = &binding
        {
            let route = store.sqlite.task_execution_route(id)?;
            anyhow::ensure!(
                route.machine_id == store.local_machine().await?.id,
                "Task input is on Machine {}; run the preview directly there",
                route.machine_id
            );
        }
        Ok(binding)
    })?;
    let mut launch = cli.process_options();
    launch.context = true;
    if let (Some(binding), Some(cwd)) = (&mut binding, &cli.bound_cwd) {
        binding.cwd = cwd.clone();
    }
    let cwd = binding
        .as_ref()
        .map(|binding| binding.cwd.as_path())
        .or(cli.bound_cwd.as_deref())
        .unwrap_or(repo);
    if let Some(name) = skill {
        let invocation = if let Some(path) = &cli.skill_input {
            let invocation = SkillInvocation::read(path)?;
            anyhow::ensure!(
                invocation.skill.name == name,
                "captured skill does not match {name}"
            );
            invocation
        } else {
            let skill = match resolve_definition(cwd, name, kind)? {
                Target::Skill(skill) => skill,
                Target::Flow(flow) => {
                    return preview_flow(&flow, message, &launch, cwd, binding.as_ref())
                        .map(|preview| ContextPreview::Flow(Box::new(preview)));
                }
                _ => unreachable!("definitions resolve to skills or Flows"),
            };
            SkillInvocation {
                skill,
                arguments: message.unwrap_or_default().into(),
            }
        };
        launch.resolved_invocation = Some(invocation);
    }
    run::preview_prompt(
        cwd,
        skill,
        message,
        &launch,
        binding.as_ref(),
        cli.skill_input.is_some(),
    )
    .map(|preview| ContextPreview::Prompt(Box::new(preview)))
}

/// A direct prompt or a Flow's graph and presently knowable input.
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum ContextPreview {
    Prompt(Box<PromptPreview>),
    Flow(Box<FlowContextPreview>),
}

impl ContextPreview {
    pub fn render(&self) -> String {
        match self {
            Self::Prompt(prompt) => prompt.render(),
            Self::Flow(flow) => flow.render(),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct PromptPreview {
    pub checkout: PathBuf,
    pub system_prompt: String,
    pub task_prompt: String,
    pub skill_invocation: Option<SkillInvocation>,
    pub context: crate::engine::context_budget::ContextBudgetReport,
    pub unwritten_sources: Vec<crate::engine::context_budget::ContextSource>,
}

impl PromptPreview {
    pub fn render(&self) -> String {
        let mut text = format!("{}\n\n{}", self.system_prompt, self.task_prompt);
        if let Some(invocation) = &self.skill_invocation {
            text.push_str(&format!(
                "\n\nNative skill: {}\nArguments: {}",
                invocation.skill.name, invocation.arguments
            ));
        }
        if !self.unwritten_sources.is_empty() {
            text.push_str(&format!("\n\nPreview only: {} excerpt sources were not written; JSON includes their complete bytes.", self.unwritten_sources.len()));
        }
        text
    }
}

/// A fresh Flow preview, not a resumed cursor or a prediction of branch outcomes.
#[derive(Debug, serde::Serialize)]
pub struct FlowContextPreview {
    pub checkout: PathBuf,
    pub graph: FlowGraph,
    pub inputs: Vec<FlowNodeInput>,
}

#[derive(Debug, serde::Serialize)]
pub struct FlowNodeInput {
    pub node: u32,
    #[serde(flatten)]
    pub input: FlowInput,
}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FlowInput {
    Current { input: Box<PromptPreview> },
    Unavailable { reason: String },
    NotAgent,
}

fn preview_flow(
    flow: &FlowDefinition,
    message: Option<&str>,
    cli: &Cli,
    repo: &Path,
    binding: Option<&WorkBinding>,
) -> Result<FlowContextPreview> {
    let items = compile_flow(flow, repo)?;
    flow::require_autonomous_steps(&items)?;
    let graph = FlowGraph::new(&flow.name, &items);
    let mut current = if let Some(skill) =
        crate::engine::execution::current_skill(&items, &ExecutionCursor::default())
    {
        let mut launch = cli.process_options();
        launch.resolved_invocation = Some(SkillInvocation {
            skill: skill.skill.clone(),
            arguments: message.unwrap_or_default().to_owned(),
        });
        let mut input = message.unwrap_or_default().to_owned();
        if let Some(instructions) = items.first().and_then(FlowOutput::for_step_instructions) {
            input.push_str(&instructions);
        }
        Some(Box::new(run::preview_prompt(
            repo,
            Some(&skill.skill.name),
            Some(&input),
            &launch,
            binding,
            true,
        )?))
    } else {
        None
    };
    fn collect(
        nodes: &[FlowNode],
        current: &mut Option<Box<PromptPreview>>,
        inputs: &mut Vec<FlowNodeInput>,
    ) {
        for node in nodes {
            let input = if node.kind == FlowNodeKind::Op {
                FlowInput::NotAgent
            } else if let Some(input) = current.take() {
                FlowInput::Current { input }
            } else {
                FlowInput::Unavailable {
                    reason: "Future input depends on preceding execution, branch choices, feedback and then-current files/Work. No preceding steps ran; repeat-pass input is also unavailable.".into(),
                }
            };
            inputs.push(FlowNodeInput {
                node: node.key,
                input,
            });
            for path in &node.paths {
                collect(&path.steps, current, inputs);
            }
        }
    }
    let mut inputs = Vec::new();
    collect(&graph.steps, &mut current, &mut inputs);
    Ok(FlowContextPreview {
        checkout: repo.to_path_buf(),
        graph,
        inputs,
    })
}

impl FlowContextPreview {
    pub fn render(&self) -> String {
        fn labels(nodes: &[FlowNode], text: &mut String) {
            for node in nodes {
                text.push_str(&format!("\n{}: {}", node.key, node.label));
                if let Some(target) = node.returns_to {
                    text.push_str(&format!(" (returns to {target})"));
                }
                for path in &node.paths {
                    text.push_str(&format!("\n  path {}: {}", path.name, path.description));
                    labels(&path.steps, text);
                }
            }
        }
        let mut text = format!("Flow {} — fresh start, current snapshot only; no steps ran. Repeat-pass input is unavailable.", self.graph.name);
        labels(&self.graph.steps, &mut text);
        for node in &self.inputs {
            text.push_str(&format!("\n\nNode {} input: ", node.node));
            match &node.input {
                FlowInput::Current { input } => {
                    text.push_str("current snapshot\n");
                    text.push_str(&input.render());
                }
                FlowInput::Unavailable { reason } => {
                    text.push_str(&format!("unavailable: {reason}"))
                }
                FlowInput::NotAgent => text.push_str("not an agent prompt (command not executed)"),
            }
        }
        text
    }
}
