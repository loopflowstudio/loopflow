//! `lf machine use`: run work on one of your Machines. A Task moves by
//! handoff; a Wave or repository default is personal, stored on this Machine
//! and copied to your other Machines. A scope's Tasks and its Session each
//! have their own Machine; `--tasks` or `--session` sets one, neither sets both.
//! Naming a pool or on-demand instead sends the scope's new Tasks there and
//! leaves its Session where it runs; pools and on-demands live on this
//! Machine, so that default is not copied.
use crate::durable::{DefaultScope, MachineId, MachineSetting, RepositoryId};
use crate::id::WaveId;
use crate::lf::MachineCommand;
use crate::ops::task_ownership::{change, render, resolve_machine, OwnershipChange};
use crate::repository::CanonicalRepo;
use crate::session::PrimaryScope;
use crate::store::sqlite::SqliteStore;
use crate::store::{open_store, storage_config_from_env, SharedStore};
use anyhow::{anyhow, Context};
use std::path::Path;
use std::sync::Arc;

/// What `lf machine use` applies to.
#[derive(Debug)]
enum Scope {
    Task(String),
    Wave { id: WaveId, name: String },
    Repository,
}

pub fn run(repo: &Path, command: &MachineCommand) -> anyhow::Result<()> {
    let MachineCommand::Use {
        machine,
        wave,
        task,
        repo: repository_scope,
        session,
        tasks,
        here,
        json,
    } = command
    else {
        unreachable!("Machine use dispatcher");
    };
    let runtime = tokio::runtime::Runtime::new()?;
    let (store, scope) = runtime.block_on(async {
        let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
        let store = Arc::new(
            open_store(&config)
                .await
                .context("open the shared Loopflow store")?,
        );
        let scope = select_scope(
            &store,
            repo,
            wave.as_deref(),
            task.as_deref(),
            *repository_scope,
            *session || *tasks,
        )
        .await?;
        anyhow::Ok((store, scope))
    })?;
    let capacity =
        store.sqlite.pool(machine)?.is_some() || store.sqlite.ondemand(machine)?.is_some();
    if capacity {
        return use_capacity(&store.sqlite, repo, scope, machine, *session, *json);
    }
    if let Scope::Task(task) = &scope {
        let ownership = change(repo, task, OwnershipChange::Handoff(machine.clone()))?;
        if *json {
            println!("{}", serde_json::to_string(&ownership)?);
        } else {
            println!("Task {task} handed off\n{}", render(&ownership));
        }
        return Ok(());
    }
    let target = resolve_machine(&store.sqlite, machine)?;
    let canonical = CanonicalRepo::discover(repo)?;
    let repository = repository_id(&store.sqlite, &canonical)?;
    let (default_scope, primary, wave_name) = match scope {
        Scope::Wave { id, name } => (
            DefaultScope::Wave(id.clone()),
            PrimaryScope::Wave(id),
            Some(name),
        ),
        Scope::Repository => (
            DefaultScope::Repository(repository.clone()),
            PrimaryScope::Repository(canonical),
            None,
        ),
        Scope::Task(_) => unreachable!("Task scope hands off above"),
    };
    let settings = match (*session, *tasks) {
        (true, _) => vec![MachineSetting::Session],
        (_, true) => vec![MachineSetting::Tasks],
        _ => vec![MachineSetting::Tasks, MachineSetting::Session],
    };
    for setting in &settings {
        store
            .sqlite
            .set_machine_default(&default_scope, *setting, &target)?;
    }
    if settings.contains(&MachineSetting::Session) {
        runtime.block_on(move_primary_session(
            &store,
            repo,
            wave_name.as_deref(),
            &primary,
            &target,
        ))?;
    }
    if *json {
        println!(
            "{}",
            serde_json::json!({ "scope": default_scope, "settings": settings, "machine_id": target })
        );
    } else {
        let label = match &wave_name {
            Some(name) => format!("Wave {name}"),
            None => "This repository".to_string(),
        };
        let parts = match settings.as_slice() {
            [MachineSetting::Session] => "its Session runs",
            [MachineSetting::Tasks] => "your new Tasks run",
            _ => "your new Tasks and its Session run",
        };
        println!("{label}: {parts} on {}", describe(&store.sqlite, &target)?);
    }
    if !here {
        let mut args = vec![
            "--repository".to_string(),
            repository.to_string(),
            "machine".into(),
            "use".into(),
            target.to_string(),
            wave_name.unwrap_or_else(|| "--repo".into()),
            "--here".into(),
        ];
        if *session {
            args.push("--session".into());
        }
        if *tasks {
            args.push("--tasks".into());
        }
        runtime.block_on(copy_to_my_machines(&store.sqlite, &args))?;
    }
    Ok(())
}

