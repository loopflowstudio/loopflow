//! The hub: one reachable Machine that a plan without Linear or Asana is
//! exchanged with live. It carries stamped records and decides nothing; this
//! is which Machine it is, one exchange with it, and its answers.
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};

use crate::engine::{Error, ErrorCode};
use crate::machine::client::Connection;
use crate::machine::wire::{PlanChanges, Secrets, Selection, Sender, ServerOperation};
use loopflow_model::durable::{Machine, RepositoryId};
use loopflow_storage::sqlite::plan_carriers::Carrier;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::edit::StampedRecord;

/// About half a control message, so one request or answer never needs a
/// ticketed connection.
const BUDGET: usize = crate::machine::wire::MAX_MESSAGE / 2;

/// The Machine this one exchanges its plan with; `None` on a Machine with no
/// hub or that is its own hub.
pub fn hub(store: &SqliteStore) -> Result<Option<Machine>> {
    let config = crate::config::placement::global_settings_at(&store.home_dir()?)?;
    let Some(id) = config.hub else {
        return Ok(None);
    };
    if crate::store::machines::local_machine(store)?.id == id {
        return Ok(None);
    }
    crate::store::machines::machine_by_id(store, &id)?
        .map(Some)
        .ok_or_else(|| anyhow!("hub {id} is not an added Machine; run `lf machine add` for it, or `lf machine hub --clear`"))
}

/// `lf machine hub <machine>` and `--clear` (`None`). The plan stays on this
/// Machine either way; a new hub is sent all of it once.
pub fn set_hub(
    store: &SqliteStore,
    hub: Option<&loopflow_model::durable::MachineId>,
) -> Result<()> {
    crate::config::placement::write_global(
        &store.home_dir()?,
        &["hub"],
        hub.map(|id| id.as_str().into()),
    )?;
    if hub.is_none() {
        store.forget_carrier(Carrier::Hub)?;
    }
    Ok(())
}

// ---- Another Machine's side ----

/// Exchange `repo`'s plan with this Machine's hub, when it has one and the
/// repository has no Linear or Asana. With `send_only`, see [`exchange`].
pub async fn sync(store: &crate::store::Store, repo: &str, send_only: bool) -> Result<()> {
    if crate::ops::tracker::connected(repo) {
        return Ok(());
    }
    let Some(hub) = hub(&store.sqlite)? else {
        return Ok(());
    };
    let (sqlite, repo) = (store.sqlite.clone(), repo.to_owned());
    crate::journal::request::spawn_blocking(move || exchange(&sqlite, &repo, &hub, send_only))
        .await?
}

/// After a command on this Machine's server answers: one exchange for its
/// repository, on a thread of its own. Commands arriving while one runs, or
/// within [`QUIET`] of the last, share it.
pub fn after_command(directory: std::path::PathBuf) {
    std::thread::spawn(move || {
        if let Err(error) = exchange_after_command(&directory) {
            tracing::debug!(error = %format!("{error:#}"), "hub exchange after a command failed");
        }
    });
}

const QUIET: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
struct Turn {
    running: bool,
    again: bool,
    finished: Option<Instant>,
}

static TURNS: Mutex<BTreeMap<String, Turn>> = Mutex::new(BTreeMap::new());

fn exchange_after_command(directory: &Path) -> Result<()> {
    // Most Machines have no hub: answer that before any Git or SQLite.
    if crate::config::placement::global_settings_at(&crate::store::lf_home_dir())?
        .hub
        .is_none()
    {
        return Ok(());
    }
    let repo = crate::repository::discover(directory)?.to_string();
    if crate::ops::tracker::connected(&repo) {
        return Ok(());
    }
    let path = crate::store::default_db_path();
    if !path.exists() {
        return Ok(());
    }
    let store = crate::store::open_sqlite(&path)?;
    let Some(hub) = hub(&store)? else {
        return Ok(());
    };
    {
        let mut turns = TURNS.lock().expect("hub turns are never poisoned");
        let turn = turns.entry(repo.clone()).or_default();
        if turn.running {
            turn.again = true;
            return Ok(());
        }
        if turn.finished.is_some_and(|at| at.elapsed() < QUIET) {
            return Ok(());
        }
        turn.running = true;
    }
    loop {
        let result = exchange(&store, &repo, &hub, false);
        let mut turns = TURNS.lock().expect("hub turns are never poisoned");
        let turn = turns.entry(repo.clone()).or_default();
        if std::mem::take(&mut turn.again) && result.is_ok() {
            continue;
        }
        turn.running = false;
        turn.finished = Some(Instant::now());
        return result;
    }
}

