//! Work chooses execution; SSH transports the ordinary command unchanged.
use crate::durable::{RepositoryId, TaskExecutionRoute};
use crate::lf::{Cli, Commands, TaskCommand};
use crate::store::sqlite::SqliteStore;
use anyhow::{anyhow, Result};

pub fn identity(json: bool) -> Result<()> {
    let repo = crate::repository::CanonicalRepo::current()?
        .ok_or_else(|| anyhow!("run `lf repo identity` in a repository"))?;
    let runtime = tokio::runtime::Runtime::new()?;
    let id = runtime.block_on(async {
        let store = crate::store::open_store(&crate::store::storage_config_from_env()?).await?;
        Ok::<_, anyhow::Error>(store.ensure_repository(&repo.to_string()).await?)
    })?;
    if json {
        println!("{}", serde_json::to_string(&id)?);
    } else {
        println!("{id}");
    }
    Ok(())
}

pub fn repository_path(id: &RepositoryId) -> Result<std::path::PathBuf> {
    let path = read_registry()?
        .map(|store| store.repository_path(id))
        .transpose()?
        .flatten()
        .ok_or_else(|| anyhow!("repository plan {id} is not selected on this Machine; select it through planning sync before running its Work"))?;
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
    if cli.repo.is_none() && cli.repository.is_none() {
        return Ok(());
    }
    let (command_task, parent_task) = match &mut cli.command {
        Some(Commands::Task {
            cmd:
                TaskCommand::Run {
                    issue, stack_on, ..
                }
                | TaskCommand::Checkout {
                    issue, stack_on, ..
                },
        }) => (Some(issue), stack_on.as_mut()),
        Some(Commands::Task { cmd }) => (cmd.selector_mut(), None),
        Some(Commands::Context { task, .. }) => (task.as_mut(), None),
        _ => (None, None),
    };
    if cli.task.is_none() && command_task.is_none() {
        return Ok(());
    }
    let repo = crate::repository::CanonicalRepo::current()?
        .ok_or_else(|| anyhow!("selected repository is unavailable"))?;
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
        if let Some(id) = store.resolve_task_id(selector, Some(&repo.to_string()))? {
            *selector = id.to_string();
        } else if store.resolve_task_id(selector, None)?.is_some() {
            return Err(anyhow!(
                "Task {selector} does not belong to selected repository {repo}"
            ));
        }
    }
    Ok(())
}

fn launch_task(cli: &Cli) -> Option<&str> {
    match &cli.command {
        Some(Commands::Desktop {
            cmd: crate::lf::DesktopCommand::Open,
        }) => cli.task.as_deref(),
        Some(Commands::Desktop { .. }) => None,
        Some(Commands::Task {
            cmd: TaskCommand::Run { issue, .. } | TaskCommand::Checkout { issue, .. },
        }) => Some(issue),
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
    super::ssh::run(machine.as_str(), false, &args[1..], Some(route))?;
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
    let repo = cli
        .repo
        .as_ref()
        .map(|_| crate::repository::CanonicalRepo::current())
        .transpose()?
        .flatten();
    let repo = repo.map(|repo| repo.to_string());
    let Some(task) = store.resolve_task_id(selector, repo.as_deref())? else {
        return Ok(None);
    };
    let route = store.task_execution_route(&task)?;
    if cli
        .repository
        .as_ref()
        .is_some_and(|id| id != &route.repository_id)
    {
        return Err(anyhow!(
            "Task {selector} does not belong to the selected repository plan"
        ));
    }
    Ok(Some(route))
}

#[cfg(test)]
mod tests {
    use super::{launch_task, repository_path, resolve};
    use crate::durable::RepositoryId;
    use crate::lf::Cli;
    use crate::store::sqlite::SqliteStore;
    use clap::Parser;

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
    fn desktop_open_keeps_work_routing_but_pane_reads_do_not_prepare_work() {
        for (operation, expected) in [("open", Some("LOO-427")), ("list", None)] {
            let cli =
                Cli::try_parse_from(["lf", "--task", "LOO-427", "desktop", operation]).unwrap();
            assert_eq!(launch_task(&cli), expected);
        }
    }
}
