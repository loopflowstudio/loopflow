use std::collections::{HashMap, HashSet};
use std::io::{IsTerminal, Read};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use clap::Parser;
use tracing::debug;
use tracing_subscriber::EnvFilter;

use loopflow::engine::target::DefinitionKind;
use loopflow::journal::{self, with_runtime, LfEventFields, LfEventType, LfNode};
use loopflow::lf::{
    Cli, Commands, FlowCommand, InstallCommand, SkillCommand, TaskCommand, WaveCommand,
};

use loopflow::ops::project::update_plan;

#[derive(Clone, Default)]
struct FlagTables {
    /// Flags that take a value (the next arg belongs to them).
    value: HashSet<String>,
    /// Boolean flags (no value).
    boolean: HashSet<String>,
}

impl FlagTables {
    fn insert(&mut self, arg: &clap::Arg) {
        let flags = if arg.get_action().takes_values() {
            &mut self.value
        } else {
            &mut self.boolean
        };
        if let Some(short) = arg.get_short() {
            flags.insert(format!("-{short}"));
        }
        if let Some(long) = arg.get_long() {
            flags.insert(format!("--{long}"));
        }
    }

    fn contains(&self, arg: &str) -> bool {
        self.boolean.contains(flag_name(arg)) || self.takes_value(arg)
    }

    fn takes_value(&self, arg: &str) -> bool {
        self.value.contains(flag_name(arg))
    }

    fn extend(&mut self, other: &Self) {
        self.value.extend(other.value.iter().cloned());
        self.boolean.extend(other.boolean.iter().cloned());
    }
}

struct CommandArgTables {
    /// Flags owned directly by this command.
    direct: FlagTables,
    /// Flags owned here or by any descendant, used only to find the command path.
    recursive: FlagTables,
    /// Direct subcommands, indexed by canonical name.
    subcommands: HashMap<String, CommandArgTables>,
}

fn command_arg_tables(command: &clap::Command) -> CommandArgTables {
    let mut direct = FlagTables::default();
    for arg in command.get_arguments() {
        direct.insert(arg);
    }

    let mut recursive = direct.clone();
    let mut subcommands = HashMap::new();
    for subcommand in command.get_subcommands() {
        let child = command_arg_tables(subcommand);
        recursive.extend(&child.recursive);
        subcommands.insert(subcommand.get_name().to_string(), child);
    }

    CommandArgTables {
        direct,
        recursive,
        subcommands,
    }
}

/// Derive flag ownership from the same command tree used for navigation.
fn arg_tables() -> &'static CommandArgTables {
    static TABLES: OnceLock<CommandArgTables> = OnceLock::new();
    TABLES.get_or_init(|| command_arg_tables(&loopflow::lf::navigation::command_tree()))
}

fn flag_name(arg: &str) -> &str {
    arg.split_once('=').map_or(arg, |(name, _)| name)
}

fn has_inline_value(arg: &str) -> bool {
    arg.starts_with('-') && arg.contains('=')
}

fn is_value_flag(arg: &str) -> bool {
    arg_tables().direct.takes_value(arg)
}

fn is_known_flag(arg: &str) -> bool {
    arg_tables().direct.contains(arg)
}

fn push_flag(args: &[String], output: &mut Vec<String>, index: &mut usize, takes_value: bool) {
    output.push(args[*index].clone());
    if takes_value && !has_inline_value(&args[*index]) && *index + 1 < args.len() {
        *index += 1;
        output.push(args[*index].clone());
    }
}

fn first_target_index(args: &[String]) -> Option<usize> {
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" {
            return None;
        }
        if !arg.starts_with('-') {
            return Some(index);
        }
        if is_value_flag(arg) && !has_inline_value(arg) {
            index += 1;
        }
        index += 1;
    }
    None
}

#[derive(Clone, Copy)]
struct SelectedCommand<'a> {
    index: usize,
    args: &'a CommandArgTables,
}

fn selected_command_path<'a>(
    rest: &[String],
    command_index: usize,
    command: &'a CommandArgTables,
) -> Vec<SelectedCommand<'a>> {
    let mut path = vec![SelectedCommand {
        index: command_index,
        args: command,
    }];
    let mut index = command_index + 1;

    while index < rest.len() {
        let arg = &rest[index];
        if arg == "--" {
            break;
        }

        let current = path.last().expect("command path is never empty").args;
        if arg.starts_with('-') {
            let recognized = current.recursive.contains(arg) || is_known_flag(arg);
            let takes_value = current.recursive.takes_value(arg) || is_value_flag(arg);
            if recognized && takes_value && !has_inline_value(arg) && index + 1 < rest.len() {
                index += 1;
            }
            index += 1;
            continue;
        }

        let Some(child) = current.subcommands.get(arg) else {
            break;
        };
        path.push(SelectedCommand { index, args: child });
        index += 1;
    }

    path
}

