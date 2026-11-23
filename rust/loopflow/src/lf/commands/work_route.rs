//! Work chooses execution; SSH transports the ordinary command unchanged.
use std::path::Path;

use crate::lf::{Cli, Commands, SessionCommand, TaskCommand};
use anyhow::{anyhow, Result};
use loopflow_execution::durable::TaskExecutionRoute;
use loopflow_execution::machine::task::{read_registry, resolve_task_id};
use loopflow_execution::ops::human_session::moves::conversation_task;
use loopflow_model::durable::{MachineId, RepositoryId, RepositoryIdentity, TaskId};
use loopflow_storage::sqlite::SqliteStore;

pub fn identity(
    bind: Option<&RepositoryId>,
    relocate: Option<&RepositoryId>,
    path: bool,
    json: bool,
) -> Result<()> {
    let (repo, identity) = repository_identity(&std::env::current_dir()?, bind, relocate)?;
    if path {
        println!("{repo}");
    } else if json {
        println!("{}", serde_json::to_string(&identity)?);
    } else {
        println!("{}", identity.id);
    }
    Ok(())
}

/// The repository containing `directory` and its planning identity,
/// registering it on first sight as `lf config repo` does.
pub fn repository_identity(
    directory: &std::path::Path,
    bind: Option<&RepositoryId>,
    relocate: Option<&RepositoryId>,
) -> Result<(
    loopflow_execution::repository::CanonicalRepo,
    RepositoryIdentity,
)> {
    let repo = loopflow_execution::repository::containing(directory)?
        .ok_or_else(|| anyhow!("run `lf repo` in a repository"))?;
    let runtime = tokio::runtime::Runtime::new()?;
    let identity = runtime.block_on(async {
        let store = loopflow_execution::store::open_store(
            &loopflow_execution::store::storage_config_from_env()?,
        )
        .await?;
        loopflow_execution::repository::location::reconcile(&store.sqlite, &repo, bind, relocate)?;
        store
            .sqlite
            .repository_identity(&repo.to_string())?
            .ok_or_else(|| anyhow!("repository identity disappeared after registration"))
    })?;
    Ok((repo, identity))
}

pub fn repository_path(id: &RepositoryId) -> Result<std::path::PathBuf> {
    let path = read_registry()?
        .map(|store| store.repository_path(id))
        .transpose()?
        .flatten()
        .ok_or_else(|| anyhow!("repository plan {id} has no local checkout; run `lf config repo --bind {id}` in its repository on this Machine"))?;
    Ok(path.into())
}

/// Resolve scoped selectors once, carrying exact IDs to downstream consumers.
/// Routing reads existing records only: initialization and schema upgrades
/// belong to the operation that takes up Work, and that command fetches
/// unknown Tasks from the repository's tracker.
pub fn resolve_repository_selection(cli: &mut Cli) -> Result<()> {
    if matches!(
        cli.command,
        Some(Commands::Task {
            cmd: TaskCommand::Location { .. }
        })
    ) {
        return Ok(()); // Keep the requested portable ID in the reply envelope.
    }
    let explicit_repository = cli.selects_repository();
    let (command_task, parent_task) = match &mut cli.command {
        Some(Commands::Task {
            cmd:
                TaskCommand::Run {
                    issue, stack_on, ..
                }
                | TaskCommand::Checkout {
                    issue, stack_on, ..
                },
        }) => (issue.as_mut(), stack_on.as_mut()),
        Some(Commands::Task { cmd }) => (cmd.selector_mut(), None),
        Some(Commands::Context { task, .. }) => (task.as_mut(), None),
        _ => (None, None),
    };
    if cli.task.is_none() && command_task.is_none() {
        return Ok(());
    }
    // Compare simultaneous selectors by identity, not spelling, even without
    // --repo. Keep single unscoped selectors on their ordinary resolution path.
    if !explicit_repository && (cli.task.is_none() || command_task.is_none()) {
        return Ok(());
    }
    let repo = if explicit_repository {
        Some(
            loopflow_execution::repository::current()?
                .ok_or_else(|| anyhow!("selected repository is unavailable"))?
                .to_string(),
        )
    } else {
        None
    };
    let Some(store) = read_registry()? else {
        return Ok(());
    };
    for selector in cli
        .task
        .as_mut()
        .into_iter()
        .chain(command_task)
        .chain(parent_task)
    {
        if let Some(id) = resolve_task_id(&store, selector, repo.as_deref())? {
            *selector = id.to_string();
        }
    }
    Ok(())
}

