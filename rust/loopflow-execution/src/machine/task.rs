//! Task routing and preparation of ordinary commands sent over SSH.
use crate::durable::TaskExecutionRoute;
use crate::ops::task::remote::TaskSource;
use anyhow::{anyhow, Result};
use loopflow_model::durable::{Machine, TaskId};
use loopflow_storage::sqlite::SqliteStore;
use std::path::Path;

pub fn resident_args(
    args: &[String],
    task: Option<&String>,
    path_selected: bool,
    routed: bool,
    source: Option<&TaskSource>,
) -> Vec<String> {
    // Automatic Work routing carries the resolved plan, not a path on this Machine.
    let task_flag = task.map(|task| format!("--task={task}"));
    let mut remove_repo = routed && path_selected;
    let mut result = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--" {
            result.push(arg.clone());
            result.extend(iter.cloned());
            break;
        }
        if remove_repo && arg == "--repo" {
            iter.next();
            remove_repo = false;
        } else if remove_repo && arg.starts_with("--repo=") {
            remove_repo = false;
        } else if let Some(source) = source.filter(|_| {
            (arg == "--task" && iter.as_slice().first() == task) || task_flag.as_ref() == Some(arg)
        }) {
            // Linear mappings resolve independently retained Task IDs on the target.
            if arg == "--task" {
                iter.next();
            }
            result.push(format!(
                "--task={}",
                source
                    .issue_id
                    .as_deref()
                    .unwrap_or(source.task_id.as_str())
            ));
        } else {
            result.push(arg.clone());
        }
    }
    result
}

pub async fn resolve_task_source(
    selector: Option<&str>,
    directory: &std::path::Path,
) -> anyhow::Result<Option<TaskSource>> {
    let Some(selector) = selector else {
        return Ok(None);
    };
    let Some(store) = crate::store::open_existing_store().await? else {
        return Ok(None);
    };
    // Validate pushed code before any planning I/O can hide missing source work.
    let source = TaskSource::resolve(&store, selector).await?;
    let repo = match store.get_task_by_issue(selector).await? {
        Some(task) => Some(crate::ops::task::repository_for_task(&store, &task)?),
        None => crate::repo::discover_repo_root(directory).ok().flatten(),
    };
    if let Some(repo) = repo {
        if crate::ops::tracker::connected(&repo.to_string_lossy()) {
            if let Some(task) = store.get_task_by_issue(selector).await? {
                let owner = loopflow_storage::sqlite::tracker_actions::ActionOwner::Task(task.id);
                if let Err(error) = crate::ops::tracker::drain::drain_repository(
                    &store,
                    &repo.to_string_lossy(),
                    crate::ops::tracker::drain::Scope::First(&owner),
                    crate::ops::tracker::drain::Bound::PASS,
                )
                .await
                {
                    tracing::warn!(%error,"Linear publication pending; remote resolution uses retained planning");
                }
            }
            let source = TaskSource::resolve(&store, selector).await?;
            if source
                .as_ref()
                .is_some_and(|source| source.issue_id.is_none())
            {
                return Err(anyhow!("Task {selector} is saved locally but has not reached Linear; retry after planning synchronization"));
            }
            return Ok(source);
        }
        if let Err(error) =
            crate::ops::planning_sync::publish_repository(&store, &repo.to_string_lossy()).await
        {
            tracing::warn!(%error,"Git planning publication pending; remote resolution uses retained planning");
        }
    }

    Ok(source)
}

/// The caller's repository by identity; `None` outside a repository or before
/// `lf repo`, when the destination runs in its home directory.
pub fn current_repository(
    directory: &std::path::Path,
) -> anyhow::Result<Option<loopflow_model::durable::RepositoryId>> {
    let Some(repo) = crate::repository::containing(directory)? else {
        return Ok(None);
    };
    let Some(store) = crate::store::read_existing_registry()? else {
        return Ok(None);
    };
    Ok(store.sqlite.repository_id(&repo.to_string())?)
}

pub fn forwarding_environment(
    directory: &Path,
    target: &Machine,
    source: Option<&TaskSource>,
) -> Result<Vec<(String, String)>> {
    let user_name = crate::config::participant_name_in(directory)?.unwrap_or_default();
    let origin =
        tokio::runtime::Runtime::new()?.block_on(crate::machine::connection::local_machine())?;
    let profile = crate::config::remote::outgoing(Some(directory), origin.id.as_str())?;
    let mut environment = vec![
        (
            crate::config::remote::PROFILE_ENV.into(),
            serde_json::to_string(&profile)?,
        ),
        (
            crate::machine::ssh::EXPECTED_MACHINE_ID_ENV.into(),
            target.id.to_string(),
        ),
        (crate::config::USER_NAME_ENV.into(), user_name),
    ];
    for name in [crate::config::AGENT_OVERRIDE_ENV, "LF_SETTINGS"] {
        if let Ok(value) = std::env::var(name) {
            environment.push((name.into(), value));
        }
    }
    if let Some(source) = source {
        environment.push((
            crate::ops::task::TASK_SOURCE_ENV.into(),
            serde_json::to_string(source)?,
        ));
    }
    Ok(environment)
}

