//! Render compiled Flow topology as a native chat skill's checklist.
use clap::CommandFactory;

use crate::error::LoadError;
use crate::flow::ConcreteStep;
use crate::flow::Skill;
use crate::flow::{return_target, split_frontmatter};
use crate::lf::navigation::resolve_child;

pub fn render_flow_instructions(
    name: &str,
    steps: &[ConcreteStep],
    flow_names: &[String],
) -> Result<String, LoadError> {
    let mut output = format!("# Flow: {name}\n\n");
    output.push_str("Carry out these steps using the current request. Wait for each command to finish and inspect its result; stop on failure. Give headless skill runs the relevant request, findings and next action as their message.\n\n\
For conversational steps, load and follow the named skill here, retaining its native controls and resource base. If it requires user invocation, ask here. After interruption, inspect results before repeating a successful command. Completing this checklist does not move the Task's Workflow.\n\n");
    render_steps(
        steps,
        "",
        flow_names,
        &crate::lf::Cli::command(),
        &mut output,
    )?;
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
    let source = crate::builtins::get_builtin_skill(name).expect("known builtin decision skill");
    let body = split_frontmatter(source).map_or(source, |(_, body)| body);
    skill.name == name && skill.source.is_none() && skill.content.as_deref() == Some(body)
}

fn headless_command(
    skill: &Skill,
    flow_names: &[String],
    commands: &clap::Command,
) -> Result<String, LoadError> {
    let mut argv = vec!["lf", "-b"];
    if let Some(agent) = &skill.agent {
        argv.extend(["-a", agent]);
    }
    if flow_names.contains(&skill.name)
        || skill.name.starts_with('-')
        || !matches!(resolve_child(commands, &skill.name, &[]), Ok(None))
    {
        argv.extend(["skill", "--"]);
    }
    argv.push(&skill.name);
    shlex::try_join(argv).map_err(|error| LoadError::InvalidFlow(error.to_string()))
}

