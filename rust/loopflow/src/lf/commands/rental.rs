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

use crate::lf::{OndemandCommand, PoolCommand};
use loopflow_execution::machine::login;
use loopflow_execution::machine_records::CopyCounts;
use loopflow_execution::provider_auth::Provider as LoginProvider;
use loopflow_execution::rental::recovery::{
    delete_sandbox, give_back, held_ids, now, open_store, refresh_all,
};
use loopflow_execution::rental::sandbox::Sandboxes;
use loopflow_model::durable::{CapacityDefault, Machine, RepositoryId};
use loopflow_storage::rental::{
    self, Hardware, OnDemand, Pool, PoolMachine, Provider, Rental, RentalState, Usd,
};
use loopflow_storage::sqlite::SqliteStore;

/// An image-built sandbox answers SSH within seconds; RunPod and AWS
/// install the tools on stock Ubuntu first, which takes minutes.
const SSH_WAIT: Duration = Duration::from_secs(20 * 60);

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
    let mut defaults = loopflow_execution::store::placement::capacity_defaults(sqlite)?;
    defaults.retain(|default| default.capacity == capacity);
    Ok(defaults)
}

fn print_defaults(defaults: &[CapacityDefault]) {
    for default in defaults {
        let scope = match default.scope {
            loopflow_model::durable::DefaultScope::Wave(_) => "Wave",
            loopflow_model::durable::DefaultScope::Repository(_) => "repository",
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
            loopflow_execution::store::rentals::add_pool(sqlite, name)?;
            println!("Added pool {name}; `lf machine add <dest> --pool {name}` adds a machine");
        }
        PoolCommand::List { json } => {
            let listings = loopflow_execution::store::rentals::pools(sqlite)?
                .into_iter()
                .map(|pool| {
                    let machines =
                        loopflow_execution::store::machines::pool_machines(sqlite, &pool.name)?;
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
            loopflow_execution::store::rentals::set_pool_off(sqlite, name, true)?;
            println!("Pool {name} takes no new work; what it holds stays");
        }
        PoolCommand::On { name } => {
            loopflow_execution::store::rentals::set_pool_off(sqlite, name, false)?;
            println!("Pool {name} takes new work");
        }
        PoolCommand::Remove { name } => {
            loopflow_execution::store::rentals::remove_pool(sqlite, name)?;
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
    tasks: Vec<loopflow_model::durable::TaskId>,
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
            logins,
        } => {
            for provider in logins {
                login::require_reach(*provider, Some("a rental"))?;
            }
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
                logins: logins.clone(),
            };
            let price = ondemand.usd_per_hour().map_err(|reason| anyhow!(reason))?;
            loopflow_execution::store::rentals::add_ondemand(sqlite, &ondemand)?;
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
            let listings = loopflow_execution::store::rentals::ondemands(sqlite)?
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
            loopflow_execution::store::rentals::set_ondemand_max(sqlite, name, *max)?;
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
                        "Next: `lf machine login <provider> {label}` for each login it needs; \
                         launches carry `forward`; \
                         `lf machine end {label}` gives it back"
                    );
                }
            }
        }
        OndemandCommand::Remove { name } => {
            loopflow_execution::store::rentals::remove_ondemand(sqlite, name)?;
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
    loopflow_execution::store::rentals::ondemand(sqlite, name)?.ok_or_else(|| {
        anyhow!("no on-demand {name}; `lf ondemand add {name} --provider daytona …` names one")
    })
}

