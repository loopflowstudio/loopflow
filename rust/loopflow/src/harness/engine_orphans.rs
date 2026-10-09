//! Reaps provider engines that outlived their driver.
//!
//! Every engine runs in its own process group bound to its driver's life by
//! `engine::process::bind_group_to_driver`, so a live engine normally implies
//! a live driver. This is the backstop for what that cannot cover: engines
//! started before lifelines existed, and a driver killed between spawning an
//! engine and binding it.
//!
//! The authority is exactly this narrow: the recorded pid and start time still
//! name a live process whose command line is the provider's server, it leads
//! the group Loopflow created for it, and every Session naming it is headless
//! with a driver Process that is provably dead. Unknown evidence, a live
//! driver, an interactive Session or a reused pid leaves the process alone.
//!
//! The same pass records the exit of any driver that was killed before it
//! could: nothing else would, and the Session would read as driven forever.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::process::Command;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::engine::process::terminate_process_group;
use crate::id::ProcessLfid;
use crate::journal::{
    process_evidence, process_identity_evidence, process_started_at, ProcessIdentityEvidence,
};
use crate::store::sqlite::engine_orphans::RecordedEngine;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineReapReport {
    /// Process groups proven orphaned, whether or not they were then reaped.
    pub orphaned: Vec<u32>,
    pub reaped: u32,
    /// Drivers that were killed before recording their own exit.
    pub settled_drivers: u32,
    pub errors: Vec<String>,
}

/// Reap every provably orphaned engine recorded in this Home's store.
/// `dry_run` proves and reports without signalling or writing.
///
/// # Errors
///
/// Fails only when the store cannot be opened or read; one engine that will
/// not stop is reported in `errors` and does not hide the others.
pub fn reap_orphaned_engines(dry_run: bool) -> Result<EngineReapReport> {
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    Ok(reap_orphaned_engines_in(&store, dry_run, |driver| {
        process_evidence(&store, driver)
    })?)
}

fn reap_orphaned_engines_in(
    store: &SqliteStore,
    dry_run: bool,
    driver_evidence: impl Fn(&ProcessLfid) -> ProcessIdentityEvidence,
) -> StoreResult<EngineReapReport> {
    let mut engines = BTreeMap::<(u32, i64), Vec<RecordedEngine>>::new();
    for engine in store.recorded_engines()? {
        engines
            .entry((engine.pid, engine.started_at))
            .or_default()
            .push(engine);
    }
    let mut report = EngineReapReport::default();
    for ((pid, started_at), sessions) in engines {
        // Cheapest proof first: almost every recorded engine has long exited.
        if !is_engine(pid, started_at, sessions[0].provider.as_deref()) {
            continue;
        }
        // An engine can serve several conversations; all of them must agree.
        let orphaned = sessions.iter().all(|session| {
            !session.interactive
                && session.provider == sessions[0].provider
                && session
                    .driver
                    .as_ref()
                    .is_some_and(|driver| driver_evidence(driver) == ProcessIdentityEvidence::Dead)
        });
        if !orphaned {
            continue;
        }
        report.orphaned.push(pid);
        if dry_run {
            continue;
        }
        let provider = sessions[0].provider.clone();
        let mut terminated = false;
        for session in &sessions {
            let settled = store.reap_session_engine(session, || {
                if !terminated {
                    terminate_engine(pid, started_at, provider.as_deref())?;
                    terminated = true;
                }
                Ok(())
            });
            match settled {
                Ok(true) => {}
                // The row moved on: another driver claimed or replaced it.
                Ok(false) if !terminated => break,
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(pid, session = %session.session, %error, "orphaned engine not reaped");
                    report
                        .errors
                        .push(format!("Session {}: {error}", session.session));
                    if !terminated {
                        break;
                    }
                }
            }
        }
        report.reaped += u32::from(terminated);
    }
    if !dry_run {
        settle_dead_drivers(store, &driver_evidence, &mut report)?;
    }
    Ok(report)
}

