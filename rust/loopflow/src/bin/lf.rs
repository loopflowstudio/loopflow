use std::collections::{HashMap, HashSet};
use std::io::IsTerminal;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use anyhow::Context;
use clap::Parser;
use tracing::debug;
use tracing_subscriber::EnvFilter;

use loopflow::lf::commands::mutation::{selected_task_repository, task_edit_update};
use loopflow::lf::commands::read::Read;
use loopflow::lf::{
    Cli, Commands, GraphCommand, InstallCommand, SkillCommand, TaskCommand, WaveCommand,
};
use loopflow_execution::context_block::ContextMoment;
use loopflow_execution::definition::DefinitionKind;
use loopflow_execution::journal::{self, with_runtime, LfEventFields, LfEventType, LfNode};

use loopflow_execution::ops::project::update_project;
use loopflow_execution::store::planning_sync::LostAnswer;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FlagValue {
    None,
    Optional,
    Required,
}

#[derive(PartialEq, Eq)]
enum SeparateValue {
    None,
    Next,
    Missing,
}

impl FlagValue {
    fn separate_value(self, args: &[String], index: usize) -> SeparateValue {
        if self == Self::None || args[index].contains('=') {
            SeparateValue::None
        } else if args
            .get(index + 1)
            .is_some_and(|next| next == "-" || !next.starts_with('-'))
        {
            SeparateValue::Next
        } else if self == Self::Required {
            SeparateValue::Missing
        } else {
            SeparateValue::None
        }
    }
}

struct CommandArgTables {
    /// Flags owned directly by this command.
    direct: HashMap<String, FlagValue>,
    /// Flags owned here or by any descendant, used only to find the command path.
    recursive: HashMap<String, FlagValue>,
    /// Direct subcommands, indexed by canonical name.
    subcommands: HashMap<String, CommandArgTables>,
}

fn command_arg_tables(command: &clap::Command, inherited: &HashSet<clap::Id>) -> CommandArgTables {
    let mut direct = HashMap::new();
    for arg in command
        .get_arguments()
        .filter(|arg| !inherited.contains(arg.get_id()))
    {
        let takes = if !arg.get_action().takes_values() {
            FlagValue::None
        } else if arg
            .get_num_args()
            .is_some_and(|range| range.min_values() == 0)
        {
            FlagValue::Optional
        } else {
            FlagValue::Required
        };
        if let Some(short) = arg.get_short() {
            direct.insert(format!("-{short}"), takes);
        }
        if let Some(long) = arg.get_long() {
            direct.insert(format!("--{long}"), takes);
        }
    }

    let mut recursive = direct.clone();
    let mut subcommands = HashMap::new();
    let mut inherited = inherited.clone();
    inherited.extend(
        command
            .get_arguments()
            .filter(|arg| arg.is_global_set())
            .map(|arg| arg.get_id().clone()),
    );
    for subcommand in command.get_subcommands() {
        let child = command_arg_tables(subcommand, &inherited);
        // During path discovery a spelling may still belong to several descendants.
        for (flag, &takes) in &child.recursive {
            let entry = recursive.entry(flag.clone()).or_insert(takes);
            *entry = (*entry).max(takes);
        }
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
    TABLES.get_or_init(|| {
        command_arg_tables(&loopflow::lf::navigation::command_tree(), &HashSet::new())
    })
}

fn flag_name(arg: &str) -> &str {
    arg.split_once('=').map_or(arg, |(name, _)| name)
}

fn first_target_index(args: &[String]) -> Option<usize> {
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" {
            return None;
        }
        if !arg.starts_with('-') {
            return Some(index);
        }
        if arg_tables()
            .direct
            .get(flag_name(arg))
            .is_some_and(|takes| takes.separate_value(args, index) == SeparateValue::Next)
        {
            index += 1;
        }
        index += 1;
    }
    None
}

struct SelectedCommand {
    index: usize,
    args: &'static CommandArgTables,
}

fn selected_command_path(args: &[String], target: usize) -> Vec<SelectedCommand> {
    // Insert moved root flags after any leading options, just before the target.
    let root = arg_tables();
    let mut path = vec![SelectedCommand {
        index: target - 1,
        args: root,
    }];
    let mut current = root;
    let mut index = target;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" {
            break;
        }

        if arg.starts_with('-') {
            let name = flag_name(arg);
            let takes = current.recursive.get(name).max(root.direct.get(name));
            if takes.is_some_and(|takes| takes.separate_value(args, index) == SeparateValue::Next) {
                index += 1;
            }
            index += 1;
            continue;
        }

        let Some(child) = current.subcommands.get(arg) else {
            break;
        };
        current = child;
        path.push(SelectedCommand { index, args: child });
        index += 1;
    }

    path
}

/// Accept top-level flags on either side of a target when their meaning is
/// unambiguous. Local command flags win collisions, and `--` ends reordering.
fn reorder_args(args: Vec<String>) -> Vec<String> {
    let Some(target) = first_target_index(&args) else {
        return args;
    };
    let path = selected_command_path(&args, target);
    let mut moved: HashMap<usize, Vec<String>> = HashMap::new();
    let mut retained = vec![true; args.len()];
    let mut index = target + 1;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" {
            break;
        }
        let name = flag_name(arg);
        if path[1..]
            .iter()
            .rfind(|command| command.index < index)
            .is_some_and(|command| command.args.direct.contains_key(name))
        {
            index += 1;
            continue;
        }
        let Some(depth) = path
            .iter()
            .rposition(|command| command.args.direct.contains_key(name))
        else {
            index += 1;
            continue;
        };
        let owner = &path[depth];
        let takes = owner.args.direct[name];
        let start = index;
        match takes.separate_value(&args, index) {
            SeparateValue::Missing => {
                // End at the flag so Clap cannot consume a command name as its value.
                let mut result = vec![args[0].clone()];
                result.extend(
                    path[1..=depth]
                        .iter()
                        .map(|command| args[command.index].clone()),
                );
                result.push(arg.clone());
                return result;
            }
            SeparateValue::Next => index += 1,
            SeparateValue::None => {}
        }
        retained[start..=index].fill(false);
        moved
            .entry(owner.index)
            .or_default()
            .extend_from_slice(&args[start..=index]);
        index += 1;
    }

    if moved.is_empty() {
        return args;
    }
    let mut result = Vec::with_capacity(args.len());
    for (index, arg) in args.into_iter().enumerate() {
        if retained[index] {
            result.push(arg);
        }
        if let Some(flags) = moved.remove(&index) {
            result.extend(flags);
        }
    }
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
    let repo_root = loopflow_execution::repo::find_repo_root()?;
    with_runtime(&repo_root, command, || run(&repo_root))
}

fn in_directory_runtime<T>(
    command: &[String],
    run: impl FnOnce(&Path) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let directory = loopflow_execution::repo::working_directory()?;
    with_runtime(&directory, command, || run(&directory))
}

fn run_default_agent(
    kind: loopflow_execution::harness::LaunchKind,
    cli: &Cli,
    command: &[String],
) -> anyhow::Result<()> {
    let repo_root = loopflow_execution::repo::find_repo_root()?;
    let moved = loopflow_execution::git::worktrees::move_default_agent_to_worktree(&repo_root)?;
    match moved {
        Some(worktree) => {
            eprintln!("moved to `{}`", worktree.path.display());
            let _cwd = CwdGuard::enter(&worktree.path)?;
            with_runtime(&worktree.path, command, || {
                loopflow::lf::commands::run::run(kind, &worktree.path, Some("default"), None, cli)
            })
        }
        None => with_runtime(&repo_root, command, || {
            loopflow::lf::commands::run::run(kind, &repo_root, Some("default"), None, cli)
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

#[derive(Debug)]
struct DefinitionInvocation<'a> {
    name: &'a str,
    kind: Option<DefinitionKind>,
    message: Option<String>,
}

/// Read invocation syntax once for launch and preview, without loading definitions.
fn definition_invocation(cli: &Cli) -> anyhow::Result<Option<DefinitionInvocation<'_>>> {
    let (rest, kind) = match &cli.command {
        Some(Commands::Graph {
            cmd: GraphCommand::External(rest),
        }) => (rest, Some(DefinitionKind::Graph)),
        Some(Commands::Skill {
            cmd: SkillCommand::External(rest),
        }) => (rest, Some(DefinitionKind::Skill)),
        Some(Commands::External(rest)) => (rest, None),
        _ => return Ok(None),
    };
    let (name, messages) = rest.split_first().context("no skill specified")?;
    let name = name.strip_suffix(':').unwrap_or(name);
    anyhow::ensure!(!name.is_empty(), "no skill specified");
    Ok(Some(DefinitionInvocation {
        name,
        kind,
        message: join_args(messages),
    }))
}

fn resolve_cli_target(
    cli: &mut Cli,
) -> anyhow::Result<Option<(loopflow_execution::definition::Target, Option<String>)>> {
    use loopflow_execution::definition::Target;

    let Some(DefinitionInvocation {
        name,
        kind,
        message,
    }) = definition_invocation(cli)?
    else {
        return Ok(None);
    };
    let repo = loopflow_execution::repo::working_directory()?;
    if let Some(path) = &cli.skill_input {
        let invocation = loopflow_execution::skills::invocation::SkillInvocation::read(
            path,
            &repo,
            message.as_deref(),
        )?;
        anyhow::ensure!(
            invocation.skill.name == name,
            "captured skill does not match {name}"
        );
        let target = Target::Skill(invocation.skill.clone());
        cli.resolved_invocation = Some(invocation);
        return Ok(Some((target, message)));
    }
    let target = loopflow_execution::definition::resolve_definition(&repo, name, kind)?;
    Ok(Some((target, message)))
}

fn execute_target(
    kind: loopflow_execution::harness::LaunchKind,
    target: loopflow_execution::definition::Target,
    message: Option<&str>,
    cli: &Cli,
    args: &[String],
    binding: Option<&loopflow_execution::ops::WorkBinding>,
) -> anyhow::Result<()> {
    use loopflow_execution::definition::Target;

    match target {
        Target::Skill(skill) => {
            let repo_root = loopflow_execution::repo::working_directory()?;
            let name = skill.name.as_str();
            let mut selected = cli.process_options();
            selected.resolved_invocation.get_or_insert_with(|| {
                loopflow_execution::skills::invocation::SkillInvocation {
                    skill: skill.clone(),
                    arguments: message.unwrap_or_default().to_string(),
                }
            });
            let cli = &selected;
            with_runtime(&repo_root, args, || {
                loopflow_execution::engine::skill::carry(name)?;
                with_skill_runtime(&repo_root, name, || {
                    let shared = binding.is_some()
                        || loopflow::lf::commands::run::implicit_binding(cli)?.is_some();
                    match binding {
                        Some(binding) => loopflow::lf::commands::run::run_bound(
                            kind,
                            Some(name),
                            message,
                            cli,
                            binding,
                        )?,
                        None => loopflow::lf::commands::run::run(
                            kind,
                            &repo_root,
                            Some(name),
                            message,
                            cli,
                        )?,
                    }
                    // Shared contributions leave checkpoint composition to the caller.
                    if !shared
                        && !loopflow_execution::journal::has_caller()
                        && loopflow_execution::repo::discover_repo_root(&repo_root)?.is_some()
                    {
                        // Best-effort: the skill ran whether or not its edits commit.
                        if loopflow_execution::ops::step_commit_pending(&repo_root) {
                            commit_skill_step(&repo_root, name);
                        }
                    }
                    Ok(())
                })
            })
        }
        composed => {
            let graph = composed.into_graph();
            let repo_root = loopflow_execution::repo::working_directory()?;
            with_runtime(&repo_root, args, || {
                loopflow_execution::engine::graph::carry(&graph.name)?;
                loopflow::lf::commands::graph::run(&graph, message, cli, &repo_root, binding)
            })
        }
    }
}

/// A child `lf commit --step`, as a Graph runs after each skill step, so the
/// message-writing agent is a Process of its own.
fn commit_skill_step(repo_root: &Path, skill: &str) {
    let committed = loopflow_execution::os_process::resolve_lf_binary()
        .map_err(|error| error.to_string())
        .and_then(|lf| {
            let mut command = std::process::Command::new(lf);
            loopflow_execution::config::settings::independent_agent(&mut command)
                .map_err(|e| e.to_string())?;
            command
                .args(["commit", "--step", skill])
                .current_dir(repo_root)
                .stdin(std::process::Stdio::null())
                .status()
                .map_err(|error| error.to_string())
        });
    match committed {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!(skill, %status, "could not commit the skill's edits"),
        Err(error) => tracing::warn!(skill, %error, "could not commit the skill's edits"),
    }
}

/// Start a conversation's prepared input in this `lf session resume`, as `lf -i`
/// would in the conversation's directory with its agent and Work. This process
/// stays the provider's launcher and its one LfProcess.
fn launch_conversation(
    launch: loopflow_execution::ops::human_session::ConversationLaunch,
    args: &[String],
) -> anyhow::Result<()> {
    use loopflow_execution::durable::WorkRef;
    let loopflow_execution::ops::human_session::ConversationLaunch {
        agent,
        cwd,
        work,
        skill,
        first_turn,
        prepared_capture,
        lock,
        kind,
        ..
    } = launch;
    let mut cli = Cli {
        interactive: true,
        agent: Some(agent),
        prepared_capture,
        launch_lock: Some(lock),
        command: Some(match skill {
            Some(skill) => Commands::Skill {
                cmd: loopflow::lf::SkillCommand::External(vec![skill, first_turn]),
            },
            None => Commands::Inline {
                prompt: vec![first_turn],
            },
        }),
        ..Cli::default()
    };
    // The provider starts on this thread, so its agent is selected here.
    let _settings = loopflow_execution::config::settings::select_invocation(cli.setting_values()?);
    let _cwd = CwdGuard::enter(&cwd)?;
    // Work reaches the provider's environment as `lf --task X -i` hands it.
    let (_work_env, binding) = match work {
        Some(mut binding) if matches!(binding.work, WorkRef::Task(_)) => {
            let declaration = EnvGuard::set(
                loopflow_execution::ops::WORK_DECLARATION_ENV,
                format!("task:{}", binding.work.id()),
            );
            cli.wave = binding.wave_name.clone();
            binding.cwd = cwd;
            (Some(declaration), Some(binding))
        }
        Some(binding) => {
            cli.wave = binding.wave_name;
            let wave = binding.wave_id.map(|wave| {
                EnvGuard::set(
                    loopflow_execution::work::wave::context::WAVE_ID_ENV,
                    wave.to_string(),
                )
            });
            (wave, None)
        }
        None => (None, None),
    };
    finish_command(dispatch_launch(kind, &mut cli, args, binding.as_ref()))
}

fn prepare_work_binding(
    selector: &str,
    repo: &Path,
) -> anyhow::Result<loopflow_execution::ops::WorkBinding> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|error| anyhow::anyhow!("cannot resolve {selector}: {error}"))?;
    runtime.block_on(async {
        let store = loopflow_execution::store::open_existing_store()
            .await
            .map_err(|error| anyhow::anyhow!("cannot resolve {selector}: {error}"))?
            .ok_or_else(|| {
                anyhow::anyhow!("cannot resolve {selector}: planning registry unavailable")
            })?;
        let store = Arc::new(store);
        loopflow_execution::ops::resolve_work_binding(&store, repo, selector)
            .await
            .map_err(anyhow::Error::from)
    })
}

