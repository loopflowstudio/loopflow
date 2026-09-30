//! Command ownership, shorthand, and read-only inspection share Clap metadata.
use std::path::Path;

use anyhow::Result;
use clap::{Command, CommandFactory};

use crate::lf::discovery::{definition_source, resolve_local_definition, DefinitionKind, Target};
use crate::lf::{Cli, Commands, FlowCommand, SkillCommand};

pub fn command_tree() -> Command {
    let mut command = Cli::command();
    command.build();
    command
}

fn named(command: &Command, name: &str) -> bool {
    command.get_name() == name || command.get_all_aliases().any(|alias| alias == name)
}

fn descendants(command: &Command, name: &str, prefix: &[String], matches: &mut Vec<Vec<String>>) {
    for child in command
        .get_subcommands()
        .filter(|child| !child.is_hide_set())
    {
        let mut path = prefix.to_vec();
        path.push(child.get_name().to_string());
        if named(child, name) {
            matches.push(path.clone());
        }
        descendants(child, name, &path, matches);
    }
}

pub fn resolve_child(
    command: &Command,
    name: &str,
    prefix: &[String],
) -> Result<Option<Vec<String>>, clap::Error> {
    // Hidden callbacks are still exact commands; they never become shortcuts.
    if let Some(child) = command.get_subcommands().find(|child| named(child, name)) {
        return Ok(Some(vec![child.get_name().to_string()]));
    }
    let mut matches = Vec::new();
    descendants(command, name, &[], &mut matches);
    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.pop()),
        _ => {
            let choices = matches
                .iter()
                .map(|path| {
                    format!(
                        "  lf {}",
                        prefix
                            .iter()
                            .chain(path)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            Err(clap::Error::raw(
                clap::error::ErrorKind::InvalidSubcommand,
                format!("ambiguous command '{name}'; use its owner:\n{choices}\n"),
            ))
        }
    }
}

fn flag<'a>(command: &'a Command, value: &str) -> Option<&'a clap::Arg> {
    let value = value.split('=').next().unwrap_or(value);
    command.get_arguments().find(|arg| {
        if let Some(long) = value.strip_prefix("--") {
            arg.get_long() == Some(long)
                || arg.get_all_aliases().unwrap_or_default().contains(&long)
        } else if let Some(short) = value.strip_prefix('-').and_then(|v| v.chars().next()) {
            arg.get_short() == Some(short)
                || arg
                    .get_all_short_aliases()
                    .unwrap_or_default()
                    .contains(&short)
        } else {
            false
        }
    })
}

fn descendant_flag<'a>(command: &'a Command, value: &str) -> Option<&'a clap::Arg> {
    flag(command, value).or_else(|| {
        command
            .get_subcommands()
            .find_map(|child| descendant_flag(child, value))
    })
}

/// Expand command owners and route help before execution or account selection.
pub fn normalize_args(args: Vec<String>) -> Result<Vec<String>, clap::Error> {
    if args.len() < 2 {
        return Ok(args);
    }
    let tree = command_tree();
    let mut current = &tree;
    let mut path = Vec::new();
    let mut output = vec![args[0].clone()];
    let mut index = 1;
    let mut boundary = false;
    let mut help = false;
    while index < args.len() {
        let value = &args[index];
        if value == "--" {
            output.extend_from_slice(&args[index..]);
            break;
        }
        // SSH's target and everything following it belong to the transport.
        if current.get_name() == "ssh" && !value.starts_with('-') {
            output.extend_from_slice(&args[index..]);
            break;
        }
        if value == "--help" || value == "-h" {
            help = true;
            index += 1;
            continue;
        }
        if value.starts_with('-') {
            output.push(value.clone());
            let argument = flag(current, value)
                .or_else(|| flag(&tree, value))
                .or_else(|| {
                    (!boundary)
                        .then(|| descendant_flag(current, value))
                        .flatten()
                });
            let attached_short_value = !value.starts_with("--") && value.len() > 2;
            if argument.is_some_and(|arg| arg.get_action().takes_values())
                && !value.contains('=')
                && !attached_short_value
                && index + 1 < args.len()
            {
                index += 1;
                output.push(args[index].clone());
            }
            index += 1;
            continue;
        }
        if !boundary {
            if let Some(expansion) = resolve_child(current, value, &path)? {
                for owner in &expansion {
                    current = current
                        .find_subcommand(owner)
                        .expect("resolved child exists");
                }
                output.extend(expansion.clone());
                path.extend(expansion);
                if current.get_subcommands().next().is_none()
                    && !current.is_allow_external_subcommands_set()
                    && current.get_name() != "run"
                {
                    boundary = true;
                }
            } else {
                // Keep unknown owner children in help requests too: they must
                // fail identically instead of becoming the owner's help page.
                path.push(value.clone());
                output.push(value.clone());
                boundary = true;
            }
        } else {
            output.push(value.clone());
        }
        index += 1;
    }
    if help {
        let mut request = vec![args[0].clone(), "help".to_string()];
        request.extend(path);
        Ok(request)
    } else {
        Ok(output)
    }
}

