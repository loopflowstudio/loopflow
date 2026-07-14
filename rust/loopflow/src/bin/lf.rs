use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use clap::{CommandFactory, Parser};
use tracing::debug;
use tracing_subscriber::EnvFilter;

use loopflow::journal::{self, LfEventFields, LfEventType, LfNode};
use loopflow::lf::{Cli, Commands, ProjectCommand, TaskCommand};

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
        for alias in arg.get_all_short_aliases().into_iter().flatten() {
            flags.insert(format!("-{alias}"));
        }
        if let Some(long) = arg.get_long() {
            flags.insert(format!("--{long}"));
        }
        for alias in arg.get_all_aliases().into_iter().flatten() {
            flags.insert(format!("--{alias}"));
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

#[derive(Clone, Default)]
struct CommandArgTables {
    /// Flags owned directly by this command.
    direct: FlagTables,
    /// Flags owned here or by any descendant, used only to find the command path.
    recursive: FlagTables,
    /// Direct subcommands, indexed by canonical name and aliases.
    subcommands: HashMap<String, CommandArgTables>,
}

/// What `reorder_args` needs to know about the CLI, derived from the clap
/// definition so it can never drift from it (the old hand-maintained lists
/// were missing the uppercase short aliases `-D`/`-C`/`-M`/`-I`/`-B`/`-W`,
/// misrouting e.g. `lf debug -M codex`).
struct ArgTables {
    /// Top-level subcommands, indexed by canonical name and aliases.
    commands: HashMap<String, CommandArgTables>,
    /// Top-level flags accepted on either side of an unambiguous command.
    top_level: FlagTables,
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
        let names = std::iter::once(subcommand.get_name()).chain(subcommand.get_all_aliases());
        for name in names {
            subcommands.insert(name.to_string(), child.clone());
        }
    }

    CommandArgTables {
        direct,
        recursive,
        subcommands,
    }
}

fn arg_tables() -> &'static ArgTables {
    static TABLES: OnceLock<ArgTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut cli = Cli::command();
        // Materialize the built-ins (help subcommand, -h/--help, -V/--version).
        cli.build();

        let mut commands = HashMap::new();
        for sub in cli.get_subcommands() {
            let table = command_arg_tables(sub);
            let names = std::iter::once(sub.get_name()).chain(sub.get_all_aliases());
            for name in names {
                commands.insert(name.to_string(), table.clone());
            }
        }

        let mut top_level = FlagTables::default();
        for arg in cli.get_arguments() {
            top_level.insert(arg);
        }

        ArgTables {
            commands,
            top_level,
        }
    })
}

fn flag_name(arg: &str) -> &str {
    arg.split_once('=').map_or(arg, |(name, _)| name)
}

fn has_inline_value(arg: &str) -> bool {
    arg.starts_with('-') && arg.contains('=')
}

fn is_value_flag(arg: &str) -> bool {
    arg_tables().top_level.takes_value(arg)
}