fn launch_task(cli: &Cli) -> Option<&str> {
    match &cli.command {
        Some(Commands::Desktop { .. } | Commands::Open(_))
        | Some(Commands::Task {
            cmd: TaskCommand::Location { .. },
        }) => None,
        Some(Commands::Task {
            cmd: TaskCommand::Run { issue, .. } | TaskCommand::Checkout { issue, .. },
        }) => issue.as_deref().or(cli.task.as_deref()),
        // `resume --task X` opens the Task's conversation where the Task runs.
        Some(Commands::Session {
            cmd:
                SessionCommand::Resume {
                    id: None,
                    connection,
                    ..
                },
        }) if connection.task.is_some() => connection.task.as_deref(),
        // Ownership is written only on the Task's writer.
        Some(Commands::Task {
            cmd:
                TaskCommand::Claim { issue, .. }
                | TaskCommand::Unassign { issue, .. }
                | TaskCommand::Pause { issue }
                | TaskCommand::Unpause { issue, .. },
        }) => Some(issue.as_str()),
        _ => cli.task.as_deref(),
    }
}

/// Bare `lf resume` in a Task's worktree acts on that Task, so it routes to
/// its Machine exactly as `--task` does.
fn worktree_task(cli: &Cli, directory: &Path) -> Result<Option<String>> {
    let bare = match &cli.command {
        Some(Commands::Session {
            cmd: SessionCommand::Resume { id: None, .. },
        }) => cli.wave.is_none() && !cli.selects_repository(),
        _ => false,
    };
    if !bare || cli.task.is_some() {
        return Ok(None);
    }
    let Some(store) = read_registry()? else {
        return Ok(None);
    };
    let cwd = directory
        .canonicalize()
        .unwrap_or_else(|_| directory.to_path_buf());
    Ok(store
        .task_checkouts()?
        .into_iter()
        .find(|checkout| {
            let worktree = checkout
                .worktree
                .canonicalize()
                .unwrap_or_else(|_| checkout.worktree.clone());
            cwd.starts_with(worktree)
        })
        .map(|checkout| checkout.task_id.to_string()))
}

/// A command `lf` sends to another Machine instead of running here.
#[derive(Debug)]
pub struct Forward {
    pub machine: MachineId,
    args: Vec<String>,
    pub(super) route: Option<TaskExecutionRoute>,
    /// The Wave a bare resume resolved here, which the destination opens.
    pub(super) wave: Option<String>,
    /// The repository the destination resolves its own checkout from.
    pub(super) repository: Option<RepositoryId>,
}

impl Forward {
    pub(super) fn run(
        self,
        cli: &Cli,
        directory: &Path,
        io: &mut loopflow_execution::command::CommandIo<'_>,
    ) -> Result<()> {
        // `directory` selected Work on this host. Its spelling is not a
        // destination path; the route carries the repository's portable ID.
        let mut args = Vec::new();
        let mut words = self.args.iter();
        while let Some(word) = words.next() {
            if word == "--" {
                args.push(word.clone());
                args.extend(words.cloned());
                break;
            }
            if word == "--cwd" {
                words.next();
            } else if !word.starts_with("--cwd=") {
                args.push(word.clone());
            }
        }
        super::ssh::run_with_io(
            self.machine.as_str(),
            false,
            cli,
            &args,
            self.route,
            directory,
            Some(io),
        )
    }
}

