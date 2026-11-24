//! Task routing and what a command or operation sent over SSH carries.
use crate::durable::TaskExecutionRoute;
use crate::ops::task::remote::TaskSource;
use anyhow::{anyhow, Result};
use loopflow_model::durable::TaskId;
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
        if let Err(error) = crate::ops::plan_carriers::send(&store, &repo.to_string_lossy()).await {
            tracing::warn!(error = %format!("{error:#}"), "plan send pending; remote resolution uses this Machine's plan");
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

impl crate::machine::wire::Selection {
    /// What this `lf` runs with in `directory`, for a request it sends to
    /// another Machine.
    pub fn capture(directory: &Path, source: Option<TaskSource>) -> Result<Self> {
        let origin =
            tokio::runtime::Runtime::new()?.block_on(crate::machine::connection::local_machine())?;
        Ok(Self {
            profile: crate::config::remote::outgoing(Some(directory), origin.id.as_str())?,
            participant: crate::config::participant_name_in(directory)?,
            agent: std::env::var(crate::config::AGENT_OVERRIDE_ENV).ok(),
            settings: std::env::var("LF_SETTINGS").ok(),
            source,
        })
    }

    /// The environment a launch it starts here runs with, `secrets` included.
    pub fn environment(
        &self,
        secrets: &crate::machine::wire::Secrets,
    ) -> Result<Vec<(String, String)>> {
        use secrecy::ExposeSecret;
        let mut environment = vec![(
            crate::config::remote::PROFILE_ENV.into(),
            serde_json::to_string(&self.profile)?,
        )];
        let named = [
            (crate::config::USER_NAME_ENV, self.participant.clone()),
            (crate::config::AGENT_OVERRIDE_ENV, self.agent.clone()),
            ("LF_SETTINGS", self.settings.clone()),
            (
                crate::ops::task::TASK_SOURCE_ENV,
                self.source
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
            ),
        ];
        for (name, value) in named {
            if let Some(value) = value {
                environment.push((name.into(), value));
            }
        }
        for (name, value) in &secrets.0 {
            environment.push((name.clone(), value.expose_secret().clone()));
        }
        Ok(environment)
    }
}

/// The route of the Task `selector` names, when its Work lives on another
/// Machine. Reads records only.
pub fn remote_task(selector: &str, directory: Option<&Path>) -> Result<Option<TaskExecutionRoute>> {
    let Some(store) = read_registry()? else {
        return Ok(None);
    };
    remote_task_in(&store, selector, directory)
}

fn remote_task_in(
    store: &SqliteStore,
    selector: &str,
    directory: Option<&Path>,
) -> Result<Option<TaskExecutionRoute>> {
    // Short IDs and tracker labels can name different Tasks in different
    // repositories. A client outside a repository can still address a Task.
    let scoped = match directory.filter(|_| TaskId::parse(selector).is_err()) {
        Some(path) => {
            crate::store::children::resolve_task_id(store, selector, Some(&path.to_string_lossy()))?
        }
        None => None,
    };
    let task = match scoped {
        Some(task) => task,
        None => match resolve_task_id(store, selector, None)? {
            Some(task) => task,
            None => return Ok(None),
        },
    };
    let route = crate::store::ownership::task_execution_route(store, &task)?;
    Ok((route.machine_id != crate::store::machines::local_machine(store)?.id).then_some(route))
}

/// The Task's route to `machine`, wherever the Task runs now; `None` when
/// `machine` is this one or the Task is unknown here. A shell stays on the
/// Machine that started it.
pub fn task_on(
    selector: &str,
    machine: &loopflow_model::durable::MachineId,
) -> Result<Option<TaskExecutionRoute>> {
    let Some(store) = read_registry()? else {
        return Ok(None);
    };
    if *machine == crate::store::machines::local_machine(&store)?.id {
        return Ok(None);
    }
    let Some(task) = resolve_task_id(&store, selector, None)? else {
        return Ok(None);
    };
    let mut route = crate::store::ownership::task_execution_route(&store, &task)?;
    route.machine_id = machine.clone();
    Ok(Some(route))
}

/// The other Machine that writes the Task's ownership: where it runs, unless
/// a move from here has not arrived, which leaves the next entry here. A sent
/// move's target is asked first: one that took the Task in, its answer lost,
/// writes it like any Task that runs there.
pub fn remote_ownership_writer(
    selector: &str,
    directory: Option<&Path>,
) -> Result<Option<TaskExecutionRoute>> {
    let Some(store) = read_registry()? else {
        return Ok(None);
    };
    let Some(route) = remote_task_in(&store, selector, directory)? else {
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
