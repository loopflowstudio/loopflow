use std::collections::{HashMap, HashSet};
use std::io::{IsTerminal, Read};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use clap::Parser;
use tracing::debug;
use tracing_subscriber::EnvFilter;

use loopflow::journal::{self, with_runtime, LfEventFields, LfEventType, LfNode};
use loopflow::lf::discovery::DefinitionKind;
use loopflow::lf::{
    Cli, Commands, FlowCommand, InstallCommand, SkillCommand, TaskCommand, WaveCommand,
};

use loopflow::ops::chapter::update_plan;
use loopflow::ops::task_execution::TaskExecutionState;

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

/// Insert clap's internal `--` at the public `lf ssh` target boundary.
///
/// The public syntax omits it, but making the boundary explicit before parsing
/// prevents a remote `--account` from being consumed by the origin command.
fn normalize_ssh_args(mut args: Vec<String>) -> Vec<String> {
    if args.len() <= 1 {
        return args;
    }
    let rest = &args[1..];
    let Some(command_index) = first_target_index(rest) else {
        return args;
    };
    if rest[command_index] != "ssh" {
        return args;
    }

    let ssh_args = &arg_tables()
        .subcommands
        .get("ssh")
        .expect("ssh command has derived argument metadata")
        .direct;
    let mut index = command_index + 2;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" {
            return args;
        }
        if arg.starts_with('-') {
            let takes_value = ssh_args.takes_value(arg) || is_value_flag(arg);
            if takes_value && !has_inline_value(arg) {
                index += 1;
            }
            index += 1;
            continue;
        }

        args.insert(index + 1, "--".to_string());
        return args;
    }
    args
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
        // `lf ssh` has a deliberate positional boundary: origin options come
        // before the target and every later token belongs to the remote lf.
        // Moving global flags across that boundary changes which machine owns
        // an account selection.
        if rest[target_index] == "ssh" {
            return args;
        }
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
                loopflow::lf::commands::run::run(Some("loopflow"), None, cli)
            })
        }
        None => with_runtime(&repo_root, command, || {
            loopflow::lf::commands::run::run(Some("loopflow"), None, cli)
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
    cli: &Cli,
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
    if kind == Some(DefinitionKind::Skill) {
        if let Some(flow) = loopflow::lf::commands::run::saved_flow(cli)? {
            let skill = loopflow::engine::current_skill(&flow.invocation.steps, &flow.cursor)
                .ok_or_else(|| anyhow::anyhow!("saved FlowSession has no current skill"))?;
            anyhow::ensure!(
                skill.skill.name == name,
                "skill does not match saved FlowSession"
            );
            return Ok(Some((Target::Skill(skill.skill), message)));
        }
    }
    let target = loopflow::lf::discovery::resolve_definition(&repo, &name, kind)?;
    Ok(Some((target, message)))
}

fn execute_target(
    target: loopflow::engine::target::Target,
    message: Option<&str>,
    cli: &Cli,
    args: &[String],
    binding: Option<&loopflow::ops::WorkBinding>,
    account_selection: &loopflow::provider_account::lease::AccountSelection,
) -> anyhow::Result<()> {
    use loopflow::engine::target::Target;

    match target {
        Target::Command(command) => {
            execute_command(&command, cli, args, binding, account_selection)
        }
        Target::Skill(skill) => {
            let repo_root = loopflow::repo::working_directory()?;
            let name = skill.name.as_str();
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
                        None => loopflow::lf::commands::run::run(Some(name), message, cli)?,
                    }
                    // Shared contributions leave checkpoint composition to the caller.
                    if !shared && std::env::var_os(loopflow::durable::RUN_ID_ENV).is_none() {
                        let options = loopflow::ops::CommitOptions {
                            add: true,
                            message: Some(format!("lf task commit: {name}")),
                            ..loopflow::ops::CommitOptions::for_task(name)
                        };
                        loopflow::ops::commit_workflow(
                            &repo_root,
                            &options,
                            &loopflow::ops::NullProgress,
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
        .map_err(|error| anyhow::anyhow!("cannot resolve --as {selector}: {error}"))?;
    runtime.block_on(async {
        let store = loopflow::store::open_existing_store()
            .await
            .ok_or_else(|| {
                anyhow::anyhow!("cannot resolve --as {selector}: planning registry unavailable")
            })?;
        let store = Arc::new(store);
        loopflow::ops::resolve_work_binding(&store, repo, selector)
            .await
            .map_err(anyhow::Error::from)
    })
}

fn prepare_hierarchical_work_binding(
    task: Option<&str>,
    wave: Option<&str>,
    repo: &Path,
) -> anyhow::Result<loopflow::ops::WorkBinding> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|error| anyhow::anyhow!("cannot resolve Work selection: {error}"))?;
    runtime.block_on(async {
        let store = loopflow::store::open_existing_store()
            .await
            .ok_or_else(|| {
                anyhow::anyhow!("cannot resolve Work selection: planning registry unavailable")
            })?;
        let store = Arc::new(store);
        loopflow::ops::resolve_work_selection(
            &store,
            repo,
            loopflow::ops::WorkSelection { task, wave },
        )
        .await
        .map_err(anyhow::Error::from)
    })
}

fn validate_work_selector(selector: &str) -> anyhow::Result<()> {
    let (kind, value) = selector.split_once(':').ok_or_else(|| {
        anyhow::anyhow!("invalid --as {selector:?}; use task:<selector> or wave:<selector>")
    })?;
    if !matches!(kind, "task" | "wave") {
        anyhow::bail!("invalid --as kind {kind:?}; expected task or wave");
    }
    if value.trim().is_empty() {
        anyhow::bail!("{kind} selector cannot be empty");
    }
    Ok(())
}

fn require_bound_invocation(command: &Option<Commands>) -> anyhow::Result<()> {
    match command {
        Some(Commands::Skill { .. } | Commands::Flow { .. } | Commands::Run { .. } | Commands::External(_) | Commands::Inline { .. }) => Ok(()),
        _ => anyhow::bail!("Work selectors run a skill, flow or inline prompt; use `lf --task LOO-123 design`, `lf --task LOO-123 code` or `lf --wave product : \"question\"`"),
    }
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
        let status = match snapshot.execution.state {
            TaskExecutionState::Blocked => "blocked".to_string(),
            TaskExecutionState::Stalled => "stalled".to_string(),
            _ => snapshot.status.to_string(),
        };
        println!(
            "{}  {}\n  task: {}\n  body: {}\n  worktree: {}\n  branch: {}\n  PM writeback: {}",
            snapshot.issue_identifier,
            status,
            snapshot.task_id,
            body,
            snapshot.worktree,
            branch,
            pm_writeback,
        );
        println!("  execution: {}", snapshot.execution.reason);
        if let Some(run) = &snapshot.execution.captured {
            println!("  Session event: {run}");
        }
        for run in &snapshot.runs {
            println!(
                "  Run: {}  {}  {}  {}",
                run.selector(),
                run.label(),
                run.surface,
                run.status()
            );
        }
        if snapshot.runs_truncated {
            println!("  Run history truncated; inspect exact Run IDs for older evidence");
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
        WaveCommand::List { .. } | WaveCommand::Status { .. } => {
            unreachable!("read commands dispatch separately")
        }
        WaveCommand::Connect {
            wave,
            wave_flag,
            all,
            team_key,
            team_name,
        } => loopflow::lf::commands::ops::connect_wave(
            repo,
            wave.as_deref().or(wave_flag.as_deref()),
            *all,
            team_key.as_deref(),
            team_name.as_deref(),
        ),
        WaveCommand::Sync {
            wave,
            wave_flag,
            all,
        } => loopflow::lf::commands::ops::sync_planning(
            repo,
            wave.as_deref().or(wave_flag.as_deref()),
            *all,
            false,
            false,
        ),
        WaveCommand::Rename { wave, title } => {
            loopflow::lf::commands::ops::rename_wave(repo, wave, title)
        }
        WaveCommand::Forget { .. }
        | WaveCommand::Place { .. }
        | WaveCommand::Relocate { .. }
        | WaveCommand::Retire { .. } => loopflow::lf::commands::placement::wave(repo, command),
        WaveCommand::UpdatePlan { wave, plan } => {
            let content = serde_json::from_slice(&std::fs::read(plan)?)?;
            update_plan(repo, wave.as_deref(), &content)?;
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

fn run_task_command(repo: &Path, command: &TaskCommand, cli: &Cli) -> anyhow::Result<()> {
    let agent = cli.model.as_deref();
    let _skill_options = EnvGuard::set(
        loopflow::lf::TASK_SKILL_OPTIONS_ENV,
        serde_json::to_string(&cli.step_args())?,
    );
    match command {
        TaskCommand::Pr { cmd } => loopflow::lf::commands::ops::run_pr(cmd.as_ref(), agent),
        TaskCommand::Wt { cmd } => loopflow::lf::commands::ops::run_wt(cmd),
        TaskCommand::Rebase(args) => loopflow::lf::commands::ops::run_rebase(args),
        TaskCommand::Commit { message, no_add } => {
            loopflow::lf::commands::ops::run_commit(message.as_deref(), *no_add, agent)
        }
        TaskCommand::Worker { .. } => unreachable!("Task worker dispatches at process entry"),
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
        TaskCommand::Run {
            issue,
            name,
            flow,
            stack_on,
            directive,
            reason,
            retry,
            json,
        } => {
            let task = loopflow::ops::task::task_run(
                repo,
                issue,
                loopflow::ops::task::TaskExecOptions {
                    retry: *retry,
                    reason: reason.clone(),
                    agent: agent.map(str::to_string),
                    name: name.clone(),
                    flow: flow.clone(),
                    stack_on: stack_on.clone(),
                    directive: directive.clone(),
                },
            )?;
            print_task_snapshot(&task, *json)
        }
        TaskCommand::Create {
            wave,
            title,
            notes,
            run,
            name,
            flow,
            stack_on,
            json,
        } => {
            let report = match notes {
                Some(notes) => Some(notes.clone()),
                None => piped_task_report()?,
            };
            let result = loopflow::ops::task::task_create(
                repo,
                wave.as_deref(),
                title.clone(),
                report,
                run.then(|| loopflow::ops::task::TaskExecOptions {
                    retry: false,
                    reason: None,
                    agent: agent.map(str::to_string),
                    name: name.clone(),
                    flow: flow.clone(),
                    stack_on: stack_on.clone(),
                    directive: None,
                }),
            )?;
            match result {
                loopflow::ops::task::TaskCreateResult::Started(task) => {
                    print_task_snapshot(&task, *json)
                }
                loopflow::ops::task::TaskCreateResult::Created(issue) => {
                    if *json {
                        println!("{}", serde_json::to_string_pretty(&issue)?);
                    } else {
                        println!("{} · {}", issue.identifier, issue.name);
                    }
                    Ok(())
                }
            }
        }
        TaskCommand::Status { issue, json } => {
            let task = loopflow::ops::task::task_status(issue.as_deref())?;
            print_task(&task, *json)
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
        TaskCommand::Complete {
            issue,
            summary,
            json,
        } => match loopflow::ops::task::task_complete(repo, issue, summary.clone())? {
            Some(task) => print_task(&task, *json),
            None => {
                let resolved = loopflow::ops::pm::pm_resolve_task(repo, issue)?;
                if *json {
                    println!("{}", serde_json::to_string_pretty(&resolved.item)?);
                } else {
                    println!(
                        "{}: completed {}",
                        resolved.item.identifier, resolved.item.name
                    );
                }
                Ok(())
            }
        },
        TaskCommand::Delete { issue } => {
            let identifier = loopflow::ops::task::task_delete(repo, issue)?;
            println!("{identifier}: deleted");
            Ok(())
        }
        TaskCommand::Edit {
            issue,
            title,
            notes,
            wave,
        } => {
            let result = loopflow::ops::task::task_edit(
                repo,
                issue,
                wave.as_deref(),
                title.clone(),
                notes.clone(),
            )?;
            println!("{}: updated task {}", result.wave, result.id);
            Ok(())
        }
        TaskCommand::Comment {
            issue,
            message,
            wave,
            json,
        } => {
            let result = loopflow::ops::task::task_comment(
                repo,
                issue,
                wave.as_deref(),
                message.as_deref(),
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
                    println!("── {author} · {date}\n{}\n", comment.body.trim_end());
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
        TaskCommand::Restart {
            issue,
            advice,
            flow,
            json,
        } => {
            let task = loopflow::ops::task::task_restart(
                issue,
                advice.clone(),
                flow.clone(),
                agent.map(str::to_string),
            )?;
            print_task_snapshot(&task, *json)
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
    let result = journal::with_process(run);
    let code = journal::command_exit_code(&result);
    if let Err(error) = result {
        if error
            .downcast_ref::<loopflow::exec::CommandExit>()
            .is_none()
        {
            eprintln!("Error: {error:?}");
        }
    }
    std::process::ExitCode::from(code)
}

fn run() -> anyhow::Result<()> {
    loopflow::machine_install::dispatch_entry_gate(&loopflow::machine_install::ArtifactRole::Cli)?;
    // Ensure Ctrl+C terminates lf and the child agent. Without this,
    // child.wait() retries on EINTR and hangs while the agent catches
    // SIGINT and keeps running. SIGTERM the agent first so it doesn't
    // survive as an orphan. The `termination` feature extends the handler
    // to SIGTERM and SIGHUP: `tmux kill-session` delivers SIGHUP, which
    // otherwise bypasses every cleanup (observed live: it orphaned the wave
    // loop's codex app-server pair and left a stale .wave-endpoint).
    // Initialize tracing with RUST_LOG env filter
    // Usage: RUST_LOG=lf=debug lf unbreak
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lf=info,loopflow=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .init();

    // Reorder args so flags can appear after the skill name
    let normalized =
        loopflow::lf::navigation::normalize_args(std::env::args().collect()).map_err(|error| {
            let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
            let _ = error.print();
            loopflow::exec::CommandExit(code)
        })?;
    let args = reorder_args(normalize_ssh_args(normalized));

    let mut cli = match Cli::try_parse_from(args.clone()) {
        Ok(cli) => cli,
        Err(error) => {
            let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
            let _ = error.print();
            return Err(loopflow::exec::CommandExit(code).into());
        }
    };
    if let Some(result) = loopflow::lf::navigation::inspect(&cli) {
        return finish_command(result);
    }
    // Installation owns its promotion/recovery authority. In particular,
    // read-only candidate preflight must work before a first install settles.
    let bypasses_machine_startup_gate = matches!(&cli.command, Some(Commands::Install { .. }));
    if !bypasses_machine_startup_gate {
        let switch_id = std::env::var(loopflow::machine_install::INSTALL_SWITCH_ENV)
            .ok()
            .filter(|value| !value.is_empty());
        loopflow::machine_install::authorize_current_for_switch(
            &loopflow::machine_install::ArtifactRole::Cli,
            switch_id.as_deref(),
        )?;
        if !matches!(
            &cli.command,
            Some(Commands::Screenshot { .. } | Commands::ScreenshotSupervisor { .. })
        ) {
            loopflow::store::isolate_branch_data()?;
        }
    }
    ctrlc::set_handler(|| loopflow::engine::agent::exit_on_interrupt())
        .expect("failed to set Ctrl+C handler");

    if matches!(
        &cli.command,
        Some(
            Commands::Install { .. }
                | Commands::Screenshot { .. }
                | Commands::ScreenshotSupervisor { .. }
        )
    ) {
        journal::observe_process(&args);
    }

    // Screenshot capture owns no Home, repository, account, or Run state. Its
    // hidden supervisor must also be able to clean up after its public parent
    // dies, so both forms dispatch before those unrelated boundaries.
    match &cli.command {
        Some(Commands::Screenshot { screenshot }) => {
            return loopflow::lf::commands::screenshot::run(screenshot);
        }
        Some(Commands::ScreenshotSupervisor { screenshot }) => {
            return loopflow::lf::commands::screenshot::run_supervisor(screenshot);
        }
        _ => {}
    }

    // Global-promotion commands dispatch before home routing, journal emission,
    // and any ordinary store open: a candidate that does not know the live
    // migration frontier must reach the preflight refusal, not fail in
    // trace/store capture. `lf install` opens the store only read-only, inside
    // its own preflight.
    if let Some(Commands::Install { cmd }) = &cli.command {
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
            Some(InstallCommand::LocalPreflight { store, json }) => {
                loopflow::lf::commands::install::local_preflight(store, *json)
            }
            Some(InstallCommand::AdvanceSwitch { switch }) => {
                loopflow::lf::commands::install::advance_switch(switch)
            }
            Some(InstallCommand::Promote {
                from_build,
                coordinated_build,
                fresh,
                reuse_home,
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
                from_build.as_deref(),
                coordinated_build.as_deref(),
                *fresh,
                reuse_home.as_deref(),
            ),
            Some(InstallCommand::Rollback {
                cli_target,
                candidate,
            }) => loopflow::lf::commands::install::rollback(cli_target, candidate),
        };
    }

    // Exec admission records this process's cwd. Each operation resolves the
    // repository it needs after dispatch; machine inspection needs no Git.
    let directory = std::env::current_dir()?;
    journal::admit_process(&directory, &args);
    {
        let explicit_wave = cli
            .wave
            .as_deref()
            .map(loopflow::work::wave::context::resolve_explicit_wave)
            .transpose()?;
        if let Some(wave) = &explicit_wave {
            cli.wave = Some(wave.name().to_string());
        }
        // One resolved wave identity drives prompt context, registry attribution,
        // journaling, and every child process.
        let _explicit_wave_env = explicit_wave.as_ref().map(|wave| {
            EnvGuard::set(
                loopflow::work::wave::context::WAVE_ID_ENV,
                wave.id().to_string(),
            )
        });
        // Account flags before an SSH target shape the origin grant. Flags in the
        // remote lf arguments become preferences over its merged local/forwarded
        // catalog through LF_ACCOUNT_SELECTION.
        let mut preferred_accounts = cli.account.clone();
        let mut restricted_accounts = cli.only_account.clone();
        if let Some(Commands::Ssh {
            origin_account,
            origin_only_account,
            ..
        }) = &cli.command
        {
            preferred_accounts.extend(origin_account.iter().cloned());
            restricted_accounts.extend(origin_only_account.iter().cloned());
        }
        let account_selection = loopflow::provider_account::lease::AccountSelection::from_flags(
            &preferred_accounts,
            &restricted_accounts,
        )?;
        let inherited_account_lease = loopflow::provider_account::lease::account_lease_active();
        let _forwarded_account_selection =
            if inherited_account_lease && !account_selection.is_default() {
                Some(EnvGuard::set(
                    loopflow::provider_account::lease::ACCOUNT_SELECTION_ENV,
                    account_selection.env_value()?,
                ))
            } else {
                None
            };
        if cli.account_lease_probe {
            return loopflow::provider_account::lease::probe_forwarded_authority()
                .map_err(anyhow::Error::from);
        }
        debug!(?cli, "parsed CLI arguments");

        dispatch(
            cli,
            &args,
            explicit_wave,
            account_selection,
            inherited_account_lease,
        )
    }
}

fn dispatch(
    mut cli: Cli,
    args: &[String],
    explicit_wave: Option<loopflow::work::wave::Wave>,
    account_selection: loopflow::provider_account::lease::AccountSelection,
    inherited_account_lease: bool,
) -> anyhow::Result<()> {
    // Every HomeId-addressed SSH hop proves it reached the intended authority
    // before reads or mutations dispatch. Raw-host bootstrap carries no
    // expectation and falls through.
    loopflow::lf::commands::home::validate_expected_home_process()?;

    // SSH commands build and forward their broker in the transport path. Every
    // local command with a selection gets an in-process broker here.
    let is_ssh = matches!(cli.command, Some(loopflow::lf::Commands::Ssh { .. }));
    let _local_account_lease =
        if is_ssh || inherited_account_lease || account_selection.is_default() {
            None
        } else {
            build_local_account_lease(&account_selection)?
        };

    let mut direct_binding = None;
    let mut _work_declaration = None;
    let mut _bound_cwd = None;
    let selects_direct_work = cli.as_work.is_some()
        || cli.task.is_some()
        || (cli.wave.is_some()
            && matches!(
                &cli.command,
                Some(Commands::Skill { .. })
                    | Some(Commands::Flow { .. })
                    | Some(Commands::Run { .. })
                    | Some(Commands::External(_))
            ));
    if selects_direct_work {
        let repo = loopflow::lf::commands::util::find_repo_root()?;
        let mut binding = if let Some(selector) = cli.as_work.as_deref() {
            validate_work_selector(selector)?;
            prepare_work_binding(selector, &repo)?
        } else {
            prepare_hierarchical_work_binding(cli.task.as_deref(), cli.wave.as_deref(), &repo)?
        };
        if let Some(cwd) = cli.bound_cwd.clone() {
            binding.cwd = cwd;
        }
        if let Some(wave) = &explicit_wave {
            if wave.id() != &binding.wave_id {
                anyhow::bail!(
                    "--wave {} does not own --as {}:{}",
                    wave.name(),
                    binding.work.kind(),
                    binding.work.id(),
                );
            }
        }
        cli.wave = Some(binding.wave_name.clone());
        if cli.model.is_none() {
            cli.model = binding.agent.clone();
        }
        let cwd = CwdGuard::enter(&binding.cwd)?;
        require_bound_invocation(&cli.command)?;
        _work_declaration = Some(EnvGuard::set(
            loopflow::lf::WORK_DECLARATION_ENV,
            format!("{}:{}", binding.work.kind(), binding.work.id()),
        ));
        direct_binding = Some(binding);
        _bound_cwd = Some(cwd);
    }

    let result = resolve_cli_target(&cli, args).and_then(|selected| match selected {
        Some((target, message)) => execute_target(
            target,
            message.as_deref(),
            &cli,
            args,
            direct_binding.as_ref(),
            &account_selection,
        ),
        None => run_default_agent(&cli, args),
    });

    finish_command(result)
}

fn execute_command(
    command: &loopflow::engine::Command,
    cli: &Cli,
    args: &[String],
    binding: Option<&loopflow::ops::WorkBinding>,
    account_selection: &loopflow::provider_account::lease::AccountSelection,
) -> anyhow::Result<()> {
    let parsed = Cli::try_parse_from(command.argv())?;
    match &parsed.command {
        Some(Commands::Inline { prompt }) => {
            let text = prompt.join(" ");
            in_directory_runtime(args, |_| match binding {
                Some(binding) => {
                    loopflow::lf::commands::run::run_bound(None, Some(&text), cli, binding)
                }
                None => loopflow::lf::commands::run::run(None, Some(&text), cli),
            })
        }
        Some(Commands::Desktop) => loopflow::lf::commands::desktop::run(),
        Some(Commands::ProviderSession) => loopflow::lf::commands::runs::observe_provider_session(),
        Some(Commands::Ask { ask }) => loopflow::lf::commands::ask::run(ask),
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
            loopflow::lf::RepoCommand::Release { .. } => {
                in_repo_runtime(args, |_| loopflow::lf::commands::ops::run_repo(cmd))
            }
            loopflow::lf::RepoCommand::Tokens { .. } | loopflow::lf::RepoCommand::Ci { .. } => {
                loopflow::lf::commands::ops::run_repo(cmd)
            }
            _ => in_directory_runtime(args, |_| loopflow::lf::commands::ops::run_repo(cmd)),
        },
        Some(Commands::Home { cmd }) => loopflow::lf::commands::home::run(cmd),
        Some(Commands::SyncSkills { yes, no_prune }) => {
            loopflow::lf::commands::ops::run_sync_skills(*yes, *no_prune)
        }
        Some(Commands::Cron {
            cmd:
                cmd
                @ (loopflow::lf::CronCommand::List { .. } | loopflow::lf::CronCommand::Remove { .. }),
        }) => loopflow::lf::commands::ops::cron_cmd(cmd),
        Some(Commands::Cron { cmd }) => {
            in_repo_runtime(args, |_| loopflow::lf::commands::ops::cron_cmd(cmd))
        }
        Some(Commands::Wave {
            cmd: WaveCommand::List { json, all, current },
        }) => loopflow::lf::commands::waves::ls(*json, *all, *current),
        Some(Commands::Wave {
            cmd:
                WaveCommand::Status {
                    wave,
                    json,
                    sync,
                },
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
            cmd:
                cmd @ (WaveCommand::Connect { .. }
                | WaveCommand::Sync { .. }
                | WaveCommand::Rename { .. }
                | WaveCommand::Forget { .. }
                | WaveCommand::Place { .. }
                | WaveCommand::Relocate { .. }
                | WaveCommand::Retire { .. }),
        }) => in_directory_runtime(args, |repo| run_wave_command(repo, cmd)),
        Some(Commands::Wave { cmd }) => in_repo_runtime(args, |repo| run_wave_command(repo, cmd)),
        Some(Commands::Task {
            cmd: cmd @ TaskCommand::Rebase(_),
        }) => {
            let repo =
                loopflow::repo::require_repo_root(&std::env::current_dir()?, "lf task rebase")?;
            with_runtime(&repo, args, || {
                run_task_command(&repo, cmd, cli.model.as_deref())
            })
        }
        Some(Commands::Task {
            cmd: TaskCommand::Worker { task_id },
        }) => in_repo_runtime(args, |_| {
            tokio::runtime::Runtime::new()?
                .block_on(loopflow::controller::task::run_worker(task_id.clone()))
        }),
        // Local document access owns no Run lifecycle. Placement and the recorded
        // Git base come from the Task registry inside these operations.
        Some(Commands::Task {
            cmd:
                cmd @ (TaskCommand::Diff { .. } | TaskCommand::File { .. } | TaskCommand::Save { .. }),
        }) => run_task_command(&std::env::current_dir()?, cmd, cli),
        Some(Commands::Task { cmd }) => in_repo_runtime(args, |repo| {
            run_task_command(repo, cmd, cli)
        }),
        Some(Commands::Usage {
            json,
            days,
            wave,
            project,
            task,
        }) => loopflow::lf::commands::usage::run(
            *json,
            *days,
            wave.as_deref(),
            project.as_deref(),
            task.as_deref(),
        ),
        Some(Commands::TelemetryScorecard { json }) => in_repo_runtime(args, |repo| {
            let item = loopflow::engine::flow::Command {
                command: "__telemetry-scorecard".to_string(),
                args: if *json {
                    vec!["--json".to_string()]
                } else {
                    Vec::new()
                },
            };
            loopflow::ops::execute_flow_command(repo, &item, &loopflow::ops::NullProgress)
                .map_err(Into::into)
        }),
        Some(Commands::Ps { json }) => loopflow::lf::commands::top::run_ps(*json),
        Some(Commands::Top { json }) => loopflow::lf::commands::top::run_top(*json),
        Some(Commands::Prune { dry_run, json }) => {
            loopflow::lf::commands::top::run_prune(*json, *dry_run)
        }
        Some(Commands::Doctor { json, planning }) => {
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
        Some(Commands::Roadmap { wave, json, all }) => {
            loopflow::lf::commands::waves::roadmap(wave.as_deref(), *json, *all)
        }
        Some(Commands::Activity {
            since,
            limit,
            wave,
            project,
            task,
            json,
        }) => loopflow::lf::commands::activity::run(
            since,
            *limit,
            wave.as_deref(),
            project.as_deref(),
            task.as_deref(),
            *json,
        ),
        Some(Commands::FlowStep { id, version }) => in_directory_runtime(args, |_| {
            loopflow::lf::commands::flow::execute_step(id, *version)
        }),
        Some(Commands::Exec { cmd }) => loopflow::lf::commands::exec::run(cmd),
        Some(Commands::Runs {
            active,
            watch,
            run,
            parent,
            events,
            final_answer,
            task,
            wave,
            project,
            json,
        }) => match run {
            None if *active => {
                loopflow::lf::commands::runs::list_active(*json, *watch, task.as_deref())
            }
            Some(run) => loopflow::lf::commands::runs::inspect(run, *events, *final_answer, *json),
            None => loopflow::lf::commands::runs::list(
                *json,
                wave.as_deref(),
                project.as_deref(),
                task.as_deref(),
                parent.as_deref(),
            ),
        },
        Some(Commands::Replay { run }) => loopflow::lf::commands::replay::run(run),
        Some(Commands::Discord {
            cmd: loopflow::lf::DiscordCommand::Serve { wave },
        }) => in_repo_runtime(args, |repo| {
            loopflow::lf::commands::discord::serve(repo, wave)
        }),
        Some(Commands::Install { .. }) => {
            unreachable!("install dispatches before home routing")
        }
        Some(Commands::Screenshot { .. } | Commands::ScreenshotSupervisor { .. }) => {
            unreachable!("screenshot dispatches before home routing")
        }
        Some(Commands::Ssh {
            target,
            repo,
            secret,
            forward_agent,
            origin_account: _,
            origin_only_account: _,
            lf_args,
        }) => loopflow::lf::commands::ssh::run(
            target,
            repo.as_deref(),
            secret,
            *forward_agent,
            account_selection,
            lf_args,
        ),
        Some(Commands::Flow { cmd }) => match cmd {
            FlowCommand::List { json, inventory } if inventory.sessions => {
                loopflow::lf::commands::flow_inventory::list(inventory, *json)
            }
            FlowCommand::Show {
                name,
                json,
                sessions: true,
            } => loopflow::lf::commands::flow_inventory::inspect(name, *json),
            _ => loopflow::lf::commands::flow::control(cmd, cli),
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
            return Err(loopflow::exec::CommandExit(code).into());
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
            return Err(loopflow::exec::CommandExit(code).into());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        format_task_pr_line, normalize_ssh_args, reorder_args, validate_work_selector, CwdGuard,
        EnvGuard,
    };

    use clap::Parser;
    use loopflow::lf::{Cli, Commands, PrCommand, TaskCommand, WaveCommand};
    use loopflow::work::task::{GithubPr, PrPublication, TaskId, TaskPr, TaskPrId};

    #[test]
    fn bound_work_selector_requires_a_readable_registry() {
        let _lock = PROCESS_STATE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = tempfile::tempdir().unwrap();
        let registry_blocker = directory.path().join("unreadable-registry");
        std::fs::create_dir(&registry_blocker).unwrap();
        let _db = EnvGuard::set("LF_DB_PATH", registry_blocker.display().to_string());

        validate_work_selector("task:LOO-265").unwrap();
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
        std::fs::write(
            repo.path().join(".lf/flows/land.yaml"),
            "- cmd: task pr land\n",
        )
        .unwrap();
        let _cwd = CwdGuard::enter(repo.path()).unwrap();
        let resolve = |args: &[&str]| {
            let args = loopflow::lf::navigation::normalize_args(
                args.iter().map(|arg| arg.to_string()).collect(),
            )
            .unwrap();
            let args = reorder_args(args);
            let cli = Cli::try_parse_from(&args).unwrap();
            super::resolve_cli_target(&cli, &args).unwrap().unwrap()
        };

        let (target, message) = resolve(&[
            "lf",
            "-m",
            "codex",
            "land",
            "--message",
            "Keep this together",
        ]);
        let Target::Command(command) = target else {
            panic!("expected command")
        };
        assert_eq!(
            command.argv(),
            [
                "lf",
                "task",
                "pr",
                "land",
                "--message",
                "Keep this together"
            ]
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
        let args = vec![
            "lf".to_string(),
            "implement".to_string(),
            "--task".to_string(),
            "LOO-123".to_string(),
            "--wave".to_string(),
            "context".to_string(),
        ];
        assert_eq!(
            reorder_args(args),
            vec!["lf", "--task", "LOO-123", "--wave", "context", "implement"]
        );
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
    fn desktop_remains_an_explicit_app_command() {
        let cli = Cli::try_parse_from(["lf", "desktop"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Desktop)));
    }

    #[test]
    fn bare_lf_has_a_terminal_control_skill() {
        let cli = Cli::try_parse_from(["lf"]).unwrap();
        assert!(cli.command.is_none());
        let skill = loopflow::engine::builtins::get_builtin_skill("loopflow")
            .expect("builtin terminal control skill");
        assert!(skill.contains("lf session list --json"));
        assert!(skill.contains("lf session open <session-id> --json"));
        assert!(skill.contains("Keep this conversation open"));
    }

    #[test]
    fn reorder_args_flag_after_skill() {
        let args = vec!["lf".to_string(), "debug".to_string(), "-c".to_string()];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-c", "debug"]);
    }

    #[test]
    fn ssh_help_prefers_home_identity() {
        let help = Cli::try_parse_from(["lf", "ssh", "--help"])
            .expect_err("help exits through clap")
            .to_string();

        assert!(help.contains("<TARGET>"));
        assert!(help.contains("HomeId (preferred), SSH alias, or user@host"));
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
            "task".to_string(),
            "commit".to_string(),
            "-m".to_string(),
            "msg".to_string(),
        ];
        let result = reorder_args(args);
        // `-m` is local to commit, so the local meaning wins.
        assert_eq!(result, vec!["lf", "task", "commit", "-m", "msg"]);
    }

    #[test]
    fn reorder_args_preserves_the_ssh_target_boundary() {
        let args = vec![
            "lf".to_string(),
            "ssh".to_string(),
            "build-vm".to_string(),
            "--account".to_string(),
            "remote@company".to_string(),
            "task".to_string(),
            "pursue".to_string(),
        ];

        assert_eq!(reorder_args(args.clone()), args);
    }

    #[test]
    fn normalize_ssh_args_makes_the_target_a_hard_boundary() {
        let args = [
            "lf",
            "ssh",
            "--account",
            "origin@example.com",
            "build-vm",
            "--account",
            "remote@example.com",
            "task",
            "pursue",
        ]
        .map(str::to_string)
        .to_vec();

        let normalized = normalize_ssh_args(args);
        assert_eq!(
            normalized,
            [
                "lf",
                "ssh",
                "--account",
                "origin@example.com",
                "build-vm",
                "--",
                "--account",
                "remote@example.com",
                "task",
                "pursue",
            ]
        );
        let cli = Cli::try_parse_from(normalized).expect("parse normalized SSH command");
        assert!(matches!(
            cli.command,
            Some(Commands::Ssh {
                origin_account,
                lf_args,
                ..
            }) if origin_account == ["origin@example.com"]
                && lf_args == ["--account", "remote@example.com", "task", "pursue"]
        ));
    }

    #[test]
    fn reorder_args_preserves_local_collision_after_leading_global() {
        let args: Vec<String> = ["lf", "--wave", "goals", "commit", "-m", "ship it"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            reorder_args(loopflow::lf::navigation::normalize_args(args).unwrap()),
            vec!["lf", "--wave", "goals", "task", "commit", "-m", "ship it"]
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
        let args: Vec<String> = ["lf", "wave", "--wave", "systems", "sync"]
            .map(String::from)
            .to_vec();
        let reordered = reorder_args(args);
        assert_eq!(reordered, vec!["lf", "wave", "sync", "--wave", "systems"]);
        assert!(matches!(
            Cli::try_parse_from(reordered).unwrap().command,
            Some(Commands::Wave {
                cmd: WaveCommand::Sync { .. }
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

        let args: Vec<String> = ["lf", "task", "pr", "-m", "codex", "open"]
            .map(String::from)
            .to_vec();
        let reordered = reorder_args(args);
        assert_eq!(reordered, vec!["lf", "task", "pr", "open", "-m", "codex"]);
        assert!(matches!(
            Cli::try_parse_from(reordered).unwrap().command,
            Some(Commands::Task {
                cmd: TaskCommand::Pr {
                    cmd: Some(PrCommand::Open { .. })
                }
            })
        ));

        let args: Vec<String> = ["lf", "task", "pr", "--strict", "submit"]
            .map(String::from)
            .to_vec();
        let reordered = reorder_args(args);
        assert_eq!(reordered, vec!["lf", "task", "pr", "submit", "--strict"]);
        assert!(matches!(
            Cli::try_parse_from(reordered).unwrap().command,
            Some(Commands::Task {
                cmd: TaskCommand::Pr {
                    cmd: Some(PrCommand::Submit { strict: true, .. })
                }
            })
        ));

        let args: Vec<String> = ["lf", "task", "wt", "--force", "remove", "old-tree"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            reorder_args(args),
            vec!["lf", "task", "wt", "remove", "--force", "old-tree"]
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
