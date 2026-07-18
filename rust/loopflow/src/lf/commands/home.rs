//! Place Work on durable Homes and route execution to those Homes.
//!
//! Placement is the only execution-location authority. `lf start` groups Waves
//! by Home and asks each Home's one resident to serve them. Remote durable
//! lifecycle commands use that Home's observed SSH route without forwarding
//! the origin machine's provider, GitHub, PM, or secret authority.

use std::collections::HashMap;
use std::path::Path;

use anyhow::anyhow;

use crate::engine::wave_context::{resolve_managed_wave_name_sync, resolve_run_wave_name};
use crate::engine::wave_home::{
    HomeActionDto, HomeRoute, HomeRuntimeDto, HomeState, HOME_ROUTED_ENV,
};
use crate::lf::{Commands, HomeCommand};
use crate::provider_account::lease::AccountSelection;

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

pub fn stop(name: &str, repo: &Path) -> anyhow::Result<()> {
    let name = crate::ops::util::normalize_wave_name(name)
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;
    let runtime = tokio::runtime::Runtime::new()?;
    let target = runtime.block_on(resolve_stop_target(&name, repo))?;
    match target {
        StopTarget::Local => crate::wave::stop(&name),
        StopTarget::Remote { home_id, repo } => {
            let cmd = vec!["lf".to_string(), "stop".to_string(), name];
            crate::lf::commands::ssh::capture_remote_native(&home_id, &repo, &cmd)
                .map(|stdout| print!("{stdout}"))
                .map_err(|error| anyhow!("remote Home {home_id} stop failed: {error}"))
        }
    }
}

enum StopTarget {
    Local,
    Remote {
        home_id: crate::durable::HomeId,
        repo: String,
    },
}

async fn resolve_stop_target(name: &str, repo: &Path) -> anyhow::Result<StopTarget> {
    let repo =
        crate::engine::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let store = crate::store::open_existing_store()
        .await
        .ok_or_else(|| anyhow!("lf stop needs an initialized local store"))?;
    let local = store.local_home().await?;
    validate_expected_home(&local.id)?;
    let addressed_home = std::env::var_os(crate::lf::commands::ssh::EXPECTED_HOME_ID_ENV).is_some();
    let wave = store
        .get_wave_by_name(name)
        .await?
        .ok_or_else(|| anyhow!("Wave '{name}' was not found"))?;
    let placement = store
        .placement(&crate::durable::WorkRef::Wave(wave.id().clone()))
        .await?;
    if addressed_home && placement.home_id != local.id {
        return Err(anyhow!(
            "refusing remote stop for Wave '{name}': this Home places it on {}",
            placement.home_id
        ));
    }
    if placement.home_id == local.id {
        return Ok(StopTarget::Local);
    }
    Ok(StopTarget::Remote {
        home_id: placement.home_id,
        repo: home_relative_repo(&repo)?,
    })
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
    let addressed_home = std::env::var_os(crate::lf::commands::ssh::EXPECTED_HOME_ID_ENV).is_some();
    let repo =
        crate::engine::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let wave_ids = parse_wave_ids(raw_wave_ids)?;
    let selected = if names.is_empty() {
        if !wave_ids.is_empty() {
            return Err(anyhow!("--wave-id requires explicit Wave names"));
        }
        store.list_waves(Some(&repo.display().to_string())).await?
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
        return Err(anyhow!(
            "no Waves found in {}; create one with `lf wave create <name>`",
            repo.display()
        ));
    }

    let mut groups: HashMap<crate::durable::HomeId, Vec<(crate::id::WaveId, String)>> =
        HashMap::new();
    for wave in selected {
        let placement = store
            .placement(&crate::durable::WorkRef::Wave(wave.id().clone()))
            .await?;
        if addressed_home && placement.home_id != local.id {
            return Err(anyhow!(
                "refusing remote start for Wave '{}': this Home places it on {}",
                wave.name(),
                placement.home_id
            ));
        }
        groups
            .entry(placement.home_id)
            .or_default()
            .push((wave.id().clone(), wave.name().to_string()));
    }
    let mut responses = Vec::with_capacity(groups.len());
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
    for (home_id, waves) in groups {
        if home_id == local.id {
            let wave_ids = waves.iter().map(|(id, _)| id.clone()).collect();
            crate::home_resident::ensure(&home_id, &repo).await?;
            crate::home_resident::start_waves(&home_id, wave_ids).await?;
            for (_, name) in waves {
                let wave = store
                    .get_wave_by_name(&name)
                    .await?
                    .ok_or_else(|| anyhow!("Wave '{name}' disappeared after start"))?;
                responses.push(crate::lf::commands::waves::snapshot_wave(&store, &wave).await?);
            }
            continue;
        }
        let remote_repo = home_relative_repo(&repo)?;
        let mut cmd = vec!["lf".to_string(), "start".to_string()];
        for (id, name) in waves {
            cmd.push(name.clone());
            cmd.push("--wave-id".to_string());
            cmd.push(format!("{name}={id}"));
        }
        cmd.push("--json".to_string());
        let remote_home = home_id.clone();
        let stdout = tokio::task::spawn_blocking(move || {
            crate::lf::commands::ssh::capture_remote_native(&remote_home, &remote_repo, &cmd)
        })
        .await
        .map_err(|error| anyhow!("remote Home start task failed: {error}"))?
        .map_err(|error| anyhow!("remote Home {home_id} start failed: {error}"))?;
        responses.extend(
            serde_json::from_str::<Vec<crate::lf::commands::waves::WaveSnapshot>>(stdout.trim())
                .map_err(|error| {
                    anyhow!("remote Home {home_id} returned invalid Wave status JSON: {error}")
                })?,
        );
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

fn home_relative_repo(repo: &Path) -> anyhow::Result<String> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow!("cannot resolve the current home directory"))?;
    let relative = repo.strip_prefix(&home).map_err(|_| {
        anyhow!(
            "repo {} is outside {}; remote Home routing needs a home-relative path",
            repo.display(),
            home.display()
        )
    })?;
    relative
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("repo path {} is not UTF-8", repo.display()))
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