struct CwdGuard(std::path::PathBuf);

impl CwdGuard {
    fn enter(path: &Path) -> anyhow::Result<Self> {
        let previous = std::env::current_dir()?;
        std::env::set_current_dir(previous.join(path))
            .map_err(|error| anyhow::anyhow!("enter {}: {error}", path.display()))?;
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

/// One PR's line in `lf task status`. A degraded Linear linkage is named here
/// because this reading is where an operator already looks for writeback health —
/// the Task's `tracker writeback` line sits directly above. Silence means linked.
fn format_task_pr_line(
    pr: &loopflow_execution::work::task::TaskPr,
    stack_parent: Option<&loopflow_execution::ops::task::TaskStackParent>,
) -> String {
    let provider = pr
        .github()
        .map(|github| format!("GitHub #{}", github.number))
        .unwrap_or_else(|| "not opened on GitHub".to_string());
    let placement = stack_parent
        .map(|parent| {
            let number = parent
                .pr_number
                .map(|number| format!("#{number}, "))
                .unwrap_or_default();
            format!(
                "  stacked on {} ({number}{})",
                parent.identifier, parent.branch
            )
        })
        .unwrap_or_default();
    let linkage = pr
        .linear_link_error
        .as_ref()
        .map(|error| format!("  Linear link degraded: {error}"))
        .unwrap_or_default();
    format!(
        "  Pull request: {}  {}  {}{}{}",
        pr.phase().as_str(),
        provider,
        pr.branch,
        placement,
        linkage,
    )
}

fn print_task(task: &loopflow_execution::work::task::Task, json: bool) -> anyhow::Result<()> {
    let snapshot = loopflow_execution::ops::task::task_snapshot(task)?;
    print_task_snapshot(&snapshot, json)
}

fn print_task_snapshot(
    snapshot: &loopflow_execution::ops::task::TaskSnapshot,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
    } else {
        let tracker_writeback = match &snapshot.tracker_writeback {
            loopflow_execution::work::task::LinearWritebackState::Current => "current".to_string(),
            loopflow_execution::work::task::LinearWritebackState::Pending { error, .. } => {
                format!("pending: {error}")
            }
        };
        let branch = snapshot.branch.as_deref().unwrap_or("unplaced");
        let body = format!(
            "provider {}",
            snapshot.provider.as_deref().unwrap_or("not selected")
        );
        let auto = snapshot
            .auto
            .as_ref()
            .map(loopflow::lf::commands::task_row::auto_phrase)
            .unwrap_or_default();
        println!(
            "{}  {}{auto}\n  task: {}\n  body: {}\n  worktree: {}\n  branch: {}\n  tracker writeback: {}",
            snapshot.issue_identifier,
            snapshot.status,
            snapshot.task_id,
            body,
            snapshot.worktree.as_deref().unwrap_or("unplaced"),
            branch,
            tracker_writeback,
        );
        if let Some(graph) = &snapshot.work.graph {
            println!("  Graph {}: {}", graph.name, graph.sentence);
        }
        if let Some(conflict) = &snapshot.completion_pending {
            println!("  error: {conflict}");
        }
        match &snapshot.started_by {
            Some(starter) => println!(
                "  latest Graph: {} · started by {starter}",
                snapshot.execution.reason
            ),
            None => println!("  latest Graph: {}", snapshot.execution.reason),
        }
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
        // A run that let its launcher go named this log after its Graph run.
        let log = |id: &str| {
            let path = loopflow_execution::process::launch_log::graph_log_path(id);
            if path.exists() {
                format!("  log: {}", path.display())
            } else {
                String::new()
            }
        };
        for graph in &snapshot.work.graph_processes {
            println!(
                "  Graph: {}  {}  {:?}{}",
                graph.summary.id,
                graph.summary.name,
                graph.summary.state,
                log(&graph.summary.id),
            );
        }
        if let Some(project) = &snapshot.project_id {
            println!("  project: {project}");
        }
        if let Some(pr) = &snapshot.pr {
            println!(
                "{}",
                format_task_pr_line(pr, snapshot.stack_parent.as_ref())
            );
        }
        match &snapshot.observation {
            loopflow_execution::work::task::Observation::Cached { observed_at } => {
                println!("  PR observation: cached from {observed_at}");
            }
            loopflow_execution::work::task::Observation::Degraded {
                reason,
                cached_as_of,
                retry_at,
            } => {
                println!(
                    "  PR observation: degraded — {reason} (cached from {cached_as_of}; retry after {retry_at})"
                );
            }
            loopflow_execution::work::task::Observation::NotRequired
            | loopflow_execution::work::task::Observation::Fresh { .. } => {}
        }
        let actions = &snapshot.actions;
        if let Some(recommended) = actions.recommended {
            println!("  action: {}  ({})", recommended.as_str(), actions.reason);
        }
    }
    Ok(())
}

fn print_task_control(
    result: &loopflow_execution::ops::task::TaskControlResult,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(result)?);
    } else {
        println!("{}", result.receipt.summary(&result.issue_id));
        match &result.observation {
            loopflow_execution::work::task::Observation::Cached { observed_at } => {
                println!("PR observation: cached from {observed_at}");
            }
            loopflow_execution::work::task::Observation::Degraded {
                reason,
                cached_as_of,
                retry_at,
            } => {
                println!(
                    "PR observation: degraded — {reason} (cached from {cached_as_of}; retry after {retry_at})"
                );
            }
            loopflow_execution::work::task::Observation::NotRequired
            | loopflow_execution::work::task::Observation::Fresh { .. } => {}
        }
    }
    Ok(())
}

fn dispatch_repository(cmd: &loopflow::lf::RepoCommand, args: &[String]) -> anyhow::Result<()> {
    use loopflow::lf::RepoCommand;
    match cmd {
        RepoCommand::Tracker {
            cmd,
            json,
            subscribe,
            ..
        } => in_directory_runtime(args, |_| {
            loopflow::lf::commands::tracker::run(cmd.as_ref(), *json, *subscribe)
        }),
        RepoCommand::Path => in_directory_runtime(args, |_| {
            loopflow::lf::commands::work_route::identity(None, None, true, false)
        }),
        RepoCommand::Release { cmd } => {
            in_repo_runtime(args, |_| loopflow::lf::commands::ops::run_release(cmd))
        }
        RepoCommand::Ci {
            since,
            wave,
            repo,
            json,
        } => loopflow::lf::commands::ci::run(since, wave.as_deref(), repo.as_deref(), *json),
        RepoCommand::People { json } => loopflow::lf::commands::people::run(*json),
        RepoCommand::Tokens { json, days } => loopflow::lf::commands::tokens::run(*json, *days),
    }
}

fn run_wave_command(repo: &Path, command: &WaveCommand) -> anyhow::Result<()> {
    use loopflow_execution::ops::project;

    match command {
        WaveCommand::EditProject {
            target,
            name,
            summary,
            description,
        } => {
            let saved = tokio::runtime::Runtime::new()?.block_on(project::edit(
                repo,
                target.as_project_target(),
                name.as_deref(),
                summary.as_deref(),
                description.as_deref(),
            ))?;
            print_planning_sync(&saved.1);
            Ok(())
        }
        WaveCommand::Graph { cmd } => {
            use loopflow::lf::WaveGraphCommand;
            tokio::runtime::Runtime::new()?.block_on(async {
                match cmd {
                    WaveGraphCommand::Show {
                        wave,
                        no_wave,
                        json,
                    } => {
                        let selected =
                            project::wave_graph(repo, wave.as_deref(), !*no_wave).await?;
                        if *json {
                            println!("{}", serde_json::to_string_pretty(&selected)?);
                        } else {
                            println!(
                                "{} · {} ({} default)",
                                selected.wave.as_deref().unwrap_or("repository"),
                                selected.default.name,
                                selected.default.source.as_str()
                            );
                        }
                    }
                }
                Ok(())
            })
        }

        WaveCommand::Plan { cmd: None, .. } => {
            unreachable!("read commands dispatch separately")
        }
        WaveCommand::Plan { cmd: Some(cmd), .. } => {
            use loopflow::lf::WavePlanCommand;
            match cmd {
                WavePlanCommand::Connect {
                    wave,
                    all,
                    team_key,
                    team_name,
                    json,
                } => loopflow::lf::commands::ops::connect_wave(
                    repo,
                    wave.as_deref(),
                    *all,
                    team_key.as_deref(),
                    team_name.as_deref(),
                    *json,
                ),
                WavePlanCommand::Reteam { apply, json } => {
                    loopflow::lf::commands::ops::reteam(repo, *apply, *json)
                }
                WavePlanCommand::Bind {
                    replace,
                    wave,
                    project,
                    json,
                } => {
                    let result = loopflow_execution::engine::wave::planning_bind(
                        repo, wave, project, *replace,
                    )?;
                    if *json {
                        println!("{}", serde_json::to_string_pretty(&result)?);
                    } else {
                        println!("Wave {wave}: bound Project {} ({})", result.name, result.id);
                    }
                    Ok(())
                }
            }
        }
        WaveCommand::Create { .. } => {
            unreachable!("shared mutation dispatch precedes this command")
        }
        WaveCommand::Remove {
            wave,
            preview,
            delete_backlog,
            json,
        } => {
            let removal = loopflow_execution::ops::wave_lifecycle::remove(
                repo,
                wave,
                *preview,
                *delete_backlog,
            )?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&removal)?);
            } else {
                println!(
                    "{}",
                    loopflow_execution::ops::wave_lifecycle::render_removed(&removal)
                );
            }
            Ok(())
        }
        WaveCommand::List { .. }
        | WaveCommand::Status { .. }
        | WaveCommand::Sync { .. }
        | WaveCommand::Show { .. } => {
            unreachable!("read commands dispatch separately")
        }
        WaveCommand::Rename {
            wave,
            new_name,
            repo: target,
            title,
            json,
        } => {
            let renamed = tokio::runtime::Runtime::new()?.block_on(
                loopflow_execution::ops::wave_lifecycle::rename(
                    repo,
                    wave,
                    new_name.as_deref(),
                    target.as_deref(),
                    title.as_deref(),
                ),
            )?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&renamed)?);
            } else {
                println!(
                    "{}",
                    loopflow_execution::ops::wave_lifecycle::render_renamed(&renamed)
                );
            }
            Ok(())
        }
        WaveCommand::UpdateProject { wave, file } => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                krs: Vec<loopflow_execution::work::project::ProjectKR>,
                metric_targets: Vec<loopflow_execution::work::wave::metrics::MetricTarget>,
            }
            let input: Input = serde_json::from_slice(&std::fs::read(file)?)?;
            let (_, sync) = tokio::runtime::Runtime::new()?.block_on(update_project(
                repo,
                wave.as_deref(),
                &input.krs,
                &input.metric_targets,
            ))?;
            print_planning_sync(&sync);
            Ok(())
        }
    }
}

/// A Task read, answered by the Machine that runs the Task.
fn task_read<T: serde::de::DeserializeOwned>(read: Read) -> anyhow::Result<T> {
    Ok(serde_json::from_value(read.read()?)?)
}

fn read_task_draft() -> anyhow::Result<String> {
    loopflow::lf::commands::mutation::read_task_draft(&mut std::io::stdin().lock())
}

