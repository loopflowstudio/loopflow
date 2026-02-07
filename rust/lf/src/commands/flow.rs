use crate::output::Colors;
use crate::Cli;
use anyhow::Result;
use loopflow_engine::{expand_flow, next_action, ConcreteItem, Flow, FlowAction};
use std::path::Path;

/// Run a flow: print pipeline header, then execute each step sequentially.
pub fn run(flow: &Flow, step_args: &[String], cli: &Cli, repo: &Path) -> Result<()> {
    let items = expand_flow(flow, repo)?;
    print_pipeline_header(&flow.name, &items);
    run_steps(&items, step_args, cli)
}

fn print_pipeline_header(flow_name: &str, items: &[ConcreteItem]) {
    let colors = Colors::new();
    let step_names: Vec<&str> = items
        .iter()
        .map(|item| match item {
            ConcreteItem::Step(s) => s.step.name.as_str(),
            ConcreteItem::Fork(_) => "[fork]",
        })
        .collect();

    let pipeline = step_names.join(&format!(
        " {dim}\u{2192}{reset} ",
        dim = colors.dim,
        reset = colors.reset,
    ));

    eprintln!(
        "\n{dim}\u{2500}\u{2500} flow {reset}{bold}{name}{reset} {dim}{pipeline}{reset}\n",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        name = flow_name,
        pipeline = pipeline,
    );
}

fn run_steps(items: &[ConcreteItem], step_args: &[String], cli: &Cli) -> Result<()> {
    let total = items.len();

    for index in 0..total {
        let action = next_action(items, index);
        match action {
            FlowAction::RunStep { step } | FlowAction::WaitInteractive { step } => {
                let colors = Colors::new();
                eprintln!(
                    "{dim}[{current}/{total}]{reset} {bold}{name}{reset}",
                    dim = colors.dim,
                    reset = colors.reset,
                    bold = colors.bold,
                    current = index + 1,
                    total = total,
                    name = step.step.name,
                );
                crate::commands::run::run(Some(&step.step.name), step_args, None, cli)?;
            }
            FlowAction::Fork { fork: _ } => {
                // TODO: fork execution (parallel branches)
                eprintln!("fork execution not yet supported in CLI — skipping");
            }
            FlowAction::Complete => break,
        }
    }

    Ok(())
}
