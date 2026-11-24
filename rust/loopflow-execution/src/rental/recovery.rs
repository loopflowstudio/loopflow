//! Read-triggered rental recovery and returning Machines after their work drains.
use crate::machine_records::CopyCounts;
use crate::rental::sandbox::Sandboxes;
use crate::store::Store;
use anyhow::{anyhow, Context};
use loopflow_model::durable::{Machine, MachineId};
use loopflow_storage::rental::{self, GiveBack, Holdings, Rental, RentalState, Resolution};
use loopflow_storage::sqlite::SqliteStore;
use std::io::{BufReader, Seek};
use std::process::{Command, Stdio};
use time::OffsetDateTime;

pub fn open_store(runtime: &tokio::runtime::Runtime) -> anyhow::Result<Store> {
    runtime
        .block_on(crate::store::open_existing_store())?
        .ok_or_else(|| anyhow!("this command needs an initialized local store"))
}

pub fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

pub fn held_ids(
    sqlite: &SqliteStore,
    machine: &MachineId,
) -> anyhow::Result<Vec<loopflow_model::durable::TaskId>> {
    let held = sqlite.tasks_held_by(machine)?;
    Ok(held.into_iter().map(|(task, _)| task).collect())
}

fn live_rentals(sqlite: &SqliteStore) -> anyhow::Result<Vec<Rental>> {
    let mut rentals = sqlite.rentals(None)?;
    rentals.retain(|rental| rental.state.is_live());
    Ok(rentals)
}