/// Return whether dispatch completed remotely. No Task Graph or checkout is
/// mutated before routing. Unknown tracker aliases are still fetched as usual.
pub fn dispatch(cli: &Cli, args: &[String]) -> Result<bool> {
    let Some(forward) = forward(cli, args, &std::env::current_dir()?)? else {
        return Ok(false);
    };
    let task_run = matches!(
        cli.command,
        Some(Commands::Task {
            cmd: TaskCommand::Run { .. }
        })
    );
    let result = super::ssh::run(
        forward.machine.as_str(),
        false,
        cli,
        &forward.args,
        forward.route,
    );
    if task_run {
        // The laptop waited for the run; a rented machine whose work drained goes back now.
        loopflow_execution::rental::recovery::settle(&forward.machine);
    }
    result?;
    Ok(true)
}

/// Where `lf <args>` run from `directory` goes instead of running here, if
/// anywhere. Reads records only; nothing is sent.
pub fn forward(cli: &Cli, args: &[String], directory: &Path) -> Result<Option<Forward>> {
    if let Some(forward) = moved_session(cli, args)? {
        return Ok(Some(forward));
    }
    let worktree_task = worktree_task(cli, directory)?;
    let Some(selector) = launch_task(cli).or(worktree_task.as_deref()) else {
        return primary_session(cli, args, directory);
    };
    let Some(store) = read_registry()? else {
        return Ok(None);
    };
    let Some(route) = resolve_task(&store, cli, selector, directory)? else {
        return Ok(None);
    };
    if route.machine_id == loopflow_execution::store::machines::local_machine(&store)?.id {
        return Ok(None);
    }
    let ownership = matches!(
        cli.command,
        Some(Commands::Task {
            cmd: TaskCommand::Claim { .. } | TaskCommand::Unassign { .. }
        })
    );
    // A move from here that has not arrived leaves the next entry here.
    if ownership && store.task_move_in_flight(&route.task_id)?.is_some() {
        return Ok(None);
    }
    // A command that arrived over SSH runs where it arrived.
    if std::env::var_os(loopflow_execution::machine::ssh::EXPECTED_MACHINE_ID_ENV).is_some() {
        return Err(anyhow!("Task {selector} resolves to Machine {} on the destination; reconcile its planning/location before retrying (no work was started)", route.machine_id));
    }
    let mut forwarded = args[1..].to_vec();
    if ownership {
        // Ownership needs no checkout on the writer; name the exact Task in place.
        match forwarded.iter().rposition(|arg| arg == selector) {
            Some(position) => forwarded[position] = route.task_id.to_string(),
            None => forwarded.extend(["--task".into(), route.task_id.to_string()]),
        }
        return Ok(Some(Forward {
            machine: route.machine_id.clone(),
            args: forwarded,
            route: Some(route),
            wave: None,
            repository: None,
        }));
    }
    // Carry the observed exact target in addition to the positional selector.
    // Destination resolution rejects an alias that now names different Work.
    let mut found = false;
    let mut i = 0;
    while i < forwarded.len() {
        if forwarded[i] == "--" {
            break;
        }
        if forwarded[i] == "--task" && i + 1 < forwarded.len() {
            forwarded[i + 1] = route.task_id.to_string();
            found = true;
            i += 1;
        } else if forwarded[i].starts_with("--task=") {
            forwarded[i] = format!("--task={}", route.task_id);
            found = true;
        }
        i += 1;
    }
    if !found {
        forwarded.splice(0..0, ["--task".into(), route.task_id.to_string()]);
    }
    if matches!(
        cli.command,
        Some(Commands::Task {
            cmd: TaskCommand::Run { .. }
        })
    ) {
        // Placement already chose this Machine; the destination runs in place.
        forwarded = without_placement(forwarded);
    }
    Ok(Some(Forward {
        machine: route.machine_id.clone(),
        args: forwarded,
        route: Some(route),
        wave: None,
        repository: None,
    }))
}