/// The route of the Task `selector` names, when its Work lives on another
/// Machine. Reads records only.
pub fn remote_task(selector: &str) -> Result<Option<TaskExecutionRoute>> {
    let Some(store) = read_registry()? else {
        return Ok(None);
    };
    remote_task_in(&store, selector)
}

fn remote_task_in(store: &SqliteStore, selector: &str) -> Result<Option<TaskExecutionRoute>> {
    let Some(task) = resolve_task_id(store, selector, None)? else {
        return Ok(None);
    };
    let route = crate::store::ownership::task_execution_route(store, &task)?;
    Ok((route.machine_id != crate::store::machines::local_machine(store)?.id).then_some(route))
}

/// The other Machine that writes the Task's ownership: where it runs, unless
/// a move from here has not arrived, which leaves the next entry here. A sent
/// move's target is asked first: one that took the Task in, its answer lost,
/// writes it like any Task that runs there.
pub fn remote_ownership_writer(selector: &str) -> Result<Option<TaskExecutionRoute>> {
    let Some(store) = read_registry()? else {
        return Ok(None);
    };
    let Some(route) = remote_task_in(&store, selector)? else {
        return Ok(None);
    };
    let Some(moving) = store.task_move_in_flight(&route.task_id)? else {
        return Ok(Some(route));
    };
    let sent = matches!(
        moving.stage,
        loopflow_storage::sqlite::task_moves::MoveStage::Sent { .. }
    );
    let arrived =
        sent && crate::ops::session_move::arrived_on(&route.task_id, &moving.to_machine_id)?;
    Ok(arrived.then_some(route))
}

pub fn resolve_task_id(
    store: &SqliteStore,
    selector: &str,
    repo: Option<&str>,
) -> Result<Option<TaskId>> {
    let task = crate::store::children::resolve_task_id(store, selector, repo)?;
    if let Some(repo) = repo.filter(|_| task.is_none()) {
        anyhow::ensure!(
            crate::store::children::resolve_task_id(store, selector, None)?.is_none(),
            "Task {selector} does not belong to selected repository {repo}"
        );
    }
    Ok(task)
}

pub fn read_registry() -> Result<Option<SqliteStore>> {
    Ok(crate::store::read_existing_registry()?.map(|store| store.sqlite))
}

/// The typed Task operation being forwarded; command parsing stays at the adapter.
#[derive(Debug, Clone, Copy)]
pub enum RemoteTaskOperation<'a> {
    Run {
        agent: Option<&'a str>,
        approve: bool,
    },
    Ship,
    Hold,
    Ownership,
}

/// The JSON value an ordinary Task command returns on its owner Machine.
pub fn run_remote(
    route: TaskExecutionRoute,
    directory: &Path,
    args: &[String],
    operation: RemoteTaskOperation<'_>,
) -> Result<serde_json::Value> {
    let forwarded = matches!(operation, RemoteTaskOperation::Run { .. })
        .then(|| crate::machine::forward::read(directory))
        .transpose()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let resident = matches!(operation, RemoteTaskOperation::Ownership);
    let task = matches!(operation, RemoteTaskOperation::Ship).then(|| route.task_id.to_string());
    let source = if resident {
        None
    } else {
        runtime.block_on(resolve_task_source(task.as_deref(), directory))?
    };
    let target = if resident {
        let store = crate::store::read_existing_registry()?
            .ok_or_else(|| anyhow!("Machine unavailable: local registry is absent"))?;
        runtime.block_on(crate::machine::connection::find_machine(
            &store,
            route.machine_id.as_str(),
        ))?
    } else {
        runtime.block_on(crate::machine::connection::resolve_target(
            route.machine_id.as_str(),
            false,
        ))?
    };
    let environment = forwarding_environment(directory, &target, source.as_ref())?;
    let mut command = vec![
        "lf".into(),
        "--repo-id".into(),
        route.repository_id.to_string(),
    ];
    command.extend(resident_args(
        args,
        task.as_ref(),
        false,
        true,
        source.as_ref(),
    ));
    let environment: Vec<_> = environment
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let mut preamble = crate::machine::ssh::build_preamble(&command, &environment);
    if let Some(forwarded) = &forwarded {
        let mut notices = Vec::new();
        preamble = crate::machine::forward::deliver(&target, forwarded, &preamble, &mut notices)?;
        for line in String::from_utf8_lossy(&notices).lines() {
            crate::journal::request::diagnostic(format_args!("{line}"));
        }
    }
    let (mut output, mut error) = (Vec::new(), Vec::new());
    let ran = crate::machine::ssh::run_ssh_io(
        &target.route,
        false,
        &preamble,
        false,
        &mut crate::command::CommandIo {
            input: &mut std::io::empty(),
            output: &mut output,
            error: &mut error,
        },
    );
    let diagnostics = String::from_utf8_lossy(&error);
    if let Err(failed) = ran {
        return Err(match diagnostics.trim() {
            "" => failed,
            said => failed.context(format!("the destination said: {said}")),
        });
    }
    for line in diagnostics.lines() {
        crate::journal::request::diagnostic(format_args!("{line}"));
    }
    Ok(serde_json::from_slice(&output)?)
}
