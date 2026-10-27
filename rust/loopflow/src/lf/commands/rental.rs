//! `lf pool`, `lf ondemand` and `lf machine end`: owned machines, rented
//! machines, and giving rented ones back.
//!
//! Provider calls block, so these commands keep a runtime only for the async
//! store open and SSH probe and talk to SQLite directly.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};
use serde::Serialize;
use time::OffsetDateTime;

use crate::durable::{CapacityDefault, Machine, MachineId, RepositoryId};
use crate::lf::{OndemandCommand, PoolCommand};
use crate::machine_records::CopyCounts;
use crate::rental::sandbox::Sandboxes;
use crate::rental::{
    self, GiveBack, Hardware, Holdings, OnDemand, Pool, PoolMachine, Provider, Rental, RentalState,
    Resolution, Usd,
};
use crate::store::sqlite::SqliteStore;
use crate::store::Store;

/// An image-built sandbox answers SSH within seconds; RunPod and AWS
/// install the tools on stock Ubuntu first, which takes minutes.
const SSH_WAIT: Duration = Duration::from_secs(20 * 60);

fn open_store(runtime: &tokio::runtime::Runtime) -> anyhow::Result<Store> {
    runtime
        .block_on(crate::store::open_existing_store())
        .ok_or_else(|| anyhow!("this command needs an initialized local store"))
}

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

/// Local wall-clock time, as people read deadlines.
fn clock(at: OffsetDateTime) -> String {
    chrono::DateTime::from_timestamp(at.unix_timestamp(), 0)
        .map(|at| at.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|| at.to_string())
}

// ---------------------------------------------------------------------------
// lf pool
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct PoolListing {
    #[serde(flatten)]
    pool: Pool,
    machines: Vec<PoolMachine>,
    /// The Waves and repositories whose new Tasks it takes.
    defaults: Vec<CapacityDefault>,
}

/// The scopes whose new Tasks go to this pool or on-demand.
fn defaults_for(sqlite: &SqliteStore, capacity: &str) -> anyhow::Result<Vec<CapacityDefault>> {
    let mut defaults = sqlite.capacity_defaults()?;
    defaults.retain(|default| default.capacity == capacity);
    Ok(defaults)
}

fn print_defaults(defaults: &[CapacityDefault]) {
    for default in defaults {
        let scope = match default.scope {
            crate::durable::DefaultScope::Wave(_) => "Wave",
            crate::durable::DefaultScope::Repository(_) => "repository",
        };
        println!("  takes new Tasks of {scope} {}", default.name);
    }
}

