//! `lf set` / `lf unset`: your own value of a setting, for this repository,
//! a Wave (`lf --wave <wave> set`) or every repository (`--global`). Values
//! live in this Machine's store and never edit committed config: shared setup
//! is `.lf/config.yaml` and `.lf/wave/<name>/config.yaml`, and yours wins over
//! it for you only. A Machine is copied to your other Machines so a launch
//! anywhere routes there; pools and on-demands live on this Machine, so a
//! capacity is not copied.
use crate::durable::{DefaultScope, MachineId, MachineSetting, RepositoryId};
use crate::profile::RouteScope;
use crate::provider_auth::Provider;
use crate::repository::{CanonicalRepo, RepoId};
use crate::session::PrimaryScope;
use crate::store::sqlite::SqliteStore;
use crate::store::{open_store, storage_config_from_env, SharedStore};
use anyhow::{anyhow, bail, Context};
use std::path::Path;
use std::sync::Arc;

/// A key `lf set` accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Key {
    /// `graph`, `agent` or `skill_agents.<skill>`: your value over the
    /// committed config.
    Shared,
    /// `machine`, `machine.tasks` or `machine.session`.
    Machine(&'static [MachineSetting]),
    /// `account.<provider>`: the logins that pay, in order.
    Account(Provider),
}

fn parse_key(raw: &str) -> anyhow::Result<Key> {
    Ok(match raw.split_once('.') {
        None if raw == "graph" || raw == "agent" => Key::Shared,
        None if raw == "machine" => Key::Machine(&[MachineSetting::Tasks, MachineSetting::Session]),
        Some(("machine", "tasks")) => Key::Machine(&[MachineSetting::Tasks]),
        Some(("machine", "session")) => Key::Machine(&[MachineSetting::Session]),
        Some(("skill_agents", skill)) if !skill.is_empty() && !skill.contains('.') => Key::Shared,
        Some(("account", provider)) => Key::Account(
            crate::lf::commands::profile::parse_managed_provider(provider)?,
        ),
        _ => bail!(
            "unknown key `{raw}`; keys are graph, agent, skill_agents.<skill>, machine, \
             machine.tasks, machine.session and account.<provider>"
        ),
    })
}

/// Where a value applies.
#[derive(Debug)]
enum Scope {
    Global,
    Repository,
    Wave { id: crate::id::WaveId, slug: String },
}

impl Scope {
    fn label(&self) -> String {
        match self {
            Self::Global => "Every repository".into(),
            Self::Repository => "This repository".into(),
            Self::Wave { slug, .. } => format!("Wave {slug}"),
        }
    }
}

/// `wave` is the root `--wave` the person typed; none means this repository,
/// even in a Wave's Task worktree.
pub fn run(
    repo: &Path,
    wave: Option<&str>,
    key: &str,
    value: Option<&str>,
    global: bool,
    here: bool,
) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run_async(repo, wave, key, value, global, here))
}

async fn run_async(
    repo: &Path,
    wave: Option<&str>,
    key: &str,
    value: Option<&str>,
    global: bool,
    here: bool,
) -> anyhow::Result<()> {
    let raw_key = key;
    let key = parse_key(raw_key)?;
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    let store = Arc::new(
        open_store(&config)
            .await
            .context("open the shared Loopflow store")?,
    );
    let scope = match (wave, global) {
        (Some(_), true) => bail!("--wave and --global name different scopes; pick one"),
        (None, true) => Scope::Global,
        (None, false) => Scope::Repository,
        (Some(wave), false) => {
            let wave = crate::work::wave::context::resolve_managed_wave(
                Some(&store),
                Some(repo),
                Some(wave),
                None,
            )
            .await?;
            Scope::Wave {
                id: wave.id().clone(),
                slug: wave.slug().to_owned(),
            }
        }
    };
    match key {
        Key::Shared => shared(&store.sqlite, repo, &scope, raw_key, value),
        Key::Account(provider) => account(&store, repo, &scope, provider, value).await,
        Key::Machine(settings) => {
            let default_scope = default_scope(&store.sqlite, repo, &scope)?
                .ok_or_else(|| anyhow!("a Machine is set for a Wave or repository, not --global; with none set, work runs on the Machine running it"))?;
            let command = match value {
                // A capacity lives on this Machine; your others keep theirs.
                Some(capacity) if is_capacity(&store.sqlite, capacity)? => {
                    return capacity_set(&store.sqlite, &scope, &default_scope, settings, capacity);
                }
                Some(machine) => {
                    let target =
                        crate::ops::task_ownership::resolve_machine(&store.sqlite, machine)?;
                    machine_set(&store, repo, &scope, &default_scope, settings, &target).await?;
                    vec!["set".to_owned(), raw_key.to_owned(), target.to_string()]
                }
                None => {
                    machine_unset(&store.sqlite, &scope, &default_scope, settings)?;
                    vec!["unset".to_owned(), raw_key.to_owned()]
                }
            };
            if here {
                return Ok(());
            }
            copy_to_my_machines(&store.sqlite, repo, &scope, command).await
        }
    }
}