/// Record the exit a killed driver never wrote, so its Session stops reading
/// as driven: its open turn ends in history and it no longer counts as Waiting.
/// A Session whose engine is still alive keeps its driver for the next reap.
fn settle_dead_drivers(
    store: &SqliteStore,
    driver_evidence: impl Fn(&ProcessLfid) -> ProcessIdentityEvidence,
    report: &mut EngineReapReport,
) -> StoreResult<()> {
    for session in store.driven_sessions()? {
        let Some(driver) = store.session_driver(&session)? else {
            continue;
        };
        let dead = driver
            .process_lfid
            .as_ref()
            .is_some_and(|process| driver_evidence(process) == ProcessIdentityEvidence::Dead);
        let engine_gone =
            store
                .session_provider_process(&session)?
                .is_none_or(|(pid, started_at)| {
                    process_identity_evidence(pid, started_at) == ProcessIdentityEvidence::Dead
                });
        if !dead || !engine_gone {
            continue;
        }
        match store.finish_session_driver(&session, &driver, "interrupted", || Ok(false)) {
            Ok(()) => report.settled_drivers += 1,
            // Another driver claimed the conversation since it was read.
            Err(StoreError::InvalidAuthority(_)) => {}
            Err(error) => report.errors.push(format!("Session {session}: {error}")),
        }
    }
    Ok(())
}

/// Whether `pid` is still the recorded engine: same start time, the
/// provider's server command line, and leader of its own process group.
fn is_engine(pid: u32, started_at: i64, provider: Option<&str>) -> bool {
    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    // SAFETY: getpgid only reads process metadata.
    if raw <= 1 || unsafe { libc::getpgid(raw) } != raw {
        return false;
    }
    if !matches!(process_started_at(pid), Ok(Some(actual)) if (actual - started_at).abs() <= 3) {
        return false;
    }
    let Ok(output) = Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
    else {
        return false;
    };
    let command = String::from_utf8_lossy(&output.stdout);
    let has = |word: &str| command.split_whitespace().any(|part| part == word);
    let program = |name: &str| {
        command
            .split_whitespace()
            .any(|part| part == name || part.ends_with(&format!("/{name}")))
    };
    output.status.success()
        && match provider {
            Some("codex") => program("codex") && has("app-server") && has("--listen"),
            Some("opencode") => program("opencode") && has("serve"),
            _ => false,
        }
}

/// Stop the engine's group, then any group one of its descendants leads.
/// Identity is proven again here, immediately before the first signal.
fn terminate_engine(pid: u32, started_at: i64, provider: Option<&str>) -> StoreResult<()> {
    let refused = |reason: &str| StoreError::InvalidAuthority(format!("engine {pid} {reason}"));
    let descendants = descendant_group_leaders(pid);
    if !is_engine(pid, started_at, provider) {
        return Err(refused("is no longer the recorded process"));
    }
    if !terminate_process_group(pid) {
        return Err(refused("did not exit"));
    }
    for (leader, leader_started_at) in descendants {
        // Most exit with the engine. One that survives is still the process
        // sampled above only if its start time is unchanged.
        if matches!(process_started_at(leader), Ok(Some(actual)) if (actual - leader_started_at).abs() <= 3)
            && !terminate_process_group(leader)
        {
            tracing::warn!(pid, leader, "engine descendant group outlived its engine");
        }
    }
    Ok(())
}