fn deepest_command_before(path: &[SelectedCommand<'_>], index: usize) -> usize {
    path.iter()
        .rposition(|command| command.index < index)
        .expect("the top-level command precedes its arguments")
}

fn local_flag_owner(path: &[SelectedCommand<'_>], arg: &str, current: usize) -> Option<usize> {
    if path[current].args.direct.contains(arg) {
        return Some(current);
    }
    path.iter()
        .rposition(|command| command.args.direct.contains(arg))
}

fn reorder_command_args(
    program: String,
    rest: &[String],
    command_index: usize,
    command: &CommandArgTables,
) -> Vec<String> {
    let path = selected_command_path(rest, command_index, command);
    let mut moved_globals = Vec::new();
    let mut moved_locals: HashMap<usize, Vec<String>> = HashMap::new();
    let mut retained = vec![true; rest.len()];
    let mut index = command_index + 1;

    while index < rest.len() {
        let arg = &rest[index];
        if arg == "--" {
            break;
        }

        let current = deepest_command_before(&path, index);
        let local_owner = local_flag_owner(&path, arg, current);
        let destination = if let Some(owner) = local_owner {
            (owner != current).then_some(Some(owner))
        } else if is_known_flag(arg) {
            Some(None)
        } else {
            None
        };

        let Some(destination) = destination else {
            index += 1;
            continue;
        };

        let takes_value = destination.map_or_else(
            || is_value_flag(arg),
            |owner| path[owner].args.direct.takes_value(arg),
        );
        let mut moved = Vec::new();
        push_flag(rest, &mut moved, &mut index, takes_value);
        let moved_start = index + 1 - moved.len();
        retained[moved_start..=index].fill(false);
        if let Some(owner) = destination {
            moved_locals.entry(owner).or_default().extend(moved);
        } else {
            moved_globals.extend(moved);
        }
        index += 1;
    }

    let mut result = vec![program];
    result.extend_from_slice(&rest[..command_index]);
    result.extend(moved_globals);
    for (index, arg) in rest.iter().enumerate().skip(command_index) {
        if retained[index] {
            result.push(arg.clone());
        }
        if let Some(owner) = path.iter().position(|command| command.index == index) {
            if let Some(flags) = moved_locals.remove(&owner) {
                result.extend(flags);
            }
        }
    }
    result
}

/// Accept top-level flags on either side of a target when their meaning is
/// unambiguous. Local command flags win collisions, and `--` ends reordering.
fn reorder_args(args: Vec<String>) -> Vec<String> {
    if args.len() <= 1 {
        return args;
    }

    let program = args[0].clone();
    let rest = &args[1..];

    let Some(target_index) = first_target_index(rest) else {
        return args;
    };
    if let Some(command) = arg_tables().subcommands.get(rest[target_index].as_str()) {
        return reorder_command_args(program, rest, target_index, command);
    }

    // Find where the skill name is and collect flags that come after it
    let mut flags_before: Vec<String> = Vec::new();
    let mut skill_and_args: Vec<String> = Vec::new();
    let mut flags_after: Vec<String> = Vec::new();

    let mut i = 0;
    let mut found_skill = false;

    while i < rest.len() {
        let arg = &rest[i];

        if arg == "--" {
            skill_and_args.extend_from_slice(&rest[i..]);
            break;
        }

        if !found_skill {
            if arg.starts_with('-') {
                // It's a flag before the skill
                flags_before.push(arg.clone());
                if is_value_flag(arg) && !has_inline_value(arg) && i + 1 < rest.len() {
                    i += 1;
                    flags_before.push(rest[i].clone());
                }
            } else {
                // Found the skill name
                found_skill = true;
                skill_and_args.push(arg.clone());
            }
        } else {
            // After the skill name
            if arg.starts_with('-') {
                // Check if it's a known lf flag
                if is_known_flag(arg) {
                    flags_after.push(arg.clone());
                    if is_value_flag(arg) && !has_inline_value(arg) && i + 1 < rest.len() {
                        i += 1;
                        flags_after.push(rest[i].clone());
                    }
                } else {
                    // Unknown flag - treat as skill arg
                    skill_and_args.push(arg.clone());
                }
            } else {
                // Non-flag after skill - it's a skill arg
                skill_and_args.push(arg.clone());
            }
        }
        i += 1;
    }

    // Reconstruct: program + flags_before + flags_after + skill_and_args
    let mut result = vec![program];
    result.extend(flags_before);
    result.extend(flags_after);
    result.extend(skill_and_args);
    result
}

fn join_args(args: &[String]) -> Option<String> {
    if args.is_empty() {
        None
    } else {
        Some(args.join(" "))
    }
}

fn in_repo_runtime<T>(
    command: &[String],
    run: impl FnOnce(&std::path::Path) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let repo_root = loopflow::lf::commands::util::find_repo_root()?;
    with_runtime(&repo_root, command, || run(&repo_root))
}

fn in_directory_runtime<T>(
    command: &[String],
    run: impl FnOnce(&Path) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let directory = loopflow::repo::working_directory()?;
    with_runtime(&directory, command, || run(&directory))
}

fn run_default_agent(cli: &Cli, command: &[String]) -> anyhow::Result<()> {
    let repo_root = loopflow::lf::commands::util::find_repo_root()?;
    let moved = loopflow::engine::worktrees::move_default_agent_to_worktree(&repo_root)?;
    match moved {
        Some(worktree) => {
            eprintln!("moved to `{}`", worktree.path.display());
            let _cwd = CwdGuard::enter(&worktree.path)?;
            with_runtime(&worktree.path, command, || {
                loopflow::lf::commands::run::run(&worktree.path, Some("default"), None, cli)
            })
        }
        None => with_runtime(&repo_root, command, || {
            loopflow::lf::commands::run::run(&repo_root, Some("default"), None, cli)
        }),
    }
}

fn with_skill_runtime<T>(
    repo_root: &std::path::Path,
    skill_name: &str,
    run: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    journal::emit(
        repo_root,
        LfNode::Skill,
        LfEventType::Started,
        LfEventFields {
            skill: Some(skill_name.to_string()),
            index: Some(0),
            ..LfEventFields::default()
        },
    );
    let result = run();
    match &result {
        Ok(_) => journal::emit(
            repo_root,
            LfNode::Skill,
            LfEventType::Completed,
            LfEventFields {
                skill: Some(skill_name.to_string()),
                index: Some(0),
                ..LfEventFields::default()
            },
        ),
        Err(err) => journal::emit(
            repo_root,
            LfNode::Skill,
            LfEventType::Errored,
            LfEventFields {
                skill: Some(skill_name.to_string()),
                index: Some(0),
                error: Some(err.to_string()),
                ..LfEventFields::default()
            },
        ),
    }
    result
}

fn resolve_cli_target(
    cli: &mut Cli,
    args: &[String],
) -> anyhow::Result<Option<(loopflow::engine::target::Target, Option<String>)>> {
    use loopflow::engine::target::Target;

    let (name, kind, message) = match &cli.command {
        Some(Commands::Flow {
            cmd: FlowCommand::External(rest),
        })
        | Some(Commands::Skill {
            cmd: SkillCommand::External(rest),
        }) => {
            let (name, messages) = loopflow::lf::commands::run::split_skill_args(rest)?;
            let kind = if matches!(cli.command, Some(Commands::Flow { .. })) {
                DefinitionKind::Flow
            } else {
                DefinitionKind::Skill
            };
            (name, Some(kind), join_args(&messages))
        }
        Some(Commands::Run { name, args: rest }) => (name.clone(), None, join_args(rest)),
        Some(Commands::External(rest)) => {
            let (name, messages) = loopflow::lf::commands::run::split_skill_args(rest)?;
            (name, None, join_args(&messages))
        }
        Some(_) => {
            let index =
                first_target_index(&args[1..]).expect("parsed builtin has a command token") + 1;
            return Ok(Some((
                Target::Command(loopflow::engine::Command {
                    command: args[index].clone(),
                    args: args[index + 1..].to_vec(),
                }),
                None,
            )));
        }
        None => return Ok(None),
    };
    let repo = loopflow::repo::working_directory()?;
    if let Some(path) = &cli.skill_input {
        let invocation = loopflow::engine::skill_invocation::SkillInvocation::read(path)?;
        anyhow::ensure!(
            invocation.skill.name == name,
            "captured skill does not match {name}"
        );
        let target = Target::Skill(invocation.skill.clone());
        cli.resolved_invocation = Some(invocation);
        return Ok(Some((target, message)));
    }
    let target = loopflow::engine::target::resolve_definition(&repo, &name, kind)?;
    Ok(Some((target, message)))
}

fn execute_target(
    target: loopflow::engine::target::Target,
    message: Option<&str>,
    cli: &Cli,
    args: &[String],
    binding: Option<&loopflow::ops::WorkBinding>,
) -> anyhow::Result<()> {
    use loopflow::engine::target::Target;

    match target {
        Target::Command(command) => execute_command(&command, cli, args, binding),
        Target::Skill(skill) => {
            let repo_root = loopflow::repo::working_directory()?;
            let name = skill.name.as_str();
            let mut selected = cli.process_options();
            selected.resolved_invocation.get_or_insert_with(|| {
                loopflow::engine::skill_invocation::SkillInvocation {
                    skill: skill.clone(),
                    arguments: message.unwrap_or_default().to_string(),
                }
            });
            let cli = &selected;
            with_runtime(&repo_root, args, || {
                with_skill_runtime(&repo_root, name, || {
                    let shared = binding.is_some()
                        || loopflow::lf::commands::run::implicit_binding(cli)?.is_some();
                    match binding {
                        Some(binding) => loopflow::lf::commands::run::run_bound(
                            Some(name),
                            message,
                            cli,
                            binding,
                        )?,
                        None => {
                            loopflow::lf::commands::run::run(&repo_root, Some(name), message, cli)?
                        }
                    }
                    // Shared contributions leave checkpoint composition to the caller.
                    if !shared
                        && !loopflow::journal::has_caller()
                        && loopflow::repo::discover_repo_root(&repo_root)?.is_some()
                    {
                        let options = loopflow::ops::CommitOptions {
                            add: true,
                            message: Some(format!("lf commit: {name}")),
                            ..loopflow::ops::CommitOptions::for_task(name)
                        };
                        loopflow::ops::commit_workflow(
                            &repo_root,
                            &options,
                            &loopflow::ops::NullProgress,
                            &|_| {},
                        )?;
                    }
                    Ok(())
                })
            })
        }
        composed => {
            let flow = composed.into_flow();
            let repo_root = loopflow::repo::working_directory()?;
            with_runtime(&repo_root, args, || {
                loopflow::lf::commands::flow::run(&flow, message, cli, &repo_root, binding)
            })
        }
    }
}

fn prepare_work_binding(selector: &str, repo: &Path) -> anyhow::Result<loopflow::ops::WorkBinding> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|error| anyhow::anyhow!("cannot resolve {selector}: {error}"))?;
    runtime.block_on(async {
        let store = loopflow::store::open_existing_store()
            .await
            .ok_or_else(|| {
                anyhow::anyhow!("cannot resolve {selector}: planning registry unavailable")
            })?;
        let store = Arc::new(store);
        loopflow::ops::resolve_work_binding(&store, repo, selector)
            .await
            .map_err(anyhow::Error::from)
    })
}

struct CwdGuard(std::path::PathBuf);

impl CwdGuard {
    fn enter(path: &Path) -> anyhow::Result<Self> {
        if !path.is_absolute() {
            anyhow::bail!("bound Work cwd must be absolute: {}", path.display());
        }
        let previous = std::env::current_dir()?;
        std::env::set_current_dir(path)
            .map_err(|error| anyhow::anyhow!("enter bound Work cwd {}: {error}", path.display()))?;
        Ok(Self(previous))
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl Into<String>) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value.into());
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.previous {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn parse_duration(value: &str) -> anyhow::Result<std::time::Duration> {
    let value = value.trim();
    let (number, multiplier) = if let Some(number) = value.strip_suffix('s') {
        (number, 1)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60)
    } else if let Some(number) = value.strip_suffix('h') {
        (number, 60 * 60)
    } else {
        (value, 1)
    };
    let amount: u64 = number
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid duration {value:?}; use seconds, 10s, 5m, or 1h"))?;
    Ok(std::time::Duration::from_secs(
        amount.saturating_mul(multiplier),
    ))
}

/// One PR's line in `lf task status`. A degraded Linear linkage is named here
/// because this reading is where an operator already looks for writeback health —
/// the Task's `PM writeback` line sits directly above. Silence means linked.
fn format_task_pr_line(pr: &loopflow::work::task::TaskPr) -> String {
    let provider = pr
        .github()
        .map(|github| format!("GitHub #{}", github.number))
        .unwrap_or_else(|| "not opened on GitHub".to_string());
    let placement = pr
        .parent_pr_id
        .as_ref()
        .map(|parent| format!("  stacked on {parent}"))
        .unwrap_or_default();
    let linkage = pr
        .linear_link_error
        .as_ref()
        .map(|error| format!("  Linear link degraded: {error}"))
        .unwrap_or_default();
    format!(
        "  PR {}: {}  {}  {}{}{}",
        pr.sequence,
        pr.phase().as_str(),
        provider,
        pr.branch,
        placement,
        linkage,
    )
}