fn is_known_flag(arg: &str) -> bool {
    arg_tables().top_level.contains(arg)
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
    if let Some(command) = arg_tables().commands.get(rest[target_index].as_str()) {
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

fn with_runtime<T>(
    repo_root: &std::path::Path,
    command: &[String],
    run: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    journal::emit(
        repo_root,
        LfNode::Run,
        LfEventType::Started,
        LfEventFields {
            wave_name: loopflow::engine::wave_context::resolve_run_wave_name(),
            worktree: Some(repo_root.display().to_string()),
            command: Some(command.to_vec()),
            ..LfEventFields::default()
        },
    );
    let result = run();
    match &result {
        Ok(_) => journal::emit(
            repo_root,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        ),
        Err(err) => journal::emit(
            repo_root,
            LfNode::Run,
            LfEventType::Errored,
            LfEventFields {
                error: Some(err.to_string()),
                ..LfEventFields::default()
            },
        ),
    }
    result
}

fn in_repo_runtime<T>(
    command: &[String],
    run: impl FnOnce(&std::path::Path) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let repo_root = loopflow::lf::commands::util::find_repo_root()?;
    with_runtime(&repo_root, command, || run(&repo_root))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    Flow,
    Skill,
}

/// The explicit verbs promise a kind; a name that resolves to the other one
/// is an error, not a silent fallback.
fn require_target_kind(name: &str, kind: TargetKind) -> anyhow::Result<()> {
    let repo_root = loopflow::lf::commands::util::find_repo_root()?;
    match (
        loopflow::lf::discovery::discover_target(&repo_root, name)?,
        kind,
    ) {
        (loopflow::lf::discovery::Target::Flow(_), TargetKind::Flow) => Ok(()),
        (loopflow::lf::discovery::Target::Skill(_), TargetKind::Skill) => Ok(()),
        (loopflow::lf::discovery::Target::Flow(_), TargetKind::Skill) => {
            Err(anyhow::anyhow!("'{name}' is a flow — run `lf flow {name}`"))
        }
        (loopflow::lf::discovery::Target::Skill(_), TargetKind::Flow) => Err(anyhow::anyhow!(
            "'{name}' is a skill — run `lf skill {name}`"
        )),
    }
}

fn run_target(
    name: &str,
    message: Option<&str>,
    cli: &Cli,
    command: &[String],
) -> anyhow::Result<()> {
    let repo_root = loopflow::lf::commands::util::find_repo_root()?;
    run_target_in_repo(&repo_root, name, message, cli, command)
}

fn run_target_in_repo(
    repo_root: &Path,
    name: &str,
    message: Option<&str>,
    cli: &Cli,
    command: &[String],
) -> anyhow::Result<()> {
    match loopflow::lf::discovery::discover_target(repo_root, name)? {
        loopflow::lf::discovery::Target::Skill(_) => with_runtime(repo_root, command, || {
            with_skill_runtime(repo_root, name, || {
                loopflow::lf::commands::run::run(Some(name), message, cli)?;
                // Commit any uncommitted changes left by the skill.
                // When running inside a flow, the flow executor handles this;
                // for standalone skills we must do it here.
                let options = loopflow::ops::CommitOptions {
                    add: true,
                    message: Some(format!("lf commit: {name}")),
                    ..loopflow::ops::CommitOptions::for_task(name)
                };
                loopflow::ops::commit_workflow(repo_root, &options, &loopflow::ops::NullProgress)?;
                Ok(())
            })
        }),
        loopflow::lf::discovery::Target::Flow(flow) => with_runtime(repo_root, command, || {
            loopflow::lf::commands::flow::run(&flow, message, cli, repo_root)
        }),
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

fn print_task_session(session: &loopflow::task::TaskSession, json: bool) -> anyhow::Result<()> {
    if json {
        let snapshot = loopflow::ops::task::task_snapshot(session)?;
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
    } else {
        let pm_writeback = match &session.pm_writeback {
            loopflow::task::PmWritebackState::Current => "current".to_string(),
            loopflow::task::PmWritebackState::Pending { error, .. } => {
                format!("pending: {error}")
            }
        };
        println!(
            "{}  {}  {}\n  session: {}\n  worktree: {}\n  branch: {}\n  PM writeback: {}\n  reason: {}",
            session.launch.issue.identifier,
            session.status.as_str(),
            session.provider,
            session.id,
            session.worktree.display(),
            session.branch,
            pm_writeback,
            session.status_reason,
        );
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
        let effect = result
            .effect
            .map(|effect| effect.as_str())
            .unwrap_or("none");
        println!(
            "{} → {} (state={}, effect={})",
            result.command_id,
            result.issue_id,
            result.state.as_str(),
            effect
        );
    }
    Ok(())
}

fn print_project_session(
    session: &loopflow::project_session::ProjectSession,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        let snapshot = loopflow::ops::project::project_snapshot(session)?;
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
    } else {
        println!(
            "{}  {}  {}\n  session: {}\n  iteration: {}\n  reason: {}",
            session.launch.project.slug,
            session.status.as_str(),
            session.provider,
            session.id,
            session.iteration,
            session.status_reason,
        );
    }
    Ok(())
}

fn print_project_control(
    result: &loopflow::ops::project::ProjectControlResult,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(result)?);
    } else {
        let effect = result
            .effect
            .map(|effect| effect.as_str())
            .unwrap_or("none");
        println!(
            "{} → {} (state={}, effect={})",
            result.command_id,
            result.project_id,
            result.state.as_str(),
            effect
        );
    }
    Ok(())
}

fn run_project_command(repo: &Path, command: &ProjectCommand) -> anyhow::Result<()> {
    match command {
        ProjectCommand::Run {
            project_id,
            directive,
            json,
        } => {
            let session = loopflow::ops::project::project_run(repo, project_id, directive.clone())?;
            print_project_session(&session, *json)
        }
        ProjectCommand::Start {
            title,
            wave,
            directive,
            json,
        } => {
            let session = loopflow::ops::project::project_start(
                repo,
                title,
                wave.as_deref(),
                directive.clone(),
            )?;
            print_project_session(&session, *json)
        }
        ProjectCommand::Status { project_id, json } => {
            let session = loopflow::ops::project::project_status(project_id)?;
            print_project_session(&session, *json)
        }
        ProjectCommand::FollowUp {
            project_id,
            message,
            json,
        } => {
            let result = loopflow::ops::project::project_follow_up(project_id, message.clone())?;
            print_project_control(&result, *json)
        }
        ProjectCommand::Steer {
            project_id,
            message,
            json,
        } => {
            let result = loopflow::ops::project::project_steer(project_id, message.clone())?;
            print_project_control(&result, *json)
        }
        ProjectCommand::Interrupt {
            project_id,
            message,
            json,
        } => {
            let result = loopflow::ops::project::project_interrupt(project_id, message.clone())?;
            print_project_control(&result, *json)
        }
        ProjectCommand::Receipt {
            command_id,
            until,
            timeout,
            json,
        } => {
            let read = loopflow::ops::project::project_receipt(
                command_id,
                *until,
                parse_duration(timeout)?,
            )?;
            print_project_control(&read.receipt, *json)?;
            if read.timed_out {
                std::process::exit(124);
            }
            Ok(())
        }
        ProjectCommand::Acknowledge {
            project_id,
            directive,
            summary,
            json,
        } => {
            let result = loopflow::ops::project::project_acknowledge(
                project_id,
                *directive,
                summary.clone(),
            )?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("{} incorporated directive v{}", project_id, directive);
            }
            Ok(())
        }
        ProjectCommand::Decide {
            project_id,
            decision_id,
            choice,
            message,
            json,
        } => {
            let result = loopflow::ops::project::project_decide(
                project_id,
                decision_id,
                choice.clone(),
                message.clone(),
            )?;
            print_project_control(&result, *json)
        }
        ProjectCommand::RequestDecision {
            project_id,
            prompt,
            options,
            wait,
            timeout,
            json,
        } => {
            let result = loopflow::ops::project::project_request_decision(
                project_id,
                prompt.clone(),
                options.clone(),
                *wait,
                parse_duration(timeout)?,
            )?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else if result.resolved {
                println!(
                    "{} → {}",
                    result.decision_id,
                    result.choice.as_deref().unwrap_or("resolved")
                );
            } else {
                println!("{} → pending", result.decision_id);
            }
            Ok(())
        }
        ProjectCommand::Wait {
            project_id,
            until,
            timeout,
            json,
        } => {
            let until = if until == "waiting" {
                loopflow::ops::project::ProjectWaitUntil::Waiting
            } else {
                loopflow::ops::project::ProjectWaitUntil::Terminal
            };
            let timeout = timeout.as_deref().map(parse_duration).transpose()?;
            let session = loopflow::ops::project::project_wait(project_id, until, timeout)?;
            print_project_session(&session, *json)
        }
        ProjectCommand::Resume {
            project_id,
            message,
            json,
        } => {
            let result = loopflow::ops::project::project_resume(project_id, message.clone())?;
            print_project_control(&result, *json)
        }
        ProjectCommand::Attach { project_id } => {
            loopflow::ops::project::project_attach(project_id).map_err(Into::into)
        }
        ProjectCommand::Abandon {
            project_id,
            reason,
            json,
        } => {
            let result = loopflow::ops::project::project_abandon(project_id, reason.clone())?;
            print_project_control(&result, *json)
        }
        ProjectCommand::Promote { .. } => {
            anyhow::bail!("project promote is handled by the authored promotion flow")
        }
    }
}

