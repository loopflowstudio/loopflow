//! Command ownership, shorthand, and read-only inspection share Clap metadata.
use loopflow_execution::skills::listing::{definition_source, format_written_steps};
use std::path::Path;

use anyhow::Result;
use clap::{Command, CommandFactory, Parser};

use crate::lf::{Cli, Commands, GraphCommand, SkillCommand};
use loopflow_execution::definition::{resolve_definition, DefinitionKind, Target};

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
    if let Some(child) = command.find_subcommand(name) {
        return Ok(Some(vec![child.get_name().to_string()]));
    }
    // An old spelling is not an abbreviation of its neighbor: `session steer`
    // moved to `lf steer`; it never meant `session steers`.
    let old: Vec<String> = prefix.iter().cloned().chain([name.to_string()]).collect();
    if super::aliases::find(&old, &[]).is_some_and(|moved| moved.words.is_empty()) {
        return Ok(None);
    }
    let mut matches = Vec::new();
    if prefix.is_empty() {
        let owner = match name {
            "resume" => Some("session"),
            // `lf release` stays the release command.
            "release" => Some("repository"),
            // `lf set` and `lf unset` stay the everyday spelling of `config set|unset`.
            "set" | "unset" => Some("config"),
            _ => None,
        };
        if let Some(owner) = owner.and_then(|name| command.find_subcommand(name)) {
            let child = owner
                .find_subcommand(name)
                .expect("reserved shortcut exists");
            return Ok(Some(vec![owner.get_name().into(), child.get_name().into()]));
        }
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
            // A command that names its own `--machine` (`lf task move`) keeps it.
            let is_transport = matches!(name, "--machine" | "--forward-agent")
                && (path.is_empty() || flag(current, name).is_none());
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
            if !boundary {
                if let Some(expansion) = resolve_child(current, value, &path).ok().flatten() {
                    for owner in &expansion {
                        current = current
                            .find_subcommand(owner)
                            .expect("resolved child exists");
                    }
                    // Local flag routing must see the alias's canonical owner.
                    remote.extend(expansion.clone());
                    path.extend(expansion);
                    index += 1;
                    continue;
                } else {
                    boundary = true;
                }
            }
            remote.push(value.clone());
        }
        index += 1;
    }
    // Open always presents on the caller. Its Machine selects the Session.
    if path.first().is_some_and(|command| command == "open") {
        return Ok(None);
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
    // Where each word of `path` sits in `output`.
    let mut positions = Vec::new();
    let mut location = Vec::new();
    let mut index = 1;
    let mut boundary = false;
    // Whether `path` ends in a word the tree does not know.
    let mut unknown = false;
    let mut help = false;
    while index < args.len() {
        let value = &args[index];
        if value == "--" {
            // Preserve a definition escape in help's positional path after
            // Clap consumes its own option delimiter.
            if current.get_name() == "help"
                && output
                    .last()
                    .is_some_and(|owner| matches!(owner.as_str(), "skill" | "graph"))
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
                positions.extend(output.len()..output.len() + expansion.len());
                output.extend(expansion.clone());
                path.extend(expansion);
                if current.get_subcommands().next().is_none()
                    && !current.is_allow_external_subcommands_set()
                {
                    boundary = true;
                }
            } else {
                // Keep unknown owner children in help requests too: they must
                // fail identically instead of becoming the owner's help page.
                positions.push(output.len());
                path.push(value.clone());
                output.push(value.clone());
                boundary = true;
                unknown = true;
            }
        } else {
            output.push(value.clone());
        }
        index += 1;
    }
    if let Some(start) = positions.first().copied() {
        // An old spelling is the resolved path, or that path less the unknown
        // word after it: `desktop view <expression> --plan` names `--plan`
        // after the expression the tree could not resolve.
        let splits = if unknown {
            vec![path.len(), path.len() - 1]
        } else {
            vec![path.len()]
        };
        for split in splits {
            let kept = &positions[..split];
            let rest: Vec<String> = (start..output.len())
                .filter(|index| !kept.contains(index))
                .map(|index| output[index].clone())
                .collect();
            let Some(entry) = super::aliases::find(&path[..split], &rest) else {
                continue;
            };
            let mut moved = output[..start].to_vec();
            moved.extend(entry.canonical.iter().map(|word| word.to_string()));
            let mut dropped = entry.words.to_vec();
            for (index, word) in output.iter().enumerate().skip(start) {
                if kept.contains(&index) {
                    continue;
                }
                if let Some(at) = dropped.iter().position(|old| old == word) {
                    dropped.remove(at);
                    continue;
                }
                moved.push(word.clone());
            }
            if help {
                moved.push("--help".into());
            }
            return normalize_args(moved);
        }
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
        Commands::Graph { cmd: GraphCommand::List { inventory, .. } } if inventory.runs
    ) {
        return None;
    }
    if !matches!(
        command,
        Commands::Help { .. }
            | Commands::Docs { .. }
            | Commands::List { .. }
            | Commands::Graph {
                cmd: GraphCommand::List { .. }
                    | GraphCommand::Show { .. }
                    | GraphCommand::Customize { .. }
            }
            | Commands::Skill {
                cmd: SkillCommand::Customize { .. }
            }
    ) {
        return None;
    }
    Some((|| {
        let cwd = std::env::current_dir()?;
        let repo =
            loopflow_execution::repo::discover_repo_root(&cwd)?.unwrap_or_else(|| cwd.clone());
        match command {
            Commands::Help { path, all, json } => {
                if *json || cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&describe_command(path, *all)?)?
                    );
                } else {
                    print!("{}", render_help(path, &repo, *all)?);
                }
            }
            Commands::Docs { topic, json } => {
                crate::lf::commands::docs::show(topic.as_deref(), *json || cli.json)?
            }
            Commands::List { path, json } => crate::lf::commands::list::show(path, &repo, *json)?,
            Commands::Graph {
                cmd: GraphCommand::List { json, inventory },
            } => {
                anyhow::ensure!(
                    inventory.is_empty(),
                    "filters over Graphs that ran require --runs"
                );
                crate::lf::commands::graph::list(&repo, *json)?;
            }
            Commands::Graph {
                cmd: GraphCommand::Show { name, json },
            } => {
                // A name no authored Graph has may be a run's id.
                if let Err(loopflow_execution::error::LoadError::GraphNotFound(_)) =
                    loopflow_execution::graph::load_authored_graph(name, &repo)
                {
                    if let Some(run) = crate::lf::commands::graph_inventory::find(name)? {
                        return crate::lf::commands::graph_inventory::print(&run, *json);
                    }
                }
                if *json {
                    let entry =
                        loopflow_execution::graph::compiled::graph_catalog_entry(name, &repo)?;
                    println!("{}", serde_json::to_string(&entry)?);
                } else {
                    crate::lf::commands::graph::show(name, &repo)?;
                }
            }
            Commands::Graph {
                cmd: GraphCommand::Customize { .. },
            } => {
                crate::lf::commands::mutation::run(
                    cli,
                    &std::env::args().collect::<Vec<_>>(),
                    &cwd,
                    &mut loopflow_execution::command::CommandIo {
                        input: &mut std::io::stdin(),
                        output: &mut std::io::stdout(),
                        error: &mut std::io::stderr(),
                    },
                )?;
            }
            Commands::Skill {
                cmd: SkillCommand::Customize { name, full },
            } => println!(
                "{}",
                loopflow_execution::skills::customize(name, &repo, *full)?.display()
            ),
            _ => unreachable!("inspection command selected above"),
        }
        Ok(())
    })())
}