fn print_task(task: &loopflow::work::task::Task, json: bool) -> anyhow::Result<()> {
    let snapshot = loopflow::ops::task::task_snapshot(task)?;
    print_task_snapshot(&snapshot, json)
}

fn print_task_snapshot(
    snapshot: &loopflow::ops::task::TaskSnapshot,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
    } else {
        let pm_writeback = match &snapshot.pm_writeback {
            loopflow::work::task::PmWritebackState::Current => "current".to_string(),
            loopflow::work::task::PmWritebackState::Pending { error, .. } => {
                format!("pending: {error}")
            }
        };
        let branch = snapshot
            .active_pr
            .as_ref()
            .and_then(|active| snapshot.prs.iter().find(|pr| &pr.id == active))
            .map(|pr| pr.branch.as_str())
            .unwrap_or("none");
        let body = format!(
            "agent {}, provider {}",
            snapshot.agent.as_deref().unwrap_or("default"),
            snapshot.provider
        );
        println!(
            "{}  {}\n  task: {}\n  body: {}\n  worktree: {}\n  branch: {}\n  PM writeback: {}",
            snapshot.issue_identifier,
            snapshot.status,
            snapshot.task_id,
            body,
            snapshot.worktree.as_deref().unwrap_or("unplaced"),
            branch,
            pm_writeback,
        );
        if let Some(workflow) = &snapshot.work.workflow {
            println!("  {}", workflow.summary());
        }
        if let Some(conflict) = &snapshot.planning_conflict {
            println!("  error: {conflict}");
        }
        println!("  latest Flow: {}", snapshot.execution.reason);
        if let Some(run) = &snapshot.execution.captured {
            println!("  Session event: {run}");
        }
        for session in &snapshot.work.sessions {
            println!(
                "  Session: {}  {}  {}",
                session.id,
                session.title,
                if session.completed_at.is_some() {
                    "completed"
                } else {
                    "open"
                },
            );
        }
        for flow in &snapshot.work.flow_processes {
            println!(
                "  Flow: {}  {}  {:?}",
                flow.summary.id, flow.summary.name, flow.summary.state,
            );
        }
        for process in &snapshot.work.processes {
            println!(
                "  Process: {}  {}  {}",
                process.lfid,
                process.command.as_deref().unwrap_or("unknown command"),
                process.outcome.as_deref().unwrap_or("unknown")
            );
        }
        println!("  project: {}", snapshot.project_id);
        for pr in &snapshot.prs {
            println!("{}", format_task_pr_line(pr));
        }
        match &snapshot.observation {
            loopflow::work::task::Observation::Cached { observed_at } => {
                println!("  PR observation: cached from {observed_at}");
            }
            loopflow::work::task::Observation::Degraded {
                reason,
                cached_as_of,
                retry_at,
            } => {
                println!(
                    "  PR observation: degraded — {reason} (cached from {cached_as_of}; retry after {retry_at})"
                );
            }
            loopflow::work::task::Observation::NotRequired
            | loopflow::work::task::Observation::Fresh { .. } => {}
        }
        let actions = &snapshot.actions;
        if let Some(recommended) = actions.recommended {
            println!("  action: {}  ({})", recommended.as_str(), actions.reason);
        }
    }
    Ok(())
}

fn print_task_control(
    result: &loopflow::ops::task::TaskControlResult,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(result)?);
    } else {
        println!(
            "{} → {} ({})",
            result.receipt.label(),
            result.issue_id,
            result.receipt.action(),
        );
        match &result.observation {
            loopflow::work::task::Observation::Cached { observed_at } => {
                println!("PR observation: cached from {observed_at}");
            }
            loopflow::work::task::Observation::Degraded {
                reason,
                cached_as_of,
                retry_at,
            } => {
                println!(
                    "PR observation: degraded — {reason} (cached from {cached_as_of}; retry after {retry_at})"
                );
            }
            loopflow::work::task::Observation::NotRequired
            | loopflow::work::task::Observation::Fresh { .. } => {}
        }
    }
    Ok(())
}

fn run_wave_command(repo: &Path, command: &WaveCommand) -> anyhow::Result<()> {
    match command {
        WaveCommand::NewChapter {
            wave,
            name,
            plan,
            dry_run,
            json,
        } => {
            let rotation =
                loopflow::ops::chapter::new_chapter(repo, name, plan, Some(wave), *dry_run)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&rotation)?);
            } else {
                println!(
                    "Chapter {}: {}",
                    rotation.name, rotation.waves[0].successor_id
                );
            }
            Ok(())
        }
        WaveCommand::Ensure { wave, json } => {
            let result = tokio::runtime::Runtime::new()?
                .block_on(loopflow::ops::project::ensure(repo, wave))?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "Wave {wave}: Project {} ({}) is active locally",
                    result.name, result.id
                );
            }
            Ok(())
        }
        WaveCommand::BindProject {
            wave,
            project,
            json,
        } => {
            let result = tokio::runtime::Runtime::new()?
                .block_on(loopflow::ops::project::bind_project(repo, wave, project))?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Wave {wave}: bound Project {} ({})", result.name, result.id);
            }
            Ok(())
        }
        WaveCommand::Cron { .. } => unreachable!("cron dispatches separately"),
        WaveCommand::List { .. } | WaveCommand::Status { .. } => {
            unreachable!("read commands dispatch separately")
        }
        WaveCommand::Place { .. } | WaveCommand::Rename { .. } => {
            loopflow::lf::commands::placement::wave(repo, command)
        }
        WaveCommand::Edit { wave, goal, memory } => {
            for (document, path) in [("GOAL.md", goal), ("MEMORY.md", memory)] {
                if let Some(path) = path {
                    loopflow::work::wave::config::write_wave_document(
                        repo,
                        wave,
                        document,
                        &std::fs::read_to_string(path)?,
                    )?;
                }
            }
            Ok(())
        }
        WaveCommand::UpdatePlan { wave, plan } => {
            let saved = tokio::runtime::Runtime::new()?.block_on(update_plan(
                repo,
                wave.as_deref(),
                serde_json::from_slice(&std::fs::read(plan)?)?,
            ))?;
            print_planning_sync(&saved.sync);
            Ok(())
        }
    }
}

fn read_task_draft() -> anyhow::Result<String> {
    let limit = loopflow::ops::task::MAX_FILE_BYTES;
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= limit, "Draft exceeds 1 MB");
    Ok(String::from_utf8(bytes)?)
}

