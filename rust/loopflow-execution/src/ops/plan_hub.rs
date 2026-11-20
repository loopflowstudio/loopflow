//! The hub: one reachable Machine that a plan without Linear or Asana is
//! exchanged with live. It carries stamped records and decides nothing; this
//! is which Machine it is, one exchange with it, and its answers.
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};

use crate::engine::{Error, ErrorCode};
use crate::machine::client::Connection;
use crate::machine::wire::{PlanApplied, PlanChanges, Secrets, Selection, Sender, ServerOperation};
use loopflow_model::durable::{Machine, RepositoryId};
use loopflow_storage::sqlite::plan_carriers::{Carrier, NotTaken};
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

/// [`exchange`] for `repo`, when it has no Linear or Asana, for as long as
/// the hub takes: a move waits for its plan to be there.
pub async fn sync(store: &crate::store::Store, repo: &str) -> Result<()> {
    super::plan_carriers::on_thread(store, repo, |store, repo| exchange(store, repo, Why::Asked))
        .await
}

/// Why an exchange runs, which is how long it may take.
#[derive(Debug, Clone, Copy)]
pub(super) enum Why {
    /// Someone asked for the plan: as long as the hub takes.
    Asked,
    /// A read nobody asked for, over by then.
    Read(Instant),
    /// After a save, over by then. A repository with nothing unsent that has
    /// read the hub before asks it nothing.
    Save(Instant),
}

/// Send this Machine's hub, when it has one, the records of `repo` it has not
/// taken, then take in what changed there. Either order gives the same plan.
/// One with an end waits for an exchange already running and for the hub's
/// answers only until then. The outcome is recorded for `lf repo tracker`.
/// What the hub gave and this Machine could not write is the error, and the
/// next exchange reads the hub from the same place.
pub(super) fn exchange(store: &SqliteStore, repo: &str, why: Why) -> Result<()> {
    let Some(hub) = hub(store)? else {
        return Ok(());
    };
    let Some(repository) = store.repository_id(repo)? else {
        return Ok(());
    };
    let until = match why {
        Why::Asked => None,
        Why::Read(until) | Why::Save(until) => Some(until),
    };
    let _turn = super::plan_carriers::lock(store, "plan-hub", &repository, until)?;
    let held = store.select_carrier(repo, Carrier::Hub, hub.id.as_str())?;
    if matches!(why, Why::Save(_))
        && held.seen_seq.is_some()
        && store.plan_unsent(repo, Carrier::Hub, 0)?.count == 0
    {
        return Ok(());
    }
    let result = (|| {
        let mut connection = open(store, repo, &hub, &repository, until)?;
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
        let skipped = store.apply_plan_records(
            repo,
            &records,
            Some((Carrier::Hub, since.unwrap_or(0))),
            &crate::store::ownership::machine_names(store)?,
        )?;
        NotTaken::of(&skipped).map_or(Ok(()), |not_taken| Err(not_taken.into()))
    })();
    let error = super::plan_carriers::carrier_error(&result);
    store.record_carrier_exchange(repo, Carrier::Hub, error.as_deref())?;
    result
}

fn open(
    store: &SqliteStore,
    repo: &str,
    hub: &Machine,
    repository: &RepositoryId,
    until: Option<Instant>,
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
    let connection = Connection::open_on(hub, sender, until)?;
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
/// again, and the hub taking the same records twice changes nothing. From
/// the first page the hub could not write whole, the cursor stays: the later
/// pages still go, and all of them go again at the next exchange.
fn send(
    store: &SqliteStore,
    connection: &mut Connection,
    repo: &str,
    repository: &RepositoryId,
    mut sent_seq: i64,
) -> Result<()> {
    let mut taken = true;
    loop {
        let page = store.plan_records_since(repo, sent_seq, BUDGET)?;
        if !page.records.is_empty() {
            let applied: PlanApplied = request(
                connection,
                ServerOperation::PlanApply {
                    repository: repository.clone(),
                    records: page.records,
                },
            )?;
            taken &= applied.not_taken == 0;
        }
        if taken {
            store.plan_sent(repo, Carrier::Hub, page.seq)?;
        }
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
) -> Result<PlanApplied> {
    let skipped = store.apply_plan_records(
        &checkout(store, repository)?,
        records,
        None,
        &crate::store::ownership::machine_names(store)?,
    )?;
    Ok(PlanApplied {
        not_taken: skipped.len(),
    })
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

fn checkout(store: &SqliteStore, repository: &RepositoryId) -> Result<String> {
    store
        .repository_path(repository)
        .context("read the repository's checkout")?
        .ok_or_else(|| {
            anyhow!("the hub has no checkout of repository {repository}; clone it there and run `lf config repo --bind {repository}` in it, or when the hub already holds this plan, run `lf config repo --bind <id>` here with the id `lf config repo` prints in the hub's checkout")
        })
}
