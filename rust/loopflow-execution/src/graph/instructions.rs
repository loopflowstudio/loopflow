//! Render compiled Graph topology as a native chat skill's checklist.
use std::path::PathBuf;

use crate::error::LoadError;
use crate::graph::return_target;
use crate::graph::ConcreteStep;
use crate::graph::Skill;
use loopflow_definitions::text::split_frontmatter;

/// Export compiles the authored Graph first, so even a released
/// `interactive:` item reaches the recipe as a worker followed by a breakpoint.
pub fn render_graph_instructions(
    name: &str,
    steps: &[ConcreteStep],
    capture: &mut impl FnMut(&Skill) -> Result<PathBuf, LoadError>,
) -> Result<String, LoadError> {
    let mut output = format!("# Graph: {name}\n\n");
    output.push_str("Carry out these steps using the current request. Wait for each command to finish and inspect its result; stop on failure. Give headless skill runs the relevant request, findings and next action as their message.\n\n\
For conversational steps, load and follow the named skill here, retaining its native controls and resource base. If it requires user invocation, ask here. After interruption, inspect results before repeating a successful command. This Graph needs no Task. For an associated Task, completing this checklist does not move its graph.\n\n");
    render_steps(steps, "", capture, &mut output)?;
    Ok(output)
}

fn conversational_skill(skill: &Skill) -> String {
    let mut instruction = format!("Execute the skill `{}` in this conversation.", skill.name);
    if let Some(source) = &skill.source {
        instruction.push_str(&format!(" Source: `{}`.", source.path.display()));
    }
    instruction
}

fn is_builtin_decision(skill: &Skill, name: &str) -> bool {
    let source = loopflow_definitions::builtins::get_builtin_skill(name)
        .expect("known builtin decision skill");
    let body = split_frontmatter(source).map_or(source, |(_, body)| body);
    skill.name == name && skill.source.is_none() && skill.content.as_deref() == Some(body)
}

fn headless_command(skill: &Skill, input: &std::path::Path) -> Result<String, LoadError> {
    let input = input.to_string_lossy();
    shlex::try_join([
        "lf",
        "-b",
        "--skill-input",
        &input,
        "skill",
        "--",
        &skill.name,
    ])
    .map_err(|error| LoadError::InvalidGraph(error.to_string()))
}

fn render_steps(
    steps: &[ConcreteStep],
    prefix: &str,
    capture: &mut impl FnMut(&Skill) -> Result<PathBuf, LoadError>,
    output: &mut String,
) -> Result<(), LoadError> {
    for (index, step) in steps.iter().enumerate() {
        let position = format!("{prefix}{}", index + 1);
        match step {
            ConcreteStep::Skill(occurrence) => {
                let skill = &occurrence.skill;
                if occurrence.returns.is_some() {
                    let target = return_target(steps, index).ok_or_else(|| {
                        LoadError::InvalidGraph(format!(
                            "invalid compiled return target at {position}"
                        ))
                    })?;
                    if is_builtin_decision(skill, "loop-or-next") {
                        let first = target + 1;
                        output.push_str(&format!("{position}. Review the objective, changes and remaining findings in this conversation.
   If this boundary's requirements are satisfied, continue after step {position}.
   If the last pass made meaningful progress, name a substantial next pass and its expected result. To run it, repeat steps {prefix}{first} through {prefix}{index}, including intervening commands and branches, and reassess at step {position}. Aim to finish within five passes. Continue past five when the work warrants it.
   If passes shrink into small increments that no longer converge, a failure repeats with nothing learned, or progress needs missing input, explain what needs resolving and stop. A deferred check alone is not a blocker. Leave it to CI or review.
"));
                    } else {
                        output.push_str(&format!("{position}. {}\n   Use its criteria to decide in ordinary language: advance continues after step {position}; iterate repeats steps {prefix}{} through {prefix}{index}, including intervening commands and branches, then returns to this decision; blocked explains what needs resolving and stops here.\n", conversational_skill(skill), target + 1));
                    }
                } else {
                    let command = headless_command(skill, &capture(skill)?)?;
                    output.push_str(&format!("{position}. Run `{command}`.\n"));
                }
            }
            ConcreteStep::Breakpoint(stop) => {
                output.push_str(&format!("{position}. At {}, present the completed work and its proposed continuation. Wait for a reply.\n", stop.name));
                if let Some(target) = return_target(steps, index) {
                    output.push_str(&format!("   To iterate, repeat from step {prefix}{}. To advance, continue after {position}.\n", target + 1));
                }
            }
            ConcreteStep::Command(command) => {
                let argv = command.item.argv();
                let quoted = shlex::try_join(argv.iter().map(String::as_str))
                    .map_err(|error| LoadError::InvalidGraph(error.to_string()))?;
                output.push_str(&format!("{position}. Run:\n\n```sh\n{quoted}\n```\n\n"));
            }
            ConcreteStep::Xor(branch) => {
                let instruction = if is_builtin_decision(&branch.router, "xor-route") {
                    "Review the preceding findings and the path descriptions in this conversation."
                        .to_string()
                } else {
                    conversational_skill(&branch.router)
                };
                output.push_str(&format!("{position}. {instruction} Choose exactly one declared path, explain the choice, complete its steps, then rejoin after step {position}. If the choice is unresolved, explain and stop here.\n"));
                for (name, path) in &branch.paths {
                    output.push_str(&format!("\nPath `{name}`: {}\n\n", path.description));
                    render_steps(
                        &path.steps,
                        &format!("{position}[{name}]."),
                        capture,
                        output,
                    )?;
                    output.push_str(&format!(
                        "End path `{name}`: rejoin after step {position}.\n\n"
                    ));
                }
            }
        }
    }
    Ok(())
}
