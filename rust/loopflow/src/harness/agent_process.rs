//! Settle AgentProcesses from the same inventory used by gates and live views.
//! Unknown identity, duplicate PID/birth and unknown attachment life grant no signal authority.
use crate::engine::process::terminate_process_group;
use crate::journal::{
    process_evidence, process_identity_evidence, process_started_at, ProcessIdentityEvidence,
};
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProcessReapReport {
    pub orphaned: Vec<u32>,
    pub reaped: u32,
    pub settled_drivers: u32,
    pub errors: Vec<String>,
}

pub fn reap_agent_processes(dry_run: bool) -> Result<AgentProcessReapReport> {
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    Ok(reap_in(&store, dry_run)?)
}

fn reap_in(store: &SqliteStore, dry_run: bool) -> StoreResult<AgentProcessReapReport> {
    let agents = store.agent_processes()?;
    let mut counts = HashMap::new();
    for agent in &agents {
        if let Some(identity) = agent.process.pid.zip(agent.process.os_started_at) {
            *counts.entry(identity).or_insert(0usize) += 1;
        }
    }
    let mut report = AgentProcessReapReport::default();
    for agent in &agents {
        let Some((pid, start)) = agent.process.pid.zip(agent.process.os_started_at) else {
            continue;
        };
        let evidence = process_identity_evidence(pid, start);
        let dead = evidence == ProcessIdentityEvidence::Dead;
        let orphaned = evidence == ProcessIdentityEvidence::Live
            && counts[&(pid, start)] == 1
            && !agent.interactive
            && agent.attached_process_lfid.as_ref().is_none_or(|attached| {
                process_evidence(store, attached) == ProcessIdentityEvidence::Dead
            })
            && is_agent_process(pid, start, agent.provider.as_deref());
        if orphaned {
            report.orphaned.push(pid);
        }
        if dry_run || (!dead && !orphaned) {
            continue;
        }
        let result = store.settle_agent_process(agent, || {
            if dead {
                if process_identity_evidence(pid, start) != ProcessIdentityEvidence::Dead {
                    return Err(StoreError::InvalidAuthority(
                        "AgentProcess death changed".into(),
                    ));
                }
            } else {
                // Attachment life is sampled again beneath the transfer lock.
                if agent
                    .attached_process_lfid
                    .as_ref()
                    .is_some_and(|attached| {
                        process_evidence(store, attached) != ProcessIdentityEvidence::Dead
                    })
                {
                    return Err(StoreError::InvalidAuthority(
                        "attached LfProcess death is unresolved".into(),
                    ));
                }
                terminate_agent_process(pid, start, agent.provider.as_deref())?;
            }
            Ok(())
        });
        match result {
            Ok(true) => {
                report.reaped += u32::from(orphaned);
                if let Some(session) = agent.process.agent_session_id.as_deref() {
                    if let Some(attachment) = store.session_attachment(session)? {
                        if attachment.agent_process_lfid == agent.process.lfid
                            && attachment.process_lfid.as_ref().is_some_and(|attached| {
                                process_evidence(store, attached) == ProcessIdentityEvidence::Dead
                            })
                        {
                            match store.finish_session_attachment(
                                session,
                                &attachment,
                                "interrupted",
                                || Ok(false),
                            ) {
                                Ok(()) => report.settled_drivers += 1,
                                Err(StoreError::InvalidAuthority(_)) => {}
                                Err(error) => {
                                    report.errors.push(format!("Session {session}: {error}"))
                                }
                            }
                        }
                    }
                }
            }
            Ok(false) => {}
            Err(error) => report
                .errors
                .push(format!("AgentProcess {}: {error}", agent.process.lfid)),
        }
    }
    Ok(report)
}

/// Whether `pid` is still the recorded AgentProcess: same start time, the
/// provider's server command line, and leader of its own process group.
fn is_agent_process(pid: u32, started_at: i64, provider: Option<&str>) -> bool {
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

/// Stop the AgentProcess's group, then any group one of its descendants leads.
/// Identity is proven again here, immediately before the first signal.
fn terminate_agent_process(pid: u32, started_at: i64, provider: Option<&str>) -> StoreResult<()> {
    let refused =
        |reason: &str| StoreError::InvalidAuthority(format!("AgentProcess {pid} {reason}"));
    let descendants = descendant_group_leaders(pid);
    if !is_agent_process(pid, started_at, provider) {
        return Err(refused("is no longer the recorded process"));
    }
    if !terminate_process_group(pid) {
        return Err(refused("did not exit"));
    }
    for (leader, leader_started_at) in descendants {
        // Most exit with the AgentProcess. One that survives is still the process
        // sampled above only if its start time is unchanged.
        if matches!(process_started_at(leader), Ok(Some(actual)) if (actual - leader_started_at).abs() <= 3)
            && !terminate_process_group(leader)
        {
            tracing::warn!(
                pid,
                leader,
                "AgentProcess descendant group outlived its AgentProcess"
            );
        }
    }
    Ok(())
}

/// Descendants of `pid` that lead their own process group, with start times.
/// A provider helper such as Codex's code-mode host runs outside the AgentProcess's
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
    use crate::id::ProcessLfid;
    use crate::store::sqlite::SqliteStore;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    #[test]
    fn detached_agent_is_reaped_but_duplicate_historical_identity_is_not_signal_authority() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let mut child = Command::new("/bin/sh")
            .args([
                "-c",
                "sleep 60 & wait; :",
                "codex",
                "app-server",
                "--listen",
                "fixture",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id();
        let start = crate::journal::process_started_at(pid).unwrap().unwrap();
        let parent = ProcessLfid::new();
        for name in ["first", "conflict"] {
            let session = store.test_session(name, &crate::session_record::new_artifact_key());
            rusqlite::Connection::open(home.path().join("db"))
                .unwrap()
                .execute(
                    "UPDATE agent_sessions SET provider='codex',interactive=0 WHERE id=?1",
                    [name],
                )
                .unwrap();
            let attached = store
                .claim_session_attachment(&session.id, None, &parent, true)
                .unwrap();
            store
                .record_session_provider_process(&session.id, &attached, pid, start)
                .unwrap();
            store
                .release_session_attachment(&session.id, &attached)
                .unwrap();
        }
        let conflict = super::reap_in(&store, false).unwrap();
        assert!(conflict.errors.is_empty());
        assert_eq!(conflict.reaped, 0);
        assert!(child.try_wait().unwrap().is_none());
        // Retire only the fixture's duplicate observation, not its OS process.
        rusqlite::Connection::open(home.path().join("db"))
            .unwrap()
            .execute(
                "UPDATE processes SET completed_at=1 WHERE agent_session_id='conflict'",
                [],
            )
            .unwrap();
        let report = super::reap_in(&store, false).unwrap();
        // Always clean up this fixture's own group before assertions.
        crate::engine::process::terminate_process_group(pid);
        let _ = child.wait();
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.orphaned, [pid]);
        assert_eq!(report.reaped, 1);
        assert!(store.agent_processes().unwrap().is_empty());
    }
}