fn run_task_command(cli: &Cli, repo: &Path, command: &TaskCommand) -> anyhow::Result<()> {
    match command {
        TaskCommand::Claim { .. } | TaskCommand::Unassign { .. } => {
            unreachable!("shared mutation dispatch precedes this command")
        }
        TaskCommand::Retire { .. } | TaskCommand::Location { .. } => {
            unreachable!("workspace commands dispatch before repository binding")
        }
        TaskCommand::Checkout { .. } => unreachable!("checkout is dispatched before Work binding"),
        TaskCommand::Sync {
            issue,
            keep_theirs,
            use_mine,
        } => {
            let answer = match (keep_theirs, use_mine) {
                (true, _) => Some(LostAnswer::KeepTheirs),
                (_, true) => Some(LostAnswer::UseMine),
                _ => None,
            };
            println!(
                "{}",
                loopflow_execution::ops::task::task_sync(repo, issue, answer)?
            );
            Ok(())
        }
        TaskCommand::List {
            query,
            wave,
            project,
            state,
            json,
            closed,
            since,
            after,
            limit,
            subscribe,
            ..
        } => {
            use loopflow::lf::commands::waves::{
                list_closed_tasks, list_tasks, subscribe_tasks, TaskListArgs,
            };
            tokio::runtime::Runtime::new()?.block_on(async {
                loopflow_execution::ops::plan_carriers::before_first_read(repo).await;
                loopflow_execution::ops::linear::acquire_authored_plan(repo, wave.as_deref()).await
            })?;
            let args = TaskListArgs {
                query: query.clone(),
                wave: wave.clone(),
                project: project.clone(),
                state: state.clone(),
            };
            if *subscribe {
                subscribe_tasks(repo, args)
            } else if *closed {
                list_closed_tasks(
                    repo,
                    wave.as_deref(),
                    since.as_deref(),
                    after.as_deref(),
                    *limit,
                    *json,
                )
            } else {
                list_tasks(repo, &args, *json)
            }
        }
        TaskCommand::Run { .. } => unreachable!("task run dispatches as an ordinary run"),
        TaskCommand::Auto { .. } => {
            unreachable!("auto on and shared mutation dispatch precede this command")
        }
        TaskCommand::Detail { issue, .. } => {
            loopflow::lf::commands::screens::print(Read::TaskDetail {
                directory: repo.to_owned(),
                issue: issue.clone(),
            })
        }
        TaskCommand::Show {
            issue, subscribe, ..
        } => loopflow::lf::commands::desktop::task_pane(issue, *subscribe),
        TaskCommand::Context { issue, json } => {
            loopflow::lf::commands::usage::run(true, *json, 0, None, None, Some(issue), None)
        }
        TaskCommand::Runs { issue, after, json } => loopflow::lf::commands::graph_inventory::list(
            &loopflow::lf::commands::graph_inventory::GraphProcessInventoryArgs {
                runs: true,
                for_task: Some(issue.clone()),
                after: after.clone(),
                ..Default::default()
            },
            *json,
        ),
        TaskCommand::Reopen { .. } | TaskCommand::Complete { .. } => {
            unreachable!("shared mutation dispatch precedes this command")
        }
        TaskCommand::Create { .. } => {
            let mut stdin = std::io::stdin();
            let mut empty = std::io::empty();
            loopflow::lf::commands::task_create::create(
                repo,
                command,
                &mut loopflow_execution::command::CommandIo {
                    input: if stdin.is_terminal() {
                        &mut empty
                    } else {
                        &mut stdin
                    },
                    output: &mut std::io::stdout(),
                    error: &mut std::io::stderr(),
                },
                cli.batch,
                cli.agent.as_deref(),
            )?;
            Ok(())
        }
        TaskCommand::Status {
            issue,
            json,
            subscribe,
            ..
        } => {
            let status = loopflow_execution::ops::task::task_status(repo, issue.as_deref())?;
            // A Task another Machine runs is read where it runs.
            let mut remote_line = None;
            if let Some(task) = &status.task {
                use loopflow::lf::commands::waves::RemoteStatus;
                match loopflow::lf::commands::waves::print_remote_task_status(
                    repo, task, *json, *subscribe,
                )? {
                    RemoteStatus::Printed => return Ok(()),
                    RemoteStatus::Here => {}
                    RemoteStatus::Unobserved { runs_on, reason } => {
                        if *json {
                            return loopflow::lf::commands::waves::print_task_detail(
                                status,
                                Some(reason),
                            );
                        }
                        remote_line = match status
                            .execution
                            .as_ref()
                            .map(|execution| execution.status.lifecycle(task.state.as_deref()))
                        {
                            Some(state) if state.is_terminal() => Some(state.to_string()),
                            _ => Some(loopflow::lf::commands::task_row::unobserved_reason(
                                &runs_on, &reason,
                            )),
                        };
                    }
                }
            }
            if *subscribe {
                let task = status
                    .task
                    .context("only a registered Task can be subscribed")?;
                return loopflow::lf::commands::waves::subscribe_task_status(
                    repo,
                    task.identifier,
                    loopflow_execution::read_stream::Transport::Stdio,
                )
                .map(drop);
            }
            if *json {
                loopflow::lf::commands::waves::print_task_detail(status, None)?;
            } else {
                println!(
                    "Tracker state: {}",
                    serde_json::to_value(status.tracker_state.state)?
                        .as_str()
                        .expect("planning state is a string")
                );
                if let Some(task) = &status.task {
                    println!("{} · {}", task.title, task.identifier);
                    println!(
                        "Planning: {} · {}",
                        task.state.as_deref().unwrap_or("unknown"),
                        if status.planning_stale {
                            "stale"
                        } else {
                            "current"
                        }
                    );
                    if let Some(due) = &task.due_date {
                        println!("Due: {due}");
                    }
                    if let Some(ownership) = &status.ownership {
                        println!(
                            "{}",
                            loopflow_execution::ops::task_ownership::render(ownership)
                        );
                    }
                    if let Some(line) =
                        loopflow::lf::commands::waves::task_conversation_line(&task.id)?
                    {
                        println!("Conversation: {line}");
                    }
                    for session in loopflow_execution::ops::task::task_copied_sessions(&task.id)? {
                        println!(
                            "Session {} from {}: {}",
                            session.id,
                            session.origin_name(),
                            session.title
                        );
                    }
                } else {
                    println!("No saved Task; retained tracker events are not Work.");
                }
                if let Some(at) = status.tracker_state.synced_at {
                    println!("Synced at: {at}");
                }
                if let Some(sync) = &status.sync {
                    for line in sync.lines() {
                        println!("{line}");
                    }
                }
                if let Some(error) = &status.planning_error {
                    println!("Planning: {error}");
                }
                // A move in flight reads as that, with its retry; a Task another
                // Machine runs is read from its ownership. This Machine's records
                // are what it left behind.
                let elsewhere = match &status.task {
                    Some(task) => loopflow_execution::ops::task_ownership::moving_here(
                        &task.id,
                        &task.identifier,
                    )?
                    .or(remote_line)
                    .map(|line| format!("{}  {line}", task.identifier)),
                    _ => None,
                };
                if let Some(line) = &elsewhere {
                    println!("{line}");
                }
                if let Some(task) = &status.task {
                    if let Some(check) = loopflow_execution::ops::task_ownership::checked_here(
                        &task.id,
                        &task.identifier,
                    )? {
                        println!("{}  {check}", task.identifier);
                    }
                }
                match status.task.as_ref().filter(|task| task.worktree.is_none()) {
                    // Under the line naming where it runs, indented; alone, by name.
                    // A Task holds its auto setting before it has a checkout.
                    Some(task) => println!(
                        "{}  {}{}",
                        if elsewhere.is_some() {
                            ""
                        } else {
                            task.identifier.as_str()
                        },
                        loopflow_execution::ops::task_ownership::not_checked_out_here(),
                        status
                            .execution
                            .as_ref()
                            .and_then(|execution| execution.auto.as_ref())
                            .map(loopflow::lf::commands::task_row::auto_phrase)
                            .unwrap_or_default()
                    ),
                    None if elsewhere.is_none() => match &status.execution {
                        Some(execution) => print_task_snapshot(execution, false)?,
                        None => println!("No execution allocated."),
                    },
                    None => {}
                }
                if let Some(task) = &status.task {
                    loopflow::lf::commands::waves::print_task_family(task)?;
                }
            }
            Ok(())
        }
        TaskCommand::Changes {
            issue,
            base,
            head,
            json,
        } => {
            let snapshot: loopflow_execution::ops::task::TaskChangesSnapshot =
                task_read(Read::TaskChanges {
                    issue: issue.clone(),
                    base: base.clone(),
                    head: head.clone(),
                })?;
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
            head,
            draft,
            json,
            difftool,
        } => {
            let content = draft.then(read_task_draft).transpose()?;
            loopflow::lf::commands::task_diff::run(
                issue,
                path.as_deref(),
                base,
                head.as_deref(),
                content.as_deref(),
                *json,
                *difftool,
            )
        }
        TaskCommand::Symbols {
            issue,
            name,
            path,
            json,
        } => {
            let result =
                loopflow_execution::ops::task::task_symbols(issue, name, path.as_deref(), &|| {
                    false
                })?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                for hit in result.hits {
                    let enclosing = hit
                        .container
                        .map(|name| format!(" ({name})"))
                        .unwrap_or_default();
                    println!(
                        "{}:{}:{} {}{}",
                        hit.path, hit.line, hit.kind, hit.name, enclosing
                    );
                }
                if result.truncated {
                    eprintln!("Search results truncated.");
                }
            }
            Ok(())
        }
        TaskCommand::Grep {
            issue,
            query,
            whole_word,
            json,
        } => {
            let result =
                loopflow_execution::ops::task::task_grep(issue, query, *whole_word, &|| false)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                for hit in result.hits {
                    println!("{}:{}:{}", hit.path, hit.line, hit.excerpt);
                }
                if result.truncated {
                    eprintln!("Search results truncated.");
                }
            }
            Ok(())
        }
        TaskCommand::Find {
            issue,
            query,
            limit,
            json,
        } => {
            let result = loopflow_execution::ops::task::task_find(issue, query.as_deref(), *limit)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                for path in result.paths {
                    println!("{path}");
                }
                if result.truncated {
                    eprintln!("File list truncated.");
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
            let snapshot: loopflow_execution::ops::task::TaskDirectory =
                task_read(Read::TaskFiles {
                    issue: issue.clone(),
                    path: directory.clone(),
                    cursor: cursor.clone(),
                    show_ignored: *show_ignored,
                })?;
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
            at,
            recoveries,
            json,
        } => {
            // Recovery copies are files beside this Machine's checkout.
            let snapshot: loopflow_execution::ops::task::TaskFileSnapshot = if *recoveries {
                loopflow_execution::ops::task::task_file(issue, path, at.as_deref(), true)?
            } else {
                task_read(Read::TaskFile {
                    issue: issue.clone(),
                    path: path.clone(),
                    at: at.clone(),
                })?
            };
            if *json {
                println!("{}", serde_json::to_string_pretty(&snapshot)?);
            } else if snapshot.state != loopflow_execution::ops::task::TaskFileState::Text {
                anyhow::bail!("{}: {:?}", snapshot.path, snapshot.state);
            } else {
                print!("{}", snapshot.content.as_deref().unwrap_or_default());
            }
            Ok(())
        }
        TaskCommand::Save { .. } | TaskCommand::Abandon { .. } => {
            unreachable!("shared mutation dispatch precedes this command")
        }
        TaskCommand::Delete { issue } => {
            let (identifier, pending) = loopflow_execution::ops::task::task_delete(repo, issue)?;
            println!("{identifier}: removed locally; execution history retained");
            if let Some(provider) = loopflow_execution::config::load_config_or_default(Some(repo))
                .planning_transport()
                .map(|tracker| tracker.name())
                .filter(|_| pending)
            {
                println!("Pending {provider} sync");
            }
            Ok(())
        }
        TaskCommand::Update { .. } | TaskCommand::Move { .. } => {
            unreachable!("shared mutation dispatch precedes this command")
        }
        TaskCommand::Report {
            issue,
            message,
            wave,
            json,
        } => print_thread(
            &loopflow_execution::engine::task::report(repo, issue, wave.as_deref(), message)?,
            *json,
        ),
        TaskCommand::Interrupt { issue, json } => {
            let result = loopflow_execution::engine::task::interrupt(issue)?;
            print_task_control(&result, *json)
        }
        TaskCommand::Wait {
            issue,
            until,
            timeout,
            json,
        } => {
            let timeout = timeout
                .as_deref()
                .map(loopflow_execution::config::parse_duration)
                .transpose()?;
            let until = if until == "submitted" {
                loopflow_execution::ops::task::TaskWaitUntil::Open
            } else {
                loopflow_execution::ops::task::TaskWaitUntil::Terminal
            };
            let waited = loopflow_execution::engine::task::wait(issue, until, timeout)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&waited)?);
                Ok(())
            } else {
                print_task_snapshot(&waited.task, false)
            }
        }
    }
}

/// `lf task report` prints the thread with the new entry.
fn print_thread(result: &loopflow_model::comment::TaskComments, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else if result.comments.is_empty() {
        println!("{}: no comments", result.identifier);
    } else {
        for comment in &result.comments {
            let date = comment.created_at.as_deref().unwrap_or("date unavailable");
            let sync = if result.pending_sync.contains(&comment.id) {
                " · pending sync"
            } else {
                ""
            };
            println!(
                "── {} · {date}{sync}\n{}\n",
                comment.author,
                comment.body.trim_end()
            );
            if let Some(body) = result.conflicts.get(&comment.id) {
                println!("Saved local comment; Linear’s edit is current:\n{body}\n");
            }
        }
    }
    if !json {
        if let Some(error) = &result.refresh_error {
            eprintln!("{error}; showing saved comments");
        }
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    // Before any thread exists: everything lf starts inherits this `PATH`.
    loopflow_execution::os_process::repair_path();
    loopflow::build_info::initialize();
    loopflow_execution::transport::initialize();
    // The holder outlives servers and installs. It writes no journal trace
    // and opens no store, so it runs before any of that is set up.
    if std::env::args().skip(1).eq(["holder", "run"]) {
        return match loopflow_execution::terminal_holder::run() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("lf holder: {error:#}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    let _measurement = loopflow_execution::performance::ProcessMeasurement::start();
    let result = journal::with_process(|| {
        let result = run();
        if let Err(error) = &result {
            loopflow_execution::engine::launch::report::<()>(Err(error));
        }
        loopflow_execution::transport::end_open(&result);
        loopflow_execution::transport::report_exit(journal::command_exit_code(&result));
        result
    });
    let code = journal::command_exit_code(&result);
    if let Err(error) = result {
        if error
            .downcast_ref::<loopflow_execution::process::CommandExit>()
            .is_none()
        {
            eprintln!("Error: {error:?}");
        }
    }
    std::process::ExitCode::from(code)
}

/// Diagnostics go to `logs/lf.log` in the selected home, each line ending with
/// the pid and binary that wrote it; the terminal shows what commands print and
/// the errors they return. `-v` or `RUST_LOG` shows diagnostics on stderr instead.
fn init_tracing(verbose: bool) {
    let filter = EnvFilter::try_from_default_env().ok();
    if verbose || filter.is_some() {
        tracing_subscriber::fmt()
            .with_env_filter(filter.unwrap_or_else(|| EnvFilter::new("lf=info,loopflow=info")))
            .with_writer(std::io::stderr)
            .without_time()
            .init();
        return;
    }
    // Creation and conversation setup steps are timings a Start's report reads.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(format!(
            "warn,loopflow::server=info,{}=info",
            loopflow_execution::journal::START_TIMING_TARGET
        )))
        .with_ansi(false)
        .with_writer(|| loopflow_execution::diagnostic_log::DiagnosticLog)
        .event_format(loopflow_execution::diagnostic_log::IdentifiedLines)
        .init();
}

fn reads_task_checkout(cli: &Cli) -> bool {
    matches!(
        cli.command,
        Some(Commands::Task {
            cmd: TaskCommand::Location { .. }
                | TaskCommand::Find { .. }
                | TaskCommand::Grep { .. }
                | TaskCommand::Symbols { .. }
                | TaskCommand::File {
                    recoveries: false,
                    ..
                }
        })
    )
}

fn desktop_intent_read(cli: &Cli) -> bool {
    matches!(&cli.command, Some(Commands::Desktop { cmd }) if cmd.is_read())
}