/// Bring every live rental up to date with its provider, ending none.
/// Returns why some could not be asked.
pub fn refresh_all(sqlite: &SqliteStore) -> Vec<String> {
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
    let Some(machine) = crate::store::machines::machine_by_id(sqlite, machine_id)? else {
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
                crate::store::ownership::set_rental_state(sqlite, &rental.id, RentalState::Lost)?;
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

pub fn give_back(
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
    holdings.tasks = held_ids(sqlite, &machine.id)?;
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
            let copied = match end_rental(sqlite, api, rental, || {
                copy_up(sqlite, rental, machine, true)
            })? {
                Ended::Copied(counts) => Some(counts),
                Ended::Uncopied(error) if !force => {
                    let message = format!(
                        "{label} stays: its Session records could not be copied ({error:#}); \
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
                    eprintln!("Discarded on {label}: Session records ({error:#})");
                    delete_sandbox(sqlite, api, rental)?;
                    None
                }
            };
            if force && !holdings.unpushed.is_empty() {
                eprintln!("Discarded on {label}: {}", holdings.unpushed.join(", "));
            }
            let ended = sqlite.rental(&rental.id)?;
            println!(
                "Returned {label} ({}{})",
                copied.map_or_else(String::new, |counts| format!("{counts} copied; ")),
                ended.cost
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

pub fn delete_sandbox(
    sqlite: &SqliteStore,
    api: &Sandboxes,
    rental: &Rental,
) -> anyhow::Result<()> {
    crate::store::ownership::set_rental_state(sqlite, &rental.id, RentalState::Ending)?;
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
    let (_, returned) =
        crate::store::ownership::set_rental_state(sqlite, &rental.id, RentalState::Ended)?;
    let here = crate::store::ownership::local_label();
    for task in returned {
        let name = sqlite
            .task(&task)?
            .map_or_else(|| task.to_string(), |task| task.identifier);
        println!("{name} back on {here}");
    }
    Ok(())
}

/// Pull the machine's records since its rental began (`lf history export`).
fn copy_up(
    sqlite: &SqliteStore,
    rental: &Rental,
    machine: &Machine,
    final_copy: bool,
) -> anyhow::Result<CopyCounts> {
    let home = crate::store::lf_home_dir();
    let root = crate::machine_records::copies_home(&home, &machine.id);
    std::fs::create_dir_all(&root)?;
    let lock = std::fs::File::options()
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("collection.lock"))?;
    fs2::FileExt::lock_exclusive(&lock)?;
    // On give-back revisit even closed Sessions: a provider or capture file can
    // finish writing after its Session's last database change.
    let after = if final_copy {
        None
    } else {
        sqlite.copied_cursor(&machine.id)?
    };
    // An hour early absorbs clock skew between the laptop and the sandbox.
    let since = (rental.created_at - time::Duration::hours(1))
        .format(&time::format_description::well_known::Rfc3339)?;
    let mut args = vec![
        "lf".into(),
        "session".into(),
        "history".into(),
        "export".into(),
        "--since".into(),
        since,
    ];
    if let Some(after) = after {
        args.extend(["--after".into(), serde_json::to_string(&after)?]);
    }
    let script = crate::machine::ssh::build_preamble(&args, &[]);
    // A disconnected SSH may leave valid-looking JSON lines. Only import a
    // successful command; spool stdout so the rental's size does not set RAM.
    let mut spool = tempfile::tempfile()?;
    let output = Command::new("ssh")
        .args(crate::machine_route::bounded_ssh_args(
            &machine.route,
            false,
        )?)
        .arg(script)
        .stdin(Stdio::null())
        .stdout(spool.try_clone()?)
        .stderr(Stdio::piped())
        .output()
        .context("could not run ssh")?;
    if !output.status.success() {
        return Err(crate::machine::connection::transport_failure(
            &machine.route,
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    spool.rewind()?;
    crate::machine_records::import(sqlite, &home, &machine.id, BufReader::new(spool))
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

/// SSH collection can run even when the server has no provider API credentials.
fn collect_running(sqlite: &SqliteStore, rental: &Rental) -> anyhow::Result<()> {
    if let Some(id) = &rental.machine_id {
        if let Some(machine) = crate::store::machines::machine_by_id(sqlite, id)? {
            copy_up(sqlite, rental, &machine, false)?;
        }
    }
    Ok(())
}

/// The server recovery pass's give-back. A rental whose provider this process
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
        if let Err(error) = collect_running(sqlite, &rental) {
            tracing::warn!(rental=%rental.id, "rental history collection: {error:#}");
        }
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
        return Err(crate::machine::connection::transport_failure(
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

    use super::{end_rental, read_holdings, reconcile, refresh, Ended};
    use crate::machine_records::CopyCounts;
    use crate::rental::sandbox::Sandboxes;
    use loopflow_model::durable::MachineId;
    use loopflow_storage::rental::{
        Hardware, OnDemand, Provider, Rental, RentalState, Usd, BOOT_SECONDS,
    };
    use loopflow_storage::sqlite::SqliteStore;

    type Log = Rc<RefCell<Vec<String>>>;

    /// A running rental of `code-1` and a provider that knows its sandbox.
    /// Created and confirmed, with no Machine yet: what a boot leaves.
    fn booting(dir: &std::path::Path) -> (SqliteStore, Rental, Sandboxes, Log) {
        let store = crate::store::opening::open_ephemeral(&dir.join("db")).unwrap();
        crate::store::rentals::add_ondemand(
            &store,
            &OnDemand {
                name: "code".into(),
                provider: Provider::Daytona,
                hardware: Hardware {
                    vcpus: 4,
                    memory_gb: 8,
                    disk_gb: 10,
                    gpu: None,
                    spot: false,
                    modal_sizing: None,
                },
                max_machines: 1,
                max_usd_per_hour: Usd::parse("0.40").unwrap(),
                hours: 4.0,
            },
        )
        .unwrap();
        let rental =
            crate::store::rentals::admit_rental(&store, "code", None, None, false).unwrap();
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
        crate::store::machines::add_machine(
            &store,
            &machine,
            "ssh://lf@203.0.113.7:40022",
            "code-1",
        )
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
            Ok(CopyCounts { sessions: 3 })
        })
        .unwrap();
        assert!(matches!(ended, Ended::Copied(counts) if counts.sessions == 3));
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