/// The Session an explicit `resume <session>` names.
pub(super) fn resumed_session(cli: &Cli) -> Option<&str> {
    match &cli.command {
        Some(Commands::Session {
            cmd: SessionCommand::Resume { id: Some(id), .. },
        }) => Some(id),
        _ => None,
    }
}

/// `lf resume --task X` names the Task rather than one of its Sessions.
pub(super) fn resumed_task(cli: &Cli) -> Option<&str> {
    match &cli.command {
        Some(Commands::Session {
            cmd:
                SessionCommand::Resume {
                    id: None,
                    connection,
                    ..
                },
        }) => connection.task.as_deref(),
        _ => None,
    }
}

/// A Session resumes where it is: a Task's conversation where its Task runs
/// (the ownership log), any other Session on the Machine it moved to.
fn moved_session(cli: &Cli, args: &[String]) -> Result<Option<Forward>> {
    let Some(id) = resumed_session(cli) else {
        return Ok(None);
    };
    // A command that arrived over SSH runs where it arrived.
    if std::env::var_os(loopflow_execution::machine::ssh::EXPECTED_MACHINE_ID_ENV).is_some() {
        return Ok(None);
    }
    let Some(store) = read_registry()? else {
        return Ok(None);
    };
    let id = loopflow_model::id::canonical_session(id);
    let machine = match conversation_task(&store, &id)? {
        Some(task) => {
            let runs_on =
                loopflow_execution::store::ownership::task_execution_route(&store, &task)?
                    .machine_id;
            (runs_on != loopflow_execution::store::machines::local_machine(&store)?.id)
                .then_some(runs_on)
        }
        None => store.session_moved_to(&id)?,
    };
    Ok(machine.map(|machine| Forward {
        machine,
        args: args[1..].to_vec(),
        route: None,
        wave: None,
        repository: None,
    }))
}

/// `lf task run`'s `--pool`, `--rent` and `--quote`, removed before `--`.
fn without_placement(args: Vec<String>) -> Vec<String> {
    let mut kept = Vec::with_capacity(args.len());
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--" {
            kept.push(arg);
            kept.extend(args);
            break;
        }
        match arg.as_str() {
            "--pool" | "--rent" => {
                args.next();
            }
            "--quote" => {}
            _ if arg.starts_with("--pool=") || arg.starts_with("--rent=") => {}
            _ => kept.push(arg),
        }
    }
    kept
}