fn validate_desktop_platform(cli: &Cli) -> anyhow::Result<()> {
    // Desktop pane control refuses before repository lookup or preparation.
    // Opening chooses its destination per platform in `lf open`.
    if let Some(Commands::Desktop { cmd }) = &cli.command {
        if !cli.context && !loopflow::lf::commands::desktop::runs_without_desktop(cmd) {
            loopflow::lf::commands::desktop::require_supported().inspect_err(|_| {
                journal::mark_effect_free_read();
            })?;
        }
    }
    Ok(())
}

fn configuration_directory(cli: &Cli) -> anyhow::Result<std::path::PathBuf> {
    let mut directory = match cli.repo.as_deref() {
        Some(selection) => loopflow_execution::repo::resolve_selection(selection)?,
        None => std::env::current_dir()?,
    };
    if let Some(id) = &cli.repository {
        directory = loopflow::lf::commands::work_route::repository_path(id)?;
    }
    if let Some(cwd) = &cli.cwd {
        directory = directory.join(cwd);
    }
    loopflow_execution::repo::working_directory_in(&directory)
}

fn checkout_task(directory: &Path, wave: Option<&str>) -> anyhow::Result<Option<String>> {
    use loopflow_execution::ops::context::ContextFact;

    let resolution = loopflow::lf::commands::context::explain_in(directory, wave, None, None)?;
    match resolution.task {
        ContextFact::Bound { value, .. } => Ok(Some(value)),
        ContextFact::Unbound => Ok(None),
        ContextFact::Unavailable { reason } => anyhow::bail!(reason),
    }
}

