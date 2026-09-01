//! Inspect durable Homes and run Waves on this machine.
//!
//! `lf` is machine-local: `lf start shipper` starts shipper here. To choose a
//! different machine, make that boundary explicit with
//! `lf ssh <target> start shipper`. Durable Home placement records where the
//! Wave is running; it does not silently reroute ordinary commands.

use std::path::Path;

use anyhow::anyhow;

use crate::engine::wave_home::{HomeActionDto, HomeRuntimeDto, HomeState};
use crate::lf::HomeCommand;
use crate::work::wave::context::resolve_managed_wave_sync;

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

fn probe_cmd(wave: Option<&str>, json: bool, repo: &Path) -> anyhow::Result<()> {
    let selected = resolve_managed_wave_sync(Some(repo), wave).map_err(|err| anyhow!("{err}"))?;
    let wave_id = selected.id().clone();
    let rt = tokio::runtime::Runtime::new()?;
    let (wave, runtime) = rt.block_on(async {
        let store = crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("lf home probe needs an initialized local store"))?;
        let wave = store
            .get_wave(&wave_id)
            .await?
            .ok_or_else(|| anyhow!("Wave {wave_id} was not found"))?;
        let placement = store
            .placement(&crate::durable::WorkRef::Wave(wave.id().clone()))
            .await?;
        let home = store
            .home_by_id(&placement.home_id)
            .await?
            .ok_or_else(|| anyhow!("Home {} was not found", placement.home_id))?;
        let runtime =
            crate::ops::home::probe_home(wave.name(), &home, Path::new(wave.repo())).await;
        Ok::<_, anyhow::Error>((wave, runtime))
    })?;
    if json {
        println!("{}", serde_json::to_string(&runtime)?);
    } else {
        print_runtime(wave.name(), &runtime);
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

pub fn stop(name: &str, repo: &Path) -> anyhow::Result<()> {
    let name = crate::ops::util::normalize_wave_name(name)
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;
    let locator = crate::work::wave::WaveLocator::discover(repo, &name)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let stopped = runtime.block_on(async {
        let store = crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("lf stop needs an initialized local store"))?;
        let wave = store
            .get_wave_at(&locator)
            .await?
            .ok_or_else(|| anyhow!("Wave '{name}' was not found"))?;
        let local = store.local_home().await?;
        let work = crate::durable::WorkRef::Wave(wave.id().clone());
        let placement = store.placement(&work).await?;
        if placement.home_id != local.id {
            return Err(anyhow!(
                "Wave {name} is placed on {}, not local Home {}",
                placement.home_id,
                local.id
            ));
        }
        if let Some(stopped) = crate::lfd::stop_wave(&local.id, wave.id()).await? {
            return Ok(stopped);
        }
        store.set_work_enabled(&work, false).await?;
        crate::controller::wave::request_stop(Path::new(wave.repo()), wave.name()).await
    })?;
    if stopped {
        println!("stopped wave {name}");
    } else {
        println!("wave {name} is already stopped");
    }
    Ok(())
}