fn render_steps(
    steps: &[ConcreteStep],
    prefix: &str,
    flow_names: &[String],
    commands: &clap::Command,
    output: &mut String,
) -> Result<(), LoadError> {
    for (index, step) in steps.iter().enumerate() {
        let position = format!("{prefix}{}", index + 1);
        match step {
            ConcreteStep::Skill(occurrence) => {
                let skill = &occurrence.skill;
                if occurrence.returns.is_some() {
                    let target = return_target(steps, index).ok_or_else(|| {
                        LoadError::InvalidFlow(format!(
                            "invalid compiled return target at {position}"
                        ))
                    })?;
                    if is_builtin_decision(skill, "loop-or-next") {
                        output.push_str(&format!("{position}. Review the objective, changes and remaining findings in this conversation.\n   If this boundary's requirements are satisfied, continue after step {position}.\n   If the last pass made meaningful progress and specific work remains, state the next action and repeat steps {prefix}{} through {prefix}{index}, including intervening commands and branches, then reassess at step {position}.\n   If progress requires missing input or repeats a failure without new evidence, explain what needs resolving and stop. A deferred check alone is not a blocker; leave it to its declared gate/CI or review. There is no arbitrary pass limit.\n", target + 1));
                    } else {
                        output.push_str(&format!("{position}. {}\n   Use its criteria to decide in ordinary language: advance continues after step {position}; iterate repeats steps {prefix}{} through {prefix}{index}, including intervening commands and branches, then returns to this decision; blocked explains what needs resolving and stops here.\n", conversational_skill(skill), target + 1));
                    }
                } else if occurrence.human {
                    output.push_str(&format!("{position}. {}\n", conversational_skill(skill)));
                } else {
                    let command = headless_command(skill, flow_names, commands)?;
                    output.push_str(&format!("{position}. Run `{command}`.\n"));
                }
                if occurrence.human {
                    output.push_str("   Wait for the participant's response here before continuing; never substitute another reviewer or assume approval.\n");
                }
            }
            ConcreteStep::Command(command) => {
                let argv = command.item.argv();
                let quoted = shlex::try_join(argv.iter().map(String::as_str))
                    .map_err(|error| LoadError::InvalidFlow(error.to_string()))?;
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
                let mut paths: Vec<_> = branch.paths.iter().collect();
                paths.sort_by_key(|(name, _)| *name);
                for (name, path) in paths {
                    output.push_str(&format!("\nPath `{name}`: {}\n\n", path.description));
                    render_steps(
                        &path.steps,
                        &format!("{position}[{name}]."),
                        flow_names,
                        commands,
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

#[cfg(test)]
mod tests {
    use super::render_flow_instructions;
    use crate::flow::{available_flow_names, compile_flow, load_authored_flow};
    use crate::lf::{navigation::normalize_args, Cli, Commands, SkillCommand};
    use clap::Parser;
    use std::fs;
    use tempfile::TempDir;

    fn recipe(repo: &std::path::Path, name: &str) -> String {
        let flow = load_authored_flow(name, repo).unwrap();
        let steps = compile_flow(&flow, repo).unwrap();
        render_flow_instructions(name, &steps, &available_flow_names(repo).unwrap()).unwrap()
    }

    #[test]
    fn chat_recipe_runs_work_and_keeps_reviews_and_nested_decisions_here() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        fs::write(
            repo.path().join(".lf/skills/work.md"),
            "Work body must not be bundled.",
        )
        .unwrap();
        fs::write(
            repo.path().join(".lf/flows/repair.yaml"),
            "- work\n- loop: work\n",
        )
        .unwrap();
        fs::write(
            repo.path().join(".lf/flows/chat.yaml"),
            r#"
- step: {name: work, id: first}
- cmd: commit -m don't-expand-$HOME;echo-unsafe
- xor:
    paths:
      repair:
        flow: repair
        description: Repair the findings
      skip:
        description: Nothing to repair
- loop: first
- step: {name: demo, id: review, human: true}
"#,
        )
        .unwrap();
        let plan = recipe(repo.path(), "chat");
        assert!(plan.contains("1. Run `lf -b skill -- work`."));
        assert!(plan.contains("repeat steps 1 through 3"));
        assert!(plan.contains("repeat steps 3[repair].1 through 3[repair].1"));
        assert!(plan.contains("reassess at step 3[repair].2"));
        assert!(plan.contains("rejoin after step 3"));
        assert!(plan.contains("5. Execute the skill `demo` in this conversation."));
        assert!(plan.contains("Wait for the participant's response"));
        assert!(plan.contains("Repair the findings"));
        assert!(plan.contains("A deferred check alone is not a blocker"));
        assert!(!plan.contains("Work body must not be bundled"));
        assert!(!plan.contains("JSON"));
        assert!(!plan.contains("Resolved skill bodies"));
        let command = plan
            .split("```sh\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap();
        assert_eq!(
            shlex::split(command).unwrap(),
            vec!["lf", "commit", "-m", "don't-expand-$HOME;echo-unsafe"]
        );
    }

    #[test]
    fn chat_recipe_preserves_overridden_loop_and_router_skills() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        for name in ["loop-or-next", "xor-route"] {
            fs::write(
                repo.path().join(format!(".lf/skills/{name}.md")),
                "Custom criteria",
            )
            .unwrap();
        }
        fs::write(repo.path().join(".lf/flows/chat.yaml"), "- implement\n- loop: implement\n- xor:\n    paths:\n      done:\n        description: Finished\n").unwrap();
        let plan = recipe(repo.path(), "chat");
        for name in ["loop-or-next", "xor-route"] {
            assert!(plan.contains(&format!("Execute the skill `{name}` in this conversation.")));
            assert!(plan.contains(
                &repo
                    .path()
                    .join(format!(".lf/skills/{name}.md"))
                    .display()
                    .to_string()
            ));
        }
        assert!(plan.contains("iterate repeats steps 1 through 1"));
        assert!(!plan.contains("If the last pass made meaningful progress"));
    }

    #[test]
    fn chat_recipe_uses_explicit_skills_for_flow_and_command_collisions() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        fs::write(repo.path().join(".lf/skills/sync.md"), "Skill, not command").unwrap();
        fs::write(repo.path().join(".lf/skills/ship.md"), "Skill, not Flow").unwrap();
        fs::write(repo.path().join(".lf/flows/ship.yaml"), "- implement\n").unwrap();
        fs::write(
            repo.path().join(".lf/flows/chat.yaml"),
            "- step: sync\n- step: ship\n- step: {name: implement, agent: codex:mini}\n",
        )
        .unwrap();
        let plan = recipe(repo.path(), "chat");
        assert!(plan.contains("Run `lf -b skill -- sync`"));
        assert!(plan.contains("Run `lf -b skill -- ship`"));
        assert!(plan.contains("Run `lf -b -a codex:mini implement`"));
    }

    #[test]
    fn chat_recipe_preserves_shell_characters_and_cli_syntax_in_skill_names() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        for name in ["--help", "don't-expand-$HOME;echo-unsafe"] {
            fs::write(repo.path().join(format!(".lf/skills/{name}.md")), "Work").unwrap();
            fs::write(
                repo.path().join(".lf/flows/chat.yaml"),
                format!("- step: {}\n", serde_json::to_string(name).unwrap()),
            )
            .unwrap();
            let plan = recipe(repo.path(), "chat");
            let command = plan
                .split("1. Run `")
                .nth(1)
                .unwrap()
                .split('`')
                .next()
                .unwrap();
            let argv = shlex::split(command).unwrap();
            let cli = Cli::try_parse_from(normalize_args(argv).unwrap()).unwrap();
            assert!(cli.batch);
            match cli.command.unwrap() {
                Commands::Skill {
                    cmd: SkillCommand::External(args),
                }
                | Commands::External(args) => assert_eq!(args, [name]),
                command => panic!("expected skill invocation, got {command:?}"),
            }
        }
    }
}
