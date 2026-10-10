//! Work chooses execution; SSH transports the ordinary command unchanged.
use crate::durable::{RepositoryId, TaskExecutionRoute, TaskId};
use crate::lf::{Cli, Commands, TaskCommand};
use crate::store::sqlite::SqliteStore;
use anyhow::{anyhow, Result};

pub fn identity(bind: Option<&RepositoryId>, json: bool) -> Result<()> {
    let repo = crate::repository::CanonicalRepo::current()?
        .ok_or_else(|| anyhow!("run `lf repo identity` in a repository"))?;
    let runtime = tokio::runtime::Runtime::new()?;
    let identity = runtime.block_on(async {
        let store = crate::store::open_store(&crate::store::storage_config_from_env()?).await?;
        if let Some(id) = bind {
            store.sqlite.bind_repository(&repo.to_string(), id)?;
        }
        store.ensure_repository(&repo.to_string()).await?;
        store
            .sqlite
            .repository_identity(&repo.to_string())?
            .ok_or_else(|| anyhow!("repository identity disappeared after registration"))
    })?;
    if json {
        println!("{}", serde_json::to_string(&identity)?);
    } else {
        println!("{}", identity.id);
    }
    Ok(())
}

pub fn repository_path(id: &RepositoryId) -> Result<std::path::PathBuf> {
    let path = read_registry()?
        .map(|store| store.repository_path(id))
        .transpose()?
        .flatten()
        .ok_or_else(|| anyhow!("repository plan {id} has no local checkout; run `lf repo identity --bind {id}` in its repository on this Machine"))?;
    Ok(path.into())
}

/// Routing observes existing records; initialization and schema upgrades belong
/// to the operation that takes up Work, not destination lookup.
fn read_registry() -> Result<Option<SqliteStore>> {
    Ok(crate::store::read_existing_registry()?.map(|store| store.sqlite))
}

/// Resolve scoped selectors once, carrying exact IDs to downstream consumers.
/// Unknown Tasks remain with ordinary repository-scoped planning acquisition.
pub fn resolve_repository_selection(cli: &mut Cli) -> Result<()> {
    if matches!(
        cli.command,
        Some(Commands::Task {
            cmd: TaskCommand::Location { .. }
        })
    ) {
        return Ok(()); // Keep the requested portable ID in the reply envelope.
    }
    let explicit_repository = cli.repo.is_some() || cli.repository.is_some();
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
            crate::repository::CanonicalRepo::current()?
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
        Some(Commands::Desktop { .. })
        | Some(Commands::Task {
            cmd: TaskCommand::Location { .. },
        }) => None,
        Some(Commands::Task {
            cmd: TaskCommand::Run { issue, .. } | TaskCommand::Checkout { issue, .. },
        }) => issue.as_deref().or(cli.task.as_deref()),
        _ => cli.task.as_deref(),
    }
}

/// Return whether dispatch completed remotely. No Workflow or checkout is
/// mutated before routing. Unknown provider aliases retain ordinary acquisition.
pub fn dispatch(cli: &Cli, args: &[String]) -> Result<bool> {
    let Some(selector) = launch_task(cli) else {
        return Ok(false);
    };
    let Some(store) = read_registry()? else {
        return Ok(false);
    };
    let Some(route) = resolve_task(&store, cli, selector)? else {
        return Ok(false);
    };
    if route.machine_id == store.local_machine()?.id {
        return Ok(false);
    }
    if std::env::var_os(super::ssh::EXPECTED_MACHINE_ID_ENV).is_some() {
        return Err(anyhow!("Task {selector} resolves to Machine {} on the destination; reconcile its planning/location before retrying (no work was started)", route.machine_id));
    }
    let machine = route.machine_id.clone();
    let mut forwarded = args[1..].to_vec();
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
    super::ssh::run(machine.as_str(), false, cli, &forwarded, Some(route))?;
    Ok(true)
}

/// Inspect known Work only. Provider acquisition and plan exchange have their
/// own owners; neither is run merely to choose a launch destination.
pub(super) fn resolve(cli: &Cli) -> Result<Option<TaskExecutionRoute>> {
    let Some(selector) = launch_task(cli) else {
        return Ok(None);
    };
    let Some(store) = read_registry()? else {
        return Ok(None);
    };
    resolve_task(&store, cli, selector)
}