/// Decide whether `command` runs on a remote home and, if so, run it there.
///
/// Returns `Some(result)` when the command was handled remotely (the caller
/// must not also run it locally), or `None` to fall through to local dispatch.
pub fn route(
    command: &Commands,
    wave: Option<&str>,
    account_selection: &AccountSelection,
    args: &[String],
) -> Option<anyhow::Result<()>> {
    if !is_routable(command) {
        return None;
    }
    // We are already on the home host after a forward: run locally, never loop.
    if std::env::var_os(HOME_ROUTED_ENV).is_some() {
        return None;
    }
    let name = wave.map(str::to_string).or_else(resolve_run_wave_name)?;
    let (home, wave_repo) = match resolve_home(&name) {
        Ok(resolved) => resolved,
        Err(error) => return Some(Err(error)),
    };
    if home.route == "local" {
        return None;
    }
    let remote_repo = match home_relative_repo(Path::new(&wave_repo)) {
        Ok(repo) => repo,
        Err(error) => return Some(Err(error)),
    };
    let route = match HomeRoute::parse(&home.route).filter(HomeRoute::is_remote) {
        Some(route) => route,
        None => {
            return Some(Err(anyhow!(
                "Home {} has invalid remote route {:?}",
                home.id,
                home.route
            )))
        }
    };
    let dest = route
        .ssh_destination()
        .expect("a remote Home route has an SSH destination");
    Some(crate::lf::commands::ssh::run_routed(
        &home.id,
        &dest,
        route.ssh_port(),
        Some(&remote_repo),
        account_selection,
        &remote_argv(args),
    ))
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

/// The repo/PR/release/PM operations that must run where the Wave's work lives.
/// Deliberately minimal — everything else stays local until a concrete need
/// grows the set.
fn is_routable(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Pr { .. }
            | Commands::Commit { .. }
            | Commands::Rebase { .. }
            | Commands::Release { .. }
            | Commands::Pm { .. }
    )
}

fn resolve_home(name: &str) -> anyhow::Result<(crate::durable::Home, String)> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("Home routing needs an initialized local store"))?;
        let wave = store
            .get_wave_by_name(name)
            .await?
            .ok_or_else(|| anyhow!("Wave '{name}' was not found"))?;
        let placement = store
            .placement(&crate::durable::WorkRef::Wave(wave.id().clone()))
            .await?;
        let home = store
            .home_by_id(&placement.home_id)
            .await?
            .ok_or_else(|| anyhow!("Home {} was not found", placement.home_id))?;
        Ok((home, wave.repo().to_string()))
    })
}

/// Rebuild the invocation as an `lf` command for the remote shell: the local
/// `argv[0]` is an absolute path to this machine's binary, so replace it with
/// bare `lf` (resolved against the remote PATH). Account-selection flags are
/// consumed at this origin boundary: the remote invocation inherits the fixed
/// lease handle and must not try to resolve the selectors again.
fn remote_argv(args: &[String]) -> Vec<String> {
    let mut cmd = Vec::with_capacity(args.len());
    cmd.push("lf".to_string());
    let mut args = args.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--" {
            cmd.push(arg.clone());
            cmd.extend(args.cloned());
            break;
        }
        if matches!(arg.as_str(), "--account" | "--only-account") {
            let _ = args.next();
            continue;
        }
        if arg.starts_with("--account=") || arg.starts_with("--only-account=") {
            continue;
        }
        cmd.push(arg.clone());
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::{is_routable, remote_argv};
    use crate::lf::Commands;

    #[test]
    fn remote_argv_swaps_the_binary_path_for_bare_lf() {
        let args = vec![
            "/Users/jack/src/loopflow/target/debug/lf".to_string(),
            "pr".to_string(),
            "open".to_string(),
        ];
        assert_eq!(remote_argv(&args), vec!["lf", "pr", "open"]);
    }

    #[test]
    fn remote_argv_consumes_account_selectors_at_the_origin_boundary() {
        let args = vec![
            "/local/lf",
            "--account",
            "claude=personal",
            "--only-account=codex=reserve",
            "commit",
            "-m",
            "ship it",
            "--",
            "--account",
            "literal",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

        assert_eq!(
            remote_argv(&args),
            vec![
                "lf",
                "commit",
                "-m",
                "ship it",
                "--",
                "--account",
                "literal"
            ]
        );
    }

    #[test]
    fn routable_set_is_repo_and_release_ops_only() {
        assert!(is_routable(&Commands::Commit {
            message: None,
            push: false,
            no_add: false,
        }));
        assert!(is_routable(&Commands::Pr { cmd: None }));
        assert!(!is_routable(&Commands::Status {
            wave: None,
            json: false,
        }));
        assert!(!is_routable(&Commands::Stop {
            name: "infra".to_string(),
        }));
    }
}