fn run_task_command(repo: &Path, command: &TaskCommand) -> anyhow::Result<()> {
    match command {
        TaskCommand::Automation { json } => {
            let status = loopflow::ops::task_automation::status(repo)?;
            if *json {
                println!("{}", serde_json::to_string(&status)?);
            } else {
                println!(
                    "{} · every {} seconds",
                    status.coverage, status.cadence_seconds
                );
                for task in status.tasks {
                    println!(
                        "{}: {}",
                        task.issue,
                        if task.enabled == Some(false) {
                            "CI repair held"
                        } else {
                            "CI repair enabled"
                        }
                    );
                }
            }
            Ok(())
        }
        TaskCommand::Reconcile { json } => {
            let result = loopflow::ops::pr_landing::reconcile_repository(repo)?;
            if *json {
                println!("{}", serde_json::to_string(&result)?);
            }
            if !result.errors.is_empty() {
                anyhow::bail!("{}", result.errors.join("\n"));
            }
            Ok(())
        }
        TaskCommand::FollowUp {
            issue,
            outcome,
            evidence,
            check_at,
            clear,
        } => {
            let remaining = match (outcome, evidence, check_at) {
                (Some(outcome), Some(evidence), Some(at)) => {
                    Some(loopflow::work::task::TaskFollowUp {
                        outcome: outcome.clone(),
                        evidence: evidence.clone(),
                        check_at: time::OffsetDateTime::parse(
                            at,
                            &time::format_description::well_known::Rfc3339,
                        )?
                        .unix_timestamp(),
                    })
                }
                _ => None,
            };
            println!(
                "{}",
                loopflow::ops::task::task_follow_up(
                    issue,
                    remaining,
                    clear
                        .as_deref()
                        .unwrap_or("Accepted work remains after delivery")
                )?
            );
            Ok(())
        }
        TaskCommand::Automate { issue, state } => Ok(loopflow::ops::task_automation::select(
            issue,
            state == "on",
        )?),
        TaskCommand::Repair { incident, launcher } => {
            Ok(loopflow::ops::pr_landing::run_repair(incident, launcher)?)
        }
        TaskCommand::Checkout {
            issue,
            name,
            stack_on,
            directive,
            json,
        } => {
            let task = loopflow::ops::task::task_checkout(
                repo,
                issue,
                loopflow::ops::task::TaskCheckoutOptions {
                    name: name.clone(),
                    stack_on: stack_on.clone(),
                    directive: directive.clone(),
                },
            )?;
            print_task(&task, *json)
        }
        TaskCommand::Workflow { cmd } => match cmd {
            loopflow::lf::TaskWorkflowCommand::Show { issue, json: _ } => {
                let workflow = loopflow::ops::task::workflow_show(issue)?;
                println!("{}", serde_json::to_string_pretty(&workflow)?);
                Ok(())
            }
            loopflow::lf::TaskWorkflowCommand::Restart { issue } => {
                println!(
                    "{}",
                    loopflow::ops::task::workflow_set(
                        repo,
                        issue,
                        "start",
                        Some("Restart Workflow"),
                        &loopflow::ops::task::EndOptions::default()
                    )?
                );
                Ok(())
            }
        },
        TaskCommand::Sync { issue } => {
            println!("{}", loopflow::ops::task::task_sync(issue)?);
            Ok(())
        }
        TaskCommand::Run { .. } => unreachable!("task run dispatches as an ordinary run"),
        TaskCommand::Move {
            issue,
            node,
            reason,
            force,
        } => {
            println!(
                "{}",
                loopflow::ops::task::workflow_set(
                    repo,
                    issue,
                    node,
                    reason.as_deref(),
                    &loopflow::ops::task::EndOptions { force: *force },
                )?
            );
            Ok(())
        }
        TaskCommand::Create {
            wave,
            title,
            notes,
            json,
        } => {
            let report = match notes {
                Some(notes) => Some(notes.clone()),
                None => piped_task_report()?,
            };
            let issue =
                loopflow::ops::task::task_create(repo, wave.as_deref(), title.clone(), report)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&issue)?);
            } else {
                println!("{} · {}", issue.id, issue.name);
            }
            if loopflow::engine::config::load_config_or_default(Some(repo))
                .pm
                .and_then(|pm| pm.linear_team)
                .is_some()
            {
                eprintln!("Saved locally; pending Linear sync.");
            }
            Ok(())
        }
        TaskCommand::Status { issue, json } => {
            let status = loopflow::ops::task::task_status(repo, issue.as_deref())?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!(
                    "Planning evidence: {}",
                    serde_json::to_value(status.planning_state)?
                        .as_str()
                        .expect("planning state is a string")
                );
                if let Some(planning) = &status.planning {
                    println!("{} · {}", planning.item.identifier, planning.item.name);
                    println!(
                        "Planning: {} · {} · observed at {}",
                        planning.item.state.as_deref().unwrap_or("unknown"),
                        if status.planning_stale {
                            "stale"
                        } else {
                            "current"
                        },
                        planning.observed_at
                    );
                    if planning.project.is_none() {
                        println!("No Project assigned; managed work requires ownership.");
                    }
                }
                if let Some(sync) = &status.sync {
                    for line in sync.lines() {
                        println!("{line}");
                    }
                }
                if let Some(error) = &status.planning_error {
                    println!("Planning: {error}");
                }
                if let Some(execution) = &status.execution {
                    print_task_snapshot(execution, false)?;
                } else {
                    println!("No execution allocated.");
                }
            }
            Ok(())
        }
        TaskCommand::Diff {
            issue,
            base,
            json,
            files: true,
            ..
        } => {
            let snapshot = loopflow::ops::task::task_changes(issue, base)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&snapshot)?);
            } else if snapshot.files.is_empty() {
                println!("{} has no changes from its recorded base", issue);
            } else {
                for file in snapshot.files {
                    let mut states = Vec::new();
                    if file.committed {
                        states.push("committed");
                    }
                    if file.staged {
                        states.push("staged");
                    }
                    if file.unstaged {
                        states.push("unstaged");
                    }
                    if file.untracked {
                        states.push("untracked");
                    }
                    println!("{}\t{}", states.join(","), file.path);
                }
            }
            Ok(())
        }
        TaskCommand::Diff {
            issue,
            path,
            base,
            draft,
            json,
            files: false,
        } => {
            let content = draft.then(read_task_draft).transpose()?;
            let snapshot =
                loopflow::ops::task::task_diff(issue, path.as_deref(), base, content.as_deref())?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&snapshot)?);
            } else {
                print!("{}", snapshot.patch);
                if snapshot.truncated {
                    eprintln!("\n[diff truncated at 1 MB]");
                }
            }
            Ok(())
        }
        TaskCommand::Files {
            issue,
            directory,
            cursor,
            show_ignored,
            json,
        } => {
            let snapshot = loopflow::ops::task::task_files(
                issue,
                directory,
                cursor.as_deref(),
                *show_ignored,
            )?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&snapshot)?);
            } else {
                for entry in snapshot.entries {
                    println!("{}", entry.path);
                }
                if let Some(cursor) = snapshot.next_cursor {
                    eprintln!("Next page: --cursor {cursor}");
                }
            }
            Ok(())
        }
        TaskCommand::File {
            issue,
            path,
            recoveries,
            json,
        } => {
            let snapshot = loopflow::ops::task::task_file(issue, path, *recoveries)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&snapshot)?);
            } else if snapshot.state != loopflow::ops::task::TaskFileState::Text {
                anyhow::bail!("{}: {:?}", snapshot.path, snapshot.state);
            } else {
                print!("{}", snapshot.content.as_deref().unwrap_or_default());
            }
            Ok(())
        }
        TaskCommand::Save {
            issue,
            path,
            revision,
            json,
        } => {
            let content = read_task_draft()?;
            let saved = loopflow::ops::task::task_save(issue, path, revision, &content)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&saved)?);
            } else {
                println!("{}", saved.message);
            }
            Ok(())
        }
        TaskCommand::Abandon { issue, force, json } => {
            let identifier = loopflow::ops::task::task_abandon(repo, issue.as_deref(), *force)?;
            if *json {
                println!("{}", serde_json::to_string(&identifier)?);
            } else {
                println!("{identifier}: canceled; PRs and branches removed");
            }
            Ok(())
        }
        TaskCommand::Sweep { apply, json } => {
            let entries = loopflow::ops::task::task_sweep(repo, *apply)?;
            let incomplete = entries
                .iter()
                .any(|entry| entry.outcome.starts_with("incomplete:"));
            if *json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                for entry in entries {
                    println!(
                        "{} / {}: {} — {}",
                        entry.wave,
                        entry.project,
                        entry.issue.as_deref().unwrap_or("Project"),
                        entry.outcome
                    );
                }
            }
            if incomplete {
                anyhow::bail!(
                    "chapter sweep has incomplete cancellations; see the reported retry commands"
                );
            }
            Ok(())
        }
        TaskCommand::Delete { issue } => {
            let identifier = loopflow::ops::task::task_delete(repo, issue)?;
            println!("{identifier}: removed locally; execution history retained");
            if loopflow::engine::config::load_config_or_default(Some(repo))
                .pm
                .and_then(|pm| pm.linear_team)
                .is_some()
            {
                println!("Pending Linear sync");
            }
            Ok(())
        }
        TaskCommand::Edit {
            issue,
            title,
            notes,
            rank,
            assignee,
            unassign,
            wave,
        } => {
            let result = loopflow::ops::task::task_edit(
                repo,
                issue,
                wave.as_deref(),
                loopflow::pm::PmItemUpdate {
                    name: title.clone(),
                    description: notes.clone(),
                    rank: *rank,
                    assignee: if *unassign {
                        Some(None)
                    } else {
                        assignee.clone().map(Some)
                    },
                },
            )?;
            println!("{}: updated task {}", result.wave, result.id);
            print_planning_sync(&result.sync);
            Ok(())
        }
        TaskCommand::Refile { issue, wave } => {
            let result = loopflow::ops::task::task_refile(repo, issue, wave)?;
            println!("{}: filed task {}", result.wave, result.id);
            Ok(())
        }
        TaskCommand::Comment {
            issue,
            message,
            wave,
            json,
            steer,
        } => {
            let result = loopflow::ops::task::task_comment(
                repo,
                issue,
                wave.as_deref(),
                message.as_deref(),
                *steer,
            )?;
            if *json {
                println!("{}", serde_json::to_string(&result)?);
            } else if result.comments.is_empty() {
                println!("{}: no comments", result.identifier);
            } else {
                for comment in &result.comments {
                    let author = match &comment.author {
                        loopflow::ops::pm::TaskCommentAuthor::Person { name } => {
                            name.as_deref().unwrap_or("unnamed person")
                        }
                        loopflow::ops::pm::TaskCommentAuthor::Integration => "integration",
                    };
                    let date = comment.created_at.as_deref().unwrap_or("date unavailable");
                    let sync = if result.pending_sync.contains(&comment.id) {
                        " · pending sync"
                    } else {
                        ""
                    };
                    println!("── {author} · {date}{sync}\n{}\n", comment.body.trim_end());
                    if let Some(body) = result.conflicts.get(&comment.id) {
                        println!("Saved local comment; Linear’s edit is current:\n{body}\n");
                    }
                }
            }
            if !*json {
                if let Some(error) = &result.refresh_error {
                    eprintln!("{error}; showing saved comments");
                }
            }
            Ok(())
        }
        TaskCommand::Interrupt { issue, json } => {
            let result = loopflow::ops::task::task_interrupt(issue)?;
            print_task_control(&result, *json)
        }
        TaskCommand::Wait {
            issue,
            until,
            timeout,
            json,
        } => {
            let until = if until == "submitted" {
                loopflow::ops::task::TaskWaitUntil::Open
            } else {
                loopflow::ops::task::TaskWaitUntil::Terminal
            };
            let timeout = timeout.as_deref().map(parse_duration).transpose()?;
            let task = loopflow::ops::task::task_wait(issue, until, timeout)?;
            print_task(&task, *json)
        }
    }
}