/// Descendants of `pid` that lead their own process group, with start times.
/// A provider helper such as Codex's code-mode host runs outside the engine's
/// group, so the group signal alone would leave it behind.
fn descendant_group_leaders(pid: u32) -> Vec<(u32, i64)> {
    let Ok(output) = Command::new("ps")
        .args(["-axo", "pid=,ppid=,pgid="])
        .output()
    else {
        return Vec::new();
    };
    let rows = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace().map(str::parse::<u32>);
            Some((
                fields.next()?.ok()?,
                fields.next()?.ok()?,
                fields.next()?.ok()?,
            ))
        })
        .collect::<Vec<_>>();
    let mut children = HashMap::<u32, Vec<(u32, u32)>>::new();
    for (child, parent, group) in rows {
        children.entry(parent).or_default().push((child, group));
    }
    let mut leaders = Vec::new();
    let mut seen = HashSet::from([pid]);
    let mut pending = vec![pid];
    while let Some(parent) = pending.pop() {
        for (child, group) in children.get(&parent).into_iter().flatten() {
            if !seen.insert(*child) {
                continue;
            }
            pending.push(*child);
            if group == child {
                if let Ok(Some(started_at)) = process_started_at(*child) {
                    leaders.push((*child, started_at));
                }
            }
        }
    }
    leaders
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Stdio};
    use std::time::{Duration, Instant};

    use super::*;

    /// A throwaway process whose command line reads as a Codex engine. The
    /// trailing `:` keeps the shell from exec-ing `sleep` over that line.
    fn fake_engine() -> Child {
        Command::new("sh")
            .args([
                "-c",
                "sleep 60 & wait; :",
                "codex",
                "app-server",
                "--listen",
            ])
            .arg("unix:///tmp/lf-test/engine.sock")
            .stdin(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap()
    }

    struct Fixture {
        ledger: crate::journal::TestLedgerGuard,
        store: SqliteStore,
        driver: ProcessLfid,
        engine: Child,
        started_at: i64,
    }

    impl Fixture {
        fn new() -> Self {
            let ledger = crate::journal::TestLedgerGuard::new();
            let path = ledger.home().join("loopflow.db");
            let store = SqliteStore::open_ephemeral(&path).unwrap();
            store.test_session("orphan", &crate::session_record::new_artifact_key());
            let driver = ProcessLfid::new();
            let sql = rusqlite::Connection::open(&path).unwrap();
            sql.execute(
                "INSERT INTO processes(lfid,trace_id,started_at) VALUES(?1,'fixture',1)",
                [driver.as_str()],
            )
            .unwrap();
            sql.execute(
                "UPDATE agent_sessions SET provider='codex',interactive=0 WHERE id='orphan'",
                [],
            )
            .unwrap();
            let claim = store
                .claim_session_driver("orphan", None, &driver, true)
                .unwrap();
            let engine = fake_engine();
            let started_at = process_started_at(engine.id()).unwrap().unwrap();
            store
                .record_session_provider_process("orphan", &claim, engine.id(), started_at)
                .unwrap();
            store
                .record_session_connection("orphan", &claim, "/tmp/lf-test/engine.sock", "thread")
                .unwrap();
            Self {
                ledger,
                store,
                driver,
                engine,
                started_at,
            }
        }

        fn sql(&self, statement: &str) {
            rusqlite::Connection::open(self.ledger.home().join("loopflow.db"))
                .unwrap()
                .execute(statement, [])
                .unwrap();
        }

        fn reap(&self, evidence: ProcessIdentityEvidence) -> EngineReapReport {
            reap_orphaned_engines_in(&self.store, false, |driver| {
                assert_eq!(driver, &self.driver);
                evidence
            })
            .unwrap()
        }

        /// The reaper may already have collected this child, so probe the pid.
        fn engine_alive(&self) -> bool {
            // SAFETY: signal 0 probes the throwaway child; nothing is delivered.
            unsafe { libc::kill(self.engine.id() as i32, 0) == 0 }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            crate::engine::process::kill_process_group(self.engine.id());
            let _ = self.engine.wait();
        }
    }

    #[test]
    fn reaps_an_engine_whose_driver_is_provably_dead() {
        let fixture = Fixture::new();
        let pid = fixture.engine.id();

        let report = fixture.reap(ProcessIdentityEvidence::Dead);

        assert_eq!(report.orphaned, vec![pid]);
        assert_eq!((report.reaped, report.errors.len()), (1, 0));
        // The killed driver's exit is recorded once its engine is gone.
        assert_eq!(report.settled_drivers, 1);
        let driver = fixture.store.session_driver("orphan").unwrap().unwrap();
        assert_eq!((driver.process_lfid, driver.generation), (None, 2));
        let deadline = Instant::now() + Duration::from_secs(5);
        while fixture.engine_alive() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!fixture.engine_alive());
        // The store reflects it, and history says why the engine went away.
        assert!(fixture
            .store
            .session_provider_process("orphan")
            .unwrap()
            .is_none());
        assert!(fixture
            .store
            .session_connection("orphan")
            .unwrap()
            .is_none());
        let payload: String = rusqlite::Connection::open(fixture.ledger.home().join("loopflow.db"))
            .unwrap()
            .query_row(
                "SELECT payload FROM session_events WHERE session_id='orphan'
                 AND receipt_key='provider:1:exited'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(payload["reaped"]["pid"], pid);
        assert_eq!(
            payload["reaped"]["driver_process_lfid"],
            fixture.driver.as_str()
        );
        // Nothing is left to reap.
        assert_eq!(
            fixture.reap(ProcessIdentityEvidence::Dead),
            EngineReapReport::default()
        );
    }

    #[test]
    fn dry_run_reports_without_signalling() {
        let fixture = Fixture::new();
        let report =
            reap_orphaned_engines_in(&fixture.store, true, |_| ProcessIdentityEvidence::Dead)
                .unwrap();
        assert_eq!(report.orphaned, vec![fixture.engine.id()]);
        assert_eq!(report.reaped, 0);
        assert!(fixture.engine_alive());
    }

    #[test]
    fn leaves_an_engine_with_a_live_or_unknown_driver() {
        let fixture = Fixture::new();
        for evidence in [
            ProcessIdentityEvidence::Live,
            ProcessIdentityEvidence::Unknown,
        ] {
            assert_eq!(fixture.reap(evidence), EngineReapReport::default());
        }
        assert!(fixture.engine_alive());
        assert!(fixture
            .store
            .session_provider_process("orphan")
            .unwrap()
            .is_some());
    }

    #[test]
    fn leaves_a_process_whose_start_time_differs() {
        let fixture = Fixture::new();
        fixture.sql(&format!(
            "UPDATE agent_sessions SET provider_started_at={}",
            fixture.started_at - 600
        ));
        // The recorded engine is gone, so only the dead driver is settled.
        assert_eq!(
            fixture.reap(ProcessIdentityEvidence::Dead),
            EngineReapReport {
                settled_drivers: 1,
                ..EngineReapReport::default()
            }
        );
        assert!(fixture.engine_alive());
    }

    #[test]
    fn leaves_an_interactive_session_and_a_shared_engine_still_driven() {
        let fixture = Fixture::new();
        fixture.sql("UPDATE agent_sessions SET interactive=1");
        assert_eq!(
            fixture.reap(ProcessIdentityEvidence::Dead),
            EngineReapReport::default()
        );
        fixture.sql("UPDATE agent_sessions SET interactive=0");

        // A second conversation on the same engine has a live driver.
        fixture
            .store
            .test_session("sibling", &crate::session_record::new_artifact_key());
        let sibling = ProcessLfid::new();
        fixture.sql(&format!(
            "INSERT INTO processes(lfid,trace_id,started_at) VALUES('{sibling}','fixture',1)"
        ));
        fixture.sql("UPDATE agent_sessions SET provider='codex',interactive=0 WHERE id='sibling'");
        let claim = fixture
            .store
            .claim_session_driver("sibling", None, &sibling, true)
            .unwrap();
        fixture
            .store
            .record_session_provider_process(
                "sibling",
                &claim,
                fixture.engine.id(),
                fixture.started_at,
            )
            .unwrap();
        let report = reap_orphaned_engines_in(&fixture.store, false, |driver| {
            if driver == &sibling {
                ProcessIdentityEvidence::Live
            } else {
                ProcessIdentityEvidence::Dead
            }
        })
        .unwrap();
        assert_eq!(report, EngineReapReport::default());
        assert!(fixture.engine_alive());
    }

    #[test]
    fn leaves_a_reused_pid_running_something_else() {
        let fixture = Fixture::new();
        // Same pid and start time, but the Session expected another provider's
        // server: the command line is not that engine.
        fixture.sql("UPDATE agent_sessions SET provider='opencode'");
        assert_eq!(
            fixture.reap(ProcessIdentityEvidence::Dead),
            EngineReapReport::default()
        );
        assert!(fixture.engine_alive());
    }
}
