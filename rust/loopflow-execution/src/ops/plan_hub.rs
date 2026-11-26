//! The hub: one reachable Machine that a plan without Linear or Asana is
//! exchanged with live. It carries stamped records and decides nothing; this
//! is which Machine it is, one exchange with it, and its answers.
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};

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
    exchange_with(store, repo, why, |hub, repository, until, run| {
        let mut connection = open(store, repo, hub, repository, until)?;
        run(&mut |operation| connection.request(operation).map(|answer| answer.result))
    })
}

fn exchange_with(
    store: &SqliteStore,
    repo: &str,
    why: Why,
    connect: impl FnOnce(
        &Machine,
        &RepositoryId,
        Option<Instant>,
        &mut dyn FnMut(&mut dyn FnMut(ServerOperation) -> Result<serde_json::Value>) -> Result<()>,
    ) -> Result<()>,
) -> Result<()> {
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
        && held.has_read()
        && store.plan_unsent(repo, Carrier::Hub, 0)?.count == 0
    {
        return Ok(());
    }
    let result = connect(&hub, &repository, until, &mut |call| {
        send(store, call, repo, &repository, held.sent_seq)?;
        let mut since = held.seen_seq;
        let mut records = Vec::new();
        loop {
            let page: PlanChanges = request(
                call,
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
    });
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
    Connection::open_on(hub, sender, until)
}

fn request<T: serde::de::DeserializeOwned>(
    call: &mut dyn FnMut(ServerOperation) -> Result<serde_json::Value>,
    operation: ServerOperation,
) -> Result<T> {
    Ok(serde_json::from_value(call(operation)?)?)
}

/// Every record changed here past `sent_seq`, a page a request. The cursor
/// moves once the hub has answered a page; a page whose answer is lost goes
/// again, and the hub taking the same records twice changes nothing. From
/// the first page the hub could not write whole, the cursor stays: the later
/// pages still go, and all of them go again at the next exchange.
fn send(
    store: &SqliteStore,
    call: &mut dyn FnMut(ServerOperation) -> Result<serde_json::Value>,
    repo: &str,
    repository: &RepositoryId,
    mut sent_seq: i64,
) -> Result<()> {
    let mut taken = true;
    loop {
        let page = store.plan_records_since(repo, sent_seq, BUDGET)?;
        if !page.records.is_empty() {
            let applied: PlanApplied = request(
                call,
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

#[cfg(test)]
mod tests {
    use super::{apply_here, changes_here, exchange_with, set_hub, Why};
    use crate::machine::wire::ServerOperation;
    use crate::ops::plan_carriers::first_read_with;
    use loopflow_storage::sqlite::plan_carriers::Carrier;
    use loopflow_storage::work::task::NewTask;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn git(dir: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn a_hub_only_clone_reads_the_hub_on_its_first_list() {
        let root = tempfile::tempdir().unwrap();
        let hub_repo_dir = root.path().join("hub-repo");
        let clone_repo_dir = root.path().join("clone-repo");
        std::fs::create_dir_all(&hub_repo_dir).unwrap();
        std::fs::create_dir_all(&clone_repo_dir).unwrap();
        git(&hub_repo_dir, &["init", "-q"]);
        git(&clone_repo_dir, &["init", "-q"]);
        let hub_repo = hub_repo_dir
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let clone_repo = clone_repo_dir
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

        let hub_store =
            crate::store::opening::open_ephemeral(&root.path().join("hub/loopflow.db")).unwrap();
        let clone_store =
            crate::store::opening::open_ephemeral(&root.path().join("clone/loopflow.db")).unwrap();

        let hub_repo_id =
            crate::repository::location::ensure_repository(&hub_store, &hub_repo).unwrap();
        let project =
            crate::test_support::seed_wave_project(&hub_store, &hub_repo, "plan").unwrap();
        hub_store
            .create_task(&NewTask {
                parent_task_id: None,
                repository_id: hub_repo_id.clone(),
                id: loopflow_model::durable::TaskId::new(),
                project_id: Some(project.id),
                title: "from hub".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();

        // Point the clone at the hub and bind it to the hub's repository id (no Git remote).
        let hub_machine = crate::store::machines::local_machine(&hub_store).unwrap();
        crate::store::machines::add_machine(&clone_store, &hub_machine.id, "hub", "hub").unwrap();
        set_hub(&clone_store, Some(&hub_machine.id)).unwrap();
        let registration = clone_store.repository_registration(&clone_repo).unwrap();
        assert!(clone_store
            .register_repository(&clone_repo, &registration, &hub_repo_id, true)
            .unwrap());

        let hub_calls = Arc::new(AtomicUsize::new(0));
        let run = |now: i64| {
            let clone_store = &clone_store;
            let hub_store = &hub_store;
            let clone_repo = clone_repo.as_str();
            let hub_calls = Arc::clone(&hub_calls);
            let until = Instant::now() + Duration::from_secs(60);
            async move {
                first_read_with(
                    clone_store,
                    clone_repo,
                    until,
                    now,
                    |_, _, _| async { panic!("hub-only clone must not run Git") },
                    |store, repo, until| {
                        hub_calls.fetch_add(1, Ordering::SeqCst);
                        let res =
                            exchange_with(store, repo, Why::Read(until), |_, _, _, run_rpc| {
                                run_rpc(&mut |operation| match operation {
                                    ServerOperation::PlanApply {
                                        repository,
                                        records,
                                    } => Ok(serde_json::to_value(apply_here(
                                        hub_store,
                                        &repository,
                                        &records,
                                    )?)?),
                                    ServerOperation::PlanChanges { repository, since } => {
                                        Ok(serde_json::to_value(changes_here(
                                            hub_store,
                                            &repository,
                                            since,
                                        )?)?)
                                    }
                                    other => panic!("unexpected hub op: {other:?}"),
                                })
                            });
                        async move { res }
                    },
                )
                .await
            }
        };

        assert!(run(1_000).await.unwrap());
        assert_eq!(hub_calls.load(Ordering::SeqCst), 1);
        let listed: Vec<String> = clone_store
            .open_tasks_matching(&hub_repo_id, "", None, None)
            .unwrap()
            .into_iter()
            .map(|task| task.title)
            .collect();
        assert_eq!(listed, ["from hub"]);
        assert!(clone_store
            .plan_carrier(&clone_repo, Carrier::Hub)
            .unwrap()
            .unwrap()
            .has_read());

        // A second read skips the hub.
        assert!(!run(1_001).await.unwrap());
        assert_eq!(hub_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn git_and_hub_fresh_clone_reads_git_once_and_skips_hub_on_second_read() {
        let root = tempfile::tempdir().unwrap();
        let origin = loopflow_test_support::TestRepo::new();
        let clone_dir = root.path().join("clone");
        git(
            root.path(),
            &[
                "clone",
                "-q",
                origin.bare_path().to_str().unwrap(),
                clone_dir.to_str().unwrap(),
            ],
        );
        let clone_repo = clone_dir
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

        let hub_store =
            crate::store::opening::open_ephemeral(&root.path().join("hub/loopflow.db")).unwrap();
        let clone_store =
            crate::store::opening::open_ephemeral(&root.path().join("clone/loopflow.db")).unwrap();
        let hub_machine = crate::store::machines::local_machine(&hub_store).unwrap();
        crate::store::machines::add_machine(&clone_store, &hub_machine.id, "hub", "hub").unwrap();
        set_hub(&clone_store, Some(&hub_machine.id)).unwrap();

        let git_calls = Arc::new(AtomicUsize::new(0));
        let hub_calls = Arc::new(AtomicUsize::new(0));
        let run = |now: i64| {
            let clone_store = &clone_store;
            let clone_repo = clone_repo.as_str();
            let git_calls = Arc::clone(&git_calls);
            let hub_calls = Arc::clone(&hub_calls);
            let until = Instant::now() + Duration::from_secs(60);
            async move {
                first_read_with(
                    clone_store,
                    clone_repo,
                    until,
                    now,
                    |store, repo, until| {
                        git_calls.fetch_add(1, Ordering::SeqCst);
                        let res = super::super::plan_git::exchange(store, repo, Some(until));
                        async move { res }
                    },
                    |store, repo, until| {
                        hub_calls.fetch_add(1, Ordering::SeqCst);
                        let res = exchange_with(store, repo, Why::Read(until), |_, _, _, _| {
                            Err(anyhow::anyhow!("hub does not know this repository id"))
                        });
                        async move { res }
                    },
                )
                .await
            }
        };

        // Read #1: no repository id yet, so only Git runs, minting a local id and marking Git read.
        assert!(run(1_000).await.unwrap());
        assert_eq!(git_calls.load(Ordering::SeqCst), 1);
        assert_eq!(hub_calls.load(Ordering::SeqCst), 0);
        assert!(clone_store.repository_id(&clone_repo).unwrap().is_some());
        assert!(clone_store
            .plan_carrier(&clone_repo, Carrier::Git)
            .unwrap()
            .unwrap()
            .has_read());

        // Read #2: Git already marked the clone read, so neither carrier runs and no hub error is recorded.
        assert!(!run(1_001).await.unwrap());
        assert_eq!(git_calls.load(Ordering::SeqCst), 1);
        assert_eq!(hub_calls.load(Ordering::SeqCst), 0);
        assert!(clone_store
            .plan_carrier(&clone_repo, Carrier::Hub)
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn bound_clone_with_unreachable_hub_marks_via_git_and_skips_second_read() {
        let root = tempfile::tempdir().unwrap();
        let origin = loopflow_test_support::TestRepo::new();
        let clone_dir = root.path().join("clone");
        git(
            root.path(),
            &[
                "clone",
                "-q",
                origin.bare_path().to_str().unwrap(),
                clone_dir.to_str().unwrap(),
            ],
        );
        let clone_repo = clone_dir
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

        let hub_store =
            crate::store::opening::open_ephemeral(&root.path().join("hub/loopflow.db")).unwrap();
        let clone_store =
            crate::store::opening::open_ephemeral(&root.path().join("clone/loopflow.db")).unwrap();
        let hub_machine = crate::store::machines::local_machine(&hub_store).unwrap();
        crate::store::machines::add_machine(&clone_store, &hub_machine.id, "hub", "hub").unwrap();
        set_hub(&clone_store, Some(&hub_machine.id)).unwrap();
        crate::repository::location::ensure_repository(&clone_store, &clone_repo).unwrap();

        let git_calls = Arc::new(AtomicUsize::new(0));
        let hub_calls = Arc::new(AtomicUsize::new(0));
        let run = |now: i64| {
            let clone_store = &clone_store;
            let clone_repo = clone_repo.as_str();
            let git_calls = Arc::clone(&git_calls);
            let hub_calls = Arc::clone(&hub_calls);
            let until = Instant::now() + Duration::from_secs(60);
            async move {
                first_read_with(
                    clone_store,
                    clone_repo,
                    until,
                    now,
                    |store, repo, until| {
                        git_calls.fetch_add(1, Ordering::SeqCst);
                        let res = super::super::plan_git::exchange(store, repo, Some(until));
                        async move { res }
                    },
                    |store, repo, until| {
                        hub_calls.fetch_add(1, Ordering::SeqCst);
                        let res = exchange_with(store, repo, Why::Read(until), |_, _, _, _| {
                            Err(anyhow::anyhow!("hub unreachable"))
                        });
                        async move { res }
                    },
                )
                .await
            }
        };

        // Read #1: both carriers run; Git succeeds and marks the clone read while the hub fails.
        assert!(run(1_000).await.unwrap());
        assert_eq!(git_calls.load(Ordering::SeqCst), 1);
        assert_eq!(hub_calls.load(Ordering::SeqCst), 1);
        assert!(clone_store
            .plan_carrier(&clone_repo, Carrier::Git)
            .unwrap()
            .unwrap()
            .has_read());
        assert!(clone_store
            .plan_carrier(&clone_repo, Carrier::Hub)
            .unwrap()
            .unwrap()
            .error
            .is_some());
        assert_eq!(
            clone_store
                .last_scope_attempt(&format!("plan-first-read:{clone_repo}"))
                .unwrap(),
            None
        );

        // Read #2 (immediately and after the 60 s window): neither carrier runs again.
        assert!(!run(1_001).await.unwrap());
        assert!(!run(1_061).await.unwrap());
        assert_eq!(git_calls.load(Ordering::SeqCst), 1);
        assert_eq!(hub_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_offline_hub_does_not_hold_back_a_git_remote_that_came_back() {
        let root = tempfile::tempdir().unwrap();
        let origin = loopflow_test_support::TestRepo::new();
        let clone_dir = root.path().join("clone");
        git(
            root.path(),
            &[
                "clone",
                "-q",
                origin.bare_path().to_str().unwrap(),
                clone_dir.to_str().unwrap(),
            ],
        );
        let clone_repo = clone_dir
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

        let hub_store =
            crate::store::opening::open_ephemeral(&root.path().join("hub/loopflow.db")).unwrap();
        let clone_store =
            crate::store::opening::open_ephemeral(&root.path().join("clone/loopflow.db")).unwrap();
        let hub_machine = crate::store::machines::local_machine(&hub_store).unwrap();
        crate::store::machines::add_machine(&clone_store, &hub_machine.id, "hub", "hub").unwrap();
        set_hub(&clone_store, Some(&hub_machine.id)).unwrap();
        crate::repository::location::ensure_repository(&clone_store, &clone_repo).unwrap();

        let git_online = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let git_calls = Arc::new(AtomicUsize::new(0));
        let run = |now: i64| {
            let clone_store = &clone_store;
            let clone_repo = clone_repo.as_str();
            let git_online = Arc::clone(&git_online);
            let git_calls = Arc::clone(&git_calls);
            let until = Instant::now() + Duration::from_secs(60);
            async move {
                first_read_with(
                    clone_store,
                    clone_repo,
                    until,
                    now,
                    |store, repo, until| {
                        git_calls.fetch_add(1, Ordering::SeqCst);
                        let res = if git_online.load(Ordering::SeqCst) {
                            super::super::plan_git::exchange(store, repo, Some(until))
                        } else {
                            Err(anyhow::anyhow!("network is down"))
                        };
                        async move { res }
                    },
                    |store, repo, until| {
                        let res = exchange_with(store, repo, Why::Read(until), |_, _, _, _| {
                            Err(anyhow::anyhow!("hub unreachable"))
                        });
                        async move { res }
                    },
                )
                .await
            }
        };

        // Offline: both fail, the cooldown starts, and the hub keeps its error.
        assert!(run(1_000).await.is_err());
        assert!(clone_store
            .plan_carrier(&clone_repo, Carrier::Hub)
            .unwrap()
            .unwrap()
            .error
            .is_some());

        // Git is back with no error of its own; 25 s later the hub's error does
        // not keep it waiting out the minute.
        git_online.store(true, Ordering::SeqCst);
        assert!(run(1_025).await.unwrap());
        assert_eq!(git_calls.load(Ordering::SeqCst), 2);
        assert!(clone_store
            .plan_carrier(&clone_repo, Carrier::Git)
            .unwrap()
            .unwrap()
            .has_read());
    }
}