fn piped_task_report() -> anyhow::Result<Option<String>> {
    if std::io::stdin().is_terminal() {
        return Ok(None);
    }
    let mut report = String::new();
    std::io::stdin().read_to_string(&mut report)?;
    Ok((!report.trim().is_empty()).then_some(report))
}

fn main() -> std::process::ExitCode {
    let _measurement = loopflow::performance::ProcessMeasurement::start();
    let result = journal::with_process(run);
    let code = journal::command_exit_code(&result);
    if let Err(error) = result {
        if error
            .downcast_ref::<loopflow::process::CommandExit>()
            .is_none()
        {
            eprintln!("Error: {error:?}");
        }
    }
    std::process::ExitCode::from(code)
}

fn init_tracing(verbose: bool) {
    let default_filter = if verbose {
        "lf=info,loopflow=info"
    } else {
        "warn"
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter)),
        )
        .with_writer(std::io::stderr)
        .without_time()
        .init();
}

fn run() -> anyhow::Result<()> {
    loopflow::installation::dispatch_entry_gate(&loopflow::installation::ArtifactRole::Cli)?;
    // Ensure Ctrl+C terminates lf and the child agent. Without this,
    // child.wait() retries on EINTR and hangs while the agent catches
    // SIGINT and keeps running. SIGTERM the agent first so it doesn't
    // survive as an orphan. The `termination` feature extends the handler
    // to SIGTERM and SIGHUP: `tmux kill-session` delivers SIGHUP, which
    // otherwise bypasses every cleanup (observed live: it orphaned the wave
    // loop's codex app-server pair and left a stale .wave-endpoint).
    let raw_args: Vec<String> = std::env::args().collect();
    if let Some((remote, command)) = loopflow::lf::navigation::machine_invocation(&raw_args)
        .map_err(|error| {
            let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
            let _ = error.print();
            loopflow::process::CommandExit(code)
        })?
    {
        // The target owns command flags, including --verbose; RUST_LOG controls transport logs.
        init_tracing(false);
        loopflow::installation::dispatch_default_cli()?;
        ctrlc::set_handler(|| loopflow::engine::agent::exit_on_interrupt())
            .expect("failed to set Ctrl+C handler");
        journal::admit_process(&std::env::current_dir()?, &raw_args);
        loopflow::lf::commands::machine::validate_expected_machine_process()?;
        let command = reorder_args(std::iter::once("lf".to_string()).chain(command).collect());
        return loopflow::lf::commands::ssh::run(
            remote
                .machine
                .as_deref()
                .expect("remote invocation has a machine"),
            remote.forward_agent,
            &command[1..],
        );
    }

    // Reorder args so flags can appear after the skill name
    let normalized = loopflow::lf::navigation::normalize_args(raw_args).map_err(|error| {
        let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
        let _ = error.print();
        loopflow::process::CommandExit(code)
    })?;
    let args = reorder_args(normalized);

    let cli = match Cli::try_parse_from(args.clone()).and_then(Cli::checked) {
        Ok(cli) => cli,
        Err(error) => {
            let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
            let _ = error.print();
            return Err(loopflow::process::CommandExit(code).into());
        }
    };
    init_tracing(cli.verbose);
    if cli.task.is_none() && cli.wt.is_none() {
        if let Some(result) = loopflow::lf::navigation::inspect(&cli) {
            return finish_command(result);
        }
    }
    // Installation owns its promotion/recovery authority. In particular,
    // read-only candidate preflight must work before a first install settles.
    let bypasses_installation_startup_gate = matches!(
        &cli.command,
        Some(Commands::Self_ {
            cmd: loopflow::lf::SelfCommand::Install { .. }
                | loopflow::lf::SelfCommand::Doctor {
                    planning: false,
                    ..
                }
        })
    );
    if !bypasses_installation_startup_gate {
        loopflow::installation::dispatch_default_cli()?;
    }
    ctrlc::set_handler(|| loopflow::engine::agent::exit_on_interrupt())
        .expect("failed to set Ctrl+C handler");

    if bypasses_installation_startup_gate {
        journal::observe_process(&args);
    }

    // Machine diagnosis must reach incompatible or uninitialized Machines without
    // ordinary admission creating or migrating the database first.
    if let Some(Commands::Self_ {
        cmd:
            loopflow::lf::SelfCommand::Doctor {
                json,
                planning: false,
            },
    }) = &cli.command
    {
        return loopflow::lf::commands::doctor::run(*json);
    }

    // Global-promotion commands dispatch before home routing, journal emission,
    // and any ordinary store open: a candidate that does not know the live
    // migration frontier must reach the preflight refusal, not fail in
    // trace/store capture. `lf install` opens the store only read-only, inside
    // its own preflight.
    if let Some(Commands::Self_ {
        cmd: loopflow::lf::SelfCommand::Install { cmd },
    }) = &cli.command
    {
        return match cmd.as_ref() {
            None => loopflow::lf::commands::install::latest(),
            Some(InstallCommand::Schedule { frequency }) => {
                loopflow::lf::commands::install::schedule(*frequency)
            }
            Some(InstallCommand::RecoverSwitch { switch }) => {
                loopflow::lf::commands::install::recover_switch(switch)
            }
            Some(InstallCommand::Preflight { json }) => {
                loopflow::lf::commands::install::preflight(*json)
            }
            Some(InstallCommand::AdvanceSwitch { switch }) => {
                loopflow::lf::commands::install::advance_switch(switch)
            }
            Some(InstallCommand::Promote {
                cli_target,

                app_source,
                app_target,
                legacy_app_target,
                sync_skills,
                preview,
            }) => loopflow::lf::commands::install::promote(
                loopflow::lf::commands::install::PromotionArtifacts {
                    cli_target,

                    app_source: app_source.as_deref(),
                    app_target: app_target.as_deref(),
                    legacy_app_target: legacy_app_target.as_deref(),
                },
                *sync_skills,
                *preview,
            ),
            Some(InstallCommand::Rollback {
                cli_target,
                candidate,
            }) => loopflow::lf::commands::install::rollback(cli_target, candidate),
        };
    }

    // Process admission records this process's cwd. Each operation resolves the
    // repository it needs after dispatch; machine inspection needs no Git.
    let directory = std::env::current_dir()?;
    journal::admit_process(&directory, &args);
    {
        let _account_isolation = cli
            .isolate
            .then_some(true)
            .or(cli.shared.then_some(false))
            .map(loopflow::provider_account::activation::isolation_env)
            .map(|(name, mode)| EnvGuard::set(name, mode));
        let account_selection =
            loopflow::provider_account::selection::AccountSelection::from_flags(
                &cli.account,
                &cli.only_account,
            )?;
        let _account_selection = if !account_selection.is_default() {
            Some(EnvGuard::set(
                loopflow::provider_account::selection::ACCOUNT_SELECTION_ENV,
                account_selection.env_value()?,
            ))
        } else {
            None
        };
        debug!(batch = cli.batch, "parsed CLI arguments");

        dispatch(cli, &args)
    }
}