/// A Wave's or repository's Session lives on your Session Machine for it. The
/// destination runs it wherever it arrives, even if its own copy differs. A
/// bare resume resolves its scope from the worktree exactly as the local
/// resume would, then routes the same way.
fn primary_session(cli: &Cli, args: &[String], directory: &Path) -> Result<Option<Forward>> {
    let (
        Some(Commands::Session {
            cmd: SessionCommand::Resume { id: None, .. },
        }),
        None,
    ) = (&cli.command, cli.task.as_deref())
    else {
        return Ok(None);
    };
    // A command that arrived over SSH runs where it arrived.
    if std::env::var_os(loopflow_execution::machine::ssh::EXPECTED_MACHINE_ID_ENV).is_some() {
        return Ok(None);
    }
    let Some(registry) = loopflow_execution::store::read_existing_registry()? else {
        return Ok(None);
    };
    let registry: loopflow_execution::store::SharedStore = std::sync::Arc::new(registry);
    let store = &registry.sqlite;
    let repo = loopflow_execution::repository::containing(directory)?.map(|repo| repo.to_string());
    let repository = match (&cli.repository, repo.as_deref()) {
        (Some(repository), _) => Some(repository.clone()),
        (None, Some(repo)) => store.repository_id(repo)?,
        (None, None) => None,
    };
    let wave = match cli.wave.clone() {
        Some(wave) => Some(wave),
        None if cli.selects_repository() => None,
        None => match context_scope(&registry, repo.as_deref(), directory)? {
            Some(ContextScope::Wave(name)) => Some(name),
            Some(ContextScope::Repository) => None,
            // A Task's Session follows its claim; unresolved context stays local.
            _ => return Ok(None),
        },
    };
    let (machine, scope) = match wave.as_deref() {
        Some(wave) => {
            let Some(wave_id) = store.resolve_wave_id(wave, repo.as_deref())? else {
                return Ok(None);
            };
            (
                loopflow_execution::store::placement::wave_owning_machine(
                    store,
                    &wave_id,
                    loopflow_model::durable::MachineSetting::Session,
                )?
                .0,
                format!("Wave {wave}"),
            )
        }
        None => {
            let Some(repository) = &repository else {
                return Ok(None);
            };
            (
                loopflow_execution::store::placement::repository_owning_machine(
                    store,
                    repository,
                    loopflow_model::durable::MachineSetting::Session,
                )?
                .0,
                "this repository".to_string(),
            )
        }
    };
    if machine == loopflow_execution::store::machines::local_machine(store)?.id {
        return Ok(None);
    }
    // The destination finds its own checkout by identity, never by this path.
    let mut forwarded = Vec::new();
    let mut iter = args[1..].iter();
    while let Some(arg) = iter.next() {
        if arg == "--repo" {
            iter.next();
        } else if !arg.starts_with("--repo=") {
            forwarded.push(arg.clone());
        }
    }
    if cli.wave.is_none() {
        if let Some(wave) = &wave {
            forwarded.splice(0..0, ["--wave".into(), wave.clone()]);
        }
    }
    let repository = repository
        .ok_or_else(|| anyhow!("{scope} runs on Machine {machine}; run `lf repo` here so it can address the repository"))?;
    if cli.repository.is_none() {
        forwarded.splice(0..0, ["--repo-id".into(), repository.to_string()]);
    }
    Ok(Some(Forward {
        machine,
        args: forwarded,
        route: None,
        wave,
        repository: Some(repository),
    }))
}

/// The scope a bare connect here opens, when it is a Wave or the repository.
#[derive(Debug)]
enum ContextScope {
    Wave(String),
    Repository,
}

/// Resolve as the local connect does. Task worktrees and unassociated ones are
/// `None`: a Task routes by its claim, and an error belongs to the local connect.
fn context_scope(
    store: &loopflow_execution::store::SharedStore,
    repo: Option<&str>,
    cwd: &Path,
) -> Result<Option<ContextScope>> {
    let Some(repo) = repo else {
        return Ok(None);
    };
    tokio::runtime::Runtime::new()?.block_on(async {
        use loopflow_execution::ops::human_session::{connection_scope, SessionTarget};
        use loopflow_execution::session::PrimaryScope;
        Ok(
            match connection_scope(store, cwd, SessionTarget::Context).await {
                Ok(PrimaryScope::Wave(id)) => store
                    .list_waves(Some(repo))
                    .await?
                    .into_iter()
                    .find(|wave| wave.id() == &id)
                    .map(|wave| ContextScope::Wave(wave.name().to_string())),
                Ok(PrimaryScope::Repository(_)) => Some(ContextScope::Repository),
                Ok(PrimaryScope::Task(_)) | Err(_) => None,
                Ok(_) => None,
            },
        )
    })
}

/// Inspect known Work only. Tracker fetches and plan exchange have their
/// own owners; neither is run merely to choose a launch destination.
pub(super) fn resolve(cli: &Cli) -> Result<Option<TaskExecutionRoute>> {
    let Some(selector) = launch_task(cli) else {
        return Ok(None);
    };
    let Some(store) = read_registry()? else {
        return Ok(None);
    };
    resolve_task(&store, cli, selector, &std::env::current_dir()?)
}

