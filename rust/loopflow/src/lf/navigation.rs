//! Command ownership, shorthand, and read-only inspection share Clap metadata.
use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use clap::{Command, CommandFactory, Parser};

use crate::engine::target::{resolve_definition, DefinitionKind, Target};
use crate::engine::{Step, XorPath};
use crate::lf::{Cli, Commands, FlowCommand};

pub fn command_tree() -> Command {
    let mut command = Cli::command();
    command.build();
    command
}

fn descendants(
    command: &Command,
    name: &str,
    prefix: &[String],
    matches: &mut Vec<Vec<String>>,
    abbreviated: bool,
) {
    for child in command
        .get_subcommands()
        .filter(|child| !child.is_hide_set())
    {
        let mut path = prefix.to_vec();
        path.push(child.get_name().to_string());
        if child.get_name() == name || (abbreviated && child.get_name().starts_with(name)) {
            matches.push(path.clone());
        }
        descendants(child, name, &path, matches, abbreviated);
    }
}

pub fn resolve_child(
    command: &Command,
    name: &str,
    prefix: &[String],
) -> Result<Option<Vec<String>>, clap::Error> {
    // Hidden callbacks are still exact commands; they never become shortcuts.
    if let Some(child) = command
        .get_subcommands()
        .find(|child| child.get_name() == name)
    {
        return Ok(Some(vec![child.get_name().to_string()]));
    }
    let mut matches = Vec::new();
    if prefix.is_empty() && name == "resume" && command.find_subcommand("session").is_some() {
        return Ok(Some(vec!["session".into(), "resume".into()]));
    }
    descendants(command, name, &[], &mut matches, false);
    // Exact descendant names win over abbreviations, just as exact owners do.
    if matches.is_empty() {
        descendants(command, name, &[], &mut matches, true);
    }
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
        } else if let Some(short) = value.strip_prefix('-').and_then(|v| v.chars().next()) {
            arg.get_short() == Some(short)
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

fn takes_separate_value(tree: &Command, current: &Command, value: &str, boundary: bool) -> bool {
    if value.contains('=') || (!value.starts_with("--") && value.len() > 2) {
        return false;
    }
    flag(current, value)
        .or_else(|| flag(tree, value))
        .or_else(|| {
            (!boundary)
                .then(|| descendant_flag(current, value))
                .flatten()
        })
        .is_some_and(|arg| arg.get_action().takes_values())
}

/// Remove only transport options; the target owns command parsing and placement.
pub fn machine_invocation(args: &[String]) -> Result<Option<(Cli, Vec<String>)>, clap::Error> {
    let tree = command_tree();
    let mut current = &tree;
    let mut path = Vec::new();
    let mut boundary = false;
    let mut transport = vec!["lf".to_string()];
    let mut remote = Vec::new();
    let mut index = 1;
    while index < args.len() {
        let value = &args[index];
        if value == "--" {
            remote.extend_from_slice(&args[index..]);
            break;
        }
        if value.starts_with('-') {
            let name = value.split('=').next().expect("split has a first item");
            let is_transport = matches!(name, "--machine" | "--forward-agent");
            let start = index;
            if takes_separate_value(&tree, current, value, boundary) && index + 1 < args.len() {
                index += 1;
            }
            let tokens = &args[start..=index];
            if is_transport {
                transport.extend_from_slice(tokens);
            } else {
                remote.extend_from_slice(tokens);
            }
        } else {
            remote.push(value.clone());
            if !boundary {
                if let Some(expansion) = resolve_child(current, value, &path).ok().flatten() {
                    for owner in &expansion {
                        current = current
                            .find_subcommand(owner)
                            .expect("resolved child exists");
                    }
                    path.extend(expansion);
                } else {
                    boundary = true;
                }
            }
        }
        index += 1;
    }
    let cli = Cli::try_parse_from(transport)?;
    Ok(cli.machine.is_some().then_some((cli, remote)))
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
    let mut location = Vec::new();
    let mut index = 1;
    let mut boundary = false;
    let mut help = false;
    while index < args.len() {
        let value = &args[index];
        if value == "--" {
            // Preserve a definition escape in help's positional path after
            // Clap consumes its own option delimiter.
            if current.get_name() == "help"
                && output
                    .last()
                    .is_some_and(|owner| matches!(owner.as_str(), "run" | "skill" | "flow"))
            {
                output.push("--".to_string());
            }
            output.extend_from_slice(&args[index..]);
            break;
        }
        if value == "--help" || value == "-h" {
            help = true;
            index += 1;
            continue;
        }
        if value.starts_with('-') {
            let output_start = output.len();
            let selects_location = matches!(value.split('=').next(), Some("--task" | "--wt"))
                && (path.is_empty() || flag(current, value).is_none());
            output.push(value.clone());
            if takes_separate_value(&tree, current, value, boundary) && index + 1 < args.len() {
                index += 1;
                output.push(args[index].clone());
            }
            if selects_location {
                location.extend_from_slice(&output[output_start..]);
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
        let mut request = vec![args[0].clone()];
        request.extend(location);
        request.push("help".to_string());
        request.extend(path);
        Ok(request)
    } else {
        Ok(output)
    }
}

pub fn inspect(cli: &Cli) -> Option<Result<()>> {
    let command = cli.command.as_ref()?;
    if matches!(command,
        Commands::Flow { cmd: FlowCommand::List { inventory, .. } } if inventory.processes
    ) || matches!(
        command,
        Commands::Flow {
            cmd: FlowCommand::Show {
                processes: true,
                ..
            }
        }
    ) {
        return None;
    }
    if !matches!(
        command,
        Commands::Help { .. }
            | Commands::List { .. }
            | Commands::Flow {
                cmd: FlowCommand::List { .. }
                    | FlowCommand::Show { .. }
                    | FlowCommand::Customize { .. }
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
            Commands::Flow {
                cmd: FlowCommand::List { json, inventory },
            } => {
                anyhow::ensure!(
                    inventory.is_empty(),
                    "filters over Flows that ran require --processes"
                );
                crate::lf::commands::flow::list(&repo, *json)?;
            }
            Commands::Flow {
                cmd: FlowCommand::Show { name, json, .. },
            } => {
                if *json {
                    let entry = crate::engine::flow_graph::flow_catalog_entry(name, &repo)?;
                    println!("{}", serde_json::to_string(&entry)?);
                } else {
                    crate::lf::commands::flow::show(name, &repo)?;
                }
            }
            Commands::Flow {
                cmd: FlowCommand::Customize { name },
            } => println!("{}", crate::engine::flow::customize(name, &repo)?.display()),
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
        let mut output = String::from("Usage: lf <name> [message] | <command>\n\nRun\n  lf pursue [message]        build a change through to a published PR\n  lf run <name> [message]    select a flow, otherwise a skill\n  lf skill <name> [message]  select a skill explicitly\n\nDiscover\n  lf list                   commands, skills, and flows\n  lf help <path>             explain a command or definition\n  lf help --all              show the complete command tree\n\nCommands\n");
        for row in names.chunks(5) {
            output.push_str(&format!(
                "  {}\n",
                row.iter()
                    .map(|name| format!("{name:<14}"))
                    .collect::<String>()
                    .trim_end()
            ));
        }
        output.push_str("\nSelect: --machine <label-or-id>, --task <task>, --wt <name>, --wave <wave>\n--machine runs the command in the saved remote repository.\nWith --machine: --forward-agent\n");
        output.push_str("\nOmit owners when a command is unique: lf land → lf pr land.\nCommands take precedence; lf run NAME always selects a definition.\n");
        return Ok(output);
    }
    let definition = match path {
        [owner, name] => Some((owner, name, false)),
        [owner, delimiter, name] if delimiter == "--" => Some((owner, name, true)),
        _ => None,
    };
    if let Some((owner, name, escaped)) =
        definition.filter(|(owner, _, _)| matches!(owner.as_str(), "run" | "skill" | "flow"))
    {
        let command = tree
            .find_subcommand(owner)
            .expect("definition collection exists");
        // Collection verbs win unless explicitly escaped; `run` always selects a definition.
        if escaped || owner == "run" || resolve_child(command, name, &path[..1])?.is_none() {
            let kind = match owner.as_str() {
                "skill" => Some(DefinitionKind::Skill),
                "flow" => Some(DefinitionKind::Flow),
                _ => None,
            };
            return definition_help(&tree, repo, name, kind);
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

pub(crate) fn definition_source(repo: &Path, path: Option<&Path>) -> String {
    path.map(|path| {
        path.strip_prefix(repo)
            .unwrap_or(path)
            .display()
            .to_string()
    })
    .unwrap_or_else(|| "builtin".to_string())
}

fn format_written_steps(steps: &[Step]) -> String {
    if steps.is_empty() {
        return "∅".to_string();
    }
    steps
        .iter()
        .map(format_written_step)
        .collect::<Vec<_>>()
        .join(" → ")
}

fn format_written_step(step: &Step) -> String {
    match &step.target {
        Target::Flow(flow) => flow.name.clone(),
        target => format_target(target),
    }
}

pub(crate) fn format_target(target: &Target) -> String {
    match target {
        Target::Skill(skill) => skill.name.clone(),
        Target::Command(command) => command.to_string(),
        Target::Flow(flow) => format_written_steps(&flow.items),
        Target::Xor(xor) => format_xor(xor.router.as_deref(), &xor.paths),
    }
}

fn format_xor(router: Option<&str>, paths: &HashMap<String, XorPath>) -> String {
    let label = router.map_or_else(|| "xor".to_string(), |name| format!("xor[{name}]"));
    let mut names: Vec<_> = paths.keys().collect();
    names.sort();
    let rendered = names
        .into_iter()
        .map(|name| format!("{name}: {}", format_written_steps(&paths[name].steps)))
        .collect::<Vec<_>>()
        .join(" | ");
    format!("{label}{{{rendered}}}")
}

fn definition_help(
    tree: &Command,
    repo: &Path,
    name: &str,
    kind: Option<DefinitionKind>,
) -> Result<String> {
    let target = resolve_definition(repo, name, kind)?;
    let (name, kind, description, source) = match &target {
        Target::Command(_) | Target::Xor(_) => anyhow::bail!("{name} is not a named skill or flow"),
        Target::Skill(skill) => (
            skill.name.as_str(),
            DefinitionKind::Skill,
            skill.content.clone().unwrap_or_default(),
            skill.source.as_ref().map(|source| source.path.clone()),
        ),
        Target::Flow(flow) => {
            let mut reviews = crate::engine::human_occurrence_ids(flow, repo)?;
            reviews.sort();
            let mut description = format_written_steps(&flow.items);
            if !reviews.is_empty() {
                description.push_str(&format!("\nReview steps: {}", reviews.join(", ")));
            }
            (
                flow.name.as_str(),
                DefinitionKind::Flow,
                description,
                crate::engine::flow::find_flow_source_path(&flow.name, repo)?,
            )
        }
    };
    let label = kind.as_str();
    let source = definition_source(repo, source.as_deref());
    let invocation = definition_invocation(tree, name, kind);
    let mut output =
        format!("{name} — {label} ({source})\n{description}\n\n  {invocation} [message]\n");
    match (kind, resolve_definition(repo, name, None)) {
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
    if kind == DefinitionKind::Flow {
        match resolve_definition(repo, name, Some(DefinitionKind::Skill)) {
            Ok(Target::Skill(skill)) => output.push_str(&format!(
                "\nAlso available: skill {} (flow wins untyped lookup for {name})\n  {} [message]\n",
                skill.name,
                definition_invocation(tree, &skill.name, DefinitionKind::Skill)
            )),
            Ok(_) => unreachable!("typed skill lookup returns a skill"),
            Err(crate::engine::LoadError::SkillNotFound(_)) => {}
            Err(error) => output.push_str(&format!("\nSame-named skill unavailable: {error}\n")),
        }
    }
    Ok(output)
}