fn dispatch(mut cli: Cli, args: &[String]) -> anyhow::Result<()> {
    // Remote commands prove they reached the saved machine before dispatch.
    loopflow::lf::commands::machine::validate_expected_machine_process()?;

    let mut direct_binding = None;
    let mut _work_declaration = None;
    let mut _bound_cwd = None;
    if let Some(name) = &cli.wt {
        _bound_cwd = Some(CwdGuard::enter(
            &loopflow::lf::commands::ops::resolve_worktree(name)?,
        )?);
    }
    // `lf task run` places the Task and fills its defaults; from here it is
    // `lf --task ISSUE run FLOW`.
    if let Some(Commands::Task {
        cmd:
            TaskCommand::Run {
                issue,
                flow,
                name,
                stack_on,
                directive,
                reason,
                force,
            },
    }) = &cli.command
    {
        let end = loopflow::ops::task::EndOptions { force: *force };
        let directory = loopflow::repo::working_directory()?;
        let repo = loopflow::ops::task::task_repository(&directory, Some(issue))?;
        let (task, flow) = loopflow::ops::task::task_place(
            &repo,
            issue,
            loopflow::ops::task::TaskProcessOptions {
                wave: cli.wave.clone(),
                reason: reason.clone(),
                agent: cli.agent.clone(),
                name: name.clone(),
                flow: flow.clone(),
                stack_on: stack_on.clone(),
                directive: directive.clone(),
                end: end.clone(),
            },
        )?;
        std::env::remove_var(loopflow::lf::TASK_SOURCE_ENV);
        let Some(flow) = flow else {
            // An edge that runs nothing: setting out on it is the whole move.
            println!(
                "Task {} moved along a workflow edge that runs no Flow",
                task.plan.identifier
            );
            return Ok(());
        };
        // This process carries the edge and writes where it left the Task: at
        // the edge's target once an attempt at its Flow succeeded, otherwise
        // still on the edge.
        loopflow::lf::commands::flow::run_for_task(&cli, &task.plan.identifier, &flow)?;
        return Ok(loopflow::ops::task::workflow_arrive(&task, &end)?);
    }
    if let Some(task) = cli.task.as_ref() {
        let directory = loopflow::repo::working_directory()?;
        let repo = loopflow::ops::task::task_repository(&directory, Some(task))?;
        let mut binding = prepare_work_binding(&format!("task:{task}"), &repo)?;
        // The source is input to this invocation, never a descendant's requirement.
        std::env::remove_var(loopflow::lf::TASK_SOURCE_ENV);
        if let Some(cwd) = cli.bound_cwd.clone() {
            binding.cwd = cwd;
        }
        if cli.agent.is_none() {
            cli.agent = binding.agent.clone();
        }
        _bound_cwd = Some(CwdGuard::enter(&binding.cwd)?);
        _work_declaration = Some(EnvGuard::set(
            loopflow::lf::WORK_DECLARATION_ENV,
            format!("task:{}", binding.work.id()),
        ));
        direct_binding = Some(binding);
    }
    let explicit_wave = cli
        .wave
        .as_deref()
        .map(loopflow::work::wave::context::resolve_explicit_wave)
        .transpose()?;
    if let Some(wave) = &explicit_wave {
        // Location wins: context may enrich a Task, never change its owner.
        let binding = match direct_binding.as_ref() {
            Some(binding) => Some(binding.clone()),
            None if cli.task.is_none() => {
                loopflow::lf::commands::run::implicit_binding(&Cli::default())?
            }
            None => None,
        };
        if let Some(binding) = binding {
            if matches!(binding.work, loopflow::durable::WorkRef::Task(_)) {
                anyhow::ensure!(
                    wave.id() == &binding.wave_id,
                    "--wave {} does not own Task {} (Wave {})",
                    wave.slug(),
                    binding.work.id(),
                    binding.wave_name
                );
                direct_binding = Some(binding);
            }
        }
        cli.wave = Some(wave.slug().to_string());
    } else if let Some(binding) = &direct_binding {
        cli.wave = Some(binding.wave_name.clone());
    }
    let _explicit_wave_env = explicit_wave.as_ref().map(|wave| {
        EnvGuard::set(
            loopflow::work::wave::context::WAVE_ID_ENV,
            wave.id().to_string(),
        )
    });

    if let Some(result) = loopflow::lf::navigation::inspect(&cli) {
        return finish_command(result);
    }

    let result = resolve_cli_target(&mut cli, args).and_then(|selected| match selected {
        Some((target, message)) => execute_target(
            target,
            message.as_deref(),
            &cli,
            args,
            direct_binding.as_ref(),
        ),
        None => {
            cli.interactive = !cli.batch;
            match direct_binding.as_ref() {
                Some(binding) => loopflow::lf::commands::run::run_bound(None, None, &cli, binding),
                None => run_default_agent(&cli, args),
            }
        }
    });

    finish_command(result)
}

fn execute_command(
    command: &loopflow::engine::Command,
    cli: &Cli,
    args: &[String],
    binding: Option<&loopflow::ops::WorkBinding>,
) -> anyhow::Result<()> {
    let parsed = Cli::try_parse_from(command.argv())?;
    match &parsed.command {
        Some(Commands::Inline { prompt }) => {
            let text = prompt.join(" ");
            in_directory_runtime(args, |repo| match binding {
                Some(binding) => {
                    loopflow::lf::commands::run::run_bound(None, Some(&text), cli, binding)
                }
                None => loopflow::lf::commands::run::run(repo, None, Some(&text), cli),
            })
        }
        Some(Commands::Open) => loopflow::lf::commands::open::run(),
        Some(Commands::Config {
            cmd: loopflow::lf::ConfigCommand::User { json },
        }) => loopflow::lf::commands::config::print_user(*json),
        Some(Commands::ProviderSession) => {
            loopflow::lf::commands::session_history::observe_provider_session()
        }
        Some(Commands::SessionTitle { provider }) => {
            loopflow::lf::commands::session_history::name_native_session(provider)
        }
        Some(Commands::Session {
            cmd:
                loopflow::lf::SessionCommand::Resume {
                    id: Some(id),
                    message: Some(message),
                },
        }) if cli.batch => in_directory_runtime(args, |_| {
            loopflow::lf::commands::run::resume(id, message, cli)
        }),
        Some(Commands::Session { cmd }) => loopflow::lf::commands::session::run(cmd),
        Some(Commands::Account {
            cmd,
            provider,
            cached,
            details,
            json,
        }) => {
            loopflow::lf::commands::account::run(cmd.as_ref(), *provider, *cached, *details, *json)
        }
        Some(Commands::Repo { cmd }) => match cmd {
            loopflow::lf::RepoCommand::Release {
                cmd: loopflow::lf::ReleaseCommand::Run { version, target },
            } if cli.cron_receipt.is_some() => in_repo_runtime(args, |_| {
                loopflow::lf::commands::ops::run_cron_release(
                    version.as_deref(),
                    target.as_deref(),
                    cli,
                )
            }),
            loopflow::lf::RepoCommand::Release { .. } => {
                in_repo_runtime(args, |_| loopflow::lf::commands::ops::run_repo(cmd))
            }
            // The watcher admits repairs, which needs a recorded Process.
            loopflow::lf::RepoCommand::Ci {
                cmd:
                    Some(loopflow::lf::CiCommand::Watch {
                        uninstall: false,
                        status: false,
                        ..
                    }),
                ..
            } => {
                // It runs from the main checkout so its long-lived Process never
                // counts as live work in a Task's worktree.
                let root = loopflow::engine::worktrees::main_repo_root(
                    &loopflow::lf::commands::util::find_repo_root()?,
                )?;
                let _cwd = CwdGuard::enter(&root)?;
                with_runtime(&root, args, || loopflow::lf::commands::ops::run_repo(cmd))
            }
            loopflow::lf::RepoCommand::Tokens { .. } | loopflow::lf::RepoCommand::Ci { .. } => {
                loopflow::lf::commands::ops::run_repo(cmd)
            }
            _ => in_directory_runtime(args, |_| loopflow::lf::commands::ops::run_repo(cmd)),
        },
        Some(Commands::Machine { cmd }) => loopflow::lf::commands::machine::run(cmd, cli.batch),
        Some(Commands::Self_ {
            cmd:
                loopflow::lf::SelfCommand::SyncSkills {
                    yes,
                    no_prune,
                    repo,
                },
        }) => loopflow::lf::commands::ops::run_sync_skills(*yes, *no_prune, *repo),
        Some(Commands::Wave {
            cmd:
                loopflow::lf::WaveCommand::Cron {
                    cmd:
                        cmd @ (loopflow::lf::CronCommand::List { .. }
                        | loopflow::lf::CronCommand::Remove { .. }),
                },
        }) => loopflow::lf::commands::ops::cron_cmd(cmd),
        Some(Commands::Wave {
            cmd: loopflow::lf::WaveCommand::Cron { cmd },
        }) => in_repo_runtime(args, |_| loopflow::lf::commands::ops::cron_cmd(cmd)),
        Some(Commands::Wave {
            cmd: WaveCommand::List { json, all, current },
        }) => loopflow::lf::commands::waves::ls(*json, *all, *current),
        Some(Commands::Wave {
            cmd: WaveCommand::Status { wave, json, sync },
        }) => {
            let refreshed = if *sync {
                Some(loopflow::lf::commands::ops::refresh_status(
                    wave.as_deref(),
                )?)
            } else {
                None
            };
            loopflow::lf::commands::waves::status(refreshed.as_deref().or(wave.as_deref()), *json)
        }

        Some(Commands::Wave {
            cmd: cmd @ (WaveCommand::Rename { .. } | WaveCommand::Place { .. }),
        }) => in_directory_runtime(args, |repo| run_wave_command(repo, cmd)),
        Some(Commands::Wave { cmd }) => in_repo_runtime(args, |repo| run_wave_command(repo, cmd)),
        Some(Commands::Pr { cmd }) => in_repo_runtime(args, |_| {
            loopflow::lf::commands::ops::run_pr(cmd.as_ref(), cli.agent.as_deref())
        }),
        Some(Commands::Wt { cmd }) => {
            in_repo_runtime(args, |_| loopflow::lf::commands::ops::run_wt(cmd))
        }
        Some(Commands::Commit {
            message,
            push,
            no_add,
            paths,
        }) => in_repo_runtime(args, |_| {
            loopflow::lf::commands::ops::run_commit(
                message.as_deref(),
                *push,
                *no_add,
                paths,
                cli.agent.as_deref(),
            )
        }),
        Some(Commands::Sync(sync)) => {
            let repo = loopflow::repo::require_repo_root(&std::env::current_dir()?, "lf sync")?;
            with_runtime(&repo, args, || {
                loopflow::lf::commands::ops::run_sync(
                    sync.onto.as_deref(),
                    sync.plan,
                    sync.manual,
                    sync.continue_sync,
                    sync.abort,
                    sync.adopt,
                )
            })
        }
        // Local document access owns no Run lifecycle. Placement and the recorded
        // Git base come from the Task registry inside these operations.
        Some(Commands::Task {
            cmd:
                cmd @ (TaskCommand::Diff { .. }
                | TaskCommand::File { .. }
                | TaskCommand::Files { .. }
                | TaskCommand::Save { .. }),
        }) => run_task_command(&std::env::current_dir()?, cmd),
        Some(Commands::Project {
            cmd:
                loopflow::lf::ProjectCommand::Edit {
                    project,
                    name,
                    summary,
                },
        }) => {
            let cwd = std::env::current_dir()?;
            let repo = loopflow::repo::discover_repo_root(&cwd)?.unwrap_or(cwd);
            let saved = tokio::runtime::Runtime::new()?.block_on(loopflow::ops::project::edit(
                &repo,
                project,
                name.as_deref(),
                summary.as_deref(),
            ))?;
            print_planning_sync(&saved.sync);
            Ok(())
        }
        Some(Commands::Project {
            cmd: loopflow::lf::ProjectCommand::Workflow { cmd },
        }) => {
            let cwd = std::env::current_dir()?;
            let repo = loopflow::repo::discover_repo_root(&cwd)?.unwrap_or(cwd);
            match cmd {
                loopflow::lf::ProjectWorkflowCommand::Source { project, name } => {
                    let source = tokio::runtime::Runtime::new()?.block_on(
                        loopflow::ops::project::workflow_source(&repo, project, name),
                    )?;
                    print!("{source}");
                    Ok(())
                }
                loopflow::lf::ProjectWorkflowCommand::List { json, project } => {
                    let entries = tokio::runtime::Runtime::new()?.block_on(
                        loopflow::ops::project::workflow_catalog(&repo, project.as_deref()),
                    )?;
                    if *json {
                        println!("{}", serde_json::to_string(&entries)?);
                    } else {
                        for entry in entries {
                            println!(
                                "{}{}",
                                entry.name,
                                entry
                                    .unavailable
                                    .map(|e| format!(" (unavailable: {e})"))
                                    .unwrap_or_default()
                            );
                        }
                    }
                    Ok(())
                }
                loopflow::lf::ProjectWorkflowCommand::Show { project, json } => {
                    let selected = tokio::runtime::Runtime::new()?
                        .block_on(loopflow::ops::project::workflow(&repo, project, None, None))?;
                    if *json {
                        println!("{}", serde_json::to_string_pretty(&selected)?);
                    } else {
                        println!(
                            "{} · Workflow {}",
                            selected.project.name, selected.project.workflow
                        );
                        print_planning_sync(&selected.sync);
                    }
                    Ok(())
                }
                loopflow::lf::ProjectWorkflowCommand::Set {
                    project,
                    name,
                    file,
                } => with_runtime(&repo, args, || {
                    let saved = tokio::runtime::Runtime::new()?.block_on(
                        loopflow::ops::project::workflow(
                            &repo,
                            project,
                            Some(name),
                            file.as_deref(),
                        ),
                    )?;
                    println!("Project {project}: Workflow {name}");
                    print_planning_sync(&saved.sync);
                    Ok(())
                }),
            }
        }
        Some(Commands::Task { cmd }) => {
            let directory = loopflow::repo::working_directory()?;
            let repo = loopflow::ops::task::task_repository(&directory, cmd.selector())?;
            with_runtime(&repo, args, || run_task_command(&repo, cmd))
        }
        Some(Commands::Context {
            json,
            wave,
            task,
            skill,
        }) => loopflow::lf::commands::context::run(*json, wave.as_deref(), task.as_deref(), skill),
        Some(Commands::TelemetryScorecard { json }) => in_repo_runtime(args, |repo| {
            loopflow::ops::run_telemetry_scorecard(repo, *json).map_err(Into::into)
        }),
        Some(Commands::Self_ {
            cmd: loopflow::lf::SelfCommand::Doctor { json, planning },
        }) => {
            if *planning {
                let repo = loopflow::repo::working_directory()?;
                loopflow::lf::commands::ops::sync_planning(&repo, None, true, true, *json)
            } else {
                loopflow::lf::commands::doctor::run(*json)
            }
        }
        Some(Commands::List { .. } | Commands::Help { .. }) => {
            unreachable!("inspection returned before execution")
        }
        Some(Commands::Roadmap {
            wave,
            task,
            json,
            all,
        }) => loopflow::lf::commands::waves::roadmap(wave.as_deref(), task.as_deref(), *json, *all),
        Some(Commands::Monitor { cmd, json, all }) => match cmd {
            Some(cmd) => loopflow::lf::commands::monitor::run(cmd),
            None => loopflow::lf::commands::monitor::overview(*json, *all),
        },
        Some(Commands::Replay { run }) => loopflow::lf::commands::replay::run(run),
        Some(Commands::Discord {
            cmd: loopflow::lf::DiscordCommand::Serve { wave },
        }) => in_repo_runtime(args, |repo| {
            loopflow::lf::commands::discord::serve(repo, wave)
        }),
        Some(Commands::Self_ {
            cmd: loopflow::lf::SelfCommand::Install { .. },
        }) => {
            unreachable!("install dispatches before home routing")
        }
        Some(Commands::Flow { cmd }) => match cmd {
            FlowCommand::List { json, inventory } if inventory.processes => {
                loopflow::lf::commands::flow_inventory::list(inventory, *json)
            }
            FlowCommand::Show {
                name,
                json,
                processes: true,
                ..
            } => loopflow::lf::commands::flow_inventory::inspect(name, *json),
            _ => anyhow::bail!("not a Flow inspection command: {cmd:?}"),
        },
        Some(Commands::Skill { .. } | Commands::Run { .. } | Commands::External(_)) => {
            anyhow::bail!("a command target must name a builtin command")
        }
        None => anyhow::bail!("a command target requires a command"),
    }
}