fn listing(sqlite: &SqliteStore, rental: Rental) -> anyhow::Result<RentalListing> {
    let (machine, tasks, copied) = match &rental.machine_id {
        Some(id) => (
            loopflow_execution::store::machines::machine_by_id(sqlite, id)?
                .and_then(|machine| machine.label),
            held_ids(sqlite, id)?,
            Some(sqlite.copy_counts(id)?).filter(|counts| counts.sessions > 0),
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
    let logins = match ondemand.logins.as_slice() {
        [] => String::new(),
        logins => format!(
            " · signs in {}",
            logins
                .iter()
                .map(|provider| provider.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    println!(
        "{} · {shape} · {live}/{} · {price}/h ≤ {}/h · {}h{logins}",
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
    let rental =
        loopflow_execution::store::rentals::admit_rental(sqlite, name, hours, max_usd, by_hand)?;
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
            loopflow_execution::store::ownership::set_rental_state(
                sqlite,
                &rental.id,
                RentalState::Unconfirmed,
            )?;
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
        None => first_contact(
            route,
            loopflow_execution::machine::connection::INSTALL_COMMAND,
            Stdio::null(),
        )?,
    };
    if !installed {
        return Err(anyhow!(
            "could not install lf on {}; it is deleted by {}",
            rental.provider_name,
            clock(rental.deadline)
        ));
    }
    let probe = runtime.block_on(loopflow_execution::machine::connection::probe(route, false))?;
    let label = next_label(sqlite, &rental.ondemand)?;
    let machine =
        loopflow_execution::store::machines::add_machine(sqlite, &probe.id?, route, &label)?;
    sqlite.attach_rental(&rental.id, &machine.id)?;
    Ok(machine)
}

/// `code-1`, `code-2`, …: the first label no machine carries.
fn next_label(sqlite: &SqliteStore, ondemand: &str) -> anyhow::Result<String> {
    let taken = loopflow_execution::store::machines::machines(sqlite)?
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
        let record = loopflow_execution::store::rentals::pool(sqlite, pool)?
            .ok_or_else(|| anyhow!("no pool {pool}; `lf machine pool add {pool}` creates it"))?;
        let machines = loopflow_execution::store::machines::pool_machines(sqlite, pool)?;
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
    // The Task's own repository, wherever this checkout was first recorded.
    let (_, task) = runtime.block_on(loopflow_execution::ops::task::resolve_saved_task(
        repo, None, issue,
    ))?;
    // A launch missing a value `forward` names, or with no agent login to
    // run on, is money for nothing.
    let forwarded = loopflow_execution::machine::forward::read(repo)?;
    let logins = sign_ins(&runtime, &require_ondemand(sqlite, name)?.logins)?;
    if !forwarded.carries_env(login::CLAUDE_TOKEN_ENV) && !logins.contains(&LoginProvider::Codex) {
        return Err(anyhow!(
            "a rental from {name} would have no agent login: forward {token} \
             (`lf set --global forward.env '[{token}]'` after `claude setup-token` and \
             `doppler secrets set {token}`), or sign it in to Codex (`lf machine login codex` here)",
            token = login::CLAUDE_TOKEN_ENV,
        ));
    }
    let (rental, machine) = rent(&runtime, sqlite, name, None, None, false)?;
    let label = machine.name();
    if let Err(error) = prepare(&runtime, &label, &logins, repo, &task.repository_id, batch) {
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
    let Ok((store, task)) = runtime.block_on(loopflow_execution::ops::task::resolve_saved_task(
        repo, None, issue,
    )) else {
        return Ok(());
    };
    let sqlite = &store.sqlite;
    if loopflow_execution::store::ownership::task_ownership(sqlite, &task.id)?
        .owner
        .is_some()
    {
        return Ok(());
    }
    let Some(capacity) = loopflow_execution::store::placement::task_capacity(sqlite, &task.id)?
    else {
        return Ok(());
    };
    let is_pool = loopflow_execution::store::rentals::pool(sqlite, &capacity)?.is_some();
    if !is_pool && loopflow_execution::store::rentals::ondemand(sqlite, &capacity)?.is_none() {
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
    use loopflow_execution::ops::task_ownership::{change, OwnershipChange};
    let (store, task) = runtime.block_on(loopflow_execution::ops::task::resolve_saved_task(
        repo, None, issue,
    ))?;
    if loopflow_execution::store::ownership::task_ownership(&store.sqlite, &task.id)?
        .owner
        .is_none()
    {
        change(repo, issue, OwnershipChange::Claim)?;
    }
    let ownership = change(repo, issue, OwnershipChange::Move(label.to_string()))?;
    eprintln!(
        "{}",
        loopflow_execution::ops::task_ownership::render(&ownership)
            .lines()
            .next()
            .unwrap_or("")
    );
    Ok(())
}

/// What a rental signs in to at prepare: the template's `logins`, else every
/// provider signed in here, in `sign_in_order`.
fn sign_ins(
    runtime: &tokio::runtime::Runtime,
    logins: &[LoginProvider],
) -> anyhow::Result<Vec<LoginProvider>> {
    if !logins.is_empty() {
        return Ok(sign_in_order(logins.iter().copied()));
    }
    let here = runtime.block_on(loopflow_execution::machine::connection::local_machine())?;
    Ok(sign_in_order(
        runtime
            .block_on(login::logins(&here))?
            .into_iter()
            .filter(|row| row.state == login::LoginState::SignedIn)
            .map(|row| row.provider),
    ))
}

/// `logins` signed in, then this repository cloned and bound to the Task's
/// repository identity, so the forwarded run finds its plan there.
fn prepare(
    runtime: &tokio::runtime::Runtime,
    label: &str,
    logins: &[LoginProvider],
    repo: &std::path::Path,
    repository: &RepositoryId,
    batch: bool,
) -> anyhow::Result<()> {
    let machine = runtime.block_on(loopflow_execution::machine::connection::resolve_target(
        label, false,
    ))?;
    for &provider in logins {
        runtime.block_on(login::login(&machine, provider, None, false, batch))?;
    }
    let (origin, script) = checkout_script(repo, repository)?;
    let status = Command::new("ssh")
        .args(loopflow_execution::machine_route::bounded_ssh_args(
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

/// The providers a rental signs in to, once each: those that reach another
/// Machine, GitHub first because the checkout clones with it.
fn sign_in_order(providers: impl IntoIterator<Item = LoginProvider>) -> Vec<LoginProvider> {
    let mut order = Vec::new();
    for provider in providers {
        if login::reaches_other_machines(provider) && !order.contains(&provider) {
            order.push(provider);
        }
    }
    order.sort_by_key(|provider| *provider != LoginProvider::GitHub);
    order
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
    let q = loopflow_execution::machine::ssh::sh_quote;
    // gh clones `owner/name` over HTTPS with the GitHub login just connected;
    // handed an SSH origin it would use SSH, which a fresh machine cannot verify.
    let clone = match loopflow_execution::git::worktrees::github_nwo(&origin) {
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
        "{} && {identity}mkdir -p ~/src && cd ~/src && {{ [ -d {dir} ] || {clone} {dir}; }} && cd {dir} && lf config repo --bind {repository}",
        loopflow_execution::machine::ssh::remote_path(),
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
    let machine = runtime.block_on(loopflow_execution::machine::connection::find_machine(
        &store, label,
    ))?;
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

#[cfg(test)]
mod tests {
    use super::{checkout_script, sign_in_order};
    use loopflow_execution::provider_auth::Provider;

    #[test]
    fn a_rental_signs_in_once_to_each_provider_that_reaches_it_github_first() {
        let signed_in_here = [
            Provider::Claude,
            Provider::Asana,
            Provider::Linear,
            Provider::GitHub,
            Provider::Doppler,
            Provider::Claude,
        ];
        assert_eq!(
            sign_in_order(signed_in_here),
            [Provider::GitHub, Provider::Linear]
        );
    }

    #[test]
    fn a_worktree_checkout_binds_its_tasks_repository_on_a_fresh_home() {
        let _env = loopflow_execution::journal::test_env_lock();
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
        let repository = loopflow_model::durable::RepositoryId::new();
        let (_, script) = checkout_script(&worktree, &repository).unwrap();
        assert!(script.ends_with(&format!("lf config repo --bind {repository}")));
        assert!(script.contains("gh repo clone 'loopflowstudio/loopflow'"));
        assert!(script.contains("git config --global user.email 'jack@example.com'"));
    }
}
