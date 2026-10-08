//! Work chooses execution; SSH transports the ordinary command unchanged.
use crate::durable::RepositoryId;
use crate::lf::{Cli, Commands, TaskCommand};
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
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = crate::store::open_store(&crate::store::storage_config_from_env()?).await?;
        let path = store.repository_path(id).await?
            .ok_or_else(|| anyhow!("repository plan {id} is not selected on this Machine; select it through planning sync before running its Work"))?;
        Ok(path.into())
    })
}

fn launch_task(cli: &Cli) -> Option<&str> {
    match &cli.command {
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
    let runtime = tokio::runtime::Runtime::new()?;
    let route = runtime.block_on(async {
        let Some(route) = resolve(cli).await? else {
            return Ok(None);
        };
        let store = crate::store::open_store(&crate::store::storage_config_from_env()?).await?;
        Ok::<_, anyhow::Error>(
            (route.machine_id != store.local_machine().await?.id).then_some(route),
        )
    })?;
    let Some(route) = route else { return Ok(false) };
    if std::env::var_os(super::ssh::EXPECTED_MACHINE_ID_ENV).is_some() {
        return Err(anyhow!("Task {selector} resolves to Machine {} on the destination; reconcile its planning/location before retrying (no work was started)", route.machine_id));
    }
    super::ssh::run_in_repository(
        route.machine_id.as_str(),
        false,
        &args[1..],
        Some(&route.repository_id),
    )?;
    Ok(true)
}

/// Inspect known Work only. Provider acquisition and plan exchange have their
/// own owners; neither is run merely to choose a launch destination.
pub(super) async fn resolve(cli: &Cli) -> Result<Option<crate::durable::TaskExecutionRoute>> {
    let Some(selector) = launch_task(cli) else {
        return Ok(None);
    };
    let config = crate::store::storage_config_from_env()?;
    let crate::store::StorageConfig::Sqlite { path } = &config;
    if !path.exists() {
        return Ok(None);
    }
    let store = crate::store::open_store(&config).await?;
    let Some(task) = store.get_task_by_issue(selector).await? else {
        return Ok(None);
    };
    let route = store.task_execution_route(&task.id).await?;
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