fn run_task_command(repo: &Path, command: &TaskCommand) -> anyhow::Result<()> {
    match command {
        TaskCommand::Run {
            issue,
            directive,
            json,
        } => {
            let session = loopflow::ops::task::task_run(repo, issue, directive.clone())?;
            print_task_session(&session, *json)
        }
        TaskCommand::Start {
            title,
            project_id,
            directive,
            json,
        } => {
            let session = loopflow::ops::task::task_start(
                repo,
                title.clone(),
                project_id,
                directive.clone(),
            )?;
            print_task_session(&session, *json)
        }
        TaskCommand::Status { issue, json } => {
            let session = loopflow::ops::task::task_status(issue)?;
            print_task_session(&session, *json)
        }
        TaskCommand::Changes { issue, json } => {
            let snapshot = loopflow::ops::task::task_changes(issue)?;
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
        TaskCommand::Diff { issue, path, json } => {
            let snapshot = loopflow::ops::task::task_diff(issue, path.as_deref())?;
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
        TaskCommand::File { issue, path, json } => {
            let snapshot = loopflow::ops::task::task_file(issue, path)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&snapshot)?);
            } else if snapshot.binary {
                anyhow::bail!("{} is binary", snapshot.path);
            } else {
                print!("{}", snapshot.content.as_deref().unwrap_or_default());
                if snapshot.truncated {
                    eprintln!("\n[file truncated at 1 MB]");
                }
            }
            Ok(())
        }
        TaskCommand::FollowUp {
            issue,
            message,
            json,
        } => {
            let result = loopflow::ops::task::task_follow_up(issue, message.clone())?;
            print_task_control(&result, *json)
        }
        TaskCommand::Steer {
            issue,
            message,
            json,
        } => {
            let result = loopflow::ops::task::task_steer(issue, message.clone())?;
            print_task_control(&result, *json)
        }
        TaskCommand::Interrupt {
            issue,
            message,
            json,
        } => {
            let result = loopflow::ops::task::task_interrupt(issue, message.clone())?;
            print_task_control(&result, *json)
        }
        TaskCommand::Receipt {
            command_id,
            until,
            timeout,
            json,
        } => {
            let timeout = parse_duration(timeout)?;
            let read = loopflow::ops::task::task_receipt(command_id, *until, timeout)?;
            print_task_control(&read.receipt, *json)?;
            if read.timed_out {
                std::process::exit(124);
            }
            Ok(())
        }
        TaskCommand::Acknowledge {
            issue,
            directive,
            summary,
            json,
        } => {
            let result = loopflow::ops::task::task_acknowledge(issue, *directive, summary.clone())?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("{} incorporated directive v{}", issue, directive);
            }
            Ok(())
        }
        TaskCommand::Decide {
            issue,
            decision_id,
            choice,
            message,
            json,
        } => {
            let result = loopflow::ops::task::task_decide(
                issue,
                decision_id,
                choice.clone(),
                message.clone(),
            )?;
            print_task_control(&result, *json)
        }
        TaskCommand::RequestDecision {
            issue,
            prompt,
            options,
            wait,
            timeout,
            json,
        } => {
            let result = loopflow::ops::task::task_request_decision(
                issue,
                prompt.clone(),
                options.clone(),
                *wait,
                parse_duration(timeout)?,
            )?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else if result.resolved {
                println!(
                    "{} → {}",
                    result.decision_id,
                    result.choice.as_deref().unwrap_or("resolved")
                );
            } else {
                println!("{} → pending", result.decision_id);
            }
            Ok(())
        }
        TaskCommand::Wait {
            issue,
            until,
            timeout,
            json,
        } => {
            let until = if until == "submitted" {
                loopflow::ops::task::TaskWaitUntil::Submitted
            } else {
                loopflow::ops::task::TaskWaitUntil::Terminal
            };
            let timeout = timeout.as_deref().map(parse_duration).transpose()?;
            let session = loopflow::ops::task::task_wait(issue, until, timeout)?;
            print_task_session(&session, *json)
        }
        TaskCommand::Resume {
            issue,
            message,
            json,
        } => {
            let result = loopflow::ops::task::task_resume(issue, message.clone())?;
            print_task_control(&result, *json)
        }
        TaskCommand::Attach { issue } => {
            loopflow::ops::task::task_attach(issue).map_err(Into::into)
        }
        TaskCommand::Abandon {
            issue,
            reason,
            json,
        } => {
            let result = loopflow::ops::task::task_abandon(issue, reason.clone())?;
            print_task_control(&result, *json)
        }
    }
}