fn run() -> anyhow::Result<()> {
    // A terminal relay is exec'd by the process that resolved its connection.
    // The relay avoids the command tree.
    let mut raw_args: Vec<String> = std::env::args().collect();
    if raw_args
        .get(1..3)
        .is_some_and(|args| args == ["session", "attach"])
    {
        let args = &raw_args[3..];
        journal::mark_effect_free_read();
        ctrlc::set_handler(|| loopflow_execution::agent::exit_on_interrupt())
            .expect("failed to set Ctrl+C handler");
        if args.first().is_some_and(|arg| arg == "--") {
            return loopflow_execution::transport::relay_over(&args[1..], false);
        }
        let relay = loopflow_execution::transport::relay_args(args).ok_or_else(|| {
            anyhow::anyhow!("lf session attach names `--terminal TERMINAL_ID`; got {args:?}")
        })?;
        journal::continue_after_exec()?;
        loopflow_execution::transport::relay(
            &relay.terminal,
            relay.session.as_ref(),
            relay.json_session.is_some(),
        )?;
        // JSON return opens the ordinary projection after relaying ends.
        if let Some(session) = relay.json_session {
            loopflow::lf::commands::session::print_returned(&session)?;
        }
        return Ok(());
    }
    if !raw_args
        .iter()
        .any(|arg| arg == "--machine" || arg.starts_with("--machine="))
    {
        if let Ok(normalized) = loopflow::lf::navigation::normalize_args(raw_args.clone()) {
            if let Ok(cli) = Cli::try_parse_from(reorder_args(normalized)).and_then(Cli::checked) {
                match &cli.command {
                    Some(Commands::Config {
                        cmd:
                            loopflow::lf::ConfigCommand::Import {
                                path,
                                pointer,
                                global,
                                replace,
                            },
                    }) => {
                        let repo = configuration_directory(&cli)?;
                        let saved = loopflow_execution::config::files::import(
                            &repo,
                            cli.wave.as_deref(),
                            *global,
                            path,
                            pointer,
                            *replace,
                        )?;
                        println!("Imported declarations into {}", saved.display());
                        return Ok(());
                    }
                    Some(Commands::Config {
                        cmd:
                            loopflow::lf::ConfigCommand::Set {
                                key,
                                value,
                                destination,
                                json,
                            },
                    }) => {
                        let repo = configuration_directory(&cli)?;
                        return loopflow::lf::commands::setting::declare(
                            &destination.destination(&repo, cli.wave.as_deref())?,
                            key,
                            Some(value),
                            *json,
                        );
                    }
                    Some(Commands::Config {
                        cmd:
                            loopflow::lf::ConfigCommand::Unset {
                                key,
                                destination,
                                json,
                            },
                    }) => {
                        let repo = configuration_directory(&cli)?;
                        return loopflow::lf::commands::setting::declare(
                            &destination.destination(&repo, cli.wave.as_deref())?,
                            key,
                            None,
                            *json,
                        );
                    }
                    Some(Commands::Config {
                        cmd:
                            loopflow::lf::ConfigCommand::Get {
                                key,
                                destination,
                                json,
                            },
                    }) => {
                        journal::mark_effect_free_read();
                        let repo = configuration_directory(&cli)?;
                        return loopflow::lf::commands::setting::get(
                            &destination.destination(&repo, cli.wave.as_deref())?,
                            key,
                            *json,
                        );
                    }
                    Some(Commands::Config {
                        cmd:
                            loopflow::lf::ConfigCommand::Explain {
                                key, destination, ..
                            },
                    }) => {
                        journal::mark_effect_free_read();
                        let repo = configuration_directory(&cli)?;
                        return loopflow::lf::commands::setting::explain(
                            &destination.destination(&repo, cli.wave.as_deref())?,
                            key,
                        );
                    }
                    Some(Commands::Config {
                        cmd: loopflow::lf::ConfigCommand::Scope(scope),
                    }) => {
                        journal::mark_effect_free_read();
                        return scope.run(cli.wave.as_deref(), || configuration_directory(&cli));
                    }
                    Some(Commands::Config {
                        cmd: loopflow::lf::ConfigCommand::Catalog { json },
                    }) => {
                        journal::mark_effect_free_read();
                        return loopflow::lf::commands::config::catalog(*json);
                    }
                    Some(Commands::Config {
                        cmd: loopflow::lf::ConfigCommand::Migrate { accept },
                    }) => return loopflow_execution::config::migrate::run(*accept),
                    Some(Commands::Config {
                        cmd: loopflow::lf::ConfigCommand::Retained { replace, keep },
                    }) => {
                        journal::mark_effect_free_read();
                        return loopflow::lf::commands::config::retained(*replace, *keep);
                    }
                    Some(Commands::Config {
                        cmd:
                            loopflow::lf::ConfigCommand::Settings {
                                skill,
                                graph,
                                step,
                                output,
                            },
                    }) => {
                        journal::mark_effect_free_read();
                        let _settings = loopflow_execution::config::settings::select_invocation(
                            cli.setting_values()?,
                        );
                        let repo = configuration_directory(&cli)?;
                        return loopflow::lf::commands::config::show(
                            &repo,
                            cli.wave.as_deref(),
                            skill.as_deref(),
                            graph.as_deref(),
                            step.as_deref(),
                            output,
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    loopflow_execution::installation::dispatch_entry_gate(
        &loopflow_execution::installation::ArtifactRole::Cli,
    )?;
    // Validate addressed commands once, before help, repository lookup, or admission.
    // A rejected destination must not create even a fallback Process on that Machine.
    if let Err(error) = loopflow::lf::commands::machine::validate_expected_machine_process() {
        journal::mark_effect_free_read();
        return Err(error);
    }
    // Keep presentation in the initiating CLI, outside both the execution
    // transport (which execs its relay) and remote Machine dispatch.
    if raw_args.iter().any(|arg| arg == "--open") {
        if let Ok(normalized) = loopflow::lf::navigation::normalize_args(raw_args.clone()) {
            if let Ok(cli) = Cli::try_parse_from(reorder_args(normalized)).and_then(Cli::checked) {
                if let Some(open) = task_run_open_args(&cli) {
                    return run_task_and_open(&raw_args, open);
                }
            }
        }
    }
    // Ensure Ctrl+C terminates lf and the child agent. Without this,
    // child.wait() retries on EINTR and hangs while the agent catches
    // SIGINT and keeps running. SIGTERM the agent first so it doesn't
    // survive as an orphan. The `termination` feature extends the handler
    // to SIGTERM and SIGHUP: `tmux kill-session` delivers SIGHUP, which
    // otherwise bypasses every cleanup (observed live: it orphaned the wave
    // loop's codex app-server pair and left a stale .wave-endpoint).
    if let Some((remote, command)) = loopflow::lf::navigation::machine_invocation(&raw_args)
        .map_err(|error| {
            let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
            let _ = error.print();
            loopflow_execution::process::CommandExit(code)
        })?
    {
        let args = reorder_args(std::iter::once("lf".to_string()).chain(command).collect());
        let machine = remote.machine.as_deref().expect("machine selector");
        if loopflow::lf::commands::ssh::runs_here(machine, &args[1..])? {
            raw_args = args;
        } else {
            let cli = loopflow::lf::commands::ssh::parse_remote_command(&args[1..])?;
            validate_desktop_platform(&cli)?;
            let preview = desktop_intent_read(&cli)
                || cli.context
                || cli.explain
                || reads_task_checkout(&cli);
            // The target owns command flags, including --verbose; RUST_LOG controls transport logs.
            init_tracing(false);
            if !preview {
                loopflow_execution::installation::canonicalize_selected_home()?;
                journal::admit_process(&std::env::current_dir()?, &raw_args);
            }
            ctrlc::set_handler(|| loopflow_execution::agent::exit_on_interrupt())
                .expect("failed to set Ctrl+C handler");
            return loopflow::lf::commands::ssh::run(
                machine,
                remote.forward_agent,
                &cli,
                &args[1..],
                None,
            );
        }
    }

    // Reorder args so flags can appear after the skill name
    let normalized = loopflow::lf::navigation::normalize_args(raw_args).map_err(|error| {
        let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
        let _ = error.print();
        loopflow_execution::process::CommandExit(code)
    })?;
    let args = reorder_args(normalized);

    let mut cli = match Cli::try_parse_from(args.clone()).and_then(Cli::checked) {
        Ok(cli) => cli,
        // A hook an earlier lf installed may name a value this build removed;
        // it must not fail its turn (Claude reads exit 2 at Stop as "keep working").
        Err(_) if args.get(1).is_some_and(|arg| arg.starts_with("__")) => {
            eprintln!(
                "lf: ignoring `{}`, a hook from an earlier lf; restart the Session to refresh its hooks",
                args[1]
            );
            return Ok(());
        }
        Err(error) => {
            let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
            let _ = error.print();
            return Err(loopflow_execution::process::CommandExit(code).into());
        }
    };
    // Only a command that changes Work writes a journal trace, and a Graph
    // step always does.
    if !journal::take_graph_step() && !cli.command.as_ref().is_none_or(Commands::changes_work) {
        journal::mark_read();
    }
    if cli.harness_callback() {
        journal::harness_issued();
    }
    // Provider hooks keep the callbacks their launching lf wrote. A removed
    // one must not fail: Claude treats exit 2 at Stop as "keep working".
    if let Some(Commands::External(parts)) = &cli.command {
        if parts[0].starts_with("__") {
            eprintln!(
                "lf: ignoring `{}`, a hook from an earlier lf; restart the Session to refresh its hooks",
                parts[0]
            );
            return Ok(());
        }
    }
    if cli.agent.is_some()
        && !cli.may_launch_agent()
        && !matches!(
            cli.command,
            Some(
                Commands::Config {
                    cmd: loopflow::lf::ConfigCommand::Settings { .. }
                } | Commands::Open(_)
            )
        )
    {
        eprintln!("This command does not start an agent; --agent has no effect.");
    }
    loopflow_execution::installation::canonicalize_selected_home()?;
    let _settings = loopflow_execution::config::settings::select_invocation(cli.setting_values()?);
    loopflow_execution::read_stream::select_output(loopflow::lf::selected_paths(&args));
    let planning_read = matches!(
        &cli.command,
        Some(Commands::Wave {
            cmd: WaveCommand::Show { .. }
                | WaveCommand::Graph {
                    cmd: loopflow::lf::WaveGraphCommand::Show { .. }
                }
        })
    );
    let desktop_read = desktop_intent_read(&cli);
    init_tracing(cli.verbose);
    // A turn or tool boundary binds no Work. A managed launch reports to
    // the Machine server. Steer checks open the store locally.
    if let Some(Commands::ContextBlock {
        delivery,
        moment:
            moment @ (ContextMoment::Start
            | ContextMoment::Compact
            | ContextMoment::Tool
            | ContextMoment::Steer
            | ContextMoment::Stop),
        harness,
    }) = &cli.command
    {
        return loopflow::lf::commands::context::emit_block(delivery.as_deref(), *moment, *harness);
    }
    validate_desktop_platform(&cli)?;
    let _selected_repo_cwd = cli
        .repo
        .as_deref()
        .map(|selection| CwdGuard::enter(&loopflow_execution::repo::resolve_selection(selection)?))
        .transpose()?;
    let _repository_cwd = cli
        .repository
        .as_ref()
        .map(|id| CwdGuard::enter(&loopflow::lf::commands::work_route::repository_path(id)?))
        .transpose()?;
    // With `--task`, Work binding enters this directory instead of its checkout.
    let _cwd = cli
        .cwd
        .as_deref()
        .filter(|_| cli.task.is_none())
        .map(CwdGuard::enter)
        .transpose()?;
    let reads_work = cli.may_launch_agent()
        || cli.task.is_some()
        || cli.wave.is_some()
        || matches!(
            cli.command,
            Some(
                Commands::Task { .. }
                    | Commands::Wave { .. }
                    | Commands::Repo {
                        cmd: Some(
                            loopflow::lf::RepoCommand::Release { .. }
                                | loopflow::lf::RepoCommand::Ci { .. }
                                | loopflow::lf::RepoCommand::Tracker { .. }
                        ),
                        ..
                    }
            )
        )
        || matches!(&cli.command, Some(Commands::Session { .. }));
    if reads_work
        && !cli.context
        && !cli.explain
        && !planning_read
        && !reads_task_checkout(&cli)
        && !desktop_read
        && !matches!(
            cli.command,
            Some(Commands::Config {
                cmd: loopflow::lf::ConfigCommand::Repo { .. }
            })
        )
    {
        loopflow_execution::repository::location::reconnect_current()?;
    }
    loopflow::lf::commands::work_route::resolve_repository_selection(&mut cli)?;
    if cli.context || cli.explain {
        return preview_invocation(&cli, &args);
    }
    if desktop_read {
        return dispatch(cli, &args);
    }
    if let Some(Commands::Task {
        cmd:
            TaskCommand::Location {
                issue,
                peers,
                request,
                json,
            },
    }) = &cli.command
    {
        return loopflow::lf::commands::work_route::location(
            &cli,
            issue,
            *peers,
            request.as_deref(),
            *json || cli.json,
        );
    }
    match &cli.command {
        Some(Commands::Task {
            cmd: TaskCommand::Retire { issue },
        }) => {
            journal::admit_process(&std::env::current_dir()?, &args);
            return loopflow_execution::ops::task::retirement::run(issue.as_deref())
                .map_err(Into::into);
        }
        Some(Commands::Desktop {
            cmd: loopflow::lf::DesktopCommand::Shell { args: shell },
        }) => {
            return loopflow::lf::commands::desktop::shell(shell);
        }
        _ => {}
    }
    if planning_read {
        let _cwd = cli
            .wt
            .as_deref()
            .map(|name| CwdGuard::enter(&loopflow::lf::commands::ops::resolve_worktree(name)?))
            .transpose()?;
        let _scope = if cli.wave.is_some() || cli.task.is_some() {
            let context = loopflow::lf::commands::context::explain(
                cli.wave.as_deref(),
                cli.task.as_deref(),
                None,
            )?;
            match context.wave {
                loopflow_execution::ops::context::ContextFact::Bound { value, .. } => Some(
                    EnvGuard::set(loopflow_execution::work::wave::context::WAVE_ID_ENV, value),
                ),
                loopflow_execution::ops::context::ContextFact::Unavailable { reason } => {
                    anyhow::bail!(reason)
                }
                loopflow_execution::ops::context::ContextFact::Unbound => None,
            }
        } else {
            None
        };
        return execute_command(&cli, &args, None);
    }
    if cli.task.is_none() && cli.wt.is_none() {
        if let Some(result) = loopflow::lf::navigation::inspect(&cli) {
            return finish_command(result);
        }
    }
    ctrlc::set_handler(|| loopflow_execution::agent::exit_on_interrupt())
        .expect("failed to set Ctrl+C handler");

    // Machine diagnosis must reach incompatible or uninitialized Machines without
    // ordinary admission creating or migrating the database first.
    if let Some(Commands::Machine {
        cmd:
            loopflow::lf::MachineCommand::Doctor {
                json,
                planning: false,
            },
    }) = &cli.command
    {
        return loopflow::lf::commands::doctor::run(*json);
    }

    // Availability and bootstrap are observational; no Work admission or repair.
    if let Some(Commands::Server { cmd }) = &cli.command {
        return loopflow::lf::commands::server::run(cmd);
    }

    // Global-promotion commands dispatch before home routing, journal emission,
    // and any ordinary store open: a candidate that does not know the live
    // migration frontier must reach the preflight refusal, not fail in
    // trace/store capture. `lf install` opens the store only read-only, inside
    // its own preflight. It records itself once the store has committed and
    // settled, when nothing blocks a plain Process write; the row keeps the
    // command's actual start.
    if let Some(Commands::Self_ {
        cmd:
            loopflow::lf::SelfCommand::Install {
                dev,
                preview,
                stable,
                cmd,
            },
    }) = &cli.command
    {
        use loopflow::lf::commands::install::Stream;
        let result = match cmd.as_ref() {
            None => loopflow::lf::commands::install::install(
                [
                    (*dev, Stream::Dev),
                    (*preview, Stream::Preview),
                    (*stable, Stream::Stable),
                ]
                .into_iter()
                .find_map(|(chosen, stream)| chosen.then_some(stream)),
            ),
            Some(InstallCommand::Schedule { frequency }) => {
                loopflow::lf::commands::install::schedule(*frequency)
            }
            Some(InstallCommand::Server { profile_stdin }) => {
                loopflow_execution::installation::server::delegate_selected()?;
                if *profile_stdin {
                    let profile = std::io::read_to_string(std::io::stdin().lock())?;
                    loopflow_execution::config::remote::provision(&profile)?;
                }
                println!(
                    "{}",
                    serde_json::to_string(&loopflow_execution::installation::server::ensure()?)?
                );
                Ok(())
            }
            Some(InstallCommand::Adopt) => loopflow::lf::commands::install::adopt(),
            Some(InstallCommand::RecoverSwitch { switch }) => {
                loopflow::lf::commands::install::recover_switch(switch.as_deref())
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
        return result;
    }

    {
        debug!(batch = cli.batch, "parsed CLI arguments");

        dispatch(cli, &args)
    }
}

/// Preview stops before process admission, Work preparation,
/// automatic routing or provider discovery. Unsupported shapes never fall through.
fn preview_invocation(cli: &Cli, args: &[String]) -> anyhow::Result<()> {
    let invocation = definition_invocation(cli)?;
    let message = match &cli.command {
        Some(Commands::Inline { prompt }) => Some(prompt.join(" ")),
        _ => None,
    };
    // Reject non-agent context previews before any branch can consume stdin.
    anyhow::ensure!(!cli.context || invocation.is_some() || message.is_some(),
        "--context requires a skill, Graph or inline agent request; use --explain to inspect Work without launch");
    let _cwd = cli
        .wt
        .as_deref()
        .map(|name| CwdGuard::enter(&loopflow::lf::commands::ops::resolve_worktree(name)?))
        .transpose()?;
    let mut matches = loopflow::lf::navigation::command_tree().try_get_matches_from(args)?;
    let mut json = cli.json;
    loop {
        json |= matches
            .try_get_one::<bool>("json")
            .ok()
            .flatten()
            .copied()
            .unwrap_or(false);
        match matches.remove_subcommand() {
            Some((_, child)) => matches = child,
            None => break,
        }
    }
    // `lf steer` names its flag apart from the root `--json`.
    json |= matches!(&cli.command, Some(Commands::Steer { json: true, .. }));
    // Entry resolution has normalized known selectors to exact Task IDs.
    // Every Task preview uses that same selection, regardless of its action.
    let task = if let Some(Commands::Task { cmd }) = &cli.command {
        if let (Some(global), Some(subject)) = (cli.task.as_deref(), cmd.selector()) {
            anyhow::ensure!(
                global == subject,
                "conflicting Task selections: {global} and {subject}"
            );
        }
        cmd.selector().or(cli.task.as_deref())
    } else if let Some(Commands::Steer {
        target: Some(target),
        ..
    }) = &cli.command
    {
        // A preview reads no Session: it shows the Task steer this would save.
        Some(target.as_str())
    } else {
        cli.task.as_deref()
    };
    if let Some(Commands::Open(args)) = &cli.command {
        return loopflow::lf::commands::open::explain(cli, args, json);
    }
    if let Some(Commands::Task {
        cmd:
            TaskCommand::Run {
                graph,
                from,
                name,
                stack_on,
                directive,
                reason,
                ..
            },
    }) = &cli.command
    {
        let report = loopflow::lf::commands::context::explain_task_run(
            task,
            &loopflow_execution::ops::task::TaskProcessOptions {
                wave: cli.wave.clone(),
                reason: reason.clone(),
                name: name.clone(),
                graph: graph.clone(),
                from: from.clone(),
                stack_on: stack_on.clone(),
                directive: directive.clone(),
                branch: None,
                from_last_fetch: false,
                auto_on: false,
            },
            cli.batch || cli.interactive,
        )?;
        if json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("{}", report.render());
        }
        return Ok(());
    }
    if let Some(Commands::Task {
        cmd:
            TaskCommand::Checkout {
                name,
                stack_on,
                directive,
                design,
                recover_from,
                ..
            },
    }) = &cli.command
    {
        let mut report = loopflow::lf::commands::context::explain_task_checkout(
            task,
            &loopflow_execution::ops::task::TaskProcessOptions {
                wave: cli.wave.clone(),
                name: name.clone(),
                stack_on: stack_on.clone(),
                directive: directive.clone(),
                ..Default::default()
            },
        )?;
        if let Some(commit) = recover_from {
            report.unavailable.push(format!("Explicit recovery from {commit}; commit availability and existing branch/checkout equality are checked under the checkout lease before restoring. No branch is reset."));
        }
        if let Some(path) = design {
            report.unavailable.push(format!(
                "Design handoff from {} is requested; source validation, retained receipt and child-file conflict checks have not run",
                path.display()
            ));
        }
        if json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("{}", report.render());
        }
        return Ok(());
    }
    let piped_notes;
    let file_draft;
    let planning = match &cli.command {
        Some(Commands::Task {
            cmd:
                TaskCommand::Create {
                    request,
                    title,
                    notes,
                    wave,
                    no_wave,
                    ..
                },
        }) => {
            piped_notes = match request {
                Some(request) => Some(request.clone()),
                None if notes.is_none() && !std::io::stdin().is_terminal() => {
                    loopflow::lf::commands::task_create::read_report(&mut std::io::stdin())?
                }
                None => None,
            };
            Some((
                if *no_wave {
                    None
                } else {
                    wave.as_deref().or(cli.wave.as_deref())
                },
                loopflow_execution::ops::task::TaskPlanningRequest::Create {
                    title: title.as_deref(),
                    notes: notes.as_deref().or(piped_notes.as_deref()),
                },
            ))
        }
        Some(Commands::Task {
            cmd:
                TaskCommand::Update {
                    title,
                    notes,
                    rank,
                    assignee,
                    parent,
                    wave,
                    ..
                },
        }) => Some((
            wave.as_deref().or(cli.wave.as_deref()),
            loopflow_execution::ops::task::TaskPlanningRequest::Edit {
                update: task_edit_update(title, notes, *rank, assignee),
                parent: parent.as_deref(),
            },
        )),
        Some(Commands::Task {
            cmd: TaskCommand::Report { message, wave, .. },
        }) => Some((
            wave.as_deref().or(cli.wave.as_deref()),
            loopflow_execution::ops::task::TaskPlanningRequest::Comment {
                message: Some(message),
                steer: false,
            },
        )),
        Some(Commands::Steer {
            target: Some(_),
            message,
            ..
        }) => Some((
            cli.wave.as_deref(),
            loopflow_execution::ops::task::TaskPlanningRequest::Comment {
                message: Some(message),
                steer: true,
            },
        )),
        Some(Commands::Task {
            cmd:
                TaskCommand::Move {
                    wave,
                    machine: None,
                    ..
                },
        }) => Some((
            cli.wave.as_deref(),
            loopflow_execution::ops::task::TaskPlanningRequest::Move {
                wave: wave.as_deref(),
            },
        )),
        Some(Commands::Task {
            cmd: TaskCommand::Save { path, revision, .. },
        }) => {
            file_draft = read_task_draft()?;
            Some((
                cli.wave.as_deref(),
                loopflow_execution::ops::task::TaskPlanningRequest::Save {
                    path,
                    revision,
                    content: &file_draft,
                },
            ))
        }
        _ => None,
    };
    if let Some((wave, request)) = planning {
        let report = loopflow::lf::commands::context::explain_task_planning(task, wave, &request)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("{}", report.render());
        }
        return Ok(());
    }
    if let Some(Commands::Session {
        cmd:
            loopflow::lf::SessionCommand::Resume {
                id,
                connection,
                fork,
                ..
            },
    }) = &cli.command
    {
        let report = loopflow::lf::commands::context::explain_session_resume(
            connection.target(cli, id.as_deref())?,
            *fork,
            connection.prepare,
        )?;
        if json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("{}", report.render());
        }
        return Ok(());
    }
    let wave = cli.wave.as_deref();
    let mut session = None;
    match &cli.command {
        Some(Commands::Task { .. } | Commands::Inline { .. }) | None => {}
        Some(Commands::Session {
            cmd:
                loopflow::lf::SessionCommand::History {
                    cmd: Some(loopflow::lf::commands::history::HistoryCommand::Show { id, .. }),
                    ..
                },
        }) => session = Some(id.as_str()),
        _ if invocation.is_some() => {}
        _ => anyhow::bail!(
            "invocation preview is not supported for this command (nothing was executed)"
        ),
    }
    let explanation = cli
        .explain
        .then(|| loopflow::lf::commands::context::explain(wave, task, session))
        .transpose()?;
    let input = cli
        .context
        .then(|| {
            loopflow::lf::commands::context::preview::assemble(
                &std::env::current_dir()?,
                invocation.as_ref().map(|invocation| invocation.name),
                invocation.as_ref().and_then(|invocation| invocation.kind),
                invocation
                    .as_ref()
                    .and_then(|invocation| invocation.message.as_deref())
                    .or(message.as_deref()),
                cli,
            )
        })
        .transpose()?;
    if json {
        match (&explanation, &input) {
            (Some(resolution), Some(input)) => println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"resolution": resolution, "input": input})
                )?
            ),
            (Some(resolution), None) => println!("{}", serde_json::to_string_pretty(resolution)?),
            (None, Some(input)) => println!("{}", serde_json::to_string_pretty(input)?),
            (None, None) => unreachable!("preview flag selected"),
        }
    } else {
        if let Some(explanation) = explanation {
            println!("{}", explanation.render());
        }
        if let Some(input) = input {
            println!("{}", input.render());
        }
    }
    Ok(())
}

/// The `lf open` invocation that follows a successful `task run --open`.
#[derive(Debug)]
struct TaskRunOpen {
    args: Vec<String>,
    /// The launch writes its `task.run` value to stdout; the opener's receipt
    /// then goes to stderr so stdout holds exactly one JSON document.
    json: bool,
}

fn task_run_open_args(cli: &Cli) -> Option<TaskRunOpen> {
    let Some(Commands::Task {
        cmd:
            TaskCommand::Run {
                open: true,
                quote: false,
                json,
                ..
            },
    }) = &cli.command
    else {
        return None;
    };
    if cli.context || cli.explain {
        return None;
    }
    let mut args = vec!["open".to_owned()];
    if let Some(agent) = &cli.agent {
        args.extend(["--agent".into(), agent.clone()]);
    }
    for value in &cli.settings {
        args.extend(["--setting".to_owned(), value.clone()]);
    }
    Some(TaskRunOpen {
        args,
        json: *json || cli.json,
    })
}