fn resolve_task(
    store: &SqliteStore,
    cli: &Cli,
    selector: &str,
) -> Result<Option<TaskExecutionRoute>> {
    let repo =
        if let Some(id) = &cli.repository {
            Some(store.repository_path(id)?.ok_or_else(|| {
                anyhow!("repository plan {id} has no local checkout on this Machine")
            })?)
        } else if cli.repo.is_some() {
            Some(
                crate::repository::CanonicalRepo::current()?
                    .ok_or_else(|| anyhow!("selected repository is unavailable"))?
                    .to_string(),
            )
        } else {
            None
        };
    resolve_task_id(store, selector, repo.as_deref())?
        .map(|task| {
            tokio::runtime::Runtime::new()?
                .block_on(crate::ops::task_location::resolve(store, &task))
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
    let repo = crate::repository::CanonicalRepo::current()?
        .ok_or_else(|| anyhow!("select a repository before reading execution location"))?;
    let repository = cli
        .repository
        .clone()
        .or(store.repository_id(&repo.to_string())?)
        .ok_or_else(|| anyhow!("repository identity unavailable"))?;
    // A full unknown Task ID remains an unavailable observation, not creation input.
    let task = TaskId::parse(selector)
        .ok()
        .or(resolve_task_id(&store, selector, Some(&repo.to_string()))?)
        .ok_or_else(|| anyhow!("Task {selector} is unavailable"))?;
    if let Some(global) = cli.task.as_deref() {
        anyhow::ensure!(
            global == selector || {
                let selected = resolve_task_id(&store, selector, Some(&repo.to_string()))?;
                selected.is_some()
                    && selected == resolve_task_id(&store, global, Some(&repo.to_string()))?
            },
            "conflicting Task selections: {global} and {selector}"
        );
    }
    let request = request
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let readings = tokio::runtime::Runtime::new()?.block_on(crate::ops::task_location::observe(
        &store,
        &repository,
        &task,
        peers,
        &request,
    ))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&readings)?);
    } else {
        println!("{}", crate::ops::task_location::render(&readings));
    }
    Ok(())
}

/// Scope aliases and prefixes before routing, just as entry dispatch does.
/// An unknown selector may still be acquired; known Work in another repo may not.
fn resolve_task_id(
    store: &SqliteStore,
    selector: &str,
    repo: Option<&str>,
) -> Result<Option<TaskId>> {
    let task = store.resolve_task_id(selector, repo)?;
    if let Some(repo) = repo.filter(|_| task.is_none()) {
        anyhow::ensure!(
            store.resolve_task_id(selector, None)?.is_none(),
            "Task {selector} does not belong to selected repository {repo}"
        );
    }
    Ok(task)
}

#[cfg(test)]
mod tests {
    use super::{launch_task, repository_path, resolve, resolve_task};
    use crate::durable::{RepositoryId, TaskId};
    use crate::lf::Cli;
    use crate::planning::NewTask;
    use crate::store::sqlite::SqliteStore;
    use clap::Parser;

    #[test]
    fn routing_resolves_task_prefixes_within_historical_repository_locators() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let first = TaskId::parse("task_abcd1234400080000000000000000001").unwrap();
        let second = TaskId::parse("task_abcd1235400080000000000000000002").unwrap();
        for (repo, id) in [("/selected", &first), ("/other", &second)] {
            let project = store.ensure_wave_project(repo, "inbox").unwrap();
            store
                .create_task(&NewTask {
                    due_date: None,
                    id: id.clone(),
                    project_id: project.id,
                    title: repo.into(),
                    description: String::new(),
                })
                .unwrap();
        }
        let prior = store.repository_id("/selected").unwrap().unwrap();
        let selected = RepositoryId::new();
        store.bind_repository("/selected", &selected).unwrap();
        let cli = Cli::try_parse_from(["lf", "--repository", prior.as_str()]).unwrap();
        assert_eq!(
            resolve_task(&store, &cli, "abcd").unwrap(),
            Some(store.task_execution_route(&first).unwrap())
        );
        assert_eq!(
            resolve_task(&store, &cli, first.as_str())
                .unwrap()
                .unwrap()
                .repository_id,
            selected
        );
        assert!(resolve_task(&store, &cli, second.as_str())
            .unwrap_err()
            .to_string()
            .contains("does not belong"));
        assert!(resolve_task(&store, &cli, "UNKNOWN-1").unwrap().is_none());
        let unbound = RepositoryId::new();
        let cli = Cli::try_parse_from(["lf", "--repository", unbound.as_str()]).unwrap();
        assert!(resolve_task(&store, &cli, first.as_str())
            .unwrap_err()
            .to_string()
            .contains("no local checkout"));
    }

    #[test]
    fn destination_lookup_preserves_missing_and_unreadable_registry() {
        let _lock = crate::journal::test_env_lock();
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
        let _lock = crate::journal::test_env_lock();
        let _restore = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let id = store.ensure_repository("/selected/repository").unwrap();
        assert_eq!(
            repository_path(&id).unwrap(),
            std::path::Path::new("/selected/repository")
        );
        let absent = RepositoryId::new();
        assert!(repository_path(&absent).is_err());
        assert!(store.repository_path(&absent).unwrap().is_none());
    }

    #[test]
    fn desktop_commands_do_not_route_to_the_execution_machine() {
        for operation in ["open", "list"] {
            let cli =
                Cli::try_parse_from(["lf", "--task", "LOO-427", "desktop", operation]).unwrap();
            assert_eq!(launch_task(&cli), None);
        }
    }
}