/// Send a Wave's or repository's new Tasks to a pool or on-demand.
fn use_capacity(
    sqlite: &SqliteStore,
    repo: &Path,
    scope: Scope,
    capacity: &str,
    session: bool,
    json: bool,
) -> anyhow::Result<()> {
    if session {
        return Err(anyhow!(
            "{capacity} lends machines to Tasks; a Session runs on one Machine you name"
        ));
    }
    let (default_scope, label) = match scope {
        Scope::Task(task) => {
            return Err(anyhow!(
            "{capacity} sets a Wave or repository default; `lf task run {task} --rent {capacity}` \
                 or `--pool {capacity}` runs one Task there"
        ))
        }
        Scope::Wave { id, name } => (DefaultScope::Wave(id), format!("Wave {name}")),
        Scope::Repository => {
            let canonical = CanonicalRepo::discover(repo)?;
            (
                DefaultScope::Repository(repository_id(sqlite, &canonical)?),
                "This repository".to_string(),
            )
        }
    };
    sqlite.set_tasks_capacity(&default_scope, capacity)?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "scope": default_scope, "capacity": capacity })
        );
    } else {
        println!(
            "{label}: your new Tasks take a machine from {capacity} when they run; its Session stays where it runs"
        );
    }
    Ok(())
}

/// `--task`, `--repo` and a Wave argument are explicit; bare, the worktree's
/// Task, then its Wave, then this repository. `--session` or `--tasks` names a
/// part of a Wave or repository, so a bare one skips the worktree's Task.
async fn select_scope(
    store: &SharedStore,
    repo: &Path,
    wave: Option<&str>,
    task: Option<&str>,
    repository: bool,
    scope_part: bool,
) -> anyhow::Result<Scope> {
    if let Some(task) = task {
        return Ok(Scope::Task(task.to_string()));
    }
    if repository {
        return Ok(Scope::Repository);
    }
    if let Some(wave) = wave {
        let selected = crate::work::wave::context::resolve_managed_wave(
            Some(store),
            Some(repo),
            Some(wave),
            None,
        )
        .await?;
        return Ok(Scope::Wave {
            id: selected.id().clone(),
            name: selected.name().to_string(),
        });
    }
    let cwd = std::env::current_dir()?;
    let selected = crate::ops::select_execution_work(store, &cwd).await?;
    if let Some(task) = selected
        .as_ref()
        .and_then(|work| work.task.as_ref())
        .filter(|_| !scope_part)
    {
        return Ok(Scope::Task(task.id.to_string()));
    }
    Ok(match selected.and_then(|work| work.wave) {
        Some(wave) => Scope::Wave {
            id: wave.id().clone(),
            name: wave.name().to_string(),
        },
        None => Scope::Repository,
    })
}

fn repository_id(store: &SqliteStore, repo: &CanonicalRepo) -> anyhow::Result<RepositoryId> {
    store
        .repository_id(&repo.to_string())?
        .ok_or_else(|| anyhow!("repository identity unavailable; run `lf repo identity`"))
}

fn describe(store: &SqliteStore, machine: &MachineId) -> anyhow::Result<String> {
    if *machine == store.local_machine()?.id {
        return Ok(format!(
            "{} (this Machine)",
            crate::store::sqlite::task_ownership::local_label()
        ));
    }
    Ok(store
        .machine_by_id(machine)?
        .and_then(|machine| machine.label)
        .unwrap_or_else(|| machine.to_string()))
}

/// The Session lives on your Machine for its scope: admit it here, or move
/// the conversation running here to that Machine with its history.
async fn move_primary_session(
    store: &SharedStore,
    repo: &Path,
    wave: Option<&str>,
    primary: &PrimaryScope,
    target: &MachineId,
) -> anyhow::Result<()> {
    if *target == store.sqlite.local_machine()?.id {
        crate::ops::human_session::primary::ensure(store, repo, wave, false).await?;
    } else if let Some(session) = store.sqlite.primary_session(primary)? {
        let machine = store
            .sqlite
            .machine_by_id(target)?
            .ok_or_else(|| anyhow!("Machine {target} is not registered here"))?;
        crate::lf::commands::session_move::relay(store, &session.id, &machine).await?;
    }
    Ok(())
}

/// Each of your registered Machines holds the same choice, so a launch here
/// routes there and the target knows to receive it. An unreachable Machine
/// keeps its old choice until this runs again.
async fn copy_to_my_machines(store: &SqliteStore, args: &[String]) -> anyhow::Result<()> {
    for machine in store.machines()? {
        let name = machine
            .label
            .clone()
            .unwrap_or_else(|| machine.id.to_string());
        match crate::lf::commands::ssh::request(&machine, args, None).await {
            Ok(_) => eprintln!("copied to {name}"),
            Err(error) => eprintln!(
                "warning: {name} keeps its previous choice ({error}); rerun `lf machine use` to copy it"
            ),
        }
    }
    Ok(())
}