fn resolve_task(
    store: &SqliteStore,
    cli: &Cli,
    selector: &str,
    directory: &Path,
) -> Result<Option<TaskExecutionRoute>> {
    let repo =
        if let Some(id) = &cli.repository {
            Some(store.repository_path(id)?.ok_or_else(|| {
                anyhow!("repository plan {id} has no local checkout on this Machine")
            })?)
        } else if cli.repo.is_some() {
            Some(
                loopflow_execution::repository::containing(directory)?
                    .ok_or_else(|| anyhow!("selected repository is unavailable"))?
                    .to_string(),
            )
        } else {
            None
        };
    resolve_task_id(store, selector, repo.as_deref())?
        .map(|task| {
            loopflow_execution::store::ownership::task_execution_route(store, &task)
                .map_err(Into::into)
        })
        .transpose()
}

pub fn location(
    cli: &Cli,
    selector: &str,
    peers: bool,
    request: Option<&str>,
    json: bool,
) -> Result<()> {
    let store = read_registry()?
        .ok_or_else(|| anyhow!("execution location unavailable: local registry is absent"))?;
    // Desktop and peers already name the plan. Its execution reading does not
    // require a local checkout (including an unrecorded Task on this Machine).
    let (repository, repo) = if let Some(repository) = &cli.repository {
        (repository.clone(), store.repository_path(repository)?)
    } else {
        let repo = loopflow_execution::repository::current()?
            .ok_or_else(|| anyhow!("select a repository before reading execution location"))?
            .to_string();
        let repository = store
            .repository_id(&repo)?
            .ok_or_else(|| anyhow!("repository identity unavailable"))?;
        (repository, Some(repo))
    };
    // A full unknown Task ID remains an unavailable observation, not creation input.
    let task = TaskId::parse(selector)
        .ok()
        .or(resolve_task_id(&store, selector, repo.as_deref())?)
        .ok_or_else(|| anyhow!("Task {selector} is unavailable"))?;
    if let Some(global) = cli.task.as_deref() {
        anyhow::ensure!(
            global == selector || {
                let selected = resolve_task_id(&store, selector, repo.as_deref())?;
                selected.is_some() && selected == resolve_task_id(&store, global, repo.as_deref())?
            },
            "conflicting Task selections: {global} and {selector}"
        );
    }
    let request = request
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let readings = tokio::runtime::Runtime::new()?.block_on(
        loopflow_execution::ops::task_location::observe(
            &store,
            &repository,
            &task,
            peers,
            &request,
        ),
    )?;
    if json {
        println!("{}", serde_json::to_string_pretty(&readings)?);
    } else {
        println!(
            "{}",
            loopflow_execution::ops::task_location::render(&readings)
        );
    }
    Ok(())
}