fn run_task_and_open(
    args: &[String],
    TaskRunOpen {
        args: mut open,
        json,
    }: TaskRunOpen,
) -> anyhow::Result<()> {
    let mut literal = false;
    let launch: Vec<_> = args[1..]
        .iter()
        .filter(|arg| {
            literal |= arg.as_str() == "--";
            literal || arg.as_str() != "--open"
        })
        .cloned()
        .collect();
    let executable = std::env::current_exe()?;
    let run = |args: &[String], stdout: std::process::Stdio| -> anyhow::Result<()> {
        let status = std::process::Command::new(&executable)
            .args(args)
            .stdout(stdout)
            .status()?;
        if !status.success() {
            let code = status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .unwrap_or(130);
            return Err(loopflow_execution::process::CommandExit(code).into());
        }
        Ok(())
    };
    run(&launch, std::process::Stdio::inherit())?;
    // Resolve on the same Machine with the same selectors, after launch has
    // acquired any previously unknown Task. Only portable identities cross
    // back into presentation; remote checkout paths never enter the local CLI.
    // Root preview flags also precede any end-of-options marker in the launch.
    let output = std::process::Command::new(&executable)
        .args(["--explain", "--json"])
        .args(&launch)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "Task launched, but its selection could not be read for opening"
    );
    let selected: loopflow_execution::ops::task::TaskRunExplanation =
        serde_json::from_slice(&output.stdout).context("read the launched Task's selection")?;
    for (flag, fact) in [
        ("--task", selected.resolution.task),
        ("--repo-id", selected.resolution.repository),
    ] {
        use loopflow_execution::ops::context::ContextFact;
        match fact {
            ContextFact::Bound { value, .. } => open.extend([flag.into(), value]),
            ContextFact::Unavailable { reason } => anyhow::bail!(reason),
            ContextFact::Unbound => {
                anyhow::bail!("Task launched, but {flag} is unavailable for opening")
            }
        }
    }
    let receipt = if json {
        std::io::stderr().into()
    } else {
        std::process::Stdio::inherit()
    };
    run(&open, receipt)
}

fn dispatch(cli: Cli, args: &[String]) -> anyhow::Result<()> {
    // Own direct conversations and Graphs. Delivery controllers may launch an
    // independently owned agent, but retain their own cancellation/lifetime.
    let execution = matches!(
        cli.command,
        None | Some(Commands::External(_) | Commands::Inline { .. })
            | Some(Commands::Skill {
                cmd: SkillCommand::External(_)
            })
            | Some(Commands::Graph {
                cmd: GraphCommand::External(_)
            })
            | Some(Commands::Session {
                cmd: loopflow::lf::SessionCommand::History {
                    cmd: Some(loopflow::lf::commands::history::HistoryCommand::Replay { .. }),
                    ..
                },
            })
    );
    if (matches!(
        cli.command,
        Some(Commands::Task {
            cmd: TaskCommand::Run { .. }
        })
    ) || cli.turns_auto_on())
        && loopflow_execution::transport::detach_current()?
    {
        return Ok(());
    }
    if execution {
        let kind = loopflow_execution::transport::LaunchInput {
            batch: cli.batch,
            interactive: cli.interactive,
            has_prompt: cli.command.is_some() || cli.skill_input.is_some(),
        }
        .kind();
        return dispatch_here(cli, args, |cli, binding| {
            dispatch_launch(kind, cli, args, binding)
        });
    }
    dispatch_here(cli, args, |cli, binding| {
        execute_command(cli, args, binding)
    })
}