fn finish_command(result: anyhow::Result<()>) -> anyhow::Result<()> {
    if let Err(error) = &result {
        if let Some(error) = error.downcast_ref::<clap::Error>() {
            let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
            let _ = error.print();
            return Err(loopflow::process::CommandExit(code).into());
        }
        if matches!(
            error.downcast_ref::<loopflow::engine::LoadError>(),
            Some(
                loopflow::engine::LoadError::TargetNotFound(_)
                    | loopflow::engine::LoadError::SkillNotFound(_)
                    | loopflow::engine::LoadError::FlowNotFound(_)
            )
        ) {
            let error =
                clap::Error::raw(clap::error::ErrorKind::InvalidSubcommand, error.to_string());
            let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
            let _ = error.print();
            return Err(loopflow::process::CommandExit(code).into());
        }
    }
    result
}

fn print_planning_sync(sync: &loopflow::planning::PlanningSyncStatus) {
    for line in sync.lines() {
        eprintln!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::{format_task_pr_line, reorder_args, CwdGuard, EnvGuard};

    use clap::Parser;
    use loopflow::lf::{Cli, Commands, PrCommand, TaskCommand};
    use loopflow::work::task::{GithubPr, PrPublication, TaskId, TaskPr, TaskPrId};

    #[test]
    fn bound_work_selector_requires_a_readable_registry() {
        let _lock = PROCESS_STATE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("loopflow.db")).unwrap();
        let _home = EnvGuard::set("LF_HOME", directory.path().display().to_string());

        let error = super::prepare_work_binding("task:LOO-265", directory.path())
            .expect_err("--as must not degrade to raw attribution");
        assert!(error.to_string().contains("planning registry unavailable"));
    }

    static PROCESS_STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn command_targets_keep_arguments_and_definition_execution_stays_explicit() {
        use loopflow::engine::target::Target;

        let _lock = PROCESS_STATE_LOCK.lock().unwrap();
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        std::fs::write(repo.path().join(".lf/skills/land.md"), "Review landing.").unwrap();
        std::fs::write(repo.path().join(".lf/flows/land.yaml"), "- cmd: pr land\n").unwrap();
        let _cwd = CwdGuard::enter(repo.path()).unwrap();
        let resolve = |args: &[&str]| {
            let args = loopflow::lf::navigation::normalize_args(
                args.iter().map(|arg| arg.to_string()).collect(),
            )
            .unwrap();
            let args = reorder_args(args);
            let mut cli = Cli::try_parse_from(&args).unwrap();
            super::resolve_cli_target(&mut cli, &args).unwrap().unwrap()
        };

        let (target, message) = resolve(&[
            "lf",
            "-a",
            "codex",
            "pr",
            "land",
            "--message",
            "Keep this together",
        ]);
        let Target::Command(command) = target else {
            panic!("expected command")
        };
        assert_eq!(
            command.argv(),
            ["lf", "pr", "land", "--message", "Keep this together"]
        );
        assert!(message.is_none());
        assert!(matches!(resolve(&["lf", "run", "land"]).0, Target::Flow(_)));
        assert!(matches!(
            resolve(&["lf", "skill", "land"]).0,
            Target::Skill(_)
        ));
    }

    fn published_pr() -> TaskPr {
        let now = time::OffsetDateTime::now_utc();
        TaskPr {
            id: TaskPrId::new(),
            task_id: TaskId::new(),
            sequence: 1,
            slug: "linear-pr-linkage".to_string(),
            branch: "jack/linear-pr-linkage".to_string(),
            base_commit: "abc".to_string(),
            parent_pr_id: None,
            publication: Some(PrPublication {
                requested_at: now,
                presentation: None,
                github: Some(GithubPr {
                    number: 931,
                    url: "https://github.com/loopflowstudio/loopflow/pull/931".to_string(),
                    head_sha: None,
                }),
                merge: None,
            }),
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn only_bare_lf_selects_the_default_agent_surface() {
        let bare = Cli::try_parse_from(["lf"]).unwrap();
        assert!(bare.command.is_none());

        let code = Cli::try_parse_from(["lf", "code"]).unwrap();
        assert!(matches!(
            code.command,
            Some(Commands::External(parts)) if parts == ["code"]
        ));
    }

    /// A healthy linkage says nothing: silence on the happy path keeps the status
    /// reading quiet enough that a degraded one stands out.
    #[test]
    fn task_pr_line_is_quiet_when_the_linear_link_is_healthy() {
        let line = format_task_pr_line(&published_pr());
        assert!(line.contains("GitHub #931"), "{line}");
        assert!(!line.contains("Linear"), "{line}");
    }

    /// A degraded Linear writeback is named in the same reading that already
    /// carries `PM writeback`, so an expired token cannot fail silently forever.
    #[test]
    fn task_pr_line_names_a_degraded_linear_link() {
        let mut pr = published_pr();
        pr.linear_link_error = Some("linear token expired".to_string());
        let line = format_task_pr_line(&pr);
        assert!(line.contains("Linear link degraded"), "{line}");
        assert!(line.contains("linear token expired"), "{line}");
        // The publication reading survives alongside the degraded linkage.
        assert!(line.contains("GitHub #931"), "{line}");
    }

    #[test]
    fn reorder_args_moves_work_selector_after_skill_to_global_position() {
        let args = ["lf", "implement", "--task", "LOO-123"]
            .map(str::to_owned)
            .to_vec();
        assert_eq!(reorder_args(args), ["lf", "--task", "LOO-123", "implement"]);
    }

    #[test]
    fn bound_cwd_is_entered_for_the_invocation_and_restored_afterward() {
        let _lock = PROCESS_STATE_LOCK.lock().unwrap();
        let previous = std::env::current_dir().unwrap();
        let directory = tempfile::tempdir().unwrap();

        let guard = CwdGuard::enter(directory.path()).unwrap();
        assert_eq!(
            std::env::current_dir().unwrap(),
            directory.path().canonicalize().unwrap()
        );
        drop(guard);

        assert_eq!(std::env::current_dir().unwrap(), previous);
    }

    #[test]
    fn reorder_args_flag_after_skill() {
        let args = vec!["lf".to_string(), "debug".to_string(), "-c".to_string()];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-c", "debug"]);
    }

    #[test]
    fn old_serve_surface_is_no_longer_a_builtin_command() {
        let cli = Cli::try_parse_from(["lf", "serve", "goals"]).expect("falls through to external");
        assert!(
            matches!(cli.command, Some(Commands::External(parts)) if parts[0] == "serve"),
            "`serve` survives only as an external verb, not a built-in"
        );
        assert!(Cli::try_parse_from(["lf", "wave", "serve", "goals"]).is_err());
    }

    #[test]
    fn removed_dispatch_flag_is_rejected() {
        assert!(Cli::try_parse_from(["lf", "--dispatch", "implement", "ship it"]).is_err());
    }

    #[test]
    fn reorder_args_flag_before_skill() {
        let args = vec!["lf".to_string(), "-c".to_string(), "debug".to_string()];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-c", "debug"]);
    }

    #[test]
    fn reorder_args_value_flag_before_skill() {
        // lf -a codex implement -> should stay the same (already correct order)
        let args = vec![
            "lf".to_string(),
            "-a".to_string(),
            "codex".to_string(),
            "implement".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-a", "codex", "implement"]);
    }

    #[test]
    fn reorder_args_value_flag_after_skill() {
        let args = vec![
            "lf".to_string(),
            "debug".to_string(),
            "-a".to_string(),
            "codex".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-a", "codex", "debug"]);
    }

    #[test]
    fn reorder_args_repeatable_account_flags_after_skill() {
        let args = [
            "lf",
            "implement",
            "--account",
            "claude=personal",
            "--account",
            "codex=reserve",
        ]
        .map(String::from)
        .to_vec();

        assert_eq!(
            reorder_args(args),
            vec![
                "lf",
                "--account",
                "claude=personal",
                "--account",
                "codex=reserve",
                "implement"
            ]
        );
    }

    #[test]
    fn reorder_args_mixed_flags() {
        let args = vec![
            "lf".to_string(),
            "--interactive".to_string(),
            "implement".to_string(),
            "-c".to_string(),
            "-a".to_string(),
            "claude".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(
            result,
            vec!["lf", "--interactive", "-c", "-a", "claude", "implement"]
        );
    }

    #[test]
    fn reorder_args_no_loopflow_flag_after_skill() {
        let args = vec![
            "lf".to_string(),
            "gate".to_string(),
            "--no-loopflow".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "--no-loopflow", "gate"]);
    }

    #[test]
    fn reorder_args_skill_with_args() {
        let args = vec![
            "lf".to_string(),
            "implement:".to_string(),
            "add".to_string(),
            "logout".to_string(),
            "-c".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-c", "implement:", "add", "logout"]);
    }

    #[test]
    fn reorder_args_known_command_unchanged() {
        let args = vec![
            "lf".to_string(),
            "commit".to_string(),
            "-m".to_string(),
            "msg".to_string(),
        ];
        let result = reorder_args(args);
        // `-m` is local to commit, so the local meaning wins.
        assert_eq!(result, vec!["lf", "commit", "-m", "msg"]);
    }

    #[test]
    fn reorder_args_preserves_local_collision_after_leading_global() {
        let args: Vec<String> = ["lf", "--wave", "goals", "commit", "-m", "ship it"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            reorder_args(loopflow::lf::navigation::normalize_args(args).unwrap()),
            vec!["lf", "--wave", "goals", "commit", "-m", "ship it"]
        );
    }

    #[test]
    fn reorder_args_stops_at_double_dash() {
        let args: Vec<String> = ["lf", "skill", "debug", "--", "--wave", "literal"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            reorder_args(args),
            vec!["lf", "skill", "debug", "--", "--wave", "literal"]
        );
    }

    #[test]
    fn reorder_args_moves_flags_to_nested_owners() {
        let args: Vec<String> = ["lf", "repo", "--all", "refresh"]
            .map(String::from)
            .to_vec();
        let reordered = reorder_args(args);
        assert_eq!(reordered, vec!["lf", "repo", "refresh", "--all"]);
        assert!(matches!(
            Cli::try_parse_from(reordered).unwrap().command,
            Some(Commands::Repo {
                cmd: loopflow::lf::RepoCommand::Refresh { .. }
            })
        ));

        let args: Vec<String> = [
            "lf", "task", "--wave", "systems", "create", "--title", "file it",
        ]
        .map(String::from)
        .to_vec();
        let reordered = reorder_args(args);
        assert_eq!(
            reordered,
            vec!["lf", "task", "create", "--wave", "systems", "--title", "file it"]
        );
        assert!(matches!(
            Cli::try_parse_from(reordered).unwrap().command,
            Some(Commands::Task {
                cmd: TaskCommand::Create { .. }
            })
        ));

        let args: Vec<String> = ["lf", "pr", "-a", "codex", "open"]
            .map(String::from)
            .to_vec();
        let reordered = reorder_args(args);
        assert_eq!(reordered, vec!["lf", "pr", "open", "-a", "codex"]);
        assert!(matches!(
            Cli::try_parse_from(reordered).unwrap().command,
            Some(Commands::Pr {
                cmd: Some(PrCommand::Open { .. })
            })
        ));

        let args: Vec<String> = ["lf", "pr", "--strict", "submit"]
            .map(String::from)
            .to_vec();
        let reordered = reorder_args(args);
        assert_eq!(reordered, vec!["lf", "pr", "submit", "--strict"]);
        assert!(matches!(
            Cli::try_parse_from(reordered).unwrap().command,
            Some(Commands::Pr {
                cmd: Some(PrCommand::Submit { strict: true, .. })
            })
        ));

        let args: Vec<String> = ["lf", "wt", "--force", "delete", "old-tree"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            reorder_args(args),
            vec!["lf", "wt", "delete", "--force", "old-tree"]
        );
    }

    #[test]
    fn reorder_args_leaves_explicit_targeting_alone() {
        let args: Vec<String> = ["lf", "wave", "status", "systems"]
            .map(String::from)
            .to_vec();
        assert_eq!(reorder_args(args), vec!["lf", "wave", "status", "systems"]);
    }

    #[test]
    fn reorder_args_no_skill() {
        let args = vec!["lf".to_string(), "-l".to_string()];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-l"]);
    }
}