pub fn resolve_path<'a>(tree: &'a Command, path: &[String]) -> Result<(&'a Command, Vec<String>)> {
    let mut current = tree;
    let mut canonical: Vec<String> = Vec::new();
    for (index, name) in path.iter().enumerate() {
        let Some(expansion) = resolve_child(current, name, &canonical)? else {
            // `lf help <old path>` explains the command it moved to: the old
            // spelling is the canonical path so far plus the words after it.
            for split in index..=path.len() {
                let old = [canonical.as_slice(), &path[index..split]].concat();
                let Some(entry) = super::aliases::find(&old, &path[split..]) else {
                    continue;
                };
                let mut words: Vec<String> = entry
                    .canonical
                    .iter()
                    .map(|word| word.to_string())
                    .collect();
                let mut dropped = entry.words.to_vec();
                for word in &path[split..] {
                    match dropped.iter().position(|old| old == word) {
                        Some(at) => {
                            dropped.remove(at);
                        }
                        None => words.push(word.clone()),
                    }
                }
                return resolve_path(tree, &words);
            }
            return Err(clap::Error::raw(
                clap::error::ErrorKind::InvalidSubcommand,
                format!(
                    "unknown command: lf {}",
                    [canonical.as_slice(), std::slice::from_ref(name)]
                        .concat()
                        .join(" ")
                ),
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

/// Installed syntax only. Runtime support and legal actions remain observations
/// from Desktop/Work; this read opens neither a store nor a native application.
pub fn describe_command(path: &[String], recursive: bool) -> Result<serde_json::Value> {
    fn describe(command: &Command, canonical: &[String], recursive: bool) -> serde_json::Value {
        let arguments = command.get_arguments().filter(|arg| !arg.is_hide_set()).map(|arg| {
            let arity = arg.get_num_args().unwrap_or_else(|| clap::builder::ValueRange::new(usize::from(arg.get_action().takes_values())));
            serde_json::json!({
                "name": arg.get_id().as_str(),
                "long": arg.get_long(),
                "short": arg.get_short().map(|ch| ch.to_string()),
                "position": arg.get_index(),
                "required": arg.is_required_set(),
                "global": arg.is_global_set(),
                "description": arg.get_help().map(ToString::to_string),
                "min_values": arity.min_values(),
                "max_values": (arity.max_values() != usize::MAX).then_some(arity.max_values()),
                "defaults": arg.get_default_values().iter().map(|value| value.to_string_lossy()).collect::<Vec<_>>(),
                "values": arg.get_value_parser().possible_values().map(|values| values.filter(|v| !v.is_hide_set()).map(|v| v.get_name().to_string()).collect::<Vec<_>>()),
                "conflicts": command.get_arg_conflicts_with(arg).iter().map(|a| a.get_id().as_str()).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>();
        let children = command.get_subcommands().filter(|child| !child.is_hide_set()).map(|child| {
            let mut path = canonical.to_vec();
            path.push(child.get_name().into());
            if recursive { describe(child, &path, true) } else {
                serde_json::json!({"command": format!("lf {}", path.join(" ")), "description": child.get_about().map(ToString::to_string)})
            }
        }).collect::<Vec<_>>();
        serde_json::json!({
            "command": std::iter::once("lf").chain(canonical.iter().map(String::as_str)).collect::<Vec<_>>().join(" "),
            "description": command.get_about().map(ToString::to_string),
            "usage": command.clone().render_usage().to_string(),
            "arguments": arguments,
            "commands": children,
            "result_schema": crate::lf::commands::desktop::result_schema(canonical),
        })
    }
    let tree = command_tree();
    let (command, canonical) = resolve_path(&tree, path)?;
    Ok(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "scope": "installed_cli",
        "definition": describe(command, &canonical, recursive),
        "root_arguments": describe(&tree, &[], false)["arguments"],
    }))
}

pub fn render_help(path: &[String], repo: &Path, all: bool) -> Result<String> {
    let tree = command_tree();
    if path.is_empty() {
        if all {
            let mut output = String::from("Usage: lf <command> | <name> [message]\n\n");
            render_tree(&tree, &[], &mut output);
            return Ok(output);
        }
        let names = tree
            .get_subcommands()
            .filter(|cmd| !cmd.is_hide_set())
            .map(Command::get_name)
            .collect::<Vec<_>>();
        let mut output = String::from("Usage: lf <name> [message] | <command>\n\nRun\n  lf pursue [message]        build a change through to a published PR\n  lf graph <name> [message]   select a graph explicitly\n  lf skill <name> [message]  select a skill explicitly\n\nDiscover\n  lf list                   commands, skills, and graphs\n  lf help <path>             explain a command or definition\n  lf help --all              show the complete command tree\n  lf docs [topic]            read embedded user docs\n\nCommands\n");
        for row in names.chunks(5) {
            output.push_str(&format!(
                "  {}\n",
                row.iter()
                    .map(|name| format!("{name:<14}"))
                    .collect::<String>()
                    .trim_end()
            ));
        }
        output.push_str("\nSelect: --repo <name-or-path>, --machine <label-or-id>, --task <task>, --wt <name>, --wave <wave>, --cwd <path>\n--repo resolves names under repo_root (default ~/src) on the selected Machine.\n--machine otherwise uses its saved remote repository.\nWith --machine: --forward-agent\nAgent selection: -a, --agent <harness[:model]> selects this invocation and its Graph steps; unrelated child work keeps its own preferences\n");
        output.push_str("\nOmit owners when a command is unique: lf land → lf pr land.\nCommands take precedence; lf skill NAME and lf graph NAME select definitions.\n");
        return Ok(output);
    }
    let definition = match path {
        [owner, name] => Some((owner, name, false)),
        [owner, delimiter, name] if delimiter == "--" => Some((owner, name, true)),
        _ => None,
    };
    if let Some((owner, name, escaped)) =
        definition.filter(|(owner, _, _)| matches!(owner.as_str(), "skill" | "graph"))
    {
        let command = tree
            .find_subcommand(owner)
            .expect("definition collection exists");
        // Collection verbs win unless explicitly escaped.
        if escaped || resolve_child(command, name, &path[..1])?.is_none() {
            let kind = match owner.as_str() {
                "skill" => Some(DefinitionKind::Skill),
                "graph" => Some(DefinitionKind::Graph),
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

fn definition_help(
    tree: &Command,
    repo: &Path,
    name: &str,
    kind: Option<DefinitionKind>,
) -> Result<String> {
    let target = resolve_definition(repo, name, kind)?;
    let (name, kind, description, source) = match &target {
        Target::Command(_) | Target::Xor(_) | Target::Loop(_) => {
            anyhow::bail!("{name} is not a named skill or graph")
        }
        Target::Skill(skill) => (
            skill.name.as_str(),
            DefinitionKind::Skill,
            skill.content.clone().unwrap_or_default(),
            skill.source.as_ref().map(|source| source.path.clone()),
        ),
        Target::Graph(graph) => {
            loopflow_execution::graph::compile_graph(graph, repo)?;
            let description = format_written_steps(&graph.items);
            (
                graph.name.as_str(),
                DefinitionKind::Graph,
                description,
                loopflow_execution::graph::find_graph_source_path(&graph.name, repo)?,
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
        (DefinitionKind::Skill, Ok(Target::Graph(_))) => {
            output.push_str("\nUntyped lookup selects the same-named graph.\n");
        }
        _ => {
            if matches!(resolve_child(tree, name, &[]), Ok(None)) {
                output.push_str(&format!("  lf {name} [message]\n"));
            }
        }
    }
    if kind == DefinitionKind::Graph {
        match resolve_definition(repo, name, Some(DefinitionKind::Skill)) {
            Ok(Target::Skill(skill)) => output.push_str(&format!(
                "\nAlso available: skill {} (graph wins untyped lookup for {name})\n  {} [message]\n",
                skill.name,
                definition_invocation(tree, &skill.name, DefinitionKind::Skill)
            )),
            Ok(_) => unreachable!("typed skill lookup returns a skill"),
            Err(loopflow_execution::error::LoadError::SkillNotFound(_)) => {}
            Err(error) => output.push_str(&format!("\nSame-named skill unavailable: {error}\n")),
        }
    }
    Ok(output)
}

/// Whether the actual argv names a command that can merge a PR.
pub fn command_merges(argv: &[String]) -> bool {
    normalize_args(argv.to_vec())
        .and_then(Cli::try_parse_from)
        .is_ok_and(|cli| cli.merges())
}

#[cfg(test)]
mod landing_tests {
    use super::command_merges;
    use loopflow_execution::task_graph::CapturedGraph;

    fn write(repo: &std::path::Path, path: &str, content: &str) {
        let path = repo.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    #[test]
    fn a_step_that_merges_is_a_landing_step_however_it_is_spelled() {
        let repo = tempfile::tempdir().unwrap();
        write(
            repo.path(),
            ".lf/graphs/landings.yaml",
            "- cmd: pr publish\n- cmd: land --wait-and-fix\n- cmd: task pr land\n- cmd: pr arm\n",
        );
        let graph = CapturedGraph::load("landings", repo.path())
            .unwrap()
            .unwrap();
        assert!(graph.landing_step("pr publish", command_merges).is_none());
        for step in ["land --wait-and-fix", "pr land", "pr arm"] {
            assert!(graph.landing_step(step, command_merges).is_some(), "{step}");
        }
        let deliver = CapturedGraph::load("deliver", repo.path())
            .unwrap()
            .unwrap();
        assert!(deliver
            .landing_step("land --wait-and-fix", command_merges)
            .is_some());
    }

    #[test]
    fn landing_semantics_keep_shortcuts_flags_and_explicit_definition_kinds() {
        for (words, merges) in [
            ("lf land --wait-and-fix", true),
            ("lf task pr land", false),
            ("lf pr arm", true),
            ("lf pr lan", true),
            ("lf --batch pr land", true),
            ("lf pr land --batch", false),
            ("lf skill -- land", false),
            ("lf skill -- pr-land", false),
            ("lf pr publish", false),
        ] {
            let argv = shlex::split(words).unwrap();
            assert_eq!(command_merges(&argv), merges, "{words}");
        }
    }
}
