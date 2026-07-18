use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use clap::{CommandFactory, Parser};
use tracing::{debug, warn};
use tracing_subscriber::EnvFilter;

use loopflow::journal::{self, LfEventFields, LfEventType, LfNode};
use loopflow::lf::{Cli, Commands, InstallCommand, ProjectCommand, RunsCommand, TaskCommand};

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
    let attribution = loopflow::engine::wave_context::run_attribution();
    if let Some(failure) = attribution.failure.as_deref() {
        warn!(
            error = failure,
            "ambient wave identity failed validation; run attributed to no wave \
             — pass --wave <name> to recover"
        );
    }
    journal::emit(
        repo_root,
        LfNode::Run,
        LfEventType::Started,
        LfEventFields {
            wave_name: attribution.wave,
            error: attribution.failure,
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

/// Build a local, in-process account-lease broker for a non-`ssh` command that
/// carries `--account`/`--only-account`, exporting the opaque `LF_ACCOUNT_LEASE`
/// handle so this process and its children resolve one credential through it.
/// Returns `None` when the selection resolves to no grant. The returned broker
/// and env guard must outlive the command.
fn build_local_account_lease(
    selection: &loopflow::provider_account::lease::AccountSelection,
) -> anyhow::Result<
    Option<(
        loopflow::provider_account::lease::AccountLeaseBroker,
        EnvGuard,
    )>,
> {
    use loopflow::provider_account::lease;
    let runtime = tokio::runtime::Runtime::new()?;
    let Some(broker) = runtime.block_on(lease::AccountLeaseBroker::start_root(selection))? else {
        return Ok(None);
    };
    let guard = EnvGuard::set(lease::ACCOUNT_LEASE_ENV, broker.local_env_value()?);
    Ok(Some((broker, guard)))
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

fn format_child_body(
    agent: &str,
    provider: &str,
    process: Option<&loopflow::child_session::ChildProcessGeneration>,
) -> String {
    process.map_or_else(
        || format!("none; next agent {agent}, provider {provider}"),
        |process| {
            let generation = process.generation;
            let provenance = process.provenance.as_ref().map_or_else(
                || "binary unknown".to_string(),
                |provenance| format!("binary {} ({})", provenance.version, provenance.provenance),
            );
            format!("generation {generation}; agent {agent}; provider {provider}; {provenance}")
        },
    )
}

/// One PR's line in `lf task status`. A degraded Linear linkage is named here
/// because this reading is where an operator already looks for writeback health —
/// the session's `PM writeback` line sits directly above. Silence means linked.
fn format_task_pr_line(pr: &loopflow::task::TaskPr) -> String {
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

fn print_task_session(session: &loopflow::task::TaskSession, json: bool) -> anyhow::Result<()> {
    let snapshot = loopflow::ops::task::task_snapshot(session)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
    } else {
        let pm_writeback = match &session.pm_writeback {
            loopflow::task::PmWritebackState::Current => "current".to_string(),
            loopflow::task::PmWritebackState::Pending { error, .. } => {
                format!("pending: {error}")
            }
        };
        let branch = snapshot
            .active_pr
            .as_ref()
            .and_then(|active| snapshot.prs.iter().find(|pr| &pr.id == active))
            .map(|pr| pr.branch.as_str())
            .unwrap_or("none");
        let body = format_child_body(
            &session.agent,
            &session.provider,
            session.latest_process.as_ref(),
        );
        println!(
            "{}  {}\n  session: {}\n  phase: {} cycle {}\n  flow: {} ({}, iteration {}, step {})\n  body: {}\n  worktree: {}\n  branch: {}\n  PM writeback: {}\n  reason: {}",
            session.launch.issue.identifier,
            session.status.as_str(),
            session.id,
            session.lifecycle_phase.as_str(),
            session.lifecycle_cycle(),
            session.phase_plan().flow,
            session.phase_plan().interaction_policy.as_str(),
            session.phase_iteration + 1,
            session.phase_cursor + 1,
            body,
            session.worktree.display(),
            branch,
            pm_writeback,
            session.status_reason,
        );
        // State the Task's parent-Project routing as fact: the historical owner
        // and the live successor it routes to. No "terminal wake" warning — a
        // broken chain is reported as a missing routing target, not a wake.
        if snapshot.project_route_succeeded {
            if let Some(routing) = &snapshot.routing_project_session_id {
                println!(
                    "  project: {} → routes to {}",
                    session.project_session_id, routing,
                );
            }
        } else if snapshot.routing_project_session_id.is_none() {
            println!(
                "  project: {} → no live successor; resume or restart the Project",
                session.project_session_id,
            );
        }
        for pr in &snapshot.prs {
            println!("{}", format_task_pr_line(pr));
        }
        match &snapshot.observation {
            loopflow::task::Observation::Cached { observed_at } => {
                println!("  PR observation: cached from {observed_at}");
            }
            loopflow::task::Observation::Degraded {
                reason,
                cached_as_of,
                retry_at,
            } => {
                println!(
                    "  PR observation: degraded — {reason} (cached from {cached_as_of}; retry after {retry_at})"
                );
            }
            loopflow::task::Observation::NotRequired
            | loopflow::task::Observation::Fresh { .. } => {}
        }
        let actions = &snapshot.actions;
        if let Some(recommended) = actions.recommended {
            let reason = actions
                .status(recommended)
                .map(|s| s.reason.as_str())
                .unwrap_or("");
            println!("  action: {}  ({})", recommended.as_str(), reason);
            use loopflow::task::actions::TaskAction;
            for status in &actions.actions {
                if !status.available && status.action != TaskAction::NoAction {
                    println!(
                        "    blocked: {}  ({})",
                        status.action.as_str(),
                        status.reason,
                    );
                }
            }
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
            loopflow::task::Observation::Cached { observed_at } => {
                println!("PR observation: cached from {observed_at}");
            }
            loopflow::task::Observation::Degraded {
                reason,
                cached_as_of,
                retry_at,
            } => {
                println!(
                    "PR observation: degraded — {reason} (cached from {cached_as_of}; retry after {retry_at})"
                );
            }
            loopflow::task::Observation::NotRequired
            | loopflow::task::Observation::Fresh { .. } => {}
        }
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
        let body = format_child_body(
            &session.agent,
            &session.provider,
            session.latest_process.as_ref(),
        );
        println!(
            "{}  {}\n  session: {}\n  body: {}\n  iteration: {}\n  reason: {}",
            session.launch.project.slug,
            session.status.as_str(),
            session.id,
            body,
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
        println!(
            "{} → {} ({})",
            result.receipt.label(),
            result.project_id,
            result.receipt.action(),
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
        ProjectCommand::Steer {
            project_id,
            message,
            json,
        } => {
            let result = loopflow::ops::project::project_steer(project_id, message.clone())?;
            print_project_control(&result, *json)
        }
        ProjectCommand::Interrupt { project_id, json } => {
            let result = loopflow::ops::project::project_interrupt(project_id)?;
            print_project_control(&result, *json)
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
            model,
            reason,
            json,
        } => {
            let result =
                loopflow::ops::project::project_resume(project_id, model.clone(), reason.clone())?;
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
            name,
            flow,
            stack_on,
            directive,
            headless,
            json,
        } => {
            let session = loopflow::ops::task::task_run(
                repo,
                issue,
                loopflow::ops::task::TaskLaunchOptions {
                    name: name.clone(),
                    flow: flow.clone(),
                    stack_on: stack_on.clone(),
                    directive: directive.clone(),
                    headless: *headless,
                },
            )?;
            print_task_session(&session, *json)
        }
        TaskCommand::Start {
            title,
            project_id,
            name,
            flow,
            stack_on,
            directive,
            headless,
            json,
        } => {
            let session = loopflow::ops::task::task_start(
                repo,
                title.clone(),
                project_id,
                loopflow::ops::task::TaskLaunchOptions {
                    name: name.clone(),
                    flow: flow.clone(),
                    stack_on: stack_on.clone(),
                    directive: directive.clone(),
                    headless: *headless,
                },
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
        TaskCommand::Complete {
            issue,
            summary,
            json,
        } => {
            let session = loopflow::ops::task::task_complete(issue, summary.clone())?;
            print_task_session(&session, *json)
        }
        TaskCommand::Steer {
            issue,
            message,
            json,
        } => {
            let result = loopflow::ops::task::task_steer(issue, message.clone())?;
            print_task_control(&result, *json)
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
            let session = loopflow::ops::task::task_wait(issue, until, timeout)?;
            print_task_session(&session, *json)
        }
        TaskCommand::Resume {
            issue,
            model,
            reason,
            json,
        } => {
            let result = loopflow::ops::task::task_resume(issue, model.clone(), reason.clone())?;
            print_task_control(&result, *json)
        }
        TaskCommand::Recover {
            issue,
            reason,
            json,
        } => {
            let session = loopflow::ops::task::task_recover(issue, reason.clone())?;
            print_task_session(&session, *json)
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
    // `--account`/`--only-account` are resolved once at the outer invocation.
    // Under an already-forwarded lease the grant is fixed, so a nested
    // selection is rejected rather than silently re-derived.
    let account_selection = loopflow::provider_account::lease::AccountSelection::from_flags(
        &cli.account,
        &cli.only_account,
    )?;
    loopflow::provider_account::lease::validate_account_selection(&account_selection)?;
    if cli.account_lease_probe {
        return loopflow::provider_account::lease::probe_forwarded_authority()
            .map_err(anyhow::Error::from);
    }
    debug!(?cli, "parsed CLI arguments");

    // Global-promotion commands dispatch before home routing, journal emission,
    // and any ordinary store open: a candidate that does not know the live
    // migration frontier must reach the preflight refusal, not fail in
    // trace/store capture. `lf install` opens the store only read-only, inside
    // its own preflight.
    if let Some(Commands::Install { cmd }) = &cli.command {
        return match cmd {
            InstallCommand::Preflight { json } => loopflow::lf::commands::install::preflight(*json),
            InstallCommand::Promote {
                cli_target,
                app_source,
                app_target,
                legacy_app_target,
                sync_skills,
                preview,
            } => loopflow::lf::commands::install::promote(
                cli_target,
                app_source.as_deref(),
                app_target.as_deref(),
                legacy_app_target.as_deref(),
                *sync_skills,
                *preview,
            ),
            InstallCommand::Rollback {
                cli_target,
                candidate,
            } => loopflow::lf::commands::install::rollback(cli_target, candidate),
        };
    }

    // Route repo/PR/release/PM commands to the Wave's execution home before local
    // dispatch. A remote (SSH) home forwards over `lf ssh`; a local or absent home
    // falls through and runs in-process exactly as before.
    if let Some(command) = &cli.command {
        if let Some(routed) = loopflow::lf::commands::home::route(
            command,
            cli.wave.as_deref(),
            &account_selection,
            &args,
        ) {
            return routed;
        }
    }

    // SSH and remote-Home commands build and forward their broker in the
    // transport path. Every local command with a selection gets an in-process
    // broker here, after early install and Home routing have had their chance
    // to dispatch without opening the ordinary account store.
    let is_ssh = matches!(cli.command, Some(loopflow::lf::Commands::Ssh { .. }));
    let _local_account_lease = if is_ssh || account_selection.is_default() {
        None
    } else {
        build_local_account_lease(&account_selection)?
    };

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
            Some(Commands::Desktop) => loopflow::lf::commands::desktop::run(),
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
                adopt,
                onto,
            }) => in_repo_runtime(&args, |_| {
                loopflow::lf::commands::ops::run_rebase(
                    onto.as_deref(),
                    *plan,
                    *manual,
                    *continue_rebase,
                    *abort,
                    *adopt,
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
            Some(Commands::Profile { cmd }) => in_repo_runtime(&args, |repo| {
                loopflow::lf::commands::profile::run(cmd, repo)
            }),
            Some(Commands::Route { cmd }) => in_repo_runtime(&args, |repo| {
                loopflow::lf::commands::profile::run_route(cmd, repo)
            }),
            Some(Commands::Release { cmd }) => {
                in_repo_runtime(&args, |_| loopflow::lf::commands::ops::run_release(cmd))
            }
            Some(Commands::Pm { cmd }) => {
                in_repo_runtime(&args, |_| loopflow::lf::commands::ops::run_pm(cmd))
            }
            Some(Commands::Home { cmd }) => {
                in_repo_runtime(&args, |repo| loopflow::lf::commands::home::run(cmd, repo))
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
                let parent =
                    loopflow::engine::wave_context::resolve_managed_wave_name_sync(wave.as_deref())
                        .map_err(|err| match err {
                            loopflow::engine::wave_context::WaveResolveError::NoContext => {
                                anyhow::anyhow!(
                                    "cannot determine parent wave; pass --wave <name>"
                                )
                            }
                            other => anyhow::Error::from(other),
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
            Some(Commands::Launch { cmd }) => {
                in_repo_runtime(&args, |_| loopflow::lf::commands::launch::run(cmd))
            }
            Some(Commands::Work { cmd }) => {
                in_repo_runtime(&args, |_| loopflow::lf::commands::work::run(cmd))
            }
            Some(Commands::Queue { json }) => {
                loopflow::lf::commands::work::run_queue(*json)
            }
            Some(Commands::FeedbackExitGuard {
                kind,
                id,
                launch_id,
                epoch_id,
                revision,
            }) => loopflow::lf::commands::work::run_exit_guard(
                kind,
                id,
                launch_id,
                epoch_id,
                *revision,
            ),
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
            Some(Commands::Usage {
                json,
                days,
                refresh,
                cached,
            }) => loopflow::lf::commands::usage::run(*json, *days, *refresh, *cached),
            Some(Commands::Ci {
                since,
                wave,
                repo,
                json,
            }) => loopflow::lf::commands::ci::run(
                since,
                wave.as_deref(),
                repo.as_deref(),
                *json,
            ),
            Some(Commands::Top) => loopflow::lf::commands::top::run(),
            Some(Commands::Context {
                days,
                started_after,
                started_before,
                wave,
                project,
                task,
                repo,
                flow,
                skill,
                provider,
                model,
                surface,
                outcome,
                capture_state,
                steered_only,
                current_revision_only,
                json,
            }) => loopflow::lf::commands::context::run(
                loopflow::lf::commands::context::ContextQueryOptions {
                    days: *days,
                    started_after: *started_after,
                    started_before: *started_before,
                    repo_paths: repo.clone(),
                    waves: wave.clone(),
                    projects: project.clone(),
                    tasks: task.clone(),
                    flows: flow.clone(),
                    skills: skill.clone(),
                    providers: provider.clone(),
                    models: model.clone(),
                    surfaces: surface.clone(),
                    outcomes: outcome.clone(),
                    capture_states: capture_state.clone(),
                    steered_only: *steered_only,
                    current_revision_only: *current_revision_only,
                    json: *json,
                },
            ),
            Some(Commands::Doctor { json }) => loopflow::lf::commands::doctor::run(*json),
            Some(Commands::Ls { json }) => loopflow::lf::commands::waves::ls(*json),
            Some(Commands::Status { wave, json }) => {
                loopflow::lf::commands::waves::status(wave.as_deref(), *json)
            }
            Some(Commands::Roadmap { wave, json }) => {
                loopflow::lf::commands::waves::roadmap(wave.as_deref(), *json)
            }
            Some(Commands::Runs {
                task,
                wave,
                json,
                cmd,
            }) => match cmd {
                Some(RunsCommand::Reconcile { apply, all, json }) => {
                    loopflow::lf::commands::runs::reconcile(*apply, *all, *json)
                }
                None => loopflow::lf::commands::runs::list(
                    *json,
                    wave.as_deref(),
                    task.as_deref(),
                ),
            },
            Some(Commands::Execs { json }) => loopflow::lf::commands::runs::list_execs(*json),
            Some(Commands::Trace {
                exec_id,
                json,
                content,
                events,
                jsonl,
                launch,
                turn,
            }) => loopflow::lf::commands::runs::trace(
                exec_id,
                *json,
                *content,
                *events,
                *jsonl,
                launch.as_deref(),
                turn.as_deref(),
            ),
            Some(Commands::Chat {
                text,
                follow,
                steer,
                history,
                json,
                limit,
                target,
            }) => loopflow::lf::commands::chat::run(
                text, *follow, *steer, *history, *json, *limit, target,
            ),
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
            Some(Commands::Install { .. }) => {
                unreachable!("install dispatches before home routing")
            }
            Some(Commands::RetiredSub { .. }) => unreachable!("retired sub cannot parse"),
            Some(Commands::RetiredOp { .. }) => unreachable!("retired op cannot parse"),
            Some(Commands::Memory { cmd, target }) => {
                loopflow::lf::commands::memory::run(cmd.as_ref(), target)
            }
            Some(Commands::Receipt { cmd }) => match cmd {
                loopflow::lf::ReceiptCommand::Show { token, wave, json } => {
                    loopflow::lf::commands::receipt::run(token, wave.as_deref(), *json)
                }
            },
            Some(Commands::Ssh {
                host,
                repo,
                secret,
                forward_agent,
                cmd,
            }) => loopflow::lf::commands::ssh::run(
                host,
                repo.as_deref(),
                secret,
                *forward_agent,
                &account_selection,
                cmd,
            ),
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
            None if raw_args.len() == 1 => loopflow::lf::commands::desktop::run(),
            None => anyhow::bail!(
                "no command specified; bare `lf` opens Loopflow.app; run a named skill, flow, or `lf : <prompt>` to start an agent"
            ),
        }
    };

    result
}

#[cfg(test)]
mod tests {
    use super::{arg_tables, format_task_pr_line, reorder_args};

    use clap::Parser;
    use loopflow::lf::{Cli, Commands, PmCommand, PmTaskCommand, PrCommand};
    use loopflow::task::{AfterMerge, GithubPr, PrPublication, TaskPr, TaskPrId, TaskSessionId};

    fn published_pr() -> TaskPr {
        let now = time::OffsetDateTime::now_utc();
        TaskPr {
            id: TaskPrId::new(),
            task_session_id: TaskSessionId::new(),
            sequence: 1,
            slug: "linear-pr-linkage".to_string(),
            branch: "jack/linear-pr-linkage".to_string(),
            base_commit: "abc".to_string(),
            parent_pr_id: None,
            publication: Some(PrPublication {
                requested_at: now,
                after_merge: AfterMerge::Review,
                next_slug: None,
                github: Some(GithubPr {
                    number: 931,
                    url: "https://github.com/loopflowstudio/loopflow/pull/931".to_string(),
                    head_sha: None,
                }),
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

    /// The derived tables cover everything the old hand lists carried, plus
    /// the uppercase short aliases those lists had drifted away from.
    #[test]
    fn derived_tables_cover_commands_flags_and_aliases() {
        let tables = arg_tables();
        for command in [
            ":", "desktop", "pr", "wt", "rebase", "commit", "auth", "release", "pm", "task",
            "project", "flow", "skill", "chat", "memory", "usage", "top", "ls", "status", "runs",
            "trace", "help",
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
    fn desktop_is_an_explicit_alias_for_the_bare_app_launch() {
        let cli = Cli::try_parse_from(["lf", "desktop"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Desktop)));
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

    #[test]
    fn format_child_body_shows_binary_provenance() {
        use loopflow::child_session::{BinaryProvenance, ChildLeaseState, ChildProcessGeneration};
        use time::OffsetDateTime;

        let process = ChildProcessGeneration {
            generation: 3,
            pid: None,
            process_group_id: None,
            tmux_name: "lf-task-x".to_string(),
            agent: "claude".to_string(),
            provider: "claude".to_string(),
            provider_session_id: None,
            started_at: OffsetDateTime::UNIX_EPOCH,
            state: ChildLeaseState::Active,
            outcome: None,
            provenance: Some(BinaryProvenance {
                version: "0.12.0".to_string(),
                provenance: "release".to_string(),
                source_identity: "release".to_string(),
            }),
        };
        let body = super::format_child_body("claude", "claude", Some(&process));
        assert!(
            body.contains("generation 3"),
            "body shows generation: {body}"
        );
        assert!(
            body.contains("binary 0.12.0 (release)"),
            "body shows binary provenance: {body}"
        );
    }

    #[test]
    fn format_child_body_falls_back_when_provenance_absent() {
        use loopflow::child_session::{ChildLeaseState, ChildProcessGeneration};
        use time::OffsetDateTime;

        let process = ChildProcessGeneration {
            generation: 1,
            pid: None,
            process_group_id: None,
            tmux_name: "lf-task-legacy".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            started_at: OffsetDateTime::UNIX_EPOCH,
            state: ChildLeaseState::Active,
            outcome: None,
            provenance: None,
        };
        let body = super::format_child_body("codex", "codex", Some(&process));
        assert!(body.contains("binary unknown"), "legacy body: {body}");
    }
}