fn main() -> anyhow::Result<()> {
    // Ensure Ctrl+C terminates lf and the child agent. Without this,
    // child.wait() retries on EINTR and hangs while the agent catches
    // SIGINT and keeps running. SIGTERM the agent first so it doesn't
    // survive as an orphan. The `termination` feature extends the handler
    // to SIGTERM and SIGHUP: `tmux kill-session` delivers SIGHUP, which
    // otherwise bypasses every cleanup (observed live: it orphaned the wave
    // loop's codex app-server pair and left a stale .wave-endpoint).
    ctrlc::set_handler(|| {
        loopflow::engine::agent::run_interrupt_cleanups();
        loopflow::engine::agent::kill_child_if_running();
        std::process::exit(130);
    })
    .expect("failed to set Ctrl+C handler");

    // Initialize tracing with RUST_LOG env filter
    // Usage: RUST_LOG=lf=debug lf debug
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lf=info,loopflow=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .init();

    // Reorder args so flags can appear after the skill name
    let raw_args: Vec<String> = std::env::args().collect();
    let args = reorder_args(raw_args.clone());

    let mut cli = Cli::parse_from(args.clone());
    let explicit_wave = cli
        .wave
        .as_deref()
        .map(loopflow::engine::wave_context::resolve_explicit_wave)
        .transpose()?;
    if let Some(wave) = &explicit_wave {
        cli.wave = Some(wave.name().to_string());
    }
    // One resolved wave identity drives prompt context, registry attribution,
    // journaling, and every child process. An explicit wave is also the
    // default bus channel for this invocation.
    let _explicit_wave_env = explicit_wave.as_ref().map(|wave| {
        EnvGuard::set(
            loopflow::engine::wave_context::WAVE_ID_ENV,
            wave.id().to_string(),
        )
    });
    let _explicit_channel_env = explicit_wave.as_ref().map(|wave| {
        EnvGuard::set(
            loopflow::engine::wave_context::CHANNEL_ENV,
            wave.name().to_string(),
        )
    });
    debug!(?cli, "parsed CLI arguments");

    let result = if cli.list {
        in_repo_runtime(&args, |_| loopflow::lf::commands::list::show_all())
    } else {
        match &cli.command {
            Some(Commands::Inline { prompt }) => {
                let text = prompt.join(" ");
                in_repo_runtime(&args, |_| {
                    loopflow::lf::commands::run::run(None, Some(&text), &cli)
                })
            }
            Some(Commands::Pr { cmd }) => in_repo_runtime(&args, |_| {
                loopflow::lf::commands::ops::run_pr(cmd.as_ref(), cli.model.as_deref())
            }),
            Some(Commands::Wt { cmd }) => {
                in_repo_runtime(&args, |_| loopflow::lf::commands::ops::run_wt(cmd))
            }
            Some(Commands::Rebase {
                plan,
                manual,
                continue_rebase,
                abort,
                onto,
            }) => in_repo_runtime(&args, |_| {
                loopflow::lf::commands::ops::run_rebase(
                    onto.as_deref(),
                    *plan,
                    *manual,
                    *continue_rebase,
                    *abort,
                )
            }),
            Some(Commands::Commit {
                message,
                push,
                no_add,
            }) => in_repo_runtime(&args, |_| {
                loopflow::lf::commands::ops::run_commit(
                    message.as_deref(),
                    *push,
                    *no_add,
                    cli.model.as_deref(),
                )
            }),
            Some(Commands::Auth { cmd }) => {
                in_repo_runtime(&args, |_| loopflow::lf::commands::auth::run(cmd))
            }
            Some(Commands::Release { cmd }) => {
                in_repo_runtime(&args, |_| loopflow::lf::commands::ops::run_release(cmd))
            }
            Some(Commands::Pm { cmd }) => {
                in_repo_runtime(&args, |_| loopflow::lf::commands::ops::run_pm(cmd))
            }
            Some(Commands::SyncSkills { yes, no_prune }) => in_repo_runtime(&args, |_| {
                loopflow::lf::commands::ops::run_sync_skills(*yes, *no_prune)
            }),
            Some(Commands::Cron { cmd }) => {
                in_repo_runtime(&args, |_| loopflow::lf::commands::ops::cron_cmd(cmd))
            }
            Some(Commands::Wave { name, force }) => {
                in_repo_runtime(&args, |_| loopflow::wave::run(name, *force))
            }
            Some(Commands::Stop { name }) => in_repo_runtime(&args, |_| loopflow::wave::stop(name)),
            Some(Commands::Resident { name }) => {
                in_repo_runtime(&args, |_| loopflow::wave::resident::run(name))
            }
            Some(Commands::FlowStep { flow, index, seed }) => in_repo_runtime(&args, |repo| {
                loopflow::lf::commands::flow::run_step(flow, *index, seed, &cli, repo)
            }),
            Some(Commands::Project {
                cmd: ProjectCommand::Promote { slug, wave },
            }) => in_repo_runtime(&args, |repo| {
                let parent = wave.clone().ok_or_else(|| {
                    anyhow::anyhow!("cannot determine parent wave; pass --wave <name>")
                })?;
                let message = format!(
                    "Promote project '{slug}' from parent wave '{parent}'. Complete the authored migration, PM move, parent link, and residency checks."
                );
                run_target("project-promote", Some(&message), &cli, &args)?;
                let session = loopflow::ops::project::complete_promotion(repo, &parent, slug)
                    .map_err(anyhow::Error::from)?;
                println!("promoted {slug} from {parent}; residency: {session}");
                Ok(())
            }),
            Some(Commands::Project { cmd }) => {
                in_repo_runtime(&args, |repo| run_project_command(repo, cmd))
            }
            Some(Commands::Task { cmd }) => {
                in_repo_runtime(&args, |repo| run_task_command(repo, cmd))
            }
            Some(Commands::TaskRunner {
                session_id,
                generation,
            }) => {
                let session_id = session_id.parse()?;
                tokio::runtime::Runtime::new()?.block_on(loopflow::task::runner::run_task_session(
                    session_id,
                    *generation,
                ))
            }
            Some(Commands::ProjectRunner {
                session_id,
                generation,
            }) => {
                let session_id = session_id.parse()?;
                tokio::runtime::Runtime::new()?.block_on(
                    loopflow::project_session::runner::run_project_session(session_id, *generation),
                )
            }
            Some(Commands::Tokens { json, days }) => {
                loopflow::lf::commands::tokens::run(*json, *days)
            }
            Some(Commands::Usage { json, days }) => {
                loopflow::lf::commands::usage::run(*json, *days)
            }
            Some(Commands::Context {
                days,
                wave,
                repo,
                json,
            }) => {
                loopflow::lf::commands::context::run(*days, wave.as_deref(), repo.as_deref(), *json)
            }
            Some(Commands::Doctor { json }) => loopflow::lf::commands::doctor::run(*json),
            Some(Commands::Ls { json }) => loopflow::lf::commands::waves::ls(*json),
            Some(Commands::Status { wave, json }) => {
                loopflow::lf::commands::waves::status(wave.as_deref(), *json)
            }
            Some(Commands::Runs { json }) => loopflow::lf::commands::runs::list(*json),
            Some(Commands::Trace {
                run_id,
                json,
                events,
                jsonl,
                launch,
            }) => loopflow::lf::commands::runs::trace(
                run_id,
                *json,
                *events,
                *jsonl,
                launch.as_deref(),
            ),
            Some(Commands::Chat {
                text,
                follow,
                steer,
                target,
            }) => loopflow::lf::commands::chat::run(text, *follow, *steer, target),
            Some(Commands::Radio { command }) => match command {
                loopflow::lf::RadioCommand::Pub {
                    text,
                    channel,
                    parent,
                    from,
                } => loopflow::lf::commands::radio::run_pub(
                    text,
                    channel.as_deref(),
                    *parent,
                    from.as_deref(),
                ),
                loopflow::lf::RadioCommand::Sub { channel, json } => {
                    loopflow::lf::commands::sub::run(channel.as_deref(), *json)
                }
            },
            Some(Commands::RetiredSub { .. }) => unreachable!("retired sub cannot parse"),
            Some(Commands::RetiredOp { .. }) => unreachable!("retired op cannot parse"),
            Some(Commands::Memory { cmd, target }) => {
                loopflow::lf::commands::memory::run(cmd.as_ref(), target)
            }
            Some(Commands::Ssh {
                host,
                repo,
                secret,
                forward_agent,
                cmd,
            }) => {
                loopflow::lf::commands::ssh::run(host, repo.as_deref(), secret, *forward_agent, cmd)
            }
            Some(Commands::Flow { name, args: rest }) => {
                require_target_kind(name, TargetKind::Flow)?;
                let message = join_args(rest);
                run_target(name, message.as_deref(), &cli, &args)
            }
            Some(Commands::Skill { name, args: rest }) => {
                require_target_kind(name, TargetKind::Skill)?;
                let message = join_args(rest);
                run_target(name, message.as_deref(), &cli, &args)
            }
            Some(Commands::External(external_args)) => {
                match loopflow::lf::commands::run::split_skill_args(external_args) {
                    Ok((name, skill_args)) => {
                        let message = join_args(&skill_args);
                        run_target(&name, message.as_deref(), &cli, &args)
                    }
                    Err(err) => Err(err),
                }
            }
            None => in_repo_runtime(&args, |_| {
                loopflow::lf::commands::run::run(None, None, &cli)
            }),
        }
    };

    result
}