/// Scope aliases and prefixes before routing, just as entry dispatch does.
/// An unknown selector may still be acquired; known Work in another repo may not.
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{launch_task, repository_path, resolve, resolve_task, without_placement};
    use crate::lf::Cli;

    use clap::Parser;
    use loopflow_model::durable::{RepositoryId, TaskId};
    use loopflow_storage::work::task::NewTask;

    #[test]
    fn routing_resolves_task_prefixes_within_historical_repository_locators() {
        let home = tempfile::tempdir().unwrap();
        let store =
            loopflow_execution::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let first = TaskId::parse("task_abcd1234400080000000000000000001").unwrap();
        let second = TaskId::parse("task_abcd1235400080000000000000000002").unwrap();
        let selected_path = home.path().join("selected");
        let other_path = home.path().join("other");
        std::fs::create_dir(&selected_path).unwrap();
        std::fs::create_dir(&other_path).unwrap();
        let selected_path = selected_path.canonicalize().unwrap();
        let other_path = other_path.canonicalize().unwrap();
        let selected_repo = selected_path.to_str().unwrap();
        for (repo, id) in [
            (selected_repo, &first),
            (other_path.to_str().unwrap(), &second),
        ] {
            let project = loopflow_execution::work::wave::bootstrap::ensure_wave_project(
                &store, repo, "inbox",
            )
            .unwrap();
            store
                .create_task(&NewTask {
                    parent_task_id: None,
                    repository_id: loopflow_execution::repository::location::ensure_repository(
                        &store, repo,
                    )
                    .unwrap(),
                    due_date: None,
                    id: id.clone(),
                    project_id: Some(project.id),
                    title: repo.into(),
                    description: String::new(),
                })
                .unwrap();
        }
        let prior = store.repository_id(selected_repo).unwrap().unwrap();
        let selected = RepositoryId::new();
        loopflow_execution::repository::location::bind_repository(&store, selected_repo, &selected)
            .unwrap();
        let cli = Cli::try_parse_from(["lf", "--repo-id", prior.as_str()]).unwrap();
        assert_eq!(
            resolve_task(&store, &cli, "abcd", Path::new("/")).unwrap(),
            Some(
                loopflow_execution::store::ownership::task_execution_route(&store, &first).unwrap()
            )
        );
        assert_eq!(
            resolve_task(&store, &cli, first.as_str(), Path::new("/"))
                .unwrap()
                .unwrap()
                .repository_id,
            selected
        );
        assert!(resolve_task(&store, &cli, second.as_str(), Path::new("/"))
            .unwrap_err()
            .to_string()
            .contains("does not belong"));
        assert!(resolve_task(&store, &cli, "UNKNOWN-1", Path::new("/"))
            .unwrap()
            .is_none());
        let unbound = RepositoryId::new();
        let cli = Cli::try_parse_from(["lf", "--repo-id", unbound.as_str()]).unwrap();
        assert!(resolve_task(&store, &cli, first.as_str(), Path::new("/"))
            .unwrap_err()
            .to_string()
            .contains("no local checkout"));
    }

    #[test]
    fn destination_lookup_preserves_missing_and_unreadable_registry() {
        let _lock = loopflow_execution::journal::test_env_lock();
        let _restore = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let path = home.path().join("loopflow.db");
        let cli = Cli::try_parse_from(["lf", "--task", "UNKNOWN-1"]).unwrap();
        let id = RepositoryId::new();
        assert!(resolve(&cli).unwrap().is_none());
        assert!(repository_path(&id).is_err());
        assert!(!path.exists());

        std::fs::write(&path, b"unreadable registry").unwrap();
        assert!(resolve(&cli).is_err());
        assert!(repository_path(&id).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"unreadable registry");
    }

    #[test]
    fn repository_lookup_does_not_register_an_unselected_plan() {
        let _lock = loopflow_execution::journal::test_env_lock();
        let _restore = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let store =
            loopflow_execution::store::opening::open_ephemeral(&home.path().join("loopflow.db"))
                .unwrap();
        let id = loopflow_execution::repository::location::ensure_repository(
            &store,
            "/selected/repository",
        )
        .unwrap();
        assert_eq!(
            repository_path(&id).unwrap(),
            std::path::Path::new("/selected/repository")
        );
        let absent = RepositoryId::new();
        assert!(repository_path(&absent).is_err());
        assert!(store.repository_path(&absent).unwrap().is_none());
    }

    #[test]
    fn presentation_commands_do_not_route_to_the_execution_machine() {
        for argv in [["desktop", "state"].as_slice(), ["open"].as_slice()] {
            let cli = Cli::try_parse_from(["lf", "--task", "LOO-427"].iter().chain(argv)).unwrap();
            assert_eq!(launch_task(&cli), None);
        }
    }

    #[test]
    fn a_placed_run_arrives_without_placement_flags() {
        let args = |list: &[&str]| list.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
        assert_eq!(
            without_placement(args(&[
                "--task",
                "task_1",
                "task",
                "run",
                "LOO-570",
                "--pool",
                "lab",
                "--rent=code",
                "--quote",
                "--",
                "--rent",
                "x"
            ])),
            args(&["--task", "task_1", "task", "run", "LOO-570", "--", "--rent", "x"])
        );
    }
}