pub fn run_pool(cmd: &PoolCommand) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let store = open_store(&runtime)?;
    let sqlite = &store.sqlite;
    match cmd {
        PoolCommand::Add { name } => {
            sqlite.add_pool(name)?;
            println!("Added pool {name}; `lf machine add <dest> --pool {name}` adds a machine");
        }
        PoolCommand::List { json } => {
            let listings = sqlite
                .pools()?
                .into_iter()
                .map(|pool| {
                    let machines = sqlite.pool_machines(&pool.name)?;
                    let defaults = defaults_for(sqlite, &pool.name)?;
                    Ok(PoolListing {
                        pool,
                        machines,
                        defaults,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            if *json {
                println!("{}", serde_json::to_string(&listings)?);
                return Ok(());
            }
            let now = now();
            for listing in &listings {
                let off = if listing.pool.off { " (off)" } else { "" };
                println!("{}{off}", listing.pool.name);
                print_defaults(&listing.defaults);
                for machine in &listing.machines {
                    let expiry = match machine.ownership.expires_at {
                        Some(at) if at <= now => "expired".to_string(),
                        Some(at) => format!("expires {}", at.date()),
                        None => "never expires".to_string(),
                    };
                    let held = if machine.tasks.is_empty() {
                        "free".to_string()
                    } else {
                        let tasks = machine.tasks.iter().map(ToString::to_string);
                        format!("holds {}", tasks.collect::<Vec<_>>().join(" "))
                    };
                    println!(
                        "  {} · {} · {expiry} · {held}",
                        machine.label(),
                        machine.ownership.cost_usd
                    );
                }
            }
        }
        PoolCommand::Off { name } => {
            sqlite.set_pool_off(name, true)?;
            println!("Pool {name} takes no new work; what it holds stays");
        }
        PoolCommand::On { name } => {
            sqlite.set_pool_off(name, false)?;
            println!("Pool {name} takes new work");
        }
        PoolCommand::Remove { name } => {
            sqlite.remove_pool(name)?;
            println!("Removed pool {name}; its machines stay added");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// lf ondemand
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct OndemandListing {
    #[serde(flatten)]
    ondemand: OnDemand,
    rentals: Vec<RentalListing>,
    /// The Waves and repositories whose new Tasks it rents for.
    defaults: Vec<CapacityDefault>,
}

#[derive(Debug, Serialize)]
struct RentalListing {
    #[serde(flatten)]
    rental: Rental,
    machine: Option<String>,
    tasks: Vec<crate::durable::TaskId>,
    /// What has been copied from it, once anything has.
    copied: Option<CopyCounts>,
}

pub fn run_ondemand(cmd: &OndemandCommand, batch: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let store = open_store(&runtime)?;
    let sqlite = &store.sqlite;
    match cmd {
        OndemandCommand::Add {
            name,
            provider,
            vcpus,
            memory_gb,
            disk_gb,
            gpu,
            spot,
            hours,
            max,
            max_usd_per_hour,
        } => {
            let ondemand = OnDemand {
                name: name.clone(),
                provider: *provider,
                hardware: Hardware {
                    vcpus: *vcpus,
                    memory_gb: *memory_gb,
                    disk_gb: *disk_gb,
                    gpu: gpu.clone(),
                    spot: *spot,
                },
                max_machines: *max,
                max_usd_per_hour: *max_usd_per_hour,
                hours: hours.min(rental::MAX_HOURS),
                created_at: now(),
            };
            let price = ondemand.usd_per_hour().map_err(|reason| anyhow!(reason))?;
            sqlite.add_ondemand(&ondemand)?;
            println!(
                "Added on-demand {name}: {} on {} at {price}/h; at most {max} at once, {}h each",
                ondemand.hardware,
                where_rented(*provider),
                ondemand.hours
            );
            if *spot {
                eprintln!(
                    "{provider} may end a spot machine before its deadline; \
                     work not pushed from it is lost"
                );
            }
            if price > *max_usd_per_hour {
                eprintln!(
                    "{price}/h is above its {max_usd_per_hour}/h limit, so it rents nothing yet"
                );
            }
        }
        OndemandCommand::List { json } => {
            // A listing, even one a client polls, never ends a paid machine.
            let unreached = refresh_all(sqlite);
            let listings = sqlite
                .ondemands()?
                .into_iter()
                .map(|ondemand| {
                    let rentals = sqlite
                        .rentals(Some(&ondemand.name))?
                        .into_iter()
                        .map(|rental| listing(sqlite, rental))
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    let defaults = defaults_for(sqlite, &ondemand.name)?;
                    Ok(OndemandListing {
                        ondemand,
                        rentals,
                        defaults,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            if *json {
                println!("{}", serde_json::to_string(&listings)?);
                return Ok(());
            }
            for reason in unreached {
                eprintln!("{reason}; showing recorded states");
            }
            for listing in &listings {
                print_ondemand(listing);
            }
        }
        OndemandCommand::Max { name, max } => {
            sqlite.set_ondemand_max(name, *max)?;
            println!("{name} rents at most {max} at once; current machines stay");
        }
        OndemandCommand::Rent {
            name,
            hours,
            max_usd,
            json,
        } => {
            let (rental, machine) = rent(&runtime, sqlite, name, *hours, *max_usd, true)?;
            if *json {
                println!("{}", serde_json::to_string(&rental)?);
            } else {
                let label = machine.label.as_deref().unwrap_or(machine.id.as_str());
                println!(
                    "Rented {label}: {} at {}/h, at most {} until {}",
                    rental.hardware,
                    rental.usd_per_hour,
                    rental.cap_usd,
                    clock(rental.deadline)
                );
                if !batch {
                    println!(
                        "Next: `lf machine connect {label} github` and `lf machine connect {label} claude`; \
                         `lf machine end {label}` gives it back"
                    );
                }
            }
        }
        OndemandCommand::Remove { name } => {
            sqlite.remove_ondemand(name)?;
            println!("Removed on-demand {name}; its rentals' history stays");
        }
    }
    Ok(())
}

/// The provider, and its region where lf rents in one: `aws (eu-west-1, priced as us-east-1)`.
fn where_rented(provider: Provider) -> String {
    match provider.region() {
        Some(region) => format!("{provider} ({region})"),
        None => provider.to_string(),
    }
}

fn require_ondemand(sqlite: &SqliteStore, name: &str) -> anyhow::Result<OnDemand> {
    sqlite.ondemand(name)?.ok_or_else(|| {
        anyhow!("no on-demand {name}; `lf ondemand add {name} --provider daytona …` names one")
    })
}

fn listing(sqlite: &SqliteStore, rental: Rental) -> anyhow::Result<RentalListing> {
    let (machine, tasks, copied) = match &rental.machine_id {
        Some(id) => (
            sqlite.machine_by_id(id)?.and_then(|machine| machine.label),
            sqlite.tasks_held_by(id)?,
            Some(sqlite.copy_counts(id)?).filter(|counts| counts.sessions + counts.processes > 0),
        ),
        None => (None, Vec::new(), None),
    };
    Ok(RentalListing {
        rental,
        machine,
        tasks,
        copied,
    })
}

fn print_ondemand(listing: &OndemandListing) {
    let ondemand = &listing.ondemand;
    let shape = format!("{} on {}", ondemand.hardware, ondemand.provider);
    let live = listing
        .rentals
        .iter()
        .filter(|r| r.rental.state.is_live())
        .count();
    let price = ondemand.usd_per_hour().map_or_else(
        |reason| format!("unpriced ({reason})"),
        |usd| usd.to_string(),
    );
    println!(
        "{} · {shape} · {live}/{} · {price}/h ≤ {}/h · {}h",
        ondemand.name, ondemand.max_machines, ondemand.max_usd_per_hour, ondemand.hours
    );
    print_defaults(&listing.defaults);
    for entry in &listing.rentals {
        let rental = &entry.rental;
        let name = entry.machine.as_deref().unwrap_or(rental.id.as_str());
        let when = match rental.ended_at {
            Some(at) => format!("{} {}", rental.state, clock(at)),
            None => format!("{} until {}", rental.state, clock(rental.deadline)),
        };
        let cost = rental
            .cost_usd
            .map_or_else(|| "cost unknown".into(), |cost| cost.to_string());
        let tasks = entry
            .tasks
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        let how = if rental.by_hand { " · by hand" } else { "" };
        let copied = entry
            .copied
            .map_or_else(String::new, |counts| format!(" · {counts} copied"));
        println!(
            "  {name} · {when} · {} · {}/h · {cost}{how}{}{copied}",
            rental.hardware,
            rental.usd_per_hour,
            if tasks.is_empty() {
                String::new()
            } else {
                format!(" · {tasks}")
            }
        );
    }
}

// ---------------------------------------------------------------------------
// Renting
// ---------------------------------------------------------------------------

/// Admit, create once, then install lf and add the sandbox as a Machine.
pub(crate) fn rent(
    runtime: &tokio::runtime::Runtime,
    sqlite: &SqliteStore,
    name: &str,
    hours: Option<f64>,
    max_usd: Option<Usd>,
    by_hand: bool,
) -> anyhow::Result<(Rental, Machine)> {
    let ondemand = require_ondemand(sqlite, name)?;
    let api = Sandboxes::from_env(ondemand.provider)?;
    let public_key = public_key()?;
    let linux_lf = linux_lf()?;
    let rental = sqlite.admit_rental(name, hours, max_usd, by_hand)?;
    eprintln!(
        "Renting {} on {} at {}/h until {}",
        rental.hardware,
        where_rented(ondemand.provider),
        rental.usd_per_hour,
        clock(rental.deadline)
    );
    let sandbox = match api.create(&rental, &public_key) {
        Ok(sandbox) => sandbox,
        Err(error) => {
            sqlite.set_rental_state(&rental.id, RentalState::Unconfirmed)?;
            return Err(anyhow!(
                "{error}; it is unknown whether {} created {}. `lf ondemand list` finds it by name \
                 and gives it back",
                ondemand.provider,
                rental.provider_name
            ));
        }
    };
    // A provider that reports what it charges overrides the published price;
    // the guardian shortens the deadline by the same price.
    let usd_per_hour = sandbox.usd_per_hour.unwrap_or(rental.usd_per_hour);
    let rental = sqlite.confirm_rental(&rental.id, &sandbox.id, usd_per_hour)?;
    let route = sandbox.route.expect("a created sandbox has a route");
    let machine = match attach(runtime, sqlite, &rental, &route, linux_lf.as_deref()) {
        Ok(machine) => machine,
        Err(error) => {
            // Nothing ran on it yet, so nothing is lost by giving it back now.
            if let Err(cleanup) = delete_sandbox(sqlite, &api, &rental) {
                eprintln!("{}: {cleanup:#}", rental.provider_name);
            }
            return Err(error);
        }
    };
    Ok((sqlite.rental(&rental.id)?, machine))
}

/// `LF_RENT_LINUX_BIN`: a Linux `lf` the sandbox runs instead of the published
/// release, so a development build can rent a sandbox running its own code.
fn linux_lf() -> anyhow::Result<Option<std::path::PathBuf>> {
    let Some(path) = std::env::var_os(LINUX_LF_ENV).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let path = std::path::PathBuf::from(path);
    if !path.is_file() {
        return Err(anyhow!("{LINUX_LF_ENV} names no file: {}", path.display()));
    }
    Ok(Some(path))
}

const LINUX_LF_ENV: &str = "LF_RENT_LINUX_BIN";

/// SSH that trusts a sandbox's fresh host key once; Machine routes then
/// check it strictly like any other.
fn first_contact(route: &str, script: &str, stdin: Stdio) -> std::io::Result<bool> {
    Command::new("ssh")
        .args([
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            route,
            script,
        ])
        .stdin(stdin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
}

/// A tunnel port may be reused by a later sandbox with another host key.
fn forget_host_key(route: &str) {
    let address = route.trim_start_matches("ssh://");
    let address = address.rsplit_once('@').map_or(address, |(_, host)| host);
    let entry = match address.rsplit_once(':') {
        Some((host, port)) if port != "22" => format!("[{host}]:{port}"),
        Some((host, _)) => host.to_string(),
        None => address.to_string(),
    };
    let _ = Command::new("ssh-keygen")
        .args(["-R", &entry])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Wait for SSH, install lf (published, or `LF_RENT_LINUX_BIN`), then add it
/// as a Machine.
fn attach(
    runtime: &tokio::runtime::Runtime,
    sqlite: &SqliteStore,
    rental: &Rental,
    route: &str,
    linux_lf: Option<&std::path::Path>,
) -> anyhow::Result<Machine> {
    forget_host_key(route);
    let started = Instant::now();
    while !first_contact(route, "true", Stdio::null())? {
        if started.elapsed() > SSH_WAIT {
            return Err(anyhow!(
                "{} does not answer SSH after {} minutes; it is deleted by {}",
                rental.provider_name,
                SSH_WAIT.as_secs() / 60,
                clock(rental.deadline)
            ));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    let installed = match linux_lf {
        // A development build runs as one does anywhere: on its own disposable
        // home, which it may create and which the published install never
        // claims. sshd sessions read the selection from /etc/environment.
        Some(binary) => {
            println!("Uploading {} as its lf", binary.display());
            first_contact(
                route,
                "mkdir -p ~/.local/bin && cat > ~/.local/bin/lf.new && chmod 755 ~/.local/bin/lf.new && mv ~/.local/bin/lf.new ~/.local/bin/lf \
                 && printf 'LF_HOME=%s/.lf-dev\\nLF_BIN=%s/.local/bin/lf\\n' \"$HOME\" \"$HOME\" | sudo tee -a /etc/environment >/dev/null",
                Stdio::from(std::fs::File::open(binary)?),
            )?
        }
        None => first_contact(route, super::machine::INSTALL_COMMAND, Stdio::null())?,
    };
    if !installed {
        return Err(anyhow!(
            "could not install lf on {}; it is deleted by {}",
            rental.provider_name,
            clock(rental.deadline)
        ));
    }
    let probe = runtime.block_on(super::machine::probe(route, false))?;
    let label = next_label(sqlite, &rental.ondemand)?;
    let machine = sqlite.add_machine(&probe.id?, route, &label)?;
    sqlite.attach_rental(&rental.id, &machine.id)?;
    Ok(machine)
}

/// `code-1`, `code-2`, …: the first label no machine carries.
fn next_label(sqlite: &SqliteStore, ondemand: &str) -> anyhow::Result<String> {
    let taken = sqlite
        .machines()?
        .into_iter()
        .filter_map(|machine| machine.label)
        .collect::<std::collections::HashSet<_>>();
    Ok((1..)
        .map(|n| format!("{ondemand}-{n}"))
        .find(|label| !taken.contains(label))
        .expect("an unbounded range has a free label"))
}

fn public_key() -> anyhow::Result<String> {
    let ssh = dirs::home_dir()
        .ok_or_else(|| anyhow!("no home directory"))?
        .join(".ssh");
    ["id_ed25519.pub", "id_ecdsa.pub", "id_rsa.pub"]
        .iter()
        .find_map(|name| std::fs::read_to_string(ssh.join(name)).ok())
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            anyhow!(
                "no SSH public key in {}; `ssh-keygen -t ed25519` makes one",
                ssh.display()
            )
        })
}

// ---------------------------------------------------------------------------
// Placing a Task
// ---------------------------------------------------------------------------

/// Where `lf task run` should take a Task: a free owned machine, else a rented one.
#[derive(Debug, Clone, Copy)]
pub struct Placement<'a> {
    pub pool: Option<&'a str>,
    pub rent: Option<&'a str>,
    /// Say what it would take and cost; place and run nothing.
    pub quote: bool,
}

/// Choose or rent a Machine and hand the Task to it. Returns true when
/// nothing more should run (a quote).
pub fn place(
    repo: &std::path::Path,
    issue: &str,
    placement: &Placement<'_>,
    batch: bool,
) -> anyhow::Result<bool> {
    if placement.pool.is_none() && placement.rent.is_none() {
        return Err(anyhow!("--quote needs --pool or --rent"));
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let store = open_store(&runtime)?;
    let sqlite = &store.sqlite;
    if let Some(pool) = placement.pool {
        let record = sqlite
            .pool(pool)?
            .ok_or_else(|| anyhow!("no pool {pool}; `lf pool add {pool}` creates it"))?;
        let machines = sqlite.pool_machines(pool)?;
        match rental::free_machine(&record, &machines, now()) {
            Ok(free) if placement.quote => {
                println!(
                    "would run on {} (pool {pool}, owned); runs nothing",
                    free.label()
                );
                return Ok(true);
            }
            Ok(free) => {
                hand_off(&runtime, repo, issue, free.label())?;
                return Ok(false);
            }
            Err(refusal) if placement.rent.is_some() => eprintln!("{refusal}; renting instead"),
            Err(refusal) => return Err(refusal.into()),
        }
    }
    let name = placement.rent.expect("checked: a pool or an on-demand");
    if placement.quote {
        let ondemand = require_ondemand(sqlite, name)?;
        let live = sqlite.live_rentals(name)?;
        let admission = rental::admit(&ondemand, live, None, None, now())?;
        println!(
            "would rent {} on {} at {}/h, ≤ {} until {}; runs nothing",
            admission.hardware,
            where_rented(ondemand.provider),
            admission.usd_per_hour,
            admission.cap_usd,
            clock(admission.deadline)
        );
        return Ok(true);
    }
    if batch {
        runtime.block_on(async {
            let accounts = crate::provider_account::open_account_store().await?;
            super::machine_credentials::require_unattended_logins(&accounts).await
        })?;
    }
    // The Task's own repository, wherever this checkout was first recorded.
    let (_, task) = runtime.block_on(crate::ops::task::resolve_saved_task(repo, None, issue))?;
    let (rental, machine) = rent(&runtime, sqlite, name, None, None, false)?;
    let label = machine.name();
    if let Err(error) = prepare(&runtime, &label, repo, &task.repository_id, batch) {
        // Nothing ran on it yet, so nothing is lost by giving it back now.
        let cleanup = Sandboxes::from_env(rental.provider)
            .and_then(|api| delete_sandbox(sqlite, &api, &rental));
        if let Err(cleanup) = cleanup {
            eprintln!("{label}: {cleanup:#}");
        }
        return Err(error);
    }
    hand_off(&runtime, repo, issue, &label)?;
    Ok(false)
}

/// The value Git resolves for `key` in `repo`, when one is set.
fn git_config(repo: &std::path::Path, key: &str) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", key])
        .output()
        .ok()?;
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (output.status.success() && !value.is_empty()).then_some(value)
}

/// An unowned Task whose Wave or repository names a pool or on-demand goes
/// there, as `--pool` or `--rent` would send it. Claimed Tasks stay put.
pub fn place_by_default(repo: &std::path::Path, issue: &str, batch: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    // A Task not saved yet has no scope; the run itself reports why.
    let Ok((store, task)) =
        runtime.block_on(crate::ops::task::resolve_saved_task(repo, None, issue))
    else {
        return Ok(());
    };
    let sqlite = &store.sqlite;
    if sqlite.task_ownership(&task.id)?.owner.is_some() {
        return Ok(());
    }
    let Some(capacity) = sqlite.task_capacity(&task.id)? else {
        return Ok(());
    };
    let is_pool = sqlite.pool(&capacity)?.is_some();
    if !is_pool && sqlite.ondemand(&capacity)?.is_none() {
        return Err(anyhow!(
            "{issue}'s default capacity {capacity} is no pool or on-demand here; \
             `lf set machine <machine>` replaces it"
        ));
    }
    let placement = Placement {
        pool: is_pool.then_some(capacity.as_str()),
        rent: (!is_pool).then_some(capacity.as_str()),
        quote: false,
    };
    drop(store);
    place(repo, issue, &placement, batch).map(|_| ())
}

/// Your claim moves to the Machine; an unowned Task is claimed first.
fn hand_off(
    runtime: &tokio::runtime::Runtime,
    repo: &std::path::Path,
    issue: &str,
    label: &str,
) -> anyhow::Result<()> {
    use crate::ops::task_ownership::{change, OwnershipChange};
    let (store, task) =
        runtime.block_on(crate::ops::task::resolve_saved_task(repo, None, issue))?;
    if store.sqlite.task_ownership(&task.id)?.owner.is_none() {
        change(repo, issue, OwnershipChange::Claim)?;
    }
    let ownership = change(repo, issue, OwnershipChange::Handoff(label.to_string()))?;
    eprintln!(
        "{}",
        crate::ops::task_ownership::render(&ownership)
            .lines()
            .next()
            .unwrap_or("")
    );
    Ok(())
}

/// Logins (Linear too when it holds the plan), then this repository cloned
/// and bound to the Task's repository identity, so the forwarded run finds its
/// plan there.
fn prepare(
    runtime: &tokio::runtime::Runtime,
    label: &str,
    repo: &std::path::Path,
    repository: &RepositoryId,
    batch: bool,
) -> anyhow::Result<()> {
    use crate::provider_auth::Provider as Login;
    let machine = runtime.block_on(super::ssh::resolve_target(label, false))?;
    runtime.block_on(super::machine_credentials::connect(
        &machine,
        Login::GitHub,
        None,
        None,
        batch,
    ))?;
    runtime.block_on(super::machine_credentials::connect_claude_accounts(
        &machine, batch,
    ))?;
    let planning = crate::config::load_config(Some(repo))?
        .unwrap_or_default()
        .planning_transport();
    if matches!(planning, crate::config::PlanningConfig::Linear { .. }) {
        runtime.block_on(super::machine_credentials::connect(
            &machine,
            Login::Linear,
            None,
            None,
            batch,
        ))?;
    }
    let (origin, script) = checkout_script(repo, repository)?;
    let status = Command::new("ssh")
        .args(crate::machine_route::bounded_ssh_args(
            &machine.route,
            false,
        )?)
        .arg(script)
        .stdin(Stdio::null())
        .status()
        .context("could not run ssh")?;
    if !status.success() {
        return Err(anyhow!("could not clone {origin} on {label}"));
    }
    Ok(())
}

/// The origin, and the remote script that clones it and binds `repository`.
fn checkout_script(
    repo: &std::path::Path,
    repository: &RepositoryId,
) -> anyhow::Result<(String, String)> {
    let origin = git_config(repo, "remote.origin.url")
        .ok_or_else(|| anyhow!("{} has no origin to clone", repo.display()))?;
    let directory = repo
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("{} has no directory name", repo.display()))?;
    let q = super::ssh::sh_quote;
    // gh clones `owner/name` over HTTPS with the GitHub login just connected;
    // handed an SSH origin it would use SSH, which a fresh machine cannot verify.
    let clone = match crate::git::worktrees::github_nwo(&origin) {
        Some((owner, name)) => format!("gh repo clone {}", q(&format!("{owner}/{name}"))),
        None => format!("git clone {}", q(&origin)),
    };
    // Claiming the Task there names its person by Git identity, as here.
    let identity: String = ["user.name", "user.email"]
        .iter()
        .filter_map(|key| {
            Some(format!(
                "git config --global {key} {} && ",
                q(&git_config(repo, key)?)
            ))
        })
        .collect();
    let script = format!(
        "{} && {identity}mkdir -p ~/src && cd ~/src && {{ [ -d {dir} ] || {clone} {dir}; }} && cd {dir} && lf repository --bind {repository}",
        super::machine::REMOTE_PATH,
        dir = q(directory),
    );
    Ok((origin, script))
}

// ---------------------------------------------------------------------------
// Giving back
// ---------------------------------------------------------------------------

/// `lf machine end`: give a rented machine back now.
pub fn end(label: &str, force: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let store = open_store(&runtime)?;
    let machine = runtime.block_on(super::machine::find_machine(&store, label))?;
    let sqlite = &store.sqlite;
    let rental = sqlite.rental_for_machine(&machine.id)?.ok_or_else(|| {
        anyhow!("{label} is owned, and owned machines are never deleted; `lf machine remove {label}` forgets it")
    })?;
    if !rental.state.is_live() {
        println!("{label} was already given back ({})", rental.state);
        return Ok(());
    }
    let api = Sandboxes::from_env(rental.provider)?;
    give_back(sqlite, &api, &rental, &machine, true, force)
}

fn live_rentals(sqlite: &SqliteStore) -> anyhow::Result<Vec<Rental>> {
    let mut rentals = sqlite.rentals(None)?;
    rentals.retain(|rental| rental.state.is_live());
    Ok(rentals)
}

/// Bring every live rental up to date with its provider, ending none.
/// Returns why some could not be asked.
fn refresh_all(sqlite: &SqliteStore) -> Vec<String> {
    let mut unreached = Vec::new();
    let rentals = match live_rentals(sqlite) {
        Ok(rentals) => rentals,
        Err(error) => return vec![format!("{error:#}")],
    };
    for rental in rentals {
        let result = Sandboxes::from_env(rental.provider)
            .and_then(|api| refresh(sqlite, &api, &rental).map(|_| ()));
        if let Err(error) = result {
            unreached.push(format!("{}: {error:#}", rental.provider_name));
        }
    }
    unreached
}

/// Refresh one rental, then give it back if it is a Task rental whose work
/// has drained or whose soft deadline came.
fn reconcile(sqlite: &SqliteStore, api: &Sandboxes, rental: &Rental) -> anyhow::Result<()> {
    let Some(rental) = refresh(sqlite, api, rental)? else {
        return Ok(());
    };
    let Some(machine_id) = &rental.machine_id else {
        if rental.boot_abandoned(now()) {
            delete_sandbox(sqlite, api, &rental)?;
        }
        return Ok(());
    };
    let Some(machine) = sqlite.machine_by_id(machine_id)? else {
        return Ok(());
    };
    give_back(sqlite, api, &rental, &machine, false, false)
}

/// Settle an unconfirmed create, notice a sandbox already gone, and update
/// cost. Returns the rental when its sandbox is still there and usable.
fn refresh(
    sqlite: &SqliteStore,
    api: &Sandboxes,
    rental: &Rental,
) -> anyhow::Result<Option<Rental>> {
    let id = match &rental.provider_id {
        // Requested or unconfirmed: only the name can find it. A sandbox whose
        // create never answered has no route, so it is given back unused.
        None => match rental::resolve_unconfirmed(
            api.named(&rental.provider_name)?,
            rental.created_at,
            now(),
        ) {
            Resolution::Adopt(sandbox) => {
                sqlite.confirm_rental(&rental.id, &sandbox.id, rental.usd_per_hour)?;
                delete_sandbox(sqlite, api, &sqlite.rental(&rental.id)?)?;
                return Ok(None);
            }
            Resolution::Lost => {
                sqlite.set_rental_state(&rental.id, RentalState::Lost)?;
                return Ok(None);
            }
            Resolution::Wait => return Ok(None),
        },
        Some(id) => id,
    };
    match api.get(id)? {
        Some(sandbox) if !sandbox.gone => {}
        // Its lifetime passed, or someone deleted it.
        ended => {
            // What an ended machine leaves behind (an exited pod's disk) still bills until it is deleted.
            if ended.is_some() {
                api.delete(id)?;
            }
            sqlite.set_rental_cost(&rental.id, rental.cost_until(rental.deadline.min(now())))?;
            mark_ended(sqlite, rental)?;
            return Ok(None);
        }
    }
    sqlite.set_rental_cost(&rental.id, rental.cost_until(now()))?;
    Ok(Some(sqlite.rental(&rental.id)?))
}

fn give_back(
    sqlite: &SqliteStore,
    api: &Sandboxes,
    rental: &Rental,
    machine: &Machine,
    explicit: bool,
    force: bool,
) -> anyhow::Result<()> {
    let label = machine.label.as_deref().unwrap_or(machine.id.as_str());
    let mut holdings = remote_holdings(&machine.route).unwrap_or_else(|error| {
        // An unreachable machine holds nothing we could copy or check; the
        // deadline still decides, and `--force` is what an explicit end needs.
        eprintln!("{label}: {error:#}");
        Holdings {
            unpushed: vec!["(unreachable: unpushed work unknown)".into()],
            ..Holdings::default()
        }
    });
    holdings.tasks = sqlite.tasks_held_by(&machine.id)?;
    holdings.placed = sqlite.machine_ever_held(&machine.id)?;
    holdings.finished_at = sqlite.work_finished_at(&machine.id)?;
    match rental::give_back(rental, &holdings, explicit, force, now()) {
        GiveBack::Keep(reason) => {
            if explicit {
                println!("{label} stays: {reason}");
            }
            Ok(())
        }
        GiveBack::Refuse(unpushed) => {
            let message = format!(
                "{label} holds the only copy of: {}; push it, or `lf machine end {label} --force` discards it",
                unpushed.join(", ")
            );
            if explicit {
                Err(anyhow!(message))
            } else {
                eprintln!("{message}");
                Ok(())
            }
        }
        GiveBack::End => {
            let copied = match end_rental(sqlite, api, rental, || copy_up(sqlite, rental, machine))?
            {
                Ended::Copied(counts) => Some(counts),
                Ended::Uncopied(error) if !force => {
                    let message = format!(
                        "{label} stays: its Session and Process records could not be copied ({error:#}); \
                         `lf machine end {label} --force` deletes it without them"
                    );
                    return if explicit {
                        Err(anyhow!(message))
                    } else {
                        eprintln!("{message}");
                        Ok(())
                    };
                }
                Ended::Uncopied(error) => {
                    eprintln!("Discarded on {label}: Session and Process records ({error:#})");
                    delete_sandbox(sqlite, api, rental)?;
                    None
                }
            };
            if force && !holdings.unpushed.is_empty() {
                eprintln!("Discarded on {label}: {}", holdings.unpushed.join(", "));
            }
            let ended = sqlite.rental(&rental.id)?;
            println!(
                "Gave back {label}{}{}",
                ended
                    .cost_usd
                    .map_or_else(String::new, |cost| format!(" after {cost}")),
                copied.map_or_else(String::new, |counts| format!("; {counts} copied here"))
            );
            Ok(())
        }
    }
}

#[derive(Debug)]
enum Ended {
    Copied(CopyCounts),
    /// The copy failed, so nothing was deleted.
    Uncopied(anyhow::Error),
}

/// Copy the machine's records here, then delete its sandbox. A failed copy
/// deletes nothing: the sandbox may hold the only record of its work.
fn end_rental(
    sqlite: &SqliteStore,
    api: &Sandboxes,
    rental: &Rental,
    copy: impl FnOnce() -> anyhow::Result<CopyCounts>,
) -> anyhow::Result<Ended> {
    match copy() {
        Ok(counts) => {
            delete_sandbox(sqlite, api, rental)?;
            Ok(Ended::Copied(counts))
        }
        Err(error) => Ok(Ended::Uncopied(error)),
    }
}

fn delete_sandbox(sqlite: &SqliteStore, api: &Sandboxes, rental: &Rental) -> anyhow::Result<()> {
    sqlite.set_rental_state(&rental.id, RentalState::Ending)?;
    let id = rental
        .provider_id
        .as_deref()
        .ok_or_else(|| anyhow!("rental {} has no confirmed sandbox", rental.id))?;
    api.delete(id)?;
    sqlite.set_rental_cost(&rental.id, rental.cost_until(now()))?;
    mark_ended(sqlite, rental)
}

/// Every path to `ended` comes here: the Tasks the Machine still owns come
/// back to this one, so the next `lf task run` runs here, not on a gone sandbox.
fn mark_ended(sqlite: &SqliteStore, rental: &Rental) -> anyhow::Result<()> {
    let (_, returned) = sqlite.set_rental_state(&rental.id, RentalState::Ended)?;
    let here = crate::store::sqlite::task_ownership::local_label();
    for task in returned {
        let name = sqlite
            .task(&task)?
            .map_or_else(|| task.to_string(), |task| task.identifier);
        println!("{name} back on {here}");
    }
    Ok(())
}

/// Pull the machine's records since its rental began (`lf history export`).
fn copy_up(sqlite: &SqliteStore, rental: &Rental, machine: &Machine) -> anyhow::Result<CopyCounts> {
    // An hour early absorbs clock skew between the laptop and the sandbox.
    let since = (rental.created_at - time::Duration::hours(1))
        .format(&time::format_description::well_known::Rfc3339)?;
    let script = format!(
        "{}; lf history export --since {}",
        super::machine::REMOTE_PATH,
        super::ssh::sh_quote(&since)
    );
    let output = Command::new("ssh")
        .args(crate::machine_route::bounded_ssh_args(
            &machine.route,
            false,
        )?)
        .arg(script)
        .stdin(Stdio::null())
        .output()
        .context("could not run ssh")?;
    if !output.status.success() {
        return Err(super::machine::transport_failure(
            &machine.route,
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    crate::machine_records::import(
        sqlite,
        &crate::store::lf_home_dir(),
        &machine.id,
        output.stdout.as_slice(),
    )
}

/// After a run forwarded to `machine` returns: if it is a live rental, give
/// it back when its work has drained. Problems are reported, never raised;
/// `lf machine end` or the periodic Task check gives it back later.
pub fn settle(machine: &MachineId) {
    let result = (|| -> anyhow::Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        let store = open_store(&runtime)?;
        let sqlite = &store.sqlite;
        let Some(rental) = sqlite.rental_for_machine(machine)? else {
            return Ok(());
        };
        if !rental.state.is_live() {
            return Ok(());
        }
        let api = Sandboxes::from_env(rental.provider)
            .context("`lf machine end` gives it back once its provider can be reached")?;
        reconcile(sqlite, &api, &rental)
    })();
    if let Err(error) = result {
        eprintln!("{machine}: {error:#}");
    }
}

/// Lazy recovery's give-back pass. A rental whose provider this process
/// cannot reach is reported only once its soft deadline passes; its provider
/// deletes it at the hard deadline either way.
///
/// # Errors
/// When the store cannot be read, or a due rental's provider cannot be reached.
pub fn check() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let store = open_store(&runtime)?;
    let sqlite = &store.sqlite;
    let mut due = Vec::new();
    for rental in live_rentals(sqlite)? {
        let api = match Sandboxes::from_env(rental.provider) {
            Ok(api) => api,
            Err(error) => {
                if now() >= rental.soft_deadline() {
                    due.push(format!("{}: {error:#}", rental.provider_name));
                }
                continue;
            }
        };
        if let Err(error) = reconcile(sqlite, &api, &rental) {
            eprintln!("{}: {error:#}", rental.provider_name);
        }
    }
    anyhow::ensure!(
        due.is_empty(),
        "past the soft deadline: {}; `lf machine end` gives each back, or its provider deletes it at the deadline",
        due.join("; ")
    );
    Ok(())
}

/// Live agent processes and repositories with work not on any remote.
const HOLDINGS: &str = r#"pgrep -u "$(id -u)" -x lf | wc -l
pgrep -u "$(id -u)" -x claude | wc -l
for d in "$HOME"/src/*/; do
  [ -e "$d/.git" ] || continue
  [ -n "$(git -C "$d" status --porcelain 2>/dev/null)" ] && echo "unpushed $d uncommitted changes"
  n=$(git -C "$d" rev-list --count HEAD --not --remotes 2>/dev/null || echo 0)
  [ "$n" != 0 ] && echo "unpushed $d $n commit(s) not on a remote"
done
true"#;

fn remote_holdings(route: &str) -> anyhow::Result<Holdings> {
    let output = Command::new("ssh")
        .args(crate::machine_route::bounded_ssh_args(route, false)?)
        .arg(HOLDINGS)
        .stdin(Stdio::null())
        .output()
        .context("could not run ssh")?;
    if !output.status.success() {
        return Err(super::machine::transport_failure(
            route,
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    Ok(read_holdings(&String::from_utf8_lossy(&output.stdout)))
}

fn read_holdings(text: &str) -> Holdings {
    let mut lines = text.lines();
    let mut count = || {
        lines
            .next()
            .and_then(|line| line.trim().parse::<usize>().ok())
            .unwrap_or(0)
    };
    let live_processes = count() + count();
    let unpushed = text
        .lines()
        .filter_map(|line| line.strip_prefix("unpushed "))
        .map(str::to_string)
        .collect();
    Holdings {
        placed: false,
        tasks: Vec::new(),
        live_processes,
        finished_at: None,
        unpushed,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{checkout_script, end_rental, read_holdings, reconcile, refresh, Ended};
    use crate::durable::MachineId;
    use crate::machine_records::CopyCounts;
    use crate::rental::sandbox::Sandboxes;
    use crate::rental::{Hardware, OnDemand, Provider, Rental, RentalState, Usd, BOOT_SECONDS};
    use crate::store::sqlite::SqliteStore;

    type Log = Rc<RefCell<Vec<String>>>;

    /// A running rental of `code-1` and a provider that knows its sandbox.
    /// Created and confirmed, with no Machine yet: what a boot leaves.
    fn booting(dir: &std::path::Path) -> (SqliteStore, Rental, Sandboxes, Log) {
        let store = SqliteStore::open_ephemeral(&dir.join("db")).unwrap();
        store
            .add_ondemand(&OnDemand {
                name: "code".into(),
                provider: Provider::Daytona,
                hardware: Hardware {
                    vcpus: 4,
                    memory_gb: 8,
                    disk_gb: 10,
                    gpu: None,
                    spot: false,
                },
                max_machines: 1,
                max_usd_per_hour: Usd::parse("0.40").unwrap(),
                hours: 4.0,
                created_at: time::OffsetDateTime::now_utc(),
            })
            .unwrap();
        let rental = store.admit_rental("code", None, None, false).unwrap();
        let rental = store
            .confirm_rental(&rental.id, "sb-1", rental.usd_per_hour)
            .unwrap();
        let log: Log = Rc::default();
        let kept = log.clone();
        let api = Sandboxes::with_call(
            Provider::Daytona,
            Box::new(move |op, _| {
                kept.borrow_mut().push(op.to_string());
                Ok(br#"{"id": "sb-1", "gone": false, "route": null}"#.to_vec())
            }),
        );
        (store, rental, api, log)
    }

    fn running(dir: &std::path::Path) -> (SqliteStore, Rental, Sandboxes, Log) {
        let (store, rental, api, log) = booting(dir);
        let machine = MachineId::new();
        store
            .add_machine(&machine, "ssh://lf@203.0.113.7:40022", "code-1")
            .unwrap();
        let rental = store.attach_rental(&rental.id, &machine).unwrap();
        (store, rental, api, log)
    }

    fn deleted(log: &Log) -> bool {
        log.borrow().iter().any(|op| op == "delete")
    }

    #[test]
    fn give_back_copies_before_it_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let (store, rental, api, log) = running(dir.path());
        let ended = end_rental(&store, &api, &rental, || {
            assert!(
                !deleted(&log),
                "the sandbox was deleted before its records were copied"
            );
            Ok(CopyCounts {
                sessions: 3,
                processes: 14,
            })
        })
        .unwrap();
        assert!(matches!(ended, Ended::Copied(counts) if counts.processes == 14));
        assert!(deleted(&log));
        assert_eq!(store.rental(&rental.id).unwrap().state, RentalState::Ended);
    }

    #[test]
    fn a_failed_copy_leaves_the_machine_running() {
        let dir = tempfile::tempdir().unwrap();
        let (store, rental, api, log) = running(dir.path());
        let ended = end_rental(&store, &api, &rental, || {
            Err(anyhow::anyhow!("ssh timed out"))
        })
        .unwrap();
        assert!(matches!(ended, Ended::Uncopied(_)));
        assert!(!deleted(&log));
        assert_eq!(
            store.rental(&rental.id).unwrap().state,
            RentalState::Running
        );
    }

    #[test]
    fn refreshing_a_drained_rental_leaves_it_and_its_sandbox() {
        let dir = tempfile::tempdir().unwrap();
        let (store, rental, api, log) = running(dir.path());
        // Rented for a Task, holding none, with its sandbox still billing: drained.
        assert!(!rental.by_hand);
        assert!(refresh(&store, &api, &rental).unwrap().is_some());
        assert!(!deleted(&log));
        assert_eq!(
            store.rental(&rental.id).unwrap().state,
            RentalState::Running
        );
    }

    #[test]
    fn a_boot_whose_lf_died_is_given_back_once_its_window_passes() {
        let dir = tempfile::tempdir().unwrap();
        let (store, rental, api, log) = booting(dir.path());
        assert_eq!(rental.state, RentalState::Booting);
        reconcile(&store, &api, &rental).unwrap();
        assert!(!deleted(&log), "a fresh boot may still be running");
        let later = rental.created_at + time::Duration::seconds(BOOT_SECONDS);
        assert!(rental.boot_abandoned(later));
    }

    #[test]
    fn refreshing_an_ended_machine_deletes_what_it_left() {
        let dir = tempfile::tempdir().unwrap();
        let (store, rental, _, _) = running(dir.path());
        let log: Log = Rc::default();
        let kept = log.clone();
        // An exited pod still holds its disk.
        let api = Sandboxes::with_call(
            Provider::Runpod,
            Box::new(move |op, _| {
                kept.borrow_mut().push(op.to_string());
                Ok(br#"{"id": "sb-1", "gone": true, "route": null}"#.to_vec())
            }),
        );
        assert!(refresh(&store, &api, &rental).unwrap().is_none());
        assert!(deleted(&log));
        assert_eq!(store.rental(&rental.id).unwrap().state, RentalState::Ended);
    }

    #[test]
    fn a_worktree_checkout_binds_its_tasks_repository_on_a_fresh_home() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("loopflow");
        let git = |cwd: &std::path::Path, args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(cwd)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        };
        std::fs::create_dir(&main).unwrap();
        git(&main, &["init", "-q"]);
        git(&main, &["config", "user.name", "Jack Heart"]);
        git(&main, &["config", "user.email", "jack@example.com"]);
        git(
            &main,
            &[
                "remote",
                "add",
                "origin",
                "git@github.com:loopflowstudio/loopflow.git",
            ],
        );
        git(&main, &["commit", "-q", "--allow-empty", "-m", "start"]);
        let worktree = dir.path().join("loopflow.task");
        git(
            &main,
            &["worktree", "add", "-q", worktree.to_str().unwrap()],
        );
        // No store records either checkout: the Task alone names the repository.
        let repository = crate::durable::RepositoryId::new();
        let (_, script) = checkout_script(&worktree, &repository).unwrap();
        assert!(script.ends_with(&format!("lf repository --bind {repository}")));
        assert!(script.contains("gh repo clone 'loopflowstudio/loopflow'"));
        assert!(script.contains("git config --global user.email 'jack@example.com'"));
    }

    #[test]
    fn holdings_count_agents_and_name_unpushed_work() {
        let holdings = read_holdings(
            "1\n2\nunpushed /home/lf/src/loopflow/ uncommitted changes\n\
             unpushed /home/lf/src/loopflow/ 3 commit(s) not on a remote\n",
        );
        assert_eq!(holdings.live_processes, 3);
        assert_eq!(holdings.unpushed.len(), 2);
        assert!(holdings.unpushed[1].contains("3 commit(s)"));
    }
}