// ─── Shared setup ────────────────────────────────────────────────────────

fn shared(
    store: &SqliteStore,
    repo: &Path,
    scope: &Scope,
    key: &str,
    value: Option<&str>,
) -> anyhow::Result<()> {
    if key == "graph" && matches!(scope, Scope::Global) {
        bail!("a graph is chosen per repository or Wave, never for every repository");
    }
    let default_scope = default_scope(store, repo, scope)?;
    let label = scope.label();
    match value {
        Some(value) => {
            if key == "graph" {
                crate::task_graph::validate_graph_name(value).map_err(|e| anyhow!(e))?;
                if crate::task_graph::CapturedGraph::load(value, repo)
                    .map_err(|e| anyhow!(e))?
                    .is_none()
                {
                    bail!(crate::task_graph::missing_graph(value));
                }
            } else if value.trim().is_empty() {
                bail!("{key} needs an agent, such as claude or claude:opus");
            }
            store.set_scope_setting(default_scope.as_ref(), key, value)?;
            println!("{label}: your {key} is {value}");
        }
        None => {
            if store.clear_scope_setting(default_scope.as_ref(), key)? {
                println!("{label}: your {key} is cleared; the committed config chooses");
            } else {
                println!("{label}: you had no {key} set");
            }
        }
    }
    Ok(())
}

// ─── Account routes ──────────────────────────────────────────────────────

async fn account(
    store: &SharedStore,
    repo: &Path,
    scope: &Scope,
    provider: Provider,
    value: Option<&str>,
) -> anyhow::Result<()> {
    let route = match scope {
        Scope::Global => RouteScope::Default,
        Scope::Wave { id, .. } => RouteScope::Wave(id.clone()),
        Scope::Repository => RouteScope::Repo(RepoId::discover(repo).map_err(|error| {
            anyhow!("{error}; account routes name a repository by its origin, so add one or use --global")
        })?),
    };
    let label = scope.label();
    match value {
        Some(value) => {
            let accounts = value
                .split(',')
                .map(str::trim)
                .filter(|account| !account.is_empty())
                .collect::<Vec<_>>();
            crate::lf::commands::profile::set_route(store, route, &label, provider, &accounts).await
        }
        None => {
            if store.clear_provider_route(&route, provider).await? {
                println!("{label} {provider}: route cleared");
            } else {
                println!("{label} {provider}: no route was set");
            }
            Ok(())
        }
    }
}

// ─── Machines ────────────────────────────────────────────────────────────

fn capacity_set(
    store: &SqliteStore,
    scope: &Scope,
    default_scope: &DefaultScope,
    settings: &[MachineSetting],
    capacity: &str,
) -> anyhow::Result<()> {
    if settings == [MachineSetting::Session] {
        bail!("{capacity} lends machines to Tasks; a Session runs on one Machine you name");
    }
    store.set_tasks_capacity(default_scope, capacity)?;
    println!(
        "{}: your new Tasks take a machine from {capacity} when they run; its Session stays where it runs",
        scope.label()
    );
    Ok(())
}