#[cfg(test)]
mod tests {
    use super::{arg_tables, reorder_args};

    use clap::Parser;
    use loopflow::lf::{Cli, Commands, PmCommand, PmTaskCommand, PrCommand};

    /// The derived tables cover everything the old hand lists carried, plus
    /// the uppercase short aliases those lists had drifted away from.
    #[test]
    fn derived_tables_cover_commands_flags_and_aliases() {
        let tables = arg_tables();
        for command in [
            ":", "pr", "wt", "rebase", "commit", "auth", "release", "pm", "task", "project",
            "flow", "skill", "chat", "memory", "usage", "ls", "status", "runs", "trace", "help",
        ] {
            assert!(tables.commands.contains_key(command), "command {command}");
        }
        for flag in [
            "-d",
            "-D",
            "--direction",
            "--docs",
            "-m",
            "-M",
            "--model",
            "--max-turns",
            "-w",
            "-W",
            "--wave",
        ] {
            assert!(tables.top_level.value.contains(flag), "value flag {flag}");
        }
        for flag in [
            "-l",
            "--list",
            "-c",
            "-C",
            "--clipboard",
            "--no-direction",
            "--yolo",
            "-i",
            "-I",
            "-b",
            "-B",
            "--tui",
            "--ide",
            "--chrome",
            "--no-chrome",
            "--diff-files",
            "--no-diff-files",
            "--diff",
            "--no-diff",
            "--no-loopflow",
            "-h",
            "--help",
            "-V",
            "--version",
        ] {
            assert!(tables.top_level.boolean.contains(flag), "bool flag {flag}");
        }
    }