async fn start_inner(
    names: &[String],
    raw_wave_ids: &[String],
    repo: &Path,
) -> anyhow::Result<Vec<crate::lf::commands::waves::WaveSnapshot>> {
    let store = std::sync::Arc::new(
        crate::store::open_store(&crate::store::storage_config_from_env()?)
            .await
            .map_err(|error| anyhow!("lf start cannot open this Home registry: {error}"))?,
    );
    let local = store.local_home().await?;
    validate_expected_home(&local.id)?;
    let repo = crate::repository::CanonicalRepo::discover(repo)?;
    let wave_ids = parse_wave_ids(raw_wave_ids)?;
    let normalized_names = names
        .iter()
        .map(|raw| {
            crate::ops::util::normalize_wave_name(raw)
                .ok_or_else(|| anyhow!("invalid wave name: '{raw}'"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let unique_names = normalized_names
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    if unique_names.len() != normalized_names.len() {
        return Err(anyhow!("duplicate Wave name in start request"));
    }
    for name in wave_ids.keys() {
        if !normalized_names.contains(name) {
            return Err(anyhow!("--wave-id names unselected Wave '{name}'"));
        }
    }
    crate::lfd::ensure(&local.id, repo.as_path()).await?;
    let selected = if names.is_empty() {
        if !wave_ids.is_empty() {
            return Err(anyhow!("--wave-id requires explicit Wave names"));
        }
        let repo_name = repo.to_string();
        let known = store.list_waves(Some(&repo_name)).await?;
        if known.is_empty() {
            return Err(anyhow!(
                "no Waves found in {}; create one with `lf wave create <name>`",
                repo
            ));
        }
        crate::wave_host::waves_for_home(&store, &local.id, Some(&repo_name))
            .await?
            .into_iter()
            .map(|wave| StartSelection {
                wave,
                created: false,
                prior_home: Some(local.id.clone()),
            })
            .collect()
    } else {
        let mut candidates = Vec::with_capacity(normalized_names.len());
        for name in &normalized_names {
            let locator = crate::work::wave::WaveLocator::new(repo.clone(), name)?;
            let existing_wave = store.get_wave_at(&locator).await?;
            let prior_home = match existing_wave.as_ref() {
                Some(wave) => Some(
                    store
                        .placement(&crate::durable::WorkRef::Wave(wave.id().clone()))
                        .await?
                        .home_id,
                ),
                None => None,
            };
            candidates.push((name.clone(), existing_wave, prior_home));
        }
        let mut selected = Vec::with_capacity(candidates.len());
        for (name, existing_wave, prior_home) in candidates {
            let created = existing_wave.is_none();
            let wave_result = match wave_ids.get(&name) {
                Some(id) => {
                    crate::controller::wave::registry::ensure_wave_row_with_id(
                        &store,
                        repo.as_path(),
                        &name,
                        id,
                    )
                    .await
                }
                None => {
                    crate::controller::wave::registry::ensure_wave_row(
                        &store,
                        repo.as_path(),
                        &name,
                    )
                    .await
                }
            };
            let wave = match wave_result {
                Ok(wave) => wave,
                Err(error) => {
                    let rollback = rollback_selections(&store, &selected).await;
                    return match rollback {
                        Ok(()) => Err(error.into()),
                        Err(rollback) => Err(anyhow!(
                            "Wave registration failed: {error}; registry rollback failed: {rollback}"
                        )),
                    };
                }
            };
            selected.push(StartSelection {
                wave,
                created,
                prior_home,
            });
        }
        selected
    };
    if selected.is_empty() {
        return Ok(Vec::new());
    }

    let wave_ids = selected
        .iter()
        .map(|selection| selection.wave.id().clone())
        .collect::<Vec<_>>();
    for selection in &selected {
        if let Err(error) = store
            .place_work(
                &crate::durable::WorkRef::Wave(selection.wave.id().clone()),
                &local.id,
            )
            .await
        {
            let rollback = rollback_selections(&store, &selected).await;
            return match rollback {
                Ok(()) => Err(error.into()),
                Err(rollback) => Err(anyhow!(
                    "Wave placement failed: {error}; registry rollback failed: {rollback}"
                )),
            };
        }
    }
    let outcomes = match crate::lfd::start_waves(&local.id, wave_ids).await {
        Ok(outcomes) => outcomes,
        Err(error) => {
            let mut outcomes = Vec::with_capacity(selected.len());
            for selection in &selected {
                let state = match crate::controller::wave::server::live_endpoint(
                    Path::new(selection.wave.repo()),
                    selection.wave.name(),
                )
                .await
                {
                    Some(endpoint) => crate::wave_host::WaveStartState::Live { endpoint },
                    None => crate::wave_host::WaveStartState::Failed {
                        reason: error.to_string(),
                    },
                };
                outcomes.push(crate::wave_host::WaveStartOutcome {
                    wave_id: selection.wave.id().clone(),
                    state,
                });
            }
            outcomes
        }
    };

    let mut failures = Vec::new();
    for selection in &selected {
        let outcome = outcomes
            .iter()
            .find(|outcome| outcome.wave_id == *selection.wave.id());
        let reason = match outcome.map(|outcome| &outcome.state) {
            Some(crate::wave_host::WaveStartState::Live { .. }) => continue,
            Some(crate::wave_host::WaveStartState::Failed { reason }) => reason.clone(),
            None => "lfd returned no startup outcome".to_string(),
        };
        if let Err(rollback) = rollback_selection(&store, selection).await {
            failures.push(format!(
                "Wave {} failed to start: {reason}; registry rollback failed: {rollback}",
                selection.wave.name()
            ));
        } else {
            failures.push(format!(
                "Wave {} failed to start: {reason}",
                selection.wave.name()
            ));
        }
    }
    if !failures.is_empty() {
        return Err(anyhow!(failures.join("; ")));
    }
    let mut responses = Vec::with_capacity(selected.len());
    for selection in &selected {
        let wave = store
            .get_wave(selection.wave.id())
            .await?
            .ok_or_else(|| anyhow!("Wave '{}' disappeared after start", selection.wave.name()))?;
        let snapshot = crate::lf::commands::waves::snapshot_wave(&store, &wave).await?;
        if !snapshot.live {
            let reason = format!(
                "Wave {} reported a live startup outcome but its endpoint is not answering",
                wave.name()
            );
            if let Err(rollback) = rollback_selection(&store, selection).await {
                failures.push(format!("{reason}; registry rollback failed: {rollback}"));
            } else {
                failures.push(reason);
            }
            continue;
        }
        responses.push(snapshot);
    }
    if !failures.is_empty() {
        return Err(anyhow!(failures.join("; ")));
    }
    Ok(responses)
}

struct StartSelection {
    wave: crate::work::wave::Wave,
    created: bool,
    prior_home: Option<crate::durable::HomeId>,
}

async fn rollback_selection(
    store: &crate::store::Store,
    selection: &StartSelection,
) -> anyhow::Result<()> {
    if selection.created {
        store.delete_wave(selection.wave.id()).await?;
    } else if let Some(home_id) = &selection.prior_home {
        store
            .place_work(
                &crate::durable::WorkRef::Wave(selection.wave.id().clone()),
                home_id,
            )
            .await?;
    }
    Ok(())
}

async fn rollback_selections(
    store: &crate::store::Store,
    selections: &[StartSelection],
) -> anyhow::Result<()> {
    let mut failures = Vec::new();
    for selection in selections.iter().rev() {
        if let Err(error) = rollback_selection(store, selection).await {
            failures.push(format!("{}: {error}", selection.wave.name()));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(failures.join("; ")))
    }
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