pub fn inspect(cli: &Cli) -> Option<Result<()>> {
    let command = cli.command.as_ref()?;
    if !matches!(
        command,
        Commands::Help { .. }
            | Commands::List { .. }
            | Commands::Skill {
                cmd: SkillCommand::List { .. } | SkillCommand::Show { .. }
            }
            | Commands::Flow {
                cmd: FlowCommand::List { .. }
                    | FlowCommand::Show { .. }
                    | FlowCommand::Validate { .. }
            }
    ) {
        return None;
    }
    Some((|| {
        let cwd = std::env::current_dir()?;
        let repo = crate::repo::discover_repo_root(&cwd)?.unwrap_or(cwd);
        match command {
            Commands::Help { path, all } => print!("{}", render_help(path, &repo, *all)?),
            Commands::List { path, json } => crate::lf::commands::list::show(path, &repo, *json)?,
            Commands::Skill {
                cmd: SkillCommand::List { namespace, json },
            } => {
                let mut path = vec!["skill".to_string()];
                path.extend(namespace.iter().cloned());
                crate::lf::commands::list::show(&path, &repo, *json)?;
            }
            Commands::Skill {
                cmd: SkillCommand::Show { name },
            } => print!(
                "{}",
                definition_help(&command_tree(), &repo, name, Some(DefinitionKind::Skill))?
            ),
            Commands::Flow {
                cmd: FlowCommand::List { json },
            } => crate::lf::commands::flow::list(&repo, *json)?,
            Commands::Flow {
                cmd: FlowCommand::Show { name },
            } => crate::lf::commands::flow::show(name, &repo)?,
            Commands::Flow {
                cmd: FlowCommand::Validate { name },
            } => crate::lf::commands::flow::validate(name, &repo)?,
            _ => unreachable!("inspection command selected above"),
        }
        Ok(())
    })())
}

pub fn resolve_path<'a>(tree: &'a Command, path: &[String]) -> Result<(&'a Command, Vec<String>)> {
    let mut current = tree;
    let mut canonical = Vec::new();
    for name in path {
        let Some(expansion) = resolve_child(current, name, &canonical)? else {
            return Err(clap::Error::raw(
                clap::error::ErrorKind::InvalidSubcommand,
                format!("unknown command: lf {}", path.join(" ")),
            )
            .into());
        };
        for owner in &expansion {
            current = current
                .find_subcommand(owner)
                .expect("resolved child exists");
        }
        canonical.extend(expansion);
    }
    Ok((current, canonical))
}

pub fn render_help(path: &[String], repo: &Path, all: bool) -> Result<String> {
    let tree = command_tree();
    if path.is_empty() {
        if all {
            let mut output = String::from("Usage: lf <command> | run <name> [message]\n\n");
            render_tree(&tree, &[], &mut output);
            return Ok(output);
        }
        let names = tree
            .get_subcommands()
            .filter(|cmd| !cmd.is_hide_set())
            .map(Command::get_name)
            .collect::<Vec<_>>();
        let mut output = String::from("Usage: lf <name> [message] | <command>\n\nRun\n  lf feature [message]       carry a change through its authored reviews\n  lf run <name> [message]    select a flow, otherwise a skill\n  lf skill <name> [message]  select a skill explicitly\n\nDiscover\n  lf list                   commands, skills, and flows\n  lf help <path>             explain a command or definition\n  lf help --all              show the complete command tree\n\nCommands\n");
        for row in names.chunks(5) {
            output.push_str(&format!(
                "  {}\n",
                row.iter()
                    .map(|name| format!("{name:<14}"))
                    .collect::<String>()
                    .trim_end()
            ));
        }
        output.push_str("\nOmit owners when a command is unique: lf land → lf pr land.\nCommands take precedence; lf run NAME always selects a definition.\n");
        return Ok(output);
    }
    if path.len() == 2 && matches!(path[0].as_str(), "run" | "skill" | "flow") {
        let owner = tree
            .find_subcommand(&path[0])
            .expect("definition collection exists");
        // Declared collection verbs own their names, except after `run`.
        if path[0] == "run" || resolve_child(owner, &path[1], &path[..1])?.is_none() {
            let kind = match path[0].as_str() {
                "skill" => Some(DefinitionKind::Skill),
                "flow" => Some(DefinitionKind::Flow),
                _ => None,
            };
            return definition_help(&tree, repo, &path[1], kind);
        }
    }
    if path.len() == 1 && resolve_child(&tree, &path[0], &[])?.is_none() {
        return definition_help(&tree, repo, &path[0], None);
    }
    let (command, canonical) = resolve_path(&tree, path)?;
    let mut command = command
        .clone()
        .bin_name(format!("lf {}", canonical.join(" ")));
    Ok(command.render_help().to_string())
}

