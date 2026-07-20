//! Inspect durable Homes and run Waves on this machine.
//!
//! `lf` is machine-local: `lf start shipper` starts shipper here. To choose a
//! different machine, make that boundary explicit with
//! `lf ssh <target> start shipper`. Durable Home placement records where the
//! Wave is running; it does not silently reroute ordinary commands.

use std::path::Path;

use anyhow::anyhow;

use crate::engine::wave_context::resolve_managed_wave_name_sync;
use crate::engine::wave_home::{HomeActionDto, HomeRuntimeDto, HomeState};
use crate::lf::HomeCommand;

/// `lf home <id|observe|probe>` — inspect durable Home identity and reachability.
pub fn run(cmd: &HomeCommand, repo: &Path) -> anyhow::Result<()> {
    match cmd {
        HomeCommand::Id { json } => id_cmd(*json),
        HomeCommand::Observe {
            home_id,
            route,
            json,
        } => observe_cmd(home_id, route, *json),
        HomeCommand::Probe { wave, json } => probe_cmd(wave.as_deref(), *json, repo),
    }
}

fn id_cmd(json: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let home = runtime.block_on(async {
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("lf home id needs an initialized local store"))?
            .local_home()
            .await
            .map_err(anyhow::Error::from)
    })?;
    if json {
        println!("{}", serde_json::to_string(&home)?);
    } else {
        println!("{}", home.id);
    }
    Ok(())
}

fn observe_cmd(home_id: &crate::durable::HomeId, route: &str, json: bool) -> anyhow::Result<()> {
    let parsed = crate::engine::wave_home::HomeRoute::parse(route)
        .ok_or_else(|| anyhow!("invalid Home route: {route:?}"))?;
    if !parsed.is_remote() {
        return Err(anyhow!(
            "the local Home route is managed by `lf home id`; observe only remote SSH routes"
        ));
    }
    let route = parsed.to_string();
    let runtime = tokio::runtime::Runtime::new()?;
    let home = runtime.block_on(async {
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("lf home observe needs an initialized local store"))?
            .observe_home(home_id, &route)
            .await
            .map_err(anyhow::Error::from)
    })?;
    if json {
        println!("{}", serde_json::to_string(&home)?);
    } else {
        println!("{}  {}", home.id, home.route);
    }
    Ok(())
}

fn probe_cmd(wave: Option<&str>, json: bool, _repo: &Path) -> anyhow::Result<()> {
    let name = resolve_managed_wave_name_sync(wave).map_err(|err| anyhow!("{err}"))?;
    let rt = tokio::runtime::Runtime::new()?;
    let runtime = rt.block_on(async {
        let store = crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("lf home probe needs an initialized local store"))?;
        let wave = store
            .get_wave_by_name(&name)
            .await?
            .ok_or_else(|| anyhow!("Wave '{name}' was not found"))?;
        let placement = store
            .placement(&crate::durable::WorkRef::Wave(wave.id().clone()))
            .await?;
        let home = store
            .home_by_id(&placement.home_id)
            .await?
            .ok_or_else(|| anyhow!("Home {} was not found", placement.home_id))?;
        Ok::<_, anyhow::Error>(
            crate::ops::home::probe_home(&name, &home, Path::new(wave.repo())).await,
        )
    })?;
    if json {
        println!("{}", serde_json::to_string(&runtime)?);
    } else {
        print_runtime(&name, &runtime);
    }
    Ok(())
}

pub fn start(waves: &[String], wave_ids: &[String], json: bool, repo: &Path) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let responses = runtime.block_on(start_inner(waves, wave_ids, repo))?;
    if json {
        println!("{}", serde_json::to_string(&responses)?);
        return Ok(());
    }
    for wave in responses {
        let state = if wave.live { "running" } else { "starting" };
        println!("{}  {state} on {}", wave.name, wave.home.id);
    }
    Ok(())
}

pub fn stop(name: &str, _repo: &Path) -> anyhow::Result<()> {
    let name = crate::ops::util::normalize_wave_name(name)
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;
    crate::wave::stop(&name)
}