/// Send `hub` the records of `repo` it has not taken, then take in what
/// changed there. Either order gives the same plan. With `send_only`, a
/// repository with nothing unsent that has read the hub before asks it
/// nothing. The outcome is recorded for `lf repo tracker`.
pub fn exchange(store: &SqliteStore, repo: &str, hub: &Machine, send_only: bool) -> Result<()> {
    let Some(repository) = store.repository_id(repo)? else {
        return Ok(());
    };
    // One exchange per repository at a time; the next one reads what this one wrote.
    let _turn = lock(store, &repository)?;
    let held = store.select_carrier(repo, Carrier::Hub, hub.id.as_str())?;
    if send_only && held.seen_seq.is_some() && store.plan_unsent(repo, Carrier::Hub, 0)?.count == 0
    {
        return Ok(());
    }
    let result = (|| {
        let mut connection = open(store, repo, hub, &repository)?;
        send(store, &mut connection, repo, &repository, held.sent_seq)?;
        let mut since = held.seen_seq;
        let mut records = Vec::new();
        loop {
            let page: PlanChanges = request(
                &mut connection,
                ServerOperation::PlanChanges {
                    repository: repository.clone(),
                    since,
                },
            )?;
            records.extend(page.records);
            since = Some(page.seq);
            if !page.more {
                break;
            }
        }
        store.apply_plan_records(
            repo,
            &records,
            Some((Carrier::Hub, since.unwrap_or(0))),
            &machine_names(store)?,
        )?;
        Ok(())
    })();
    let error = result.as_ref().err().map(|error| format!("{error:#}"));
    store.record_carrier_exchange(repo, Carrier::Hub, error.as_deref())?;
    result
}

fn lock(store: &SqliteStore, repository: &RepositoryId) -> Result<std::fs::File> {
    let path = store
        .home_dir()?
        .join("locks/plan-hub")
        .join(format!("{repository}.lock"));
    std::fs::create_dir_all(path.parent().expect("a lock path has a parent"))?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    fs2::FileExt::lock_exclusive(&file)?;
    Ok(file)
}

fn open(
    store: &SqliteStore,
    repo: &str,
    hub: &Machine,
    repository: &RepositoryId,
) -> Result<Connection> {
    let local = crate::store::machines::local_machine(store)?;
    let sender = Sender {
        repository: repository.clone(),
        selection: Selection {
            profile: crate::config::remote::outgoing(Some(Path::new(repo)), local.id.as_str())?,
            participant: None,
            agent: None,
            settings: None,
            source: None,
        },
        secrets: Secrets::default(),
    };
    let connection = Connection::open_on(hub, sender)?;
    if connection.welcome().server_revision < crate::machine::wire::PLAN_REVISION {
        return Err(Error::new(ErrorCode::Incompatible, crate::machine::ssh::outdated(hub)).into());
    }
    Ok(connection)
}

fn request<T: serde::de::DeserializeOwned>(
    connection: &mut Connection,
    operation: ServerOperation,
) -> Result<T> {
    let answer = connection.request(operation)?;
    Ok(serde_json::from_value(answer.result)?)
}

/// Every record changed here past `sent_seq`, a page a request. The cursor
/// moves once the hub has answered a page; a page whose answer is lost goes
/// again, and the hub taking the same records twice changes nothing.
fn send(
    store: &SqliteStore,
    connection: &mut Connection,
    repo: &str,
    repository: &RepositoryId,
    mut sent_seq: i64,
) -> Result<()> {
    loop {
        let page = store.plan_records_since(repo, sent_seq, BUDGET)?;
        if !page.records.is_empty() {
            let _: serde_json::Value = request(
                connection,
                ServerOperation::PlanApply {
                    repository: repository.clone(),
                    records: page.records,
                },
            )?;
        }
        store.plan_sent(repo, Carrier::Hub, page.seq)?;
        sent_seq = page.seq;
        if !page.more {
            return Ok(());
        }
    }
}

// ---- The hub's side ----

/// `plan.apply`: take in another Machine's records here.
pub fn apply_here(
    store: &SqliteStore,
    repository: &RepositoryId,
    records: &[StampedRecord],
) -> Result<serde_json::Value> {
    store.apply_plan_records(
        &checkout(store, repository)?,
        records,
        None,
        &machine_names(store)?,
    )?;
    Ok(serde_json::json!({}))
}

/// `plan.changes`: what changed here after `since`.
pub fn changes_here(
    store: &SqliteStore,
    repository: &RepositoryId,
    since: Option<i64>,
) -> Result<PlanChanges> {
    let page =
        store.plan_records_since(&checkout(store, repository)?, since.unwrap_or(0), BUDGET)?;
    Ok(PlanChanges {
        records: page.records,
        seq: page.seq,
        more: page.more,
    })
}

/// The person's names for their Machines, for the steers an arriving
/// ownership entry writes.
fn machine_names(
    store: &SqliteStore,
) -> Result<loopflow_storage::sqlite::plan_carriers::MachineNames> {
    let config = crate::config::placement::global_settings_at(&store.home_dir()?)?;
    Ok(config
        .machines
        .into_iter()
        .map(|(name, declaration)| (declaration.id, name))
        .collect())
}

fn checkout(store: &SqliteStore, repository: &RepositoryId) -> Result<String> {
    store
        .repository_path(repository)
        .context("read the repository's checkout")?
        .ok_or_else(|| {
            anyhow!("the hub has no checkout of repository {repository}; clone it there and run `lf config repo --bind {repository}` in it, or when the hub already holds this plan, run `lf config repo --bind <id>` here with the id `lf config repo` prints in the hub's checkout")
        })
}