async fn machine_set(
    store: &SharedStore,
    repo: &Path,
    scope: &Scope,
    default_scope: &DefaultScope,
    settings: &[MachineSetting],
    target: &MachineId,
) -> anyhow::Result<()> {
    for setting in settings {
        store
            .sqlite
            .set_machine_default(default_scope, *setting, target)?;
    }
    if settings.contains(&MachineSetting::Session) {
        let (wave, primary) = match scope {
            Scope::Wave { id, slug } => (Some(slug.as_str()), PrimaryScope::Wave(id.clone())),
            _ => (
                None,
                PrimaryScope::Repository(CanonicalRepo::discover(repo)?),
            ),
        };
        move_primary_session(store, repo, wave, &primary, target).await?;
    }
    let parts = match settings {
        [MachineSetting::Session] => "its Session runs",
        [MachineSetting::Tasks] => "your new Tasks run",
        _ => "your new Tasks and its Session run",
    };
    println!(
        "{}: {parts} on {}",
        scope.label(),
        describe(&store.sqlite, target)?
    );
    Ok(())
}

fn machine_unset(
    store: &SqliteStore,
    scope: &Scope,
    default_scope: &DefaultScope,
    settings: &[MachineSetting],
) -> anyhow::Result<()> {
    let mut cleared = false;
    for setting in settings {
        cleared |= store.clear_machine_default(default_scope, *setting)?;
    }
    let label = scope.label();
    if cleared {
        println!("{label}: your Machine is cleared; the next scope chooses");
    } else {
        println!("{label}: you had no Machine set");
    }
    Ok(())
}

fn is_capacity(store: &SqliteStore, name: &str) -> anyhow::Result<bool> {
    Ok(store.pool(name)?.is_some() || store.ondemand(name)?.is_some())
}

/// The store scope for a Wave or this repository; `None` for every repository.
fn default_scope(
    store: &SqliteStore,
    repo: &Path,
    scope: &Scope,
) -> anyhow::Result<Option<DefaultScope>> {
    Ok(match scope {
        Scope::Global => None,
        Scope::Repository => Some(DefaultScope::Repository(repository_id(store, repo)?)),
        Scope::Wave { id, .. } => Some(DefaultScope::Wave(id.clone())),
    })
}

fn repository_id(store: &SqliteStore, repo: &Path) -> anyhow::Result<RepositoryId> {
    let canonical = CanonicalRepo::discover(repo)?;
    store
        .repository_id(&canonical.to_string())?
        .ok_or_else(|| anyhow!("repository identity unavailable; run `lf repository`"))
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
        .map_or_else(|| machine.to_string(), |machine| machine.name()))
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
        crate::ops::human_session::primary::ensure(store, repo, wave).await?;
    } else if let Some(session) = store.sqlite.primary_session(primary)? {
        let machine = store
            .sqlite
            .machine_by_id(target)?
            .ok_or_else(|| anyhow!("Machine {target} is not registered here"))?;
        crate::lf::commands::session_move::relay(store, &session, &machine).await?;
    }
    Ok(())
}

/// Each of your registered Machines holds the same choice, so a launch here
/// routes there and the target knows to receive it. An unreachable Machine
/// keeps its old choice until this runs again.
async fn copy_to_my_machines(
    store: &SqliteStore,
    repo: &Path,
    scope: &Scope,
    command: Vec<String>,
) -> anyhow::Result<()> {
    let machines = store.machines()?;
    if machines.is_empty() {
        return Ok(());
    }
    let mut args = vec![
        "--repository".to_owned(),
        repository_id(store, repo)?.to_string(),
    ];
    if let Scope::Wave { slug, .. } = scope {
        args.extend(["--wave".to_owned(), slug.clone()]);
    }
    args.extend(command);
    args.push("--here".to_owned());
    for machine in machines {
        let name = machine.name();
        match crate::lf::commands::ssh::request(&machine, &args, None).await {
            Ok(_) => eprintln!("copied to {name}"),
            Err(error) => eprintln!(
                "warning: {name} keeps its previous choice ({error}); rerun `lf set machine` to copy it"
            ),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_key, Key};
    use crate::durable::MachineSetting;

    #[test]
    fn keys_name_shared_setup_machines_and_accounts() {
        assert_eq!(parse_key("skill_agents.implement").unwrap(), Key::Shared);
        assert_eq!(
            parse_key("machine.session").unwrap(),
            Key::Machine(&[MachineSetting::Session])
        );
        assert_eq!(
            parse_key("account.claude").unwrap(),
            Key::Account(crate::provider_auth::Provider::Claude)
        );
        for rejected in ["release", "skill_agents.", "account.pi", "machine.cpu"] {
            assert!(parse_key(rejected).is_err(), "{rejected}");
        }
    }
}
