//! The plan on the hub Machine: which Machine holds it, one exchange with it
//! from another Machine, and the hub's answers. A repository with Linear or
//! Asana never comes here.
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};

use crate::engine::{Error, ErrorCode};
use crate::machine::client::Connection;
use crate::machine::wire::{PlanChanges, Secrets, Selection, Sender, ServerOperation};
use loopflow_model::durable::{Machine, RepositoryId};
use loopflow_storage::sqlite::plan_hub::HubSend;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::edit::WorkEdit;

/// About half a control message, so one request or answer never needs a
/// ticketed connection.
const BUDGET: usize = crate::machine::wire::MAX_MESSAGE / 2;

/// The Machine that holds this Machine's plan; `None` when it is here, on a
/// Machine with no hub or that is its own hub.
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

/// Whether this Machine is the hub its own setting names, where other
/// Machines keep their plan.
pub fn is_hub(store: &SqliteStore) -> Result<bool> {
    let config = crate::config::placement::global_settings_at(&store.home_dir()?)?;
    Ok(config.hub.is_some_and(|id| {
        crate::store::machines::local_machine(store).is_ok_and(|local| local.id == id)
    }))
}

/// `lf machine hub <machine>` and `--clear` (`None`). Clearing keeps every
/// plan here and drops what was waiting for the hub; changing the hub reads
/// the new one's whole plan first.
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
        store.forget_hub_plans()?;
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

/// Read what changed on `hub` for `repo`, then send what is open. With
/// `send_only`, a repository with nothing open and past first contact asks
/// the hub nothing. The outcome is recorded for `lf repo tracker`.
pub fn exchange(store: &SqliteStore, repo: &str, hub: &Machine, send_only: bool) -> Result<()> {
    let Some(repository) = store.repository_id(repo)? else {
        return Ok(());
    };
    // One exchange per repository at a time; the next one reads what this one wrote.
    let _turn = lock(store, &repository)?;
    let plan = store.select_hub(repo, hub.id.as_str())?;
    if send_only && plan.seq.is_some() && store.open_hub_sends(repo)?.is_empty() {
        return Ok(());
    }
    let result = (|| {
        let mut connection = open(store, repo, hub, &repository)?;
        // Read first: at first contact the hub's values win over a plan here
        // that may be older, and only what the hub lacked is sent.
        let mut since = plan.seq;
        let mut edits = Vec::new();
        loop {
            let page: PlanChanges = request(
                &mut connection,
                ServerOperation::PlanChanges {
                    repository: repository.clone(),
                    since,
                },
            )?;
            edits.extend(page.edits);
            since = Some(page.seq);
            if !page.more {
                break;
            }
        }
        store.import_hub_plan(repo, &edits, since.unwrap_or(0))?;
        send(store, &mut connection, repo, &repository)
    })();
    let error = result.as_ref().err().map(|error| format!("{error:#}"));
    store.record_hub_exchange(repo, error.as_deref())?;
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

/// Every open send, an owner's values always in one request so the hub can
/// create a record it lacks.
fn send(
    store: &SqliteStore,
    connection: &mut Connection,
    repo: &str,
    repository: &RepositoryId,
) -> Result<()> {
    let mut owners = BTreeMap::<String, (i64, Vec<HubSend>)>::new();
    for send in store.open_hub_sends(repo)? {
        owners
            .entry(send.edit.target.clone())
            .or_insert_with(|| (send.seq, Vec::new()))
            .1
            .push(send);
    }
    let mut owners = owners.into_values().collect::<Vec<_>>();
    owners.sort_by_key(|(first, _)| *first);
    let mut batch = Vec::new();
    let mut bytes = 0;
    for (_, sends) in owners {
        let size = sends
            .iter()
            .map(|send| serde_json::to_vec(&send.edit).map(|bytes| bytes.len()))
            .sum::<Result<usize, _>>()?;
        if !batch.is_empty() && bytes + size > BUDGET {
            apply(store, connection, repository, std::mem::take(&mut batch))?;
            bytes = 0;
        }
        bytes += size;
        batch.extend(sends);
    }
    if !batch.is_empty() {
        apply(store, connection, repository, batch)?;
    }
    Ok(())
}

/// A send whose answer is lost stays open and goes again: the hub writing
/// the same values twice changes nothing.
fn apply(
    store: &SqliteStore,
    connection: &mut Connection,
    repository: &RepositoryId,
    sends: Vec<HubSend>,
) -> Result<()> {
    let seqs = sends.iter().map(|send| send.seq).collect::<Vec<_>>();
    store.mark_hub_sent(&seqs)?;
    let _: serde_json::Value = request(
        connection,
        ServerOperation::PlanApply {
            repository: repository.clone(),
            edits: sends.into_iter().map(|send| send.edit).collect(),
        },
    )?;
    store.confirm_hub_sends(&seqs)?;
    Ok(())
}

// ---- The hub's side ----

/// `plan.apply`: write another Machine's saves here.
pub fn apply_here(
    store: &SqliteStore,
    repository: &RepositoryId,
    edits: &[WorkEdit],
) -> Result<serde_json::Value> {
    store.apply_hub_edits(&checkout(store, repository)?, edits)?;
    Ok(serde_json::json!({}))
}

/// `plan.changes`: what changed here after `since`.
pub fn changes_here(
    store: &SqliteStore,
    repository: &RepositoryId,
    since: Option<i64>,
) -> Result<PlanChanges> {
    let page = store.plan_changes_since(&checkout(store, repository)?, since, BUDGET)?;
    Ok(PlanChanges {
        edits: page.edits,
        seq: page.seq,
        more: page.more,
    })
}

fn checkout(store: &SqliteStore, repository: &RepositoryId) -> Result<String> {
    store
        .repository_path(repository)
        .context("read the repository's checkout")?
        .ok_or_else(|| {
            anyhow!("the hub has no checkout of repository {repository}; clone it there and run `lf config repo --bind {repository}` in it")
        })
}
