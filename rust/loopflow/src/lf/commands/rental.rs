//! `lf pool`, `lf ondemand` and `lf machine end`: owned machines, rented
//! machines, and giving rented ones back.
//!
//! Provider calls block, so these commands keep a runtime only for the async
//! store open and SSH probe and talk to SQLite directly.

use std::io::Seek;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Context};
use serde::Serialize;
use time::OffsetDateTime;

use crate::lf::{OndemandCommand, PoolCommand};
use loopflow_execution::installation::{ArtifactIdentity, ArtifactRole};
use loopflow_execution::machine_records::CopyCounts;
use loopflow_execution::rental::recovery::{
    delete_sandbox, end as end_rental, give_back, held_ids, now, open_store, refresh_all,
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
            reserved,
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
                    modal_sizing: reserved.then_some(rental::ModalSizing::Reserved),
                },
                max_machines: *max,
                max_usd_per_hour: *max_usd_per_hour,
                hours: hours.min(rental::MAX_HOURS),
            };
            let price = ondemand.usd_per_hour().map_err(|reason| anyhow!(reason))?;
            loopflow_execution::store::rentals::add_ondemand(sqlite, &ondemand)?;
            let hardware = ondemand.rental_hardware();
            println!(
                "Added on-demand {name}: {} on {} at {}; at most {max} at once, {}h each",
                hardware,
                where_rented(*provider),
                hardware.format_hourly_rate(price),
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
            loopflow_execution::machine::forward::read_for_rental(&std::env::current_dir()?)?;
            let (rental, machine) = rent(&runtime, sqlite, name, *hours, *max_usd, true)?;
            if *json {
                println!("{}", serde_json::to_string(&rental)?);
            } else {
                let label = machine.label.as_deref().unwrap_or(machine.id.as_str());
                println!(
                    "Rented {label}: {} at {}, at most {} until {}",
                    rental.hardware,
                    rental.hardware.format_hourly_rate(rental.usd_per_hour),
                    rental.cap_usd,
                    clock(rental.deadline)
                );
                if !batch {
                    println!(
                        "Next: `lf machine connect {label} github`; launches carry `forward`; \
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
    let hardware = ondemand.rental_hardware();
    let shape = format!("{hardware} on {}", ondemand.provider);
    let live = listing
        .rentals
        .iter()
        .filter(|r| r.rental.state.is_live())
        .count();
    let price = ondemand.usd_per_hour().map_or_else(
        |reason| format!("unpriced ({reason})"),
        |usd| hardware.format_hourly_rate(usd),
    );
    println!(
        "{} · {shape} · {live}/{} · {price} ≤ {}/h · {}h",
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
        let cost = rental.cost;
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
            "  {name} · {when} · {} · {} · {cost}{how}{}{copied}",
            rental.hardware,
            rental.hardware.format_hourly_rate(rental.usd_per_hour),
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
        "Renting {} on {} at {} until {}",
        rental.hardware,
        where_rented(ondemand.provider),
        rental.hardware.format_hourly_rate(rental.usd_per_hour),
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
fn first_contact(route: &str, script: &str) -> Command {
    let mut ssh = Command::new("ssh");
    ssh.args([
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=10",
        "--",
        route,
        script,
    ])
    .stdin(Stdio::null());
    ssh
}

fn run_bootstrap(route: &str, script: &str, stdin: Stdio) -> anyhow::Result<Vec<u8>> {
    let output = first_contact(route, script).stdin(stdin).output()?;
    // SSH diagnostics can contain Daytona's access token in the destination.
    anyhow::ensure!(output.status.code() != Some(255), "SSH connection failed");
    anyhow::ensure!(
        output.status.success(),
        "remote command failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output.stdout)
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

// Source experiments can carry unmerged migrations; keep their disposable home.
// Modal sandboxes set no-new-privileges, so no sudo: the `lf` on PATH is a
// wrapper that gives every SSH command that home.
const INSTALL_PRIVATE_CLI: &str = r#"set -e
mkdir -p ~/.local/bin ~/.local/libexec
candidate=$(mktemp ~/.local/libexec/lf.XXXXXXXX)
wrapper=$(mktemp ~/.local/bin/lf.XXXXXXXX)
trap 'rm -f "$candidate" "$wrapper"' EXIT
cat > "$candidate"
chmod 755 "$candidate"
cat > "$wrapper" <<'WRAPPER'
#!/bin/sh
export LF_HOME="$HOME/.lf-dev" LF_BIN="$HOME/.local/libexec/lf"
exec "$LF_BIN" "$@"
WRAPPER
chmod 755 "$wrapper"
mv "$candidate" ~/.local/libexec/lf
mv "$wrapper" ~/.local/bin/lf"#;

const INSTALL_PACKAGED_CLI: &str = r#"set -e
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
cat > "$dir/lf"
chmod 755 "$dir/lf"
"$dir/lf" install promote --cli-target "$HOME/.local/bin/lf" --sync-skills >&2
"$HOME/.local/bin/lf" install preflight --json"#;

/// Wait for SSH, install lf (published, or `LF_RENT_LINUX_BIN`), then add it
/// as a Machine.
fn attach(
    runtime: &tokio::runtime::Runtime,
    sqlite: &SqliteStore,
    rental: &Rental,
    route: &str,
    linux_lf: Option<&Path>,
) -> anyhow::Result<Machine> {
    forget_host_key(route);
    let deadline = SystemTime::now() + SSH_WAIT;
    let arch = loop {
        let output = first_contact(route, "uname -m").output()?;
        if output.status.success() {
            break String::from_utf8(output.stdout)?;
        }
        if SystemTime::now() >= deadline {
            return Err(anyhow!(
                "{} does not answer SSH after {} minutes; it is deleted by {}",
                rental.provider_name,
                SSH_WAIT.as_secs() / 60,
                clock(rental.deadline)
            ));
        }
        std::thread::sleep(Duration::from_secs(2));
    };
    let temporary = tempfile::tempdir()?;
    let revision = crate::build_info::source_revision();
    let (binary, script) = match linux_lf {
        Some(path) => (path.to_path_buf(), INSTALL_PRIVATE_CLI),
        None => {
            let binary = super::install::rental_cli(arch.trim(), temporary.path())
                .with_context(|| format!(
                    "could not fetch Linux lf for revision {revision}; set {LINUX_LF_ENV} to its Linux executable"
                ))?;
            (binary, INSTALL_PACKAGED_CLI)
        }
    };
    let expected = ArtifactIdentity::capture(ArtifactRole::Cli, &binary)?;
    let output = run_bootstrap(route, script, Stdio::from(std::fs::File::open(&binary)?))
        .with_context(|| format!("install lf on {}", rental.provider_name))?;
    if linux_lf.is_none() {
        let preflight: super::install::PromotionPreview =
            serde_json::from_slice(&output).context("read the installed Linux build's identity")?;
        anyhow::ensure!(
            preflight.candidate.source_revision == revision,
            "rental installed revision {}, expected {revision}; set {LINUX_LF_ENV} to its Linux executable",
            preflight.candidate.source_revision
        );
    }
    let caller = sqlite.saved_local_machine()?;
    let profile = loopflow_execution::config::remote::outgoing(None, caller.id.as_str())?;
    let mut input = tempfile::tempfile()?;
    serde_json::to_writer(&mut input, &profile)?;
    input.rewind()?;
    let command = format!(
        "{}; exec lf install server --profile-stdin",
        loopflow_execution::machine::ssh::remote_path()
    );
    let output = run_bootstrap(route, &command, Stdio::from(input))
        .with_context(|| format!("start the server on {}", rental.provider_name))?;
    let status: loopflow_execution::machine::client::Status =
        serde_json::from_slice(&output).context("read rental server readiness")?;
    anyhow::ensure!(
        status.ready && status.executable_sha256.as_ref() == Some(&expected.sha256),
        "the selected Linux lf is not serving {}",
        rental.provider_name
    );
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
    if placement.quote {
        quote(sqlite, placement)?;
        return Ok(true);
    }
    let (_, task) = runtime.block_on(loopflow_execution::ops::task::resolve_saved_task(
        repo, None, issue,
    ))?;
    let local = loopflow_execution::store::machines::local_machine(sqlite)?.id;
    let ownership = loopflow_execution::store::ownership::task_ownership(sqlite, &task.id)?;
    if let Some(owner) = ownership.owner_elsewhere(&local) {
        let name = owner.name();
        // Only the owner can tell whether an interrupted launch started its Graph.
        let detail = super::waves::remote_task_detail(repo, task.id.as_str())
            .with_context(|| format!("could not read {issue} on {name}"))?
            .ok_or_else(|| {
                anyhow!("{issue} is owned on {name}; check its move before running it")
            })?;
        let detail: super::task_row::TaskDetail = serde_json::from_value(detail)?;
        eprintln!("{issue} on {name}: {}", detail.task.standing());
        return Ok(detail.task.state.is_terminal() || !detail.graph_processes.is_empty());
    }
    if let Some(label) = free_pool_machine(sqlite, placement)? {
        hand_off(repo, task.id.as_str(), &label)?;
        return Ok(false);
    }
    let name = placement.rent.expect("checked: a pool or an on-demand");
    // A rental that could not authenticate its launch is money for nothing.
    loopflow_execution::machine::forward::read_for_rental(repo)?;
    let (rental, machine) = rent(&runtime, sqlite, name, None, None, false)?;
    let label = machine.name();
    if let Err(error) = prepare(&runtime, &machine, repo, &task, sqlite, batch) {
        // Nothing ran on it yet, so nothing is lost by giving it back now.
        let cleanup = Sandboxes::from_env(rental.provider)
            .and_then(|api| delete_sandbox(sqlite, &api, &rental));
        if let Err(cleanup) = cleanup {
            eprintln!("{label}: {cleanup:#}");
        }
        return Err(error);
    }
    if let Err(error) = hand_off(repo, task.id.as_str(), &label) {
        // A failure reading the receipt can follow a committed handoff.
        if !sqlite.machine_ever_held(&machine.id)? {
            let api = Sandboxes::from_env(rental.provider)?;
            give_back(sqlite, &api, &rental, &machine, true, false)?;
        }
        return Err(error);
    }
    Ok(false)
}

/// A free owned Machine in the placement's pool. Without one, `None` when an
/// on-demand can be rented instead.
fn free_pool_machine(
    sqlite: &SqliteStore,
    placement: &Placement<'_>,
) -> anyhow::Result<Option<String>> {
    let Some(pool) = placement.pool else {
        return Ok(None);
    };
    let record = loopflow_execution::store::rentals::pool(sqlite, pool)?
        .ok_or_else(|| anyhow!("no pool {pool}; `lf machine pool add {pool}` creates it"))?;
    let machines = loopflow_execution::store::machines::pool_machines(sqlite, pool)?;
    match rental::free_machine(&record, &machines, now()) {
        Ok(free) => Ok(Some(free.label().to_owned())),
        Err(refusal) if placement.rent.is_some() => {
            eprintln!("{refusal}; renting instead");
            Ok(None)
        }
        Err(refusal) => Err(refusal.into()),
    }
}

/// Say where the placement would run and what it would cost.
fn quote(sqlite: &SqliteStore, placement: &Placement<'_>) -> anyhow::Result<()> {
    if let Some(label) = free_pool_machine(sqlite, placement)? {
        let pool = placement.pool.expect("a free pool Machine has a pool");
        println!("would run on {label} (pool {pool}, owned); runs nothing");
        return Ok(());
    }
    let name = placement.rent.expect("checked: a pool or an on-demand");
    let ondemand = require_ondemand(sqlite, name)?;
    let live = sqlite.live_rentals(name)?;
    let admission = rental::admit(&ondemand, live, None, None, now())?;
    println!(
        "would rent {} on {} at {}, ≤ {} until {}; runs nothing",
        admission.hardware,
        where_rented(ondemand.provider),
        admission
            .hardware
            .format_hourly_rate(admission.usd_per_hour),
        admission.cap_usd,
        clock(admission.deadline)
    );
    Ok(())
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

/// Place the Task with one claim naming the destination.
fn hand_off(repo: &Path, issue: &str, label: &str) -> anyhow::Result<()> {
    use loopflow_execution::ops::task_ownership::{change, OwnershipChange};
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

/// Logins (Linear too when it holds the plan), then this repository cloned
/// and bound to the Task's repository identity, so the forwarded run finds its
/// plan there.
fn prepare(
    runtime: &tokio::runtime::Runtime,
    machine: &Machine,
    repo: &Path,
    task: &loopflow_storage::work::task::Task,
    sqlite: &SqliteStore,
    batch: bool,
) -> anyhow::Result<()> {
    use loopflow_execution::provider_auth::Provider as Login;
    runtime.block_on(loopflow_execution::machine::credentials::connect(
        machine,
        Login::GitHub,
        None,
        None,
        batch,
    ))?;
    let planning = loopflow_execution::config::load_config(Some(repo))?
        .unwrap_or_default()
        .planning_transport();
    let linear = matches!(
        planning,
        Some(loopflow_execution::config::TrackerConfig::Linear { .. })
    );
    if linear {
        runtime.block_on(loopflow_execution::machine::credentials::connect(
            machine,
            Login::Linear,
            None,
            None,
            batch,
        ))?;
    }
    let (origin, mut script) = checkout_script(repo, &task.repository_id)?;
    let wave = task.wave_id.as_ref().filter(|_| linear);
    if let Some(wave) = wave {
        let id = loopflow_execution::machine::ssh::sh_quote(wave.as_str());
        script.push_str(&format!(" && lf task list --wave {id} --json >/dev/null"));
    }
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
        return Err(anyhow!("could not prepare {origin} on {}", machine.name()));
    }
    if let Some(wave) = wave {
        use crate::server::wire::Operation;
        use loopflow_execution::machine::wire::{Secrets, Selection, Sender};
        let sender = Sender {
            repository: task.repository_id.clone(),
            selection: Selection::capture(repo, None)?,
            secrets: Secrets::default(),
        };
        let mut connection =
            loopflow_execution::machine::client::Connection::open_on(machine, sender, None)?;
        let plan = connection.request(Operation::Read(super::read::Read::WavePlan {
            directory: repo.to_owned(),
            wave: wave.to_string(),
        }))?;
        let plan: super::waves::WaveDetailSnapshot = serde_json::from_value(plan.result)?;
        if plan.project_readiness.state
            == loopflow_storage::sqlite::ProjectReadinessState::Unconfigured
        {
            let project = sqlite
                .selected_project(wave)?
                .and_then(|p| p.tracker_id)
                .with_context(|| {
                    format!(
                        "Wave {} needs a Linear Project. Select one with `lf wave plan bind`.",
                        plan.wave.name
                    )
                })?;
            connection.request(Operation::Write(super::write::Write::WavePlanBind {
                directory: repo.to_owned(),
                wave: wave.to_string(),
                project: project.as_str().to_owned(),
                replace: false,
            }))?;
        }
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
    end_rental(sqlite, &api, &rental, &machine, force)
}

#[cfg(test)]
mod tests {
    use super::checkout_script;
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