fn dispatch_here(
    mut cli: Cli,
    args: &[String],
    execute: impl FnOnce(&mut Cli, Option<&loopflow_execution::ops::WorkBinding>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    // Only the operation a launcher started reports; a held terminal in
    // between passes the handoff on untouched.
    if cli.turns_auto_on()
        || matches!(
            cli.command,
            Some(Commands::Task {
                cmd: TaskCommand::Run { .. }
            }) | Some(Commands::Sync(_))
                | Some(Commands::Pr {
                    cmd: Some(loopflow::lf::PrCommand::Land { .. })
                })
                | Some(Commands::Repo {
                    cmd: Some(loopflow::lf::RepoCommand::Release {
                        cmd: loopflow::lf::ReleaseCommand::Run { .. }
                    }),
                    ..
                })
                | Some(Commands::Skill {
                    cmd: SkillCommand::External(_)
                })
                | Some(Commands::Graph {
                    cmd: GraphCommand::External(_)
                })
        )
    {
        loopflow_execution::engine::launch::expect();
    }
    if loopflow::lf::commands::mutation::accepts(&cli) {
        let directory = match cli.wt.as_deref() {
            Some(name) => loopflow::lf::commands::ops::resolve_worktree(name)?,
            None => std::env::current_dir()?,
        };
        if matches!(
            &cli.command,
            Some(Commands::Task {
                cmd: TaskCommand::Auto {
                    cmd: loopflow::lf::AutoCommand::Off { issue: None, .. }
                }
            })
        ) && cli.task.is_none()
        {
            cli.task = Some(
                checkout_task(&directory, cli.wave.as_deref())?
                    .context("No Task selected; name a Task or run from its checkout")?,
            );
        }
        journal::admit_process(&directory, args);
        let mut stdin = std::io::stdin();
        let mut empty = std::io::empty();
        let terminal_create = stdin.is_terminal()
            && matches!(
                cli.command,
                Some(Commands::Task {
                    cmd: TaskCommand::Create { .. }
                })
            );
        return loopflow::lf::commands::mutation::run(
            &cli,
            args,
            &directory,
            &mut loopflow_execution::command::CommandIo {
                input: if terminal_create {
                    &mut empty
                } else {
                    &mut stdin
                },
                output: &mut std::io::stdout(),
                error: &mut std::io::stderr(),
            },
        );
    }
    // Display location is not execution placement. Opening reads Work here;
    // the retained Desktop owners prepare/connect only the requested view.
    if let Some(Commands::Desktop { cmd }) = &cli.command {
        return loopflow::lf::commands::desktop::run(&cli, cmd);
    }
    if let Some(Commands::Open(open)) = &cli.command {
        let _cwd = cli
            .wt
            .as_deref()
            .map(|name| CwdGuard::enter(&loopflow::lf::commands::ops::resolve_worktree(name)?))
            .transpose()?;
        return loopflow::lf::commands::open::run(&cli, open);
    }

    let _worktree_cwd = cli
        .wt
        .as_deref()
        .map(|name| CwdGuard::enter(&loopflow::lf::commands::ops::resolve_worktree(name)?))
        .transpose()?;
    if cli.command.as_ref().is_none_or(Commands::changes_work) {
        loopflow_execution::config::remote::receive(Some(&std::env::current_dir()?))?;
    }
    if (cli.turns_auto_on()
        || matches!(
            &cli.command,
            Some(Commands::Task {
                cmd: TaskCommand::Run { issue: None, .. }
                    | TaskCommand::Checkout { issue: None, .. }
            })
        ))
        && cli.task.is_none()
    {
        cli.task = Some(
            match checkout_task(&std::env::current_dir()?, cli.wave.as_deref())? {
                Some(task) => task,
                // Auto on for a branch with no Task first gives it one.
                None if cli.turns_auto_on() => {
                    let task = loopflow_execution::ops::task::auto::branch_task(
                        &loopflow_execution::repo::working_directory()?,
                        cli.wave.as_deref(),
                    )?;
                    eprintln!("Created {} for this branch", task.identifier);
                    task.identifier
                }
                None => {
                    anyhow::bail!("No Task selected; name a Task or run from its checkout")
                }
            },
        );
    }
    // Placement chooses or rents the Machine and hands the Task to it, from
    // the flags or the scope's default capacity; the ordinary dispatch below
    // then forwards the run to its new owner.
    if let Some(Commands::Task {
        cmd:
            TaskCommand::Run {
                issue,
                pool,
                rent,
                quote,
                ..
            },
    }) = &cli.command
    {
        if pool.is_some() || rent.is_some() || *quote {
            let issue = issue
                .as_deref()
                .or(cli.task.as_deref())
                .context("No Task selected")?;
            let directory = loopflow_execution::repo::working_directory()?;
            let repo = selected_task_repository(&cli, &directory, Some(issue))?;
            let placement = loopflow::lf::commands::rental::Placement {
                pool: pool.as_deref(),
                rent: rent.as_deref(),
                quote: *quote,
            };
            if loopflow::lf::commands::rental::place(&repo, issue, &placement, cli.batch)? {
                return Ok(());
            }
        } else if let Some(issue) = issue.as_deref().or(cli.task.as_deref()) {
            let directory = loopflow_execution::repo::working_directory()?;
            let repo = selected_task_repository(&cli, &directory, Some(issue))?;
            loopflow::lf::commands::rental::place_by_default(&repo, issue, cli.batch)?;
        }
    }
    // Status selects a Task to read even when it has no checkout to enter.
    if let Some(Commands::Task {
        cmd: TaskCommand::Status { issue, .. },
    }) = &mut cli.command
    {
        if let Some(selected) = cli.task.take() {
            anyhow::ensure!(
                issue.as_ref().is_none_or(|issue| issue == &selected),
                "conflicting Task selections"
            );
            issue.get_or_insert(selected);
        }
    }
    if loopflow::lf::commands::work_route::dispatch(&cli, args)? {
        return Ok(());
    }
    if let Some(Commands::Task {
        cmd:
            TaskCommand::Checkout {
                issue,
                name,
                stack_on,
                directive,
                design,
                recover_from,
                json,
            },
    }) = &cli.command
    {
        let issue = issue
            .as_deref()
            .or(cli.task.as_deref())
            .context("No Task selected")?;
        let directory = std::env::current_dir()?;
        let repo = selected_task_repository(&cli, &directory, Some(issue))?;
        let wave = cli
            .wave
            .as_deref()
            .map(loopflow_execution::work::wave::context::resolve_explicit_wave)
            .transpose()?
            .map(|wave| wave.slug().to_string());
        journal::admit_process(&directory, args);
        return with_runtime(&repo, args, || {
            let task = loopflow_execution::ops::task::task_checkout(
                &directory,
                issue,
                loopflow_execution::ops::task::TaskCheckoutOptions {
                    wave,
                    name: name.clone(),
                    stack_on: stack_on.clone(),
                    directive: directive.clone(),
                    design: design.clone(),
                    recover_from: recover_from.clone(),
                },
            )?;
            loopflow_execution::ops::task::prepare_conversation_beside(&task);
            print_task(&task, *json)
        });
    }
    // `lf task run` places the Task and fills its defaults; from here it is
    // `lf --task ISSUE graph GRAPH`.
    if let Some(Commands::Task {
        cmd:
            TaskCommand::Run {
                issue,
                graph,
                from,
                name,
                stack_on,
                directive,
                reason,
                json,
                ..
            },
    }) = &cli.command
    {
        journal::admit_process(&std::env::current_dir()?, args);
        let issue = issue
            .as_deref()
            .or(cli.task.as_deref())
            .context("No Task selected")?;
        let directory = loopflow_execution::repo::working_directory()?;
        let repo = selected_task_repository(&cli, &directory, Some(issue))?;
        return loopflow::lf::commands::graph::carry_task_run(
            &cli,
            &repo,
            issue,
            loopflow_execution::ops::task::TaskProcessOptions {
                wave: cli.wave.clone(),
                reason: reason.clone(),
                name: name.clone(),
                graph: graph.clone(),
                from: from.clone(),
                stack_on: stack_on.clone(),
                directive: directive.clone(),
                branch: None,
                from_last_fetch: false,
                auto_on: false,
            },
            *json || cli.json,
        );
    }
    if let Some(Commands::Task {
        cmd:
            TaskCommand::Auto {
                cmd:
                    loopflow::lf::AutoCommand::On {
                        graph,
                        reason,
                        json,
                        ..
                    },
            },
    }) = &cli.command
    {
        journal::admit_process(&std::env::current_dir()?, args);
        let issue = cli.task.clone().context("No Task selected")?;
        let directory = loopflow_execution::repo::working_directory()?;
        let repo = selected_task_repository(&cli, &directory, Some(&issue))?;
        // The Task's run, asked for auto. It stands in at each interactive
        // step because auto is on, not because of how it was launched.
        return loopflow::lf::commands::graph::carry_task_run(
            &cli,
            &repo,
            &issue,
            loopflow_execution::ops::task::TaskProcessOptions {
                wave: cli.wave.clone(),
                graph: graph.clone(),
                reason: reason.clone(),
                auto_on: true,
                ..Default::default()
            },
            *json,
        );
    }
    if let Some(Commands::Session { cmd }) = &cli.command {
        journal::admit_process(&std::env::current_dir()?, args);
        return match loopflow::lf::commands::session::run(cmd, &cli)? {
            Some(launch) => {
                let session = launch.session_id.clone();
                launch_conversation(launch, args)?;
                if loopflow::lf::commands::session::returns_record(cmd, &cli) {
                    loopflow::lf::commands::session::print_returned(&session)?;
                }
                Ok(())
            }
            None => Ok(()),
        };
    }
    if let Some(Commands::Steer {
        target,
        message,
        json,
    }) = &cli.command
    {
        journal::admit_process(&std::env::current_dir()?, args);
        return loopflow::lf::commands::session::steer(&cli, target.as_deref(), message, *json);
    }
    // Your own settings scope by the `--wave` typed, never by the worktree's
    // Work, so they dispatch before Work binding.
    if let Some(Commands::Config {
        cmd:
            cmd @ (loopflow::lf::ConfigCommand::Get { .. }
            | loopflow::lf::ConfigCommand::Set { .. }
            | loopflow::lf::ConfigCommand::Unset { .. }
            | loopflow::lf::ConfigCommand::Explain { .. }),
    }) = &cli.command
    {
        use loopflow::lf::commands::setting;
        use loopflow::lf::ConfigCommand;
        return in_directory_runtime(args, |repo| {
            let wave = cli.wave.as_deref();
            match cmd {
                ConfigCommand::Get {
                    key,
                    destination,
                    json,
                } => setting::get(&destination.destination(repo, wave)?, key, *json),
                ConfigCommand::Set {
                    key,
                    value,
                    destination,
                    json,
                } => setting::declare(
                    &destination.destination(repo, wave)?,
                    key,
                    Some(value),
                    *json,
                ),
                ConfigCommand::Unset {
                    key,
                    destination,
                    json,
                } => setting::declare(&destination.destination(repo, wave)?, key, None, *json),
                ConfigCommand::Explain {
                    key, destination, ..
                } => setting::explain(&destination.destination(repo, wave)?, key),
                _ => unreachable!("matched above"),
            }
        });
    }
    // Task run/checkout own preparation above; ordinary invocations bind existing
    // Work. Admit this Process at the directory reached, even if binding fails.
    let mut _work_declaration = None;
    let mut _task_cwd = None;
    let placement = (|| -> anyhow::Result<Option<loopflow_execution::ops::WorkBinding>> {
        if let Some(task) = cli.task.as_ref() {
            let directory = loopflow_execution::repo::working_directory()?;
            let repo = selected_task_repository(&cli, &directory, Some(task))?;
            let mut binding = prepare_work_binding(&format!("task:{task}"), &repo)?;
            std::env::remove_var(loopflow_execution::ops::task::TASK_SOURCE_ENV);
            if let Some(cwd) = cli.cwd.clone() {
                binding.cwd = cwd;
            }
            _task_cwd = Some(CwdGuard::enter(&binding.cwd)?);
            _work_declaration = Some(EnvGuard::set(
                loopflow_execution::ops::WORK_DECLARATION_ENV,
                format!("task:{}", binding.work.id()),
            ));
            binding.cwd = std::env::current_dir()?;
            return Ok(Some(binding));
        }
        Ok(None)
    })();
    journal::admit_process(&std::env::current_dir()?, args);
    let mut direct_binding = placement?;
    let explicit_wave = cli
        .wave
        .as_deref()
        .map(loopflow_execution::work::wave::context::resolve_explicit_wave)
        .transpose()?;
    if let Some(wave) = &explicit_wave {
        // Location wins: context may enrich a Task, never change its owner.
        if direct_binding.is_none() && cli.task.is_none() {
            direct_binding = loopflow::lf::commands::run::implicit_binding(&Cli::default())?
                .filter(|binding| {
                    matches!(binding.work, loopflow_execution::durable::WorkRef::Task(_))
                });
        }
        if let Some(binding) = &direct_binding {
            anyhow::ensure!(
                Some(wave.id()) == binding.wave_id.as_ref(),
                "--wave {} does not own Task {}",
                wave.slug(),
                binding.work.id()
            );
        }
        cli.wave = Some(wave.slug().to_string());
    } else if let Some(binding) = &direct_binding {
        cli.wave = binding.wave_name.clone();
    }
    let _explicit_wave_env = explicit_wave.as_ref().map(|wave| {
        EnvGuard::set(
            loopflow_execution::work::wave::context::WAVE_ID_ENV,
            wave.id().to_string(),
        )
    });

    if let Some(result) = loopflow::lf::navigation::inspect(&cli) {
        return finish_command(result);
    }

    finish_command(execute(&mut cli, direct_binding.as_ref()))
}

fn dispatch_launch(
    kind: loopflow_execution::harness::LaunchKind,
    cli: &mut Cli,
    args: &[String],
    binding: Option<&loopflow_execution::ops::WorkBinding>,
) -> anyhow::Result<()> {
    match resolve_cli_target(cli)? {
        Some((target, message)) => {
            execute_target(kind, target, message.as_deref(), cli, args, binding)
        }
        None => match &cli.command {
            Some(Commands::Inline { prompt }) => {
                let text = prompt.join(" ");
                in_directory_runtime(args, |repo| match binding {
                    Some(binding) => loopflow::lf::commands::run::run_bound(
                        kind,
                        None,
                        Some(&text),
                        cli,
                        binding,
                    ),
                    None => loopflow::lf::commands::run::run(kind, repo, None, Some(&text), cli),
                })
            }
            None => {
                cli.interactive = !cli.batch;
                match binding {
                    Some(binding) => {
                        loopflow::lf::commands::run::run_bound(kind, None, None, cli, binding)
                    }
                    None => run_default_agent(kind, cli, args),
                }
            }
            _ => execute_command(cli, args, binding),
        },
    }
}

fn execute_command(
    cli: &Cli,
    args: &[String],
    binding: Option<&loopflow_execution::ops::WorkBinding>,
) -> anyhow::Result<()> {
    match &cli.command {
        Some(Commands::Inline { .. }) => unreachable!("inline prompts dispatch as launches"),
        Some(Commands::Desktop { .. } | Commands::Open(_)) => {
            unreachable!("Desktop and opening dispatch precede Work preparation")
        }
        Some(Commands::Config { cmd }) => match cmd {
            loopflow::lf::ConfigCommand::Import {
                path,
                pointer,
                global,
                replace,
            } => {
                let repo = configuration_directory(cli)?;
                let saved = loopflow_execution::config::files::import(
                    &repo,
                    cli.wave.as_deref(),
                    *global,
                    path,
                    pointer,
                    *replace,
                )?;
                println!("Imported declarations into {}", saved.display());
                Ok(())
            }

            loopflow::lf::ConfigCommand::Retained { replace, keep } => {
                loopflow::lf::commands::config::retained(*replace, *keep)
            }
            loopflow::lf::ConfigCommand::Migrate { accept } => {
                loopflow_execution::config::migrate::run(*accept)
            }
            loopflow::lf::ConfigCommand::Scope(scope) => in_directory_runtime(args, |repo| {
                scope.run(cli.wave.as_deref(), || Ok(repo.to_owned()))
            }),
            loopflow::lf::ConfigCommand::Catalog { json } => {
                loopflow::lf::commands::config::catalog(*json)
            }
            loopflow::lf::ConfigCommand::Settings {
                skill,
                graph,
                step,
                output,
            } => in_directory_runtime(args, |repo| {
                loopflow::lf::commands::config::show(
                    repo,
                    cli.wave.as_deref(),
                    skill.as_deref(),
                    graph.as_deref(),
                    step.as_deref(),
                    output,
                )
            }),
            loopflow::lf::ConfigCommand::User { json } => {
                loopflow::lf::commands::config::print_user(*json)
            }
            loopflow::lf::ConfigCommand::Get { .. }
            | loopflow::lf::ConfigCommand::Set { .. }
            | loopflow::lf::ConfigCommand::Unset { .. }
            | loopflow::lf::ConfigCommand::Explain { .. } => {
                unreachable!("settings dispatch precedes Work binding")
            }
            loopflow::lf::ConfigCommand::Repo {
                bind,
                relocate,
                json,
            } => in_directory_runtime(args, |_| {
                loopflow::lf::commands::work_route::identity(
                    bind.as_ref(),
                    relocate.as_ref(),
                    false,
                    *json,
                )
            }),
        },
        Some(Commands::ContextBlock {
            delivery,
            moment,
            harness,
        }) => loopflow::lf::commands::context::emit_block(delivery.as_deref(), *moment, *harness),
        Some(Commands::Holder { .. }) => unreachable!("the holder dispatches before any store"),
        Some(Commands::Session { .. } | Commands::Steer { .. }) => {
            unreachable!("Session dispatch precedes Work binding")
        }
        Some(Commands::Machine {
            cmd: loopflow::lf::MachineCommand::Doctor { json, planning },
        }) => {
            if *planning {
                let repo = loopflow_execution::repo::working_directory()?;
                loopflow::lf::commands::ops::sync_planning(&repo, None, true, *json)
            } else {
                loopflow::lf::commands::doctor::run(*json)
            }
        }
        Some(Commands::Server { cmd }) => loopflow::lf::commands::server::run(cmd),
        Some(Commands::Machine { cmd }) => loopflow::lf::commands::machine::run(cmd, cli.batch),
        Some(Commands::Pr { cmd }) => in_repo_runtime(args, |_| {
            loopflow::lf::commands::ops::run_pr(cmd.as_ref(), cli.agent.as_deref())
        }),
        Some(Commands::Wt { cmd }) => {
            in_repo_runtime(args, |_| loopflow::lf::commands::ops::run_wt(cmd))
        }
        Some(Commands::Commit {
            step: Some(step), ..
        }) => in_repo_runtime(args, |_| {
            loopflow::lf::commands::ops::run_step_commit(step, cli.agent.as_deref())
        }),
        Some(Commands::Commit {
            message,
            no_push,
            no_add,
            paths,
            json,
            step: None,
        }) => in_repo_runtime(args, |_| {
            loopflow::lf::commands::ops::run_commit(
                message.as_deref(),
                !*no_push,
                *no_add,
                paths,
                cli.agent.as_deref(),
                *json,
            )
        }),
        Some(Commands::Sync(sync)) => {
            let repo =
                loopflow_execution::repo::require_repo_root(&std::env::current_dir()?, "lf sync")?;
            with_runtime(&repo, args, || {
                loopflow::lf::commands::ops::run_sync(
                    &loopflow_execution::engine::checkout::SyncInput {
                        onto: sync.onto.as_deref(),
                        preview: sync.plan,
                        manual: sync.manual,
                        continue_sync: sync.continue_sync,
                        abort: sync.abort,
                        adopt: sync.adopt,
                    },
                    sync.json,
                )
            })
        }
        Some(Commands::Repo { cmd: Some(cmd), .. }) => dispatch_repository(cmd, args),
        // Bare `lf repo`: the identity read `lf config repo` keeps for its edits.
        Some(Commands::Repo { cmd: None, json }) => in_directory_runtime(args, |_| {
            loopflow::lf::commands::work_route::identity(None, None, false, *json)
        }),
        Some(Commands::Self_ {
            cmd:
                loopflow::lf::SelfCommand::SyncSkills {
                    yes,
                    no_prune,
                    repo,
                },
        }) => loopflow::lf::commands::ops::run_sync_skills(*yes, *no_prune, *repo),
        Some(Commands::Wave {
            cmd: WaveCommand::List { json, all },
        }) => loopflow::lf::commands::waves::ls(*json, *all),
        Some(Commands::Wave {
            cmd:
                WaveCommand::Status {
                    wave,
                    subscribe: true,
                    ..
                },
        }) => loopflow::lf::commands::waves::subscribe_status(wave.clone()),
        Some(Commands::Wave {
            cmd: WaveCommand::Status { wave, json, .. },
        }) => loopflow::lf::commands::waves::status(wave.as_deref(), *json, true),
        Some(Commands::Wave {
            cmd: WaveCommand::Sync { wave, json },
        }) => {
            let refreshed = loopflow::lf::commands::ops::refresh_status(
                &std::env::current_dir()?,
                wave.as_deref(),
            )?;
            if let loopflow::lf::commands::ops::PlanningRefresh::Unconnected(wave) = &refreshed {
                if !*json {
                    println!(
                        "{wave}: no Linear Initiative; showing local planning (lf wave plan connect {wave})"
                    );
                }
            }
            loopflow::lf::commands::waves::status(
                refreshed.wave().or(wave.as_deref()),
                *json,
                false,
            )
        }
        Some(Commands::Wave {
            cmd: WaveCommand::Plan {
                wave, cmd: None, ..
            },
        }) => loopflow::lf::commands::waves::status(wave.as_deref(), true, false),
        Some(Commands::Plan {
            cmd:
                loopflow::lf::PlanCommand::Board {
                    wave, task, all, ..
                },
        }) => loopflow::lf::commands::screens::print(Read::PlanBoard {
            directory: std::env::current_dir()?,
            wave: wave.clone(),
            task: task.clone(),
            all: *all,
        }),
        Some(Commands::Plan {
            cmd:
                loopflow::lf::PlanCommand::List {
                    wave,
                    task,
                    all,
                    subscribe,
                    ..
                },
        }) => {
            if *subscribe {
                loopflow::lf::commands::waves::subscribe_show(
                    std::env::current_dir()?,
                    wave.clone(),
                    task.clone(),
                    *all,
                    loopflow_execution::read_stream::Transport::Stdio,
                )
                .map(drop)
            } else {
                loopflow::lf::commands::waves::show(
                    wave.as_deref(),
                    task.as_deref(),
                    true,
                    *all,
                    false,
                )
            }
        }

        Some(Commands::Wave {
            cmd: cmd @ WaveCommand::Rename { .. },
        }) => in_directory_runtime(args, |repo| run_wave_command(repo, cmd)),
        Some(Commands::Wave {
            cmd: cmd @ WaveCommand::Graph { .. },
        }) => {
            let cwd = std::env::current_dir()?;
            let repo = loopflow_execution::repo::discover_repo_root(&cwd)?.unwrap_or(cwd);
            run_wave_command(&repo, cmd)
        }
        Some(Commands::Wave {
            cmd:
                WaveCommand::Show {
                    wave,
                    task,
                    all,
                    subscribe: true,
                    ..
                },
        }) => std::env::current_dir()
            .map_err(anyhow::Error::from)
            .and_then(|directory| {
                loopflow::lf::commands::waves::subscribe_show(
                    directory,
                    wave.clone(),
                    task.clone(),
                    *all,
                    loopflow_execution::read_stream::Transport::Stdio,
                )
            })
            .map(drop),
        Some(Commands::Wave {
            cmd:
                WaveCommand::Show {
                    wave,
                    task,
                    json,
                    all,
                    ..
                },
        }) => {
            loopflow::lf::commands::waves::show(wave.as_deref(), task.as_deref(), *json, *all, true)
        }
        Some(Commands::Wave { cmd }) => in_repo_runtime(args, |repo| run_wave_command(repo, cmd)),
        Some(Commands::Scratch {
            cmd: loopflow::lf::ScratchCommand::Clear,
        }) => in_repo_runtime(args, |repo| {
            let count = loopflow_execution::ops::scratch::clear(repo)?;
            println!("Cleared {count} added scratch files.");
            Ok(())
        }),
        // Local document access owns no Run lifecycle. Placement and the recorded
        // Git base come from the Task registry inside these operations.
        Some(Commands::Task {
            cmd:
                cmd @ (TaskCommand::Diff { .. }
                | TaskCommand::Changes { .. }
                | TaskCommand::Show { .. }
                | TaskCommand::Detail { .. }
                | TaskCommand::Context { .. }
                | TaskCommand::Runs { .. }
                | TaskCommand::File { .. }
                | TaskCommand::Find { .. }
                | TaskCommand::Grep { .. }
                | TaskCommand::Symbols { .. }
                | TaskCommand::Files { .. }
                | TaskCommand::Save { .. }),
        }) => run_task_command(cli, &std::env::current_dir()?, cmd),
        Some(Commands::Task { cmd }) => {
            let directory = loopflow_execution::repo::working_directory()?;
            let repo = selected_task_repository(cli, &directory, cmd.selector())?;
            with_runtime(&repo, args, || run_task_command(cli, &repo, cmd))
        }
        Some(Commands::Context { json, wave, task }) => {
            loopflow::lf::commands::context::run(*json, wave.as_deref(), task.as_deref())
        }
        Some(Commands::List { .. } | Commands::Help { .. } | Commands::Docs { .. }) => {
            unreachable!("inspection returned before execution")
        }
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
        Some(Commands::Graph { cmd }) => match cmd {
            GraphCommand::Run => {
                let repo_root = loopflow_execution::repo::working_directory()?;
                with_runtime(&repo_root, args, || {
                    loopflow::lf::commands::graph::run_task_graph(cli, &repo_root, binding)
                })
            }
            GraphCommand::List { json, inventory } if inventory.runs => {
                loopflow::lf::commands::graph_inventory::list(inventory, *json)
            }
            _ => anyhow::bail!("not a Graph inspection command: {cmd:?}"),
        },
        Some(Commands::Skill { .. } | Commands::External(_)) => {
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
            return Err(loopflow_execution::process::CommandExit(code).into());
        }
        if matches!(
            error.downcast_ref::<loopflow_execution::error::LoadError>(),
            Some(
                loopflow_execution::error::LoadError::TargetNotFound(_)
                    | loopflow_execution::error::LoadError::SkillNotFound(_)
                    | loopflow_execution::error::LoadError::GraphNotFound(_)
            )
        ) {
            let error =
                clap::Error::raw(clap::error::ErrorKind::InvalidSubcommand, error.to_string());
            let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
            let _ = error.print();
            return Err(loopflow_execution::process::CommandExit(code).into());
        }
    }
    result
}

fn print_planning_sync(sync: &loopflow_execution::store::planning_sync::TrackerSync) {
    for line in sync.lines() {
        eprintln!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::{format_task_pr_line, reorder_args, CwdGuard, EnvGuard};

    use clap::Parser;
    use loopflow::lf::{Cli, Commands, PrCommand, TaskCommand};
    use loopflow_execution::work::task::{GithubPr, PrPublication, TaskId, TaskPr, TaskPrId};

    #[test]
    fn task_run_open_keeps_presentation_options_without_execution_selectors() {
        let cli = Cli::try_parse_from([
            "lf",
            "--machine",
            "worker",
            "--repo-id",
            "repo_11111111111111111111111111111111",
            "--cwd",
            "/checkout",
            "--agent",
            "claude",
            "task",
            "run",
            "LOO-42",
            "--open",
            "--json",
        ])
        .unwrap();
        let open = super::task_run_open_args(&cli).unwrap();
        assert!(open.json);
        assert!(!open.args.iter().any(|arg| arg == "--json"));
        let opened = Cli::try_parse_from(reorder_args(
            std::iter::once("lf".to_owned()).chain(open.args).collect(),
        ))
        .unwrap();
        assert!(matches!(opened.command, Some(Commands::Open(_))));
        assert!(opened.machine.is_none());
        assert!(opened.task.is_none());
        assert!(opened.repository.is_none());
        assert!(opened.repo.is_none());
        assert!(opened.wt.is_none());
        assert!(opened.cwd.is_none());
        assert_eq!(opened.agent, cli.agent);
    }

    #[test]
    fn task_run_open_skips_reads_and_plain_runs() {
        for flags in [
            vec![],
            vec!["--open", "--context"],
            vec!["--open", "--explain"],
            vec!["--open", "--quote"],
        ] {
            let args = ["lf", "task", "run", "LOO-42"]
                .into_iter()
                .chain(flags)
                .map(str::to_owned)
                .collect();
            let cli = Cli::try_parse_from(reorder_args(args)).unwrap();
            assert!(super::task_run_open_args(&cli).is_none());
        }
    }

    /// `--as` never degrades to raw attribution: an absent registry is named
    /// as such, and a store lf cannot open names its cause, never "empty".
    #[test]
    fn bound_work_selector_requires_a_readable_registry() {
        let _lock = PROCESS_STATE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("LF_HOME", directory.path().display().to_string());

        let absent = super::prepare_work_binding("task:LOO-265", directory.path())
            .expect_err("no registry means no binding")
            .to_string();
        assert!(absent.contains("planning registry unavailable"), "{absent}");

        let path = directory.path().join("loopflow.db");
        std::fs::create_dir(&path).unwrap();
        let unopenable = super::prepare_work_binding("task:LOO-265", directory.path())
            .expect_err("an unopenable store must not read as empty")
            .to_string();
        assert!(
            unopenable.contains("cannot resolve task:LOO-265")
                && unopenable.contains(&path.display().to_string())
                && !unopenable.contains("planning registry unavailable"),
            "{unopenable}"
        );
    }

    static PROCESS_STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn definition_execution_stays_explicit() {
        use loopflow_execution::definition::Target;

        let _lock = PROCESS_STATE_LOCK.lock().unwrap();
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        std::fs::write(repo.path().join(".lf/skills/land.md"), "Review landing.").unwrap();
        std::fs::write(repo.path().join(".lf/graphs/land.yaml"), "- cmd: pr land\n").unwrap();
        let _cwd = CwdGuard::enter(repo.path()).unwrap();
        let resolve = |args: &[&str]| {
            let args = loopflow::lf::navigation::normalize_args(
                args.iter().map(|arg| arg.to_string()).collect(),
            )
            .unwrap();
            let args = reorder_args(args);
            let mut cli = Cli::try_parse_from(&args).unwrap();
            super::resolve_cli_target(&mut cli).unwrap().unwrap()
        };

        for args in [
            vec!["lf", "graph", "land", "keep", "together"],
            vec!["lf", "graph", "land:", "keep", "together"],
            vec!["lf", "land:", "keep", "together"],
        ] {
            let (target, message) = resolve(&args);
            assert!(matches!(target, Target::Graph(_)));
            assert_eq!(message.as_deref(), Some("keep together"));
        }
        std::fs::create_dir_all(repo.path().join(".lf/skills/team")).unwrap();
        std::fs::write(repo.path().join(".lf/skills/team/review.md"), "Review.").unwrap();
        assert!(resolve(&["lf", "skill", "team/review"]).1.is_none());
        for name in ["land", "land:", "team/review", "team/review:"] {
            let (target, message) = resolve(&["lf", "skill", name, "auth flow"]);
            let Target::Skill(skill) = target else {
                panic!("expected skill")
            };
            assert_eq!(skill.name, name.trim_end_matches(':'));
            assert_eq!(message.as_deref(), Some("auth flow"));
        }
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
        let line = format_task_pr_line(&published_pr(), None);
        assert!(line.contains("GitHub #931"), "{line}");
        assert!(!line.contains("Linear"), "{line}");
    }

    /// A degraded Linear writeback is named in the same reading that already
    /// carries `tracker writeback`, so an expired token cannot fail silently forever.
    #[test]
    fn task_pr_line_names_a_degraded_linear_link() {
        let mut pr = published_pr();
        pr.linear_link_error = Some("linear token expired".to_string());
        let line = format_task_pr_line(&pr, None);
        assert!(line.contains("Linear link degraded"), "{line}");
        assert!(line.contains("linear token expired"), "{line}");
        // The publication reading survives alongside the degraded linkage.
        assert!(line.contains("GitHub #931"), "{line}");
    }

    #[test]
    fn task_pr_line_names_the_stack_parent_by_task() {
        let mut parent = loopflow_execution::ops::task::TaskStackParent {
            identifier: "LOO-953".to_string(),
            branch: "jack/loo-953-parent".to_string(),
            pr_number: Some(1974),
        };
        let line = format_task_pr_line(&published_pr(), Some(&parent));
        assert!(
            line.ends_with("  stacked on LOO-953 (#1974, jack/loo-953-parent)"),
            "{line}"
        );
        parent.pr_number = None;
        let line = format_task_pr_line(&published_pr(), Some(&parent));
        assert!(
            line.ends_with("  stacked on LOO-953 (jack/loo-953-parent)"),
            "{line}"
        );
    }

    #[test]
    fn reorder_args_moves_work_selector_after_skill_to_global_position() {
        let args = ["lf", "implement", "--task", "LOO-123"]
            .map(str::to_owned)
            .to_vec();
        assert_eq!(reorder_args(args), ["lf", "--task", "LOO-123", "implement"]);
    }

    #[test]
    fn global_agent_combines_with_selectors_without_consuming_literal_arguments() {
        for invocation in [
            vec!["--task", "LOO-438", "skill", "task/session"],
            vec!["--wt", "feature", "skill", "debug"],
            vec!["-b", "-c", "--setting", "chrome=false", "graph", "ship-api"],
            vec!["task", "run", "LOO-438", "pursue"],
            vec!["pr", "publish"],
            vec!["pr", "open"],
            vec!["audit"],
        ] {
            for flag in ["-a", "--agent"] {
                for position in 0..=invocation.len() {
                    // Do not split another option from its value.
                    if position > 0
                        && matches!(invocation[position - 1], "--task" | "--wt" | "--setting")
                    {
                        continue;
                    }
                    let mut args = invocation.clone();
                    args.splice(position..position, [flag, "claude"]);
                    args.insert(0, "lf");
                    let args = loopflow::lf::navigation::normalize_args(
                        args.into_iter().map(String::from).collect(),
                    )
                    .unwrap();
                    let cli = Cli::try_parse_from(reorder_args(args)).unwrap();
                    assert_eq!(
                        cli.agent.as_deref(),
                        Some("claude"),
                        "{invocation:?} at {position}"
                    );
                }
            }
        }
        for invocation in [
            vec!["lf", "skill", "debug", "--", "-a", "literal"],
            vec!["lf", "graph", "debug", "--", "-a", "literal"],
            vec!["lf", "debug", "--", "-a", "literal"],
        ] {
            let args = invocation.into_iter().map(String::from).collect();
            let args = reorder_args(loopflow::lf::navigation::normalize_args(args).unwrap());
            assert!(args.ends_with(&["--".into(), "-a".into(), "literal".into()]));
            assert!(Cli::try_parse_from(args).unwrap().agent.is_none());
        }
        let args = [
            "lf",
            "skill",
            "debug",
            "-a",
            "claude",
            "--machine",
            "fixture",
        ]
        .map(String::from);
        let (transport, remote) = loopflow::lf::navigation::machine_invocation(&args)
            .unwrap()
            .unwrap();
        assert_eq!(transport.machine.as_deref(), Some("fixture"));
        let args = std::iter::once("lf".into()).chain(remote).collect();
        assert_eq!(
            Cli::try_parse_from(reorder_args(args))
                .unwrap()
                .agent
                .as_deref(),
            Some("claude")
        );
    }

    #[test]
    fn cwd_is_entered_for_the_invocation_and_restored_afterward() {
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

    fn argv(words: &str) -> Vec<String> {
        std::iter::once("lf")
            .chain(words.split_whitespace())
            .map(String::from)
            .collect()
    }

    #[test]
    fn reorder_args_places_global_flags_before_skills() {
        for (input, expected) in [
            ("-c debug", "-c debug"),
            ("debug -c", "-c debug"),
            ("-a codex implement", "-a codex implement"),
            ("debug -a codex", "-a codex debug"),
            (
                "--interactive implement -c -a claude",
                "--interactive -c -a claude implement",
            ),
            ("implement: add logout -c", "-c implement: add logout"),
        ] {
            assert_eq!(reorder_args(argv(input)), argv(expected), "{input}");
        }
    }

    #[test]
    fn reorder_args_keeps_nested_global_json_with_its_owner() {
        for args in [
            vec!["lf", "desktop", "portfolio", "--json"],
            vec!["lf", "desktop", "portfolio", "manual", "--json"],
            vec!["lf", "desktop", "portfolio", "--json", "manual"],
        ] {
            let cli =
                Cli::try_parse_from(reorder_args(args.into_iter().map(String::from).collect()))
                    .and_then(Cli::checked)
                    .unwrap();
            assert!(!cli.json);
            assert!(matches!(
                cli.command,
                Some(Commands::Desktop {
                    cmd: loopflow::lf::DesktopCommand::Portfolio { json: true, .. }
                })
            ));
        }
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
    fn reorder_args_reports_missing_values_at_the_flag_owner() {
        for (input, expected) in [
            ("open --repo", "--repo"),
            ("-c open --repo", "--repo"),
            ("open --agent", "--agent"),
            ("open --task", "--task"),
            ("task list --repo", "--repo"),
            ("steer session_1 m --repo", "--repo"),
            ("implement --repo", "--repo"),
            ("implement: fix it -a", "-a"),
            ("open --repo --tab", "--repo"),
            ("open --repo -c", "--repo"),
            ("implement --repo --", "--repo"),
            ("task --wave --title x create", "task create --wave"),
            ("session history show id --after", "session history --after"),
        ] {
            let reordered = reorder_args(argv(input));
            assert_eq!(reordered, argv(expected), "{input}");
            assert_eq!(reorder_args(reordered.clone()), reordered);
            let error = Cli::try_parse_from(reordered).unwrap_err();
            assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
            assert_eq!(error.exit_code(), 2);
            let flag = expected.split_whitespace().last().unwrap();
            let flag = if flag == "-a" { "--agent" } else { flag };
            let diagnostic = error.to_string();
            assert!(
                diagnostic.contains(&format!("a value is required for '{flag} "))
                    && diagnostic.contains("but none was supplied"),
                "{input}: {error}"
            );
        }
    }

    #[test]
    fn reorder_args_preserves_supplied_and_optional_values() {
        for (input, expected) in [
            ("commit -m msg", "commit -m msg"),
            ("-c open --repo x", "-c --repo x open"),
            ("open --repo=x", "--repo=x open"),
            ("open --repo x", "--repo x open"),
            ("open --repo -", "--repo - open"),
            (
                "session --primary --json list",
                "session list --primary --json",
            ),
        ] {
            let reordered = reorder_args(argv(input));
            assert_eq!(reordered, argv(expected), "{input}");
            Cli::try_parse_from(reordered).unwrap();
        }
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
        let args: Vec<String> = ["lf", "wave", "plan", "--all", "connect"]
            .map(String::from)
            .to_vec();
        let reordered = reorder_args(args);
        assert_eq!(reordered, vec!["lf", "wave", "plan", "connect", "--all"]);
        assert!(matches!(
            Cli::try_parse_from(reordered).unwrap().command,
            Some(Commands::Wave {
                cmd: loopflow::lf::WaveCommand::Plan {
                    cmd: Some(loopflow::lf::WavePlanCommand::Connect { all: true, .. }),
                    ..
                }
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

#[cfg(test)]
#[ctor::ctor(unsafe)]
fn initialize_test_build() {
    loopflow_execution::test_support::isolate_from_the_developers_machine();
    loopflow::build_info::initialize();
}