    /// Uppercase short aliases reorder exactly like their lowercase forms —
    /// the drift the hand-maintained lists had (`lf debug -M codex` used to
    /// treat `codex` as a skill arg).
    #[test]
    fn reorder_args_uppercase_value_alias_after_skill() {
        let args = vec![
            "lf".to_string(),
            "debug".to_string(),
            "-M".to_string(),
            "codex".to_string(),
        ];
        assert_eq!(reorder_args(args), vec!["lf", "-M", "codex", "debug"]);
    }

    #[test]
    fn reorder_args_uppercase_bool_alias_after_skill() {
        let args = vec!["lf".to_string(), "debug".to_string(), "-C".to_string()];
        assert_eq!(reorder_args(args), vec!["lf", "-C", "debug"]);
    }

    #[test]
    fn reorder_args_flag_after_skill() {
        let args = vec!["lf".to_string(), "debug".to_string(), "-c".to_string()];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-c", "debug"]);
    }

    /// Serving a mind is its own command. Nothing about the ambient
    /// environment can turn one of these into the other.
    #[test]
    fn wave_and_resident_are_distinct_entrypoints() {
        let served = Cli::try_parse_from(["lf", "wave", "goals"]).unwrap();
        assert!(matches!(
            served.command,
            Some(Commands::Wave { name, force: false }) if name == "goals"
        ));

        let forced = Cli::try_parse_from(["lf", "wave", "goals", "--force"]).unwrap();
        assert!(matches!(
            forced.command,
            Some(Commands::Wave { force: true, .. })
        ));

        let stopped = Cli::try_parse_from(["lf", "stop", "goals"]).unwrap();
        assert!(matches!(
            stopped.command,
            Some(Commands::Stop { name }) if name == "goals"
        ));

        // The listener's own body — hidden, but spellable, because the
        // listener spawns it by name rather than by leaking env.
        let body = Cli::try_parse_from(["lf", "__resident", "goals"]).unwrap();
        assert!(matches!(
            body.command,
            Some(Commands::Resident { name }) if name == "goals"
        ));
    }