async fn start_inner(
    names: &[String],
    raw_wave_ids: &[String],
    repo: &Path,
) -> anyhow::Result<Vec<crate::lf::commands::waves::WaveSnapshot>> {
    let store = std::sync::Arc::new(
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("lf start needs an initialized local store"))?,
    );
    let local = store.local_home().await?;
    validate_expected_home(&local.id)?;
    let repo =
        crate::engine::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let wave_ids = parse_wave_ids(raw_wave_ids)?;
    let selected = if names.is_empty() {
        if !wave_ids.is_empty() {
            return Err(anyhow!("--wave-id requires explicit Wave names"));
        }
        let repo_name = repo.display().to_string();
        let known = store.list_waves(Some(&repo_name)).await?;
        if known.is_empty() {
            return Err(anyhow!(
                "no Waves found in {}; create one with `lf wave create <name>`",
                repo.display()
            ));
        }
        crate::home_resident::waves_for_home(&store, &local.id, Some(&repo_name)).await?
    } else {
        let mut selected = Vec::with_capacity(names.len());
        for raw in names {
            let name = crate::ops::util::normalize_wave_name(raw)
                .ok_or_else(|| anyhow!("invalid wave name: '{raw}'"))?;
            let wave = match wave_ids.get(&name) {
                Some(id) => {
                    crate::wave::registry::ensure_wave_row_with_id(&store, &repo, &name, id).await?
                }
                None => crate::wave::registry::ensure_wave_row(&store, &repo, &name).await?,
            };
            selected.push(wave);
        }
        for name in wave_ids.keys() {
            if !selected.iter().any(|wave| wave.name() == name) {
                return Err(anyhow!("--wave-id names unselected Wave '{name}'"));
            }
        }
        selected
    };
    if selected.is_empty() {
        return Ok(Vec::new());
    }

    let wave_ids = selected
        .iter()
        .map(|wave| wave.id().clone())
        .collect::<Vec<_>>();
    for wave in &selected {
        store
            .place_work(&crate::durable::WorkRef::Wave(wave.id().clone()), &local.id)
            .await?;
    }
    crate::home_resident::ensure(&local.id, &repo).await?;
    crate::home_resident::start_waves(&local.id, wave_ids).await?;

    let mut responses = Vec::with_capacity(selected.len());
    for selected_wave in selected {
        let wave = store
            .get_wave_by_name(selected_wave.name())
            .await?
            .ok_or_else(|| anyhow!("Wave '{}' disappeared after start", selected_wave.name()))?;
        responses.push(crate::lf::commands::waves::snapshot_wave(&store, &wave).await?);
    }
    Ok(responses)
}

fn parse_wave_ids(
    values: &[String],
) -> anyhow::Result<std::collections::BTreeMap<String, crate::id::WaveId>> {
    let mut bindings = std::collections::BTreeMap::new();
    for value in values {
        let (raw_name, raw_id) = value
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid --wave-id {value:?}; expected NAME=ID"))?;
        let name = crate::ops::util::normalize_wave_name(raw_name)
            .ok_or_else(|| anyhow!("invalid Wave name in --wave-id {value:?}"))?;
        let id = crate::id::WaveId::parse(raw_id)
            .map_err(|error| anyhow!("invalid Wave id in --wave-id {value:?}: {error}"))?;
        if bindings.insert(name.clone(), id).is_some() {
            return Err(anyhow!("duplicate --wave-id binding for Wave '{name}'"));
        }
    }
    Ok(bindings)
}

fn validate_expected_home(local: &crate::durable::HomeId) -> anyhow::Result<()> {
    let Ok(raw) = std::env::var(crate::lf::commands::ssh::EXPECTED_HOME_ID_ENV) else {
        return Ok(());
    };
    let expected = crate::durable::HomeId::parse(&raw)
        .map_err(|error| anyhow!("invalid expected Home id: {error}"))?;
    if expected != *local {
        return Err(anyhow!(
            "refusing remote lifecycle command: expected Home {expected}, local Home is {local}"
        ));
    }
    Ok(())
}

fn print_runtime(name: &str, runtime: &HomeRuntimeDto) {
    let state = match runtime.state {
        HomeState::Unreachable => "unreachable",
        HomeState::Stopped => "stopped",
        HomeState::Running => "running",
        HomeState::Unknown => "unknown",
    };
    let action = match &runtime.action {
        HomeActionDto::Attach { endpoint } => format!("Attach ({endpoint})"),
        HomeActionDto::Start { home_id } => format!("Start on {home_id}"),
        HomeActionDto::Reason { message } => message.clone(),
    };
    println!(
        "{name}  {} ({})  [{state}]",
        runtime.home.id, runtime.home.route
    );
    println!("  reason  {}", runtime.reason);
    println!("  action  {action}");
}

pub fn validate_expected_home_process() -> anyhow::Result<()> {
    if std::env::var_os(crate::lf::commands::ssh::EXPECTED_HOME_ID_ENV).is_none() {
        return Ok(());
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let local = runtime.block_on(async {
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("Home-addressed command needs an initialized local store"))?
            .local_home()
            .await
            .map_err(anyhow::Error::from)
    })?;
    validate_expected_home(&local.id)
}