fn render_tree(command: &Command, prefix: &[String], output: &mut String) {
    for child in command.get_subcommands().filter(|cmd| !cmd.is_hide_set()) {
        let mut path = prefix.to_vec();
        path.push(child.get_name().to_string());
        output.push_str(&format!(
            "lf {}  {}\n",
            path.join(" "),
            child
                .get_about()
                .map(ToString::to_string)
                .unwrap_or_default()
                .lines()
                .next()
                .unwrap_or_default()
        ));
        render_tree(child, &path, output);
    }
}

pub(crate) fn definition_invocation(tree: &Command, name: &str, kind: DefinitionKind) -> String {
    let label = kind.as_str();
    let owner = tree
        .find_subcommand(label)
        .expect("definition kind has a command");
    let escaped =
        resolve_child(owner, name, &[label.to_string()]).map_or(true, |path| path.is_some());
    format!("lf {label} {}{name}", if escaped { "-- " } else { "" })
}

fn definition_help(
    tree: &Command,
    repo: &Path,
    name: &str,
    kind: Option<DefinitionKind>,
) -> Result<String> {
    let target = match resolve_local_definition(repo, name, kind) {
        Ok(target) => target,
        Err(error)
            if name.starts_with("npx/")
                && matches!(
                    error.downcast_ref::<crate::engine::LoadError>(),
                    Some(
                        crate::engine::LoadError::SkillNotFound(_)
                            | crate::engine::LoadError::TargetNotFound(_)
                    )
                ) =>
        {
            return Ok(format!(
                "{name} — not cached locally\nRun `lf skill {name}` to fetch and execute it.\n"
            ));
        }
        Err(error) => return Err(error),
    };
    let (name, kind, description) = match &target {
        Target::Command(_) | Target::Xor(_) => anyhow::bail!("{name} is not a named skill or flow"),
        Target::Skill(skill) => (
            skill.name.as_str(),
            DefinitionKind::Skill,
            skill.content.clone().unwrap_or_default(),
        ),
        Target::Flow(flow) => (
            flow.name.as_str(),
            DefinitionKind::Flow,
            crate::lf::discovery::format_written_steps(&flow.items),
        ),
    };
    let label = kind.as_str();
    let source = definition_source(repo, name, kind);
    let invocation = definition_invocation(tree, name, kind);
    let mut output =
        format!("{name} — {label} ({source})\n{description}\n\n  {invocation} [message]\n");
    match (kind, resolve_local_definition(repo, name, None)) {
        (_, Err(error)) => {
            output.push_str(&format!("\nUntyped lookup fails: {error}\n"));
        }
        (DefinitionKind::Skill, Ok(Target::Flow(_))) => {
            output.push_str("\nUntyped lookup selects the same-named flow.\n");
        }
        _ => {
            output.push_str(&format!("  lf run {name} [message]\n"));
            if matches!(resolve_child(tree, name, &[]), Ok(None)) {
                output.push_str(&format!("  lf {name} [message]\n"));
            }
        }
    }
    if kind == DefinitionKind::Flow
        && resolve_local_definition(repo, name, Some(DefinitionKind::Skill)).is_ok()
    {
        output.push_str(&format!(
            "\nAlso available: skill (flow wins untyped lookup)\n  {} [message]\n",
            definition_invocation(tree, name, DefinitionKind::Skill)
        ));
    }
    Ok(output)
}