    /// `serve` is retired. The parser can't reject it outright — the
    /// `external_subcommand` catch-all claims any unmatched verb — so the
    /// property that actually holds is that it no longer names a built-in
    /// command. The exec door denies `External` on top of that.
    #[test]
    fn old_serve_surface_is_no_longer_a_builtin_command() {
        let cli = Cli::try_parse_from(["lf", "serve", "goals"]).expect("falls through to external");
        assert!(
            matches!(cli.command, Some(Commands::External(parts)) if parts[0] == "serve"),
            "`serve` survives only as an external verb, not a built-in"
        );
        assert!(matches!(
            Cli::try_parse_from(["lf", "wave", "goals"])
                .expect("the replacement")
                .command,
            Some(Commands::Wave { .. })
        ));
    }

    /// The `lf op` namespace is retired, and a caller who still types it hears
    /// where the operation went. `op next` is the one with nowhere to go: the
    /// ephemeral rotation it drove was deleted, not renamed.
    #[test]
    fn retired_op_namespace_names_its_replacement() {
        let removed = Cli::try_parse_from(["lf", "op", "next"])
            .expect_err("`lf op next` cannot parse")
            .to_string();
        assert!(
            removed.contains("no replacement") && removed.contains("lf task run"),
            "`lf op next` should state the removal and how work is dispatched now: {removed}"
        );

        let landed = Cli::try_parse_from(["lf", "op", "land"])
            .expect_err("`lf op land` cannot parse")
            .to_string();
        assert!(
            landed.contains("`lf pr land`"),
            "`lf op land` should name `lf pr land`: {landed}"
        );

        // Bare `lf op` has no verb to map, so it falls to the namespace line.
        let bare = Cli::try_parse_from(["lf", "op"])
            .expect_err("bare `lf op` cannot parse")
            .to_string();
        assert!(
            bare.contains("top-level"),
            "bare `lf op` should say the operations are top-level: {bare}"
        );
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
        // lf -m codex implement -> should stay the same (already correct order)
        let args = vec![
            "lf".to_string(),
            "-m".to_string(),
            "codex".to_string(),
            "implement".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-m", "codex", "implement"]);
    }

    #[test]
    fn reorder_args_value_flag_after_skill() {
        let args = vec![
            "lf".to_string(),
            "debug".to_string(),
            "-m".to_string(),
            "codex".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-m", "codex", "debug"]);
    }

    #[test]
    fn reorder_args_mixed_flags() {
        let args = vec![
            "lf".to_string(),
            "-i".to_string(),
            "implement".to_string(),
            "-c".to_string(),
            "-m".to_string(),
            "claude".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-i", "-c", "-m", "claude", "implement"]);
    }

    #[test]
    fn reorder_args_no_direction_flag_after_skill() {
        let args = vec![
            "lf".to_string(),
            "implement".to_string(),
            "--no-direction".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "--no-direction", "implement"]);
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
            reorder_args(args),
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
        let args: Vec<String> = ["lf", "pm", "--wave", "systems", "show"]
            .map(String::from)
            .to_vec();
        let reordered = reorder_args(args);
        assert_eq!(reordered, vec!["lf", "pm", "show", "--wave", "systems"]);
        assert!(matches!(
            Cli::try_parse_from(reordered).unwrap().command,
            Some(Commands::Pm {
                cmd: PmCommand::Show { .. }
            })
        ));

        let args: Vec<String> = [
            "lf",
            "pm",
            "task",
            "--wave",
            "systems",
            "create",
            "--project",
            "wave-chat",
            "--title",
            "file it",
        ]
        .map(String::from)
        .to_vec();
        let reordered = reorder_args(args);
        assert_eq!(
            reordered,
            vec![
                "lf",
                "pm",
                "task",
                "create",
                "--wave",
                "systems",
                "--project",
                "wave-chat",
                "--title",
                "file it"
            ]
        );
        assert!(matches!(
            Cli::try_parse_from(reordered).unwrap().command,
            Some(Commands::Pm {
                cmd: PmCommand::Task {
                    cmd: PmTaskCommand::Create { .. }
                }
            })
        ));

        let args: Vec<String> = ["lf", "pr", "-m", "codex", "open"]
            .map(String::from)
            .to_vec();
        let reordered = reorder_args(args);
        assert_eq!(reordered, vec!["lf", "pr", "open", "-m", "codex"]);
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

        let args: Vec<String> = ["lf", "wt", "--force", "rm", "old-tree"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            reorder_args(args),
            vec!["lf", "wt", "rm", "--force", "old-tree"]
        );
    }

    #[test]
    fn reorder_args_keeps_valid_parent_flags_on_the_parent() {
        let args: Vec<String> = ["lf", "memory", "--wave", "systems", "add", "fact"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            reorder_args(args),
            vec!["lf", "memory", "--wave", "systems", "add", "fact"]
        );
    }

    /// `lf chat --wave X text` must reach the chat subcommand untouched —
    /// hoisting `--wave` to the top level silently retargets the publish.
    #[test]
    fn reorder_args_leaves_chat_and_memory_targeting_alone() {
        let args: Vec<String> = ["lf", "chat", "--wave", "systems", "shipped it"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            reorder_args(args),
            vec!["lf", "chat", "--wave", "systems", "shipped it"]
        );

        let args: Vec<String> = ["lf", "memory", "add", "fact", "--wave", "systems"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            reorder_args(args),
            vec!["lf", "memory", "add", "fact", "--wave", "systems"]
        );

        let args: Vec<String> = ["lf", "pm", "show", "--wave", "systems"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            reorder_args(args),
            vec!["lf", "pm", "show", "--wave", "systems"]
        );
    }

    #[test]
    fn reorder_args_no_skill() {
        let args = vec!["lf".to_string(), "-l".to_string()];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-l"]);
    }
}
