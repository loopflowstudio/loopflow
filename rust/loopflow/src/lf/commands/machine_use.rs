//! `lf machine use`: run work on one of your Machines. A Task moves by
//! handoff; a Wave or repository default is personal, stored on this Machine
//! and copied to your other Machines.
use crate::durable::{DefaultScope, MachineId, OwningMachineSource, RepositoryId};
use crate::id::WaveId;
use crate::lf::MachineCommand;
use crate::ops::task_ownership::{change, render, resolve_machine, OwnershipChange};
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
        )
        .await?;
        anyhow::Ok((store, scope))
    })?;
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
    let repository = repository_id(&store.sqlite, repo)?;
    let (default_scope, primary, label) = match &scope {
        Scope::Wave { id, name } => (
            DefaultScope::Wave(id.clone()),
            PrimaryScope::Wave(id.clone()),
            format!("Wave {name}"),
        ),
        Scope::Repository => (
            DefaultScope::Repository(repository.clone()),
            PrimaryScope::Repository(crate::repository::CanonicalRepo::discover(repo)?),
            "This repository".to_string(),
        ),
        Scope::Task(_) => unreachable!("Task scope hands off above"),
    };
    store.sqlite.set_machine_default(&default_scope, &target)?;
    runtime.block_on(move_primary_session(
        &store, repo, &scope, &primary, &target,
    ))?;
    let (resolved, source) = match &scope {
        Scope::Wave { id, .. } => store.sqlite.wave_owning_machine(id)?,
        _ => store.sqlite.repository_owning_machine(&repository)?,
    };
    if *json {
        println!(
            "{}",
            serde_json::json!({ "scope": default_scope, "machine_id": resolved })
        );
    } else {
        println!(
            "{label}: your new Tasks and its Session run on {}",
            describe(&store.sqlite, &resolved, &source)?
        );
    }
    if !here {
        let mut args = vec![
            "--repository".to_string(),
            repository.to_string(),
            "machine".into(),
            "use".into(),
            target.to_string(),
        ];
        match &scope {
            Scope::Wave { name, .. } => args.push(name.clone()),
            _ => args.push("--repo".into()),
        }
        args.push("--here".into());
        runtime.block_on(copy_to_my_machines(&store.sqlite, &args))?;
    }
    Ok(())
}

/// `--task`, `--repo` and a Wave argument are explicit; bare, the worktree's
/// Task, then its Wave, then this repository.
async fn select_scope(
    store: &SharedStore,
    repo: &Path,
    wave: Option<&str>,
    task: Option<&str>,
    repository: bool,
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
    if let Some(task) = selected.as_ref().and_then(|work| work.task.as_ref()) {
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

fn repository_id(store: &SqliteStore, repo: &Path) -> anyhow::Result<RepositoryId> {
    store
        .repository_id(&crate::repository::CanonicalRepo::discover(repo)?.to_string())?
        .ok_or_else(|| anyhow!("repository identity unavailable; run `lf repo identity`"))
}

fn describe(
    store: &SqliteStore,
    machine: &MachineId,
    source: &OwningMachineSource,
) -> anyhow::Result<String> {
    if matches!(source, OwningMachineSource::ThisMachine) || *machine == store.local_machine()?.id {
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

/// The Session lives on your Machine for its scope: admit it there, and
/// complete it everywhere else. Native history stays where it ran.
async fn move_primary_session(
    store: &SharedStore,
    repo: &Path,
    scope: &Scope,
    primary: &PrimaryScope,
    target: &MachineId,
) -> anyhow::Result<()> {
    if *target == store.sqlite.local_machine()?.id {
        let wave = match scope {
            Scope::Wave { name, .. } => Some(name.as_str()),
            _ => None,
        };
        crate::ops::human_session::primary::ensure(store, repo, wave, false).await?;
    } else if let Some(session) = store.sqlite.complete_primary_session(primary)? {
        eprintln!(
            "Session {} completed here; it continues on {target}",
            session.id
        );
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
        match crate::lf::commands::ssh::request(&machine, args).await {
            Ok(_) => eprintln!("copied to {name}"),
            Err(error) => eprintln!(
                "warning: {name} keeps its previous choice ({error}); rerun `lf machine use` to copy it"
            ),
        }
    }
    Ok(())
}
