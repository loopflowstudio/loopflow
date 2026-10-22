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

use crate::durable::{Machine, MachineId};
use crate::lf::{OndemandCommand, PoolCommand};
use crate::machine_records::CopyCounts;
use crate::rental::runpod::{self, CreatePod, RunPod};
use crate::rental::{
    self, GiveBack, Holdings, OnDemand, Pool, PoolMachine, Provider, Rental, RentalState,
    Resolution, Template, Usd,
};
use crate::store::sqlite::SqliteStore;
use crate::store::Store;

/// A booted pod has its address within minutes; tools take longer.
const ADDRESS_WAIT: Duration = Duration::from_secs(10 * 60);
const READY_WAIT: Duration = Duration::from_secs(20 * 60);

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
                    Ok(PoolListing { pool, machines })
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

#[derive(Debug, Serialize)]
struct Availability {
    ondemand: String,
    hardware: rental::Hardware,
    usd_per_hour: Usd,
    max_usd_per_hour: Usd,
    available: bool,
    in_use: u32,
    max: u32,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
}

pub fn run_ondemand(cmd: &OndemandCommand, batch: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let store = open_store(&runtime)?;
    let sqlite = &store.sqlite;
    match cmd {
        OndemandCommand::Add {
            name,
            provider,
            gpu,
            vcpus,
            memory_gb,
            disk_gb,
            image,
            data_center,
            hours,
            max,
            max_usd_per_hour,
        } => {
            let image = pin_image(image.as_deref().unwrap_or(rental::STOCK_IMAGE))?;
            sqlite.add_ondemand(&OnDemand {
                name: name.clone(),
                provider: *provider,
                template: Template {
                    gpu: gpu.clone(),
                    vcpus: *vcpus,
                    memory_gb: *memory_gb,
                    disk_gb: *disk_gb,
                    image,
                    data_center: data_center.clone(),
                },
                max_machines: *max,
                max_usd_per_hour: *max_usd_per_hour,
                hours: *hours,
                created_at: now(),
            })?;
            println!(
                "Added on-demand {name}: at most {max} at {max_usd_per_hour}/h, {hours}h each; \
                 `lf ondemand availability {name}` quotes it"
            );
        }
        OndemandCommand::List { json } => {
            // A listing, even one a client polls, never ends a paid machine.
            let api = RunPod::from_env().ok();
            if let Some(api) = &api {
                refresh_all(sqlite, api)?;
            }
            let listings = sqlite
                .ondemands()?
                .into_iter()
                .map(|ondemand| {
                    let rentals = sqlite
                        .rentals(Some(&ondemand.name))?
                        .into_iter()
                        .map(|rental| listing(sqlite, rental))
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    Ok(OndemandListing { ondemand, rentals })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            if *json {
                println!("{}", serde_json::to_string(&listings)?);
                return Ok(());
            }
            if api.is_none() && !listings.is_empty() {
                eprintln!(
                    "RUNPOD_API_KEY is not set; showing recorded states without asking RunPod"
                );
            }
            for listing in &listings {
                print_ondemand(listing);
            }
        }
        OndemandCommand::Availability { name, json } => {
            let ondemand = require_ondemand(sqlite, name)?;
            let api = RunPod::from_env()?;
            let quote = api.quote(&ondemand.template)?;
            let in_use = sqlite.live_rentals(name)?;
            let availability = Availability {
                ondemand: name.clone(),
                hardware: quote.hardware,
                usd_per_hour: quote.usd_per_hour,
                max_usd_per_hour: ondemand.max_usd_per_hour,
                available: quote.available,
                in_use,
                max: ondemand.max_machines,
                observed_at: now(),
            };
            if *json {
                println!("{}", serde_json::to_string(&availability)?);
            } else {
                let price = if availability.usd_per_hour <= availability.max_usd_per_hour {
                    "≤"
                } else {
                    ">"
                };
                let stock = if availability.available {
                    ""
                } else {
                    " · none in stock"
                };
                println!(
                    "{} · {}/h {price} {} · {}/{} in use · observed {}{stock}",
                    availability.hardware,
                    availability.usd_per_hour,
                    availability.max_usd_per_hour,
                    availability.in_use,
                    availability.max,
                    clock(availability.observed_at),
                );
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
            let api = RunPod::from_env()?;
            let (rental, machine) = rent(&runtime, sqlite, &api, name, *hours, *max_usd, true)?;
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

fn require_ondemand(sqlite: &SqliteStore, name: &str) -> anyhow::Result<OnDemand> {
    sqlite.ondemand(name)?.ok_or_else(|| {
        anyhow!("no on-demand {name}; `lf ondemand add {name} --provider runpod …` names one")
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
    let shape = match &ondemand.template.gpu {
        Some(gpu) => gpu.clone(),
        None => format!(
            "{} vCPU {} GB",
            ondemand.template.vcpus, ondemand.template.memory_gb
        ),
    };
    let live = listing
        .rentals
        .iter()
        .filter(|r| r.rental.state.is_live())
        .count();
    println!(
        "{} · {shape} · {live}/{} · ≤ {}/h · {}h",
        ondemand.name, ondemand.max_machines, ondemand.max_usd_per_hour, ondemand.hours
    );
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

/// Pin a registry image to its current digest so every rental boots the same bytes.
fn pin_image(image: &str) -> anyhow::Result<String> {
    if image.contains("@sha256:") {
        return Ok(image.to_string());
    }
    let (repository, tag) = image.rsplit_once(':').unwrap_or((image, "latest"));
    if repository.contains('.') && repository.contains('/') {
        return Err(anyhow!(
            "only Docker Hub images are pinned automatically; pass {image} as REF@sha256:DIGEST"
        ));
    }
    let repository = if repository.contains('/') {
        repository.to_string()
    } else {
        format!("library/{repository}")
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let query = serde_urlencoded::to_string([
        ("service", "registry.docker.io"),
        ("scope", &format!("repository:{repository}:pull")),
    ])?;
    let token: serde_json::Value = client
        .get(format!("https://auth.docker.io/token?{query}"))
        .send()?
        .error_for_status()?
        .json()?;
    let token = token["token"]
        .as_str()
        .ok_or_else(|| anyhow!("Docker Hub gave no pull token for {image}"))?;
    let reply = client
        .head(format!(
            "https://registry-1.docker.io/v2/{repository}/manifests/{tag}"
        ))
        .bearer_auth(token)
        .header(
            "Accept",
            "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.docker.distribution.manifest.v2+json",
        )
        .send()?
        .error_for_status()
        .with_context(|| format!("could not resolve {image}"))?;
    let digest = reply
        .headers()
        .get("Docker-Content-Digest")
        .and_then(|value| value.to_str().ok())
        .filter(|digest| digest.starts_with("sha256:"))
        .ok_or_else(|| anyhow!("Docker Hub returned no digest for {image}"))?;
    let name = image.rsplit_once(':').map_or(image, |(name, _)| name);
    Ok(format!("{name}@{digest}"))
}

// ---------------------------------------------------------------------------
// Renting
// ---------------------------------------------------------------------------

/// Admit, create once, then wait until the pod is an added Machine.
pub(crate) fn rent(
    runtime: &tokio::runtime::Runtime,
    sqlite: &SqliteStore,
    api: &RunPod,
    name: &str,
    hours: Option<f64>,
    max_usd: Option<Usd>,
    by_hand: bool,
) -> anyhow::Result<(Rental, Machine)> {
    let ondemand = require_ondemand(sqlite, name)?;
    if ondemand.provider != Provider::Runpod {
        return Err(anyhow!("{name} names a provider this lf cannot rent from"));
    }
    let public_key = public_key()?;
    let quote = api.quote(&ondemand.template)?;
    let rental = sqlite.admit_rental(name, &quote, hours, max_usd, by_hand)?;
    let (env, entrypoint) = runpod::boot(rental.deadline, rental.cap_usd, &public_key);
    let request = CreatePod {
        name: rental.provider_name.clone(),
        hardware: rental.hardware.clone(),
        image: ondemand.template.image.clone(),
        disk_gb: ondemand
            .template
            .disk_gb
            .unwrap_or_else(|| rental.hardware.disk_gb()),
        data_center: ondemand.template.data_center.clone(),
        env,
        entrypoint,
    };
    let pod = match api.create(&request) {
        Ok(pod) => pod,
        Err(error) => {
            sqlite.set_rental_state(&rental.id, RentalState::Unconfirmed)?;
            return Err(anyhow!(
                "{error}; it is unknown whether RunPod created {}. `lf ondemand list` finds it by name \
                 and adopts or forgets it; its guardian ends it by {} if it exists",
                rental.provider_name,
                clock(rental.deadline)
            ));
        }
    };
    let rental = sqlite.confirm_rental(&rental.id, &pod.id, pod.usd_per_hour)?;
    eprintln!(
        "Rented {} ({}) at {}/h; waiting for it to boot",
        rental.hardware, pod.id, rental.usd_per_hour
    );
    let machine = attach(runtime, sqlite, api, &rental)?;
    Ok((sqlite.rental(&rental.id)?, machine))
}

/// Wait for an address and the pod's ready marker, then add it as a Machine.
fn attach(
    runtime: &tokio::runtime::Runtime,
    sqlite: &SqliteStore,
    api: &RunPod,
    rental: &Rental,
) -> anyhow::Result<Machine> {
    let pod_id = rental
        .provider_id
        .as_deref()
        .ok_or_else(|| anyhow!("rental {} has no confirmed pod", rental.id))?;
    let started = Instant::now();
    let (ip, port) = loop {
        match api.pod(pod_id)? {
            Some(pod) if pod.is_gone() => {
                return Err(anyhow!("pod {pod_id} stopped before it booted"))
            }
            Some(pod) if pod.ssh.is_some() => break pod.ssh.expect("checked above"),
            None => return Err(anyhow!("pod {pod_id} disappeared before it booted")),
            Some(_) if started.elapsed() > ADDRESS_WAIT => {
                return Err(anyhow!(
                    "pod {pod_id} has no SSH address after {} minutes; its guardian ends it by {}",
                    ADDRESS_WAIT.as_secs() / 60,
                    clock(rental.deadline)
                ))
            }
            Some(_) => std::thread::sleep(Duration::from_secs(5)),
        }
    };
    let route = format!("ssh://lf@{ip}:{port}");
    // A fresh pod has a fresh host key; accept it once, then wait for its tools.
    let started = Instant::now();
    loop {
        let ready = Command::new("ssh")
            .args([
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=10",
                &route,
                "test -e /run/lf/ready",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if ready {
            break;
        }
        if started.elapsed() > READY_WAIT {
            return Err(anyhow!(
                "{route} did not finish installing lf within {} minutes; its guardian ends it by {}",
                READY_WAIT.as_secs() / 60,
                clock(rental.deadline)
            ));
        }
        std::thread::sleep(Duration::from_secs(10));
    }
    let probe = runtime.block_on(super::machine::probe(&route, false))?;
    let label = next_label(sqlite, &rental.ondemand)?;
    let machine = sqlite.add_machine(&probe.id?, &route, &label)?;
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
    let api = RunPod::from_env()?;
    if placement.quote {
        let ondemand = require_ondemand(sqlite, name)?;
        let quote = api.quote(&ondemand.template)?;
        let live = sqlite.live_rentals(name)?;
        let admission = rental::admit(&ondemand, live, &quote, None, None, now())?;
        println!(
            "would rent {} at {}/h, ≤ {} for {}h; runs nothing",
            admission.hardware, admission.usd_per_hour, admission.cap_usd, ondemand.hours
        );
        return Ok(true);
    }
    if batch {
        return Err(anyhow!(
            "renting for a Task signs a fresh machine into GitHub and Claude, one browser approval each; \
             run it without -b, or `lf ondemand rent {name}` and `lf machine connect` first"
        ));
    }
    let (_, machine) = rent(&runtime, sqlite, &api, name, None, None, false)?;
    let label = machine
        .label
        .clone()
        .unwrap_or_else(|| machine.id.to_string());
    prepare(&runtime, &label, repo, sqlite)?;
    hand_off(&runtime, repo, issue, &label)?;
    Ok(false)
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

/// Logins, then this repository cloned and bound to the same identity, so the
/// forwarded run finds its plan there.
fn prepare(
    runtime: &tokio::runtime::Runtime,
    label: &str,
    repo: &std::path::Path,
    sqlite: &SqliteStore,
) -> anyhow::Result<()> {
    use crate::provider_auth::Provider as Login;
    let machine = runtime.block_on(super::ssh::resolve_target(label, false))?;
    for login in [Login::GitHub, Login::Claude] {
        runtime.block_on(super::machine_credentials::connect(
            &machine, login, None, None, false,
        ))?;
    }
    let key = crate::repository::CanonicalRepo::discover(repo)?.to_string();
    let id = sqlite
        .repository_id(&key)?
        .ok_or_else(|| anyhow!("{} has no repository identity yet", repo.display()))?;
    let origin = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["remote", "get-url", "origin"])
        .output()
        .context("could not read the repository's origin")?;
    let origin = String::from_utf8(origin.stdout)?.trim().to_string();
    if origin.is_empty() {
        return Err(anyhow!("{} has no origin to clone", repo.display()));
    }
    let directory = repo
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("{} has no directory name", repo.display()))?;
    let q = super::ssh::sh_quote;
    let script = format!(
        "{} && mkdir -p ~/src && cd ~/src && {{ [ -d {dir} ] || gh repo clone {origin} {dir} || git clone {origin} {dir}; }} && cd {dir} && lf repo identity --bind {id}",
        super::machine::REMOTE_PATH,
        dir = q(directory),
        origin = q(&origin),
    );
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
        return Err(anyhow!(
            "could not clone {origin} on {label}; `lf machine end {label}` gives it back"
        ));
    }
    Ok(())
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
    let api = RunPod::from_env()?;
    give_back(sqlite, &api, &rental, &machine, true, force)
}

fn live_rentals(sqlite: &SqliteStore) -> anyhow::Result<Vec<Rental>> {
    let mut rentals = sqlite.rentals(None)?;
    rentals.retain(|rental| rental.state.is_live());
    Ok(rentals)
}

/// Bring every live rental up to date with its provider, ending none.
fn refresh_all(sqlite: &SqliteStore, api: &RunPod) -> anyhow::Result<()> {
    for rental in live_rentals(sqlite)? {
        if let Err(error) = refresh(sqlite, api, &rental) {
            eprintln!("{}: {error:#}", rental.provider_name);
        }
    }
    Ok(())
}

/// Refresh one rental, then give it back if it is a Task rental whose work
/// has drained or whose soft deadline came.
fn reconcile(sqlite: &SqliteStore, api: &RunPod, rental: &Rental) -> anyhow::Result<()> {
    let Some(rental) = refresh(sqlite, api, rental)? else {
        return Ok(());
    };
    let Some(machine_id) = &rental.machine_id else {
        return Ok(());
    };
    let Some(machine) = sqlite.machine_by_id(machine_id)? else {
        return Ok(());
    };
    give_back(sqlite, api, &rental, &machine, false, false)
}

/// Adopt or lose an unconfirmed create, notice a pod already gone, and update
/// cost. Returns the rental when its pod is still there.
fn refresh(sqlite: &SqliteStore, api: &RunPod, rental: &Rental) -> anyhow::Result<Option<Rental>> {
    let pod = match &rental.provider_id {
        // Requested or unconfirmed: only the name can find it.
        None => {
            match rental::resolve_unconfirmed(
                api.named(&rental.provider_name)?,
                rental.created_at,
                now(),
            ) {
                Resolution::Adopt(pod) => {
                    sqlite.confirm_rental(&rental.id, &pod.id, pod.usd_per_hour)?;
                    pod
                }
                Resolution::Lost => {
                    sqlite.set_rental_state(&rental.id, RentalState::Lost)?;
                    return Ok(None);
                }
                Resolution::Wait => return Ok(None),
            }
        }
        Some(id) => match api.pod(id)? {
            Some(pod) if !pod.is_gone() => pod,
            // Its guardian, or someone, already deleted it.
            _ => {
                sqlite.set_rental_state(&rental.id, RentalState::Ended)?;
                return Ok(None);
            }
        },
    };
    sqlite.set_rental_cost(&rental.id, api.billed(&pod, now())?)?;
    Ok(Some(sqlite.rental(&rental.id)?))
}

fn give_back(
    sqlite: &SqliteStore,
    api: &RunPod,
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
                    delete_pod(sqlite, api, rental)?;
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

/// Copy the machine's records here, then delete its pod. A failed copy
/// deletes nothing: the pod may hold the only record of its work.
fn end_rental(
    sqlite: &SqliteStore,
    api: &RunPod,
    rental: &Rental,
    copy: impl FnOnce() -> anyhow::Result<CopyCounts>,
) -> anyhow::Result<Ended> {
    match copy() {
        Ok(counts) => {
            delete_pod(sqlite, api, rental)?;
            Ok(Ended::Copied(counts))
        }
        Err(error) => Ok(Ended::Uncopied(error)),
    }
}

fn delete_pod(sqlite: &SqliteStore, api: &RunPod, rental: &Rental) -> anyhow::Result<()> {
    sqlite.set_rental_state(&rental.id, RentalState::Ending)?;
    let pod_id = rental
        .provider_id
        .as_deref()
        .ok_or_else(|| anyhow!("rental {} has no confirmed pod", rental.id))?;
    if let Some(pod) = api.delete(pod_id, &rental.provider_name)? {
        sqlite.set_rental_cost(&rental.id, api.billed(&pod, now())?)?;
    }
    sqlite.set_rental_state(&rental.id, RentalState::Ended)?;
    Ok(())
}

/// Pull the machine's records since its rental began (`lf history export`).
fn copy_up(sqlite: &SqliteStore, rental: &Rental, machine: &Machine) -> anyhow::Result<CopyCounts> {
    // An hour early absorbs clock skew between the laptop and the pod.
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
        let api = RunPod::from_env()
            .context("`lf machine end` gives it back once RUNPOD_API_KEY is set")?;
        reconcile(sqlite, &api, &rental)
    })();
    if let Err(error) = result {
        eprintln!("{machine}: {error:#}");
    }
}

/// The scheduled Task check's give-back pass. Without `RUNPOD_API_KEY` it
/// reports only rentals already past their soft deadline, which then wait for
/// `lf machine end` or their guardian.
///
/// # Errors
/// When the store cannot be read, or a rental is due and the key is missing.
pub fn check() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let store = open_store(&runtime)?;
    let sqlite = &store.sqlite;
    let live = live_rentals(sqlite)?;
    if live.is_empty() {
        return Ok(());
    }
    let Ok(api) = RunPod::from_env() else {
        let due: Vec<&str> = live
            .iter()
            .filter(|rental| now() >= rental.soft_deadline())
            .map(|rental| rental.provider_name.as_str())
            .collect();
        anyhow::ensure!(
            due.is_empty(),
            "{} past the soft deadline, and RUNPOD_API_KEY is not set here; \
             `lf machine end` gives each back, or its guardian deletes it at the deadline",
            due.join(", ")
        );
        return Ok(());
    };
    for rental in &live {
        if let Err(error) = reconcile(sqlite, &api, rental) {
            eprintln!("{}: {error:#}", rental.provider_name);
        }
    }
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
        unpushed,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use serde_json::json;

    use super::{end_rental, read_holdings, refresh_all, Ended};
    use crate::durable::MachineId;
    use crate::machine_records::CopyCounts;
    use crate::rental::runpod::RunPod;
    use crate::rental::{Hardware, OnDemand, Provider, Quote, Rental, RentalState, Template, Usd};
    use crate::store::sqlite::SqliteStore;

    type Log = Rc<RefCell<Vec<String>>>;

    /// A running rental of `code-1` and a RunPod that knows its pod.
    fn running(dir: &std::path::Path) -> (SqliteStore, Rental, RunPod, Log) {
        let store = SqliteStore::open_ephemeral(&dir.join("db")).unwrap();
        store
            .add_ondemand(&OnDemand {
                name: "code".into(),
                provider: Provider::Runpod,
                template: Template {
                    gpu: None,
                    vcpus: 8,
                    memory_gb: 16,
                    disk_gb: None,
                    image: "runpod/base@sha256:ab".into(),
                    data_center: None,
                },
                max_machines: 1,
                max_usd_per_hour: Usd::parse("0.40").unwrap(),
                hours: 4.0,
                created_at: time::OffsetDateTime::now_utc(),
            })
            .unwrap();
        let quote = Quote {
            hardware: Hardware::Cpu {
                flavor: "cpu3c".into(),
                vcpus: 8,
            },
            usd_per_hour: Usd::parse("0.24").unwrap(),
            available: true,
        };
        let rental = store
            .admit_rental("code", &quote, None, None, false)
            .unwrap();
        store
            .confirm_rental(&rental.id, "pod1", quote.usd_per_hour)
            .unwrap();
        let machine = MachineId::new();
        store
            .add_machine(&machine, "ssh://lf@203.0.113.7:40022", "code-1")
            .unwrap();
        let rental = store.attach_rental(&rental.id, &machine).unwrap();
        let log: Log = Rc::default();
        let kept = log.clone();
        let name = rental.provider_name.clone();
        let api = RunPod::with_send(Box::new(move |method, url, _| {
            kept.borrow_mut().push(format!("{method} {url}"));
            let reply = if url.contains("billing") {
                json!({"records": []})
            } else {
                json!({"id": "pod1", "name": name, "cost": 0.24,
                       "createdAt": "2026-10-21T20:10:00Z", "status": "RUNNING"})
            };
            Ok((200, reply.to_string().into_bytes()))
        }));
        (store, rental, api, log)
    }

    fn deleted(log: &Log) -> bool {
        log.borrow().iter().any(|call| call.starts_with("DELETE"))
    }

    #[test]
    fn give_back_copies_before_it_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let (store, rental, api, log) = running(dir.path());
        let ended = end_rental(&store, &api, &rental, || {
            assert!(
                !deleted(&log),
                "the pod was deleted before its records were copied"
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
    fn listing_a_drained_rental_leaves_it_and_its_pod() {
        let dir = tempfile::tempdir().unwrap();
        let (store, rental, api, log) = running(dir.path());
        // Rented for a Task, holding none, with its pod still billing: drained.
        assert!(!rental.by_hand);
        refresh_all(&store, &api).unwrap();
        assert!(!deleted(&log));
        assert_eq!(
            store.rental(&rental.id).unwrap().state,
            RentalState::Running
        );
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
