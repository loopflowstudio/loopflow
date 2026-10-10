//! Launch and settle AgentProcesses in the inventory used by gates and live views.
//! Unknown identity, duplicate PID/birth and unknown attachment life grant no signal authority.
use crate::id::LfProcessId;
use crate::journal::{
    process_evidence, process_identity_evidence, process_started_at, OsProcess,
    ProcessIdentityEvidence,
};
use crate::os_process::terminate_process_group;
use crate::process::SessionAttachment;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};
use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};

/// The store, LfSession and frozen attachment one invocation launches and
/// writes under.
pub(crate) type AttachmentOwner = (SqliteStore, String, SessionAttachment);

/// Open the invocation's attachment without refreshing its authority from the
/// store. A launch without one is refused: every AgentProcess is recorded.
pub(super) fn open_owner(
    attachment: Option<&(String, SessionAttachment)>,
) -> Result<AttachmentOwner> {
    let (session, attachment) = attachment
        .ok_or_else(|| anyhow::anyhow!("AgentProcess requires an admitted invocation"))?;
    Ok((
        SqliteStore::new(&crate::store::database_path_from_env()?)?,
        session.clone(),
        attachment.clone(),
    ))
}

/// The pre-exec recorder both launch paths share: the child's PID and birth
/// reach its record under the owner's attachment before the provider runs.
fn record_identity(
    (store, session, attachment): &AttachmentOwner,
) -> impl FnOnce(u32) -> std::io::Result<()> + Send + '_ {
    move |pid| {
        let started_at = process_started_at(pid)?
            .ok_or_else(|| std::io::Error::other("AgentProcess birth unavailable before exec"))?;
        store
            .record_agent_process_identity(session, attachment, pid, started_at)
            .map_err(std::io::Error::other)
    }
}

/// One headless launch sequence for every harness. Keep the attachment fence
/// through pre-exec recording, and retain failed attempts without inventing an
/// observed exit. Synchronous admission cannot detach a launch on async cancellation.
pub(super) fn spawn(
    command: tokio::process::Command,
    lifeline: Option<&std::path::Path>,
    owner: &AttachmentOwner,
) -> Result<tokio::process::Child> {
    let (store, session, attachment) = owner;
    super::dispatch::off_reactor(|| {
        let program = command
            .as_std()
            .get_program()
            .to_string_lossy()
            .into_owned();
        store.with_session_attachment(session, attachment, || {
            store.record_agent_process_launch(session, attachment, command.as_std())?;
            let spawned =
                crate::os_process::spawn_agent_process(command, lifeline, record_identity(owner));
            if spawned.is_err() {
                store.record_agent_process_exit(session, attachment, false)?;
            }
            spawned.map_err(|error| {
                StoreError::InvalidData(format!("failed to spawn {program}: {error}"))
            })
        })
    })
    .map_err(Into::into)
}

/// Native terminals keep their foreground process group. Remote clients do not
/// own the provider; owned native launches use the same admission fence and
/// pre-exec recording as headless launches.
pub(crate) fn spawn_native(
    command: std::process::Command,
    owner: &AttachmentOwner,
) -> Result<std::process::Child> {
    let (store, session, attachment) = owner;
    store
        .with_session_attachment(session, attachment, || {
            store.record_agent_process_launch(session, attachment, &command)?;
            let spawned =
                crate::os_process::spawn_native_agent_process(command, record_identity(owner));
            if spawned.is_err() {
                store.record_agent_process_exit(session, attachment, false)?;
            }
            // Preserve the OS error (including NotFound) for the caller's message.
            Ok(spawned)
        })?
        .map_err(Into::into)
}

/// Failed terminal setup still owns an admitted child. Serialize its cleanup
/// with takeover and record only an exact successful wait. A remote client has
/// no provider attachment; ending it must not settle the surviving provider.
pub(crate) fn stop_native(
    child: &mut std::process::Child,
    owner: Option<&AttachmentOwner>,
) -> Result<()> {
    let mut stop = || {
        let mut wait = || -> std::io::Result<()> {
            if child.try_wait()?.is_none() {
                child.kill()?;
                child.wait()?;
            }
            Ok(())
        };
        wait().map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if let Some((store, session, attachment)) = owner {
            store.record_agent_process_exit(session, attachment, true)?;
        }
        Ok(())
    };
    match owner {
        Some((store, session, attachment)) => {
            store.with_session_attachment(session, attachment, stop)
        }
        None => stop(),
    }
    .map_err(Into::into)
}

/// Stop a harness's provider without settling its invocation. Group death and
/// the exit receipt remain under the same frozen attachment fence.
pub(super) fn stop((store, session, attachment): &AttachmentOwner) -> Result<()> {
    super::dispatch::off_reactor(|| {
        store.with_session_attachment(session, attachment, || {
            if !close_session_agent_process(store, session)? {
                return Err(StoreError::InvalidAuthority(
                    "headless AgentProcess identity is unavailable".into(),
                ));
            }
            store.record_agent_process_exit(session, attachment, true)
        })
    })
    .map_err(Into::into)
}

/// Called with the exact attachment locked, never with SQLite held across I/O.
/// Foreground providers settle through their launcher, never headless group control.
pub(crate) fn close_session_agent_process(store: &SqliteStore, session: &str) -> StoreResult<bool> {
    let Some(attachment) = store.session_attachment(session)? else {
        return Ok(false);
    };
    let agents = store.agent_processes()?;
    let Some(agent) = agents
        .iter()
        .find(|agent| agent.process.id == attachment.agent_process_id)
    else {
        return Ok(false);
    };
    let Some((pid, started)) = agent.process.pid.zip(agent.process.os_started_at) else {
        return Ok(false);
    };
    match process_identity_evidence(pid, started) {
        ProcessIdentityEvidence::Dead => {
            return confirmed_group_death(pid)
                .map(|()| true)
                .map_err(|error| StoreError::InvalidAuthority(error.to_string()));
        }
        ProcessIdentityEvidence::Unknown => {
            return Err(StoreError::InvalidAuthority(
                "AgentProcess OS identity is unavailable".into(),
            ))
        }
        ProcessIdentityEvidence::Live => {}
    }
    if agent.interactive {
        return Ok(false);
    }
    if agents.iter().any(|other| {
        other.process.id != agent.process.id
            && other.process.pid == Some(pid)
            && other.process.os_started_at == Some(started)
    }) {
        return Err(StoreError::InvalidAuthority(
            "AgentProcess OS identity has multiple owners".into(),
        ));
    }
    // A Codex server may host unrelated conversations. This is a refusal check,
    // not another provider-specific signaling path.
    if agent.provider.as_deref() == Some("codex") {
        let (endpoint, thread) = store.session_connection(session)?.ok_or_else(|| {
            StoreError::InvalidAuthority("Codex AgentProcess connection is unavailable".into())
        })?;
        crate::harness::codex_connection::validate_agent_process_close((&endpoint, &thread))
            .map_err(|error| StoreError::InvalidAuthority(error.to_string()))?;
    }
    close_agent_process(pid, started)
        .map_err(|error| StoreError::InvalidAuthority(error.to_string()))?;
    Ok(true)
}

/// Stop an exactly recorded headless process group. The caller holds its Session
/// attachment fence and has refused ambiguous ownership before reaching here.
pub(crate) fn close_agent_process(pid: u32, started: i64) -> Result<()> {
    match process_identity_evidence(pid, started) {
        ProcessIdentityEvidence::Live => {}
        ProcessIdentityEvidence::Dead => return confirmed_group_death(pid),
        ProcessIdentityEvidence::Unknown => {
            return Err(anyhow!("AgentProcess OS identity is unavailable"));
        }
    }
    let group = i32::try_from(pid)?;
    // SAFETY: getpgid reads process metadata. Only the exact recorded process
    // leading the group that Loopflow created may authorize a group signal.
    let owner = unsafe { libc::getpgid(group) };
    if owner == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        // It exited on its own since the check above; reap it if it is ours.
        // SAFETY: WNOHANG only reaps our own exited child.
        unsafe {
            libc::waitpid(group, std::ptr::null_mut(), libc::WNOHANG);
        }
        return confirmed_group_death(pid);
    }
    if owner != group {
        return Err(anyhow!(
            "recorded process does not lead its own process group"
        ));
    }
    // The leader may exit before its helpers. Use the same group-wide death
    // judgment as scheduled settlement rather than ending on leader death.
    if terminate_process_group(pid) {
        Ok(())
    } else {
        Err(anyhow!("AgentProcess group {pid} death is unresolved"))
    }
}

// Leader death alone neither proves group death nor authorizes signaling helpers.
fn confirmed_group_death(pid: u32) -> Result<()> {
    if OsProcess::group_is_alive(pid)? {
        return Err(anyhow!(
            "AgentProcess group {pid} survives its recorded leader"
        ));
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct AgentProcessReapReport {
    pub orphaned: Vec<u32>,
    pub reaped: u32,
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
    let dead_lf =
        |process: &LfProcessId| process_evidence(store, process) == ProcessIdentityEvidence::Dead;
    let mut report = AgentProcessReapReport::default();
    for agent in &agents {
        let Some((pid, start)) = agent.process.pid.zip(agent.process.os_started_at) else {
            continue;
        };
        let observed = match OsProcess::read(pid) {
            Ok(process) => process,
            Err(error) => {
                report.errors.push(format!(
                    "AgentProcess {}: cannot observe PID {pid}: {error}",
                    agent.process.id
                ));
                continue;
            }
        };
        let evidence = observed
            .as_ref()
            .map_or(ProcessIdentityEvidence::Dead, |process| {
                process.evidence(start)
            });
        let dead = evidence == ProcessIdentityEvidence::Dead;
        let orphaned = counts[&(pid, start)] == 1
            && !agent.interactive
            && agent.attached_lf_process_id.as_ref().is_none_or(dead_lf)
            && observed
                .as_ref()
                .is_some_and(|process| is_agent_process(process, start, agent.provider.as_deref()));
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
                if !agent.attached_lf_process_id.as_ref().is_none_or(dead_lf) {
                    return Err(StoreError::InvalidAuthority(
                        "attached LfProcess death is unresolved".into(),
                    ));
                }
                terminate_agent_process(pid, start, agent.provider.as_deref())
                    .map_err(|error| StoreError::InvalidAuthority(error.to_string()))?;
            }
            Ok(())
        });
        match result {
            Ok(true) => report.reaped += u32::from(orphaned),
            Ok(false) => continue,
            Err(error) => {
                report
                    .errors
                    .push(format!("AgentProcess {}: {error}", agent.process.id));
                continue;
            }
        }
        let Some(session) = agent.process.agent_session_id.as_deref() else {
            continue;
        };
        let Some(attachment) = store.session_attachment(session)? else {
            continue;
        };
        if attachment.agent_process_id != agent.process.id
            || !attachment.lf_process_id.as_ref().is_some_and(dead_lf)
        {
            continue;
        }
        match store.finish_session_attachment(session, &attachment, "interrupted", || Ok(false)) {
            Ok(()) | Err(StoreError::InvalidAuthority(_)) => {}
            Err(error) => report.errors.push(format!("Session {session}: {error}")),
        }
    }
    Ok(report)
}

/// Exact live identity, the provider's headless command, and its own process group.
fn is_agent_process(process: &OsProcess, started_at: i64, provider: Option<&str>) -> bool {
    if process.pid <= 1
        || process.pgid != process.pid
        || process.evidence(started_at) != ProcessIdentityEvidence::Live
    {
        return false;
    }
    let has = |word: &str| process.command.split_whitespace().any(|part| part == word);
    let program = |name: &str| {
        process.command.split_whitespace().any(|part| {
            std::path::Path::new(part)
                .file_name()
                .is_some_and(|file| file == name)
        })
    };
    match provider {
        Some("codex") => program("codex") && has("app-server") && has("--listen"),
        Some("opencode") => program("opencode") && has("serve"),
        // Headless Claude has no subcommand to match; identity and group carry it.
        Some("claude") => program("claude"),
        _ => false,
    }
}

/// Stop the AgentProcess's group, then any group one of its descendants leads.
/// Identity is proven again here, immediately before the first signal.
fn terminate_agent_process(pid: u32, started_at: i64, provider: Option<&str>) -> Result<()> {
    let refused = |reason: &str| anyhow::anyhow!("AgentProcess {pid} {reason}");
    let processes = OsProcess::sample(time::OffsetDateTime::now_utc().unix_timestamp())?;
    let descendants = descendant_group_leaders(pid, &processes);
    if !OsProcess::read(pid)?
        .as_ref()
        .is_some_and(|process| is_agent_process(process, started_at, provider))
    {
        return Err(refused("is no longer the recorded process"));
    }
    if !terminate_process_group(pid) {
        return Err(refused("did not exit"));
    }
    for (leader, leader_started_at) in descendants {
        // Most exit with the AgentProcess. One that survives is still the process
        // sampled above only if its start time is unchanged.
        if OsProcess::read(leader)?.is_some_and(|process| {
            process.pgid == leader && process.matches_start(leader, leader_started_at)
        }) && !terminate_process_group(leader)
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
fn descendant_group_leaders(pid: u32, processes: &[OsProcess]) -> Vec<(u32, i64)> {
    let mut children = HashMap::<u32, Vec<&OsProcess>>::new();
    for process in processes {
        children.entry(process.ppid).or_default().push(process);
    }
    let mut leaders = Vec::new();
    let mut seen = HashSet::from([pid]);
    let mut pending = vec![pid];
    while let Some(parent) = pending.pop() {
        for child in children.get(&parent).into_iter().flatten() {
            if !seen.insert(child.pid) {
                continue;
            }
            pending.push(child.pid);
            if child.pgid == child.pid {
                leaders.push((child.pid, child.started_at));
            }
        }
    }
    leaders
}

#[cfg(test)]
mod tests {
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    #[test]
    fn observed_agent_death_settles_only_a_provably_dead_attachment() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let database = ledger.home().join("loopflow.db");
        let store = SqliteStore::open_ephemeral(&database).unwrap();
        let sql = rusqlite::Connection::open(database).unwrap();
        for ended in [false, true] {
            let session = store.test_session(
                if ended { "ended" } else { "unknown" },
                &crate::session_record::new_artifact_key(),
            );
            let parent = LfProcessId::new();
            sql.execute(
                "INSERT INTO processes(id,trace_id,started_at,completed_at) VALUES(?1,?1,?2,?3)",
                rusqlite::params![
                    parent,
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                    ended.then_some(1)
                ],
            )
            .unwrap();
            let attachment = store
                .claim_session_attachment(&session.id, None, &parent, true)
                .unwrap();
            let mut command = Command::new("/usr/bin/true");
            command.env_clear();
            let mut child = super::spawn_native(
                command,
                &(store.clone(), session.id.clone(), attachment.clone()),
            )
            .unwrap();
            child.wait().unwrap();
            // No launcher exit receipt: the reaper must observe the OS death.
            let report = super::reap_in(&store, false).unwrap();
            assert!(report.errors.is_empty());
            assert!(report.orphaned.is_empty());
            assert_eq!(report.reaped, 0);
            let process = store
                .process(&attachment.agent_process_id)
                .unwrap()
                .unwrap();
            assert!(process.completed_at.is_some());
            assert!(process.outcome.is_none());
            let current = store.session_attachment(&session.id).unwrap().unwrap();
            assert_eq!(current.lf_process_id.is_none(), ended);
            let history = store.session_history(&session.id, 0, 0).unwrap();
            let exits: Vec<_> = history
                .iter()
                .filter(|event| event.payload["type"] == "attachment_exit")
                .collect();
            assert_eq!(exits.len(), usize::from(ended));
            if ended {
                assert_eq!(exits[0].payload["outcome"], "interrupted");
            } else {
                assert_eq!(current, attachment);
            }
        }
    }

    #[test]
    fn native_cleanup_records_wait_without_touching_a_replacement_or_remote_provider() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("cleanup", &crate::session_record::new_artifact_key());
        let parent = LfProcessId::new();
        let first = store
            .claim_session_attachment(&session.id, None, &parent, true)
            .unwrap();
        let first_owner = (store.clone(), session.id.clone(), first.clone());
        let command = || {
            let mut command = Command::new("/bin/sleep");
            command.env_clear().arg("60");
            command
        };
        let mut child = super::spawn_native(command(), &first_owner).unwrap();
        let pid = child.id();
        super::stop_native(&mut child, Some(&first_owner)).unwrap();
        let exited = store.process(&first.agent_process_id).unwrap().unwrap();
        assert_eq!(exited.pid, Some(pid));
        assert!(exited.os_started_at.is_some());
        assert!(exited.completed_at.is_some());

        let next = store
            .prepare_session_agent_process(&session.id, &first)
            .unwrap();
        let next_owner = (store.clone(), session.id.clone(), next.clone());
        let mut provider = super::spawn_native(command(), &next_owner).unwrap();
        let recorded = store.process(&next.agent_process_id).unwrap().unwrap();
        // Reusing the current attachment is not permission for a second spawn.
        // Reject before fork, without recording failure on the running process.
        let duplicate = super::spawn_native(command(), &next_owner);
        assert!(duplicate.is_err());
        assert_eq!(
            store.process(&next.agent_process_id).unwrap(),
            Some(recorded)
        );
        // A delayed cleanup has no authority over a replacement, even if it
        // accidentally receives the replacement's child handle.
        assert!(super::stop_native(&mut provider, Some(&first_owner)).is_err());
        let provider_alive = provider.try_wait().unwrap().is_none();
        let mut client = command().spawn().unwrap();
        super::stop_native(&mut client, None).unwrap();
        let remote_unchanged = store.process(&next.agent_process_id).unwrap().unwrap();
        let remote_alive = provider.try_wait().unwrap().is_none();
        super::stop_native(&mut provider, Some(&next_owner)).unwrap();
        assert!(provider_alive && remote_alive);
        assert!(remote_unchanged.completed_at.is_none());
        assert_eq!(
            store.process(&first.agent_process_id).unwrap(),
            Some(exited)
        );
    }

    #[test]
    fn native_cleanup_cannot_kill_the_same_agent_after_takeover() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("takeover", &crate::session_record::new_artifact_key());
        let first = store
            .claim_session_attachment(&session.id, None, &LfProcessId::new(), true)
            .unwrap();
        let first_owner = (store.clone(), session.id.clone(), first.clone());
        let mut command = Command::new("/bin/sleep");
        command.env_clear().arg("60");
        let mut child = super::spawn_native(command, &first_owner).unwrap();
        let next = store
            .claim_session_attachment(&session.id, Some(&first), &LfProcessId::new(), false)
            .unwrap();
        let refused = super::stop_native(&mut child, Some(&first_owner)).is_err();
        let alive = child.try_wait().unwrap().is_none();
        let unchanged = store.process(&first.agent_process_id).unwrap().unwrap();
        super::stop_native(&mut child, Some(&(store.clone(), session.id, next.clone()))).unwrap();
        assert_eq!(first.agent_process_id, next.agent_process_id);
        assert!(refused && alive);
        assert!(unchanged.completed_at.is_none());
    }

    #[test]
    fn native_cleanup_failed_wait_retains_unknown_exit() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("failed-wait", &crate::session_record::new_artifact_key());
        let attachment = store
            .claim_session_attachment(&session.id, None, &LfProcessId::new(), true)
            .unwrap();
        let owner = (store.clone(), session.id.clone(), attachment.clone());
        let mut command = Command::new("/bin/sh");
        command.env_clear().args(["-c", "exit 42"]);
        let mut child = super::spawn_native(command, &owner).unwrap();
        // Reap only this throwaway child outside its handle, forcing ECHILD.
        // SAFETY: waitpid observes the exact fixture child; it signals nothing.
        assert_eq!(
            unsafe { libc::waitpid(child.id() as i32, std::ptr::null_mut(), 0) },
            child.id() as i32
        );
        assert!(super::stop_native(&mut child, Some(&owner)).is_err());
        let retained = store
            .process(&attachment.agent_process_id)
            .unwrap()
            .unwrap();
        assert_eq!(retained.pid, Some(child.id()));
        assert!(retained.completed_at.is_none());
        assert!(retained.outcome.is_none());
    }

    #[tokio::test]
    async fn headless_spawn_retains_attempts_and_refuses_stale_attachments() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("spawn", &crate::session_record::new_artifact_key());
        let parent = LfProcessId::new();
        let first = store
            .claim_session_attachment(&session.id, None, &parent, true)
            .unwrap();
        let owner = |attachment| (store.clone(), session.id.clone(), attachment);
        let mut command = tokio::process::Command::new("/bin/sh");
        command.env_clear().args(["-c", "exit 42"]);
        let mut child = super::spawn(command, None, &owner(first.clone())).unwrap();
        let recorded = store.process(&first.agent_process_id).unwrap().unwrap();
        assert_eq!(recorded.pid, child.id());
        assert!(recorded.os_started_at.is_some());
        assert_eq!(child.wait().await.unwrap().code(), Some(42));
        // Spawn success alone must not manufacture exit evidence.
        assert!(recorded.completed_at.is_none());
        store
            .record_agent_process_exit(&session.id, &first, true)
            .unwrap();
        let next = store
            .prepare_session_agent_process(&session.id, &first)
            .unwrap();

        let marker = home.path().join("stale-effect");
        let mut stale = tokio::process::Command::new("/bin/sh");
        stale
            .env_clear()
            .args(["-c", "printf effect > \"$1\"", "fixture"])
            .arg(&marker);
        assert!(super::spawn(stale, None, &owner(first.clone())).is_err());
        assert!(!marker.exists());
        assert_eq!(
            store.session_attachment(&session.id).unwrap(),
            Some(next.clone())
        );
        assert!(store
            .process(&next.agent_process_id)
            .unwrap()
            .unwrap()
            .pid
            .is_none());

        let missing = tokio::process::Command::new(home.path().join("absent-provider"));
        assert!(super::spawn(missing, None, &owner(next.clone())).is_err());
        let failed = store.process(&next.agent_process_id).unwrap().unwrap();
        assert!(failed.completed_at.is_some());
        assert!(
            failed.outcome.is_none(),
            "failed exec is not a provider outcome"
        );
        let retry = store
            .prepare_session_agent_process(&session.id, &next)
            .unwrap();
        assert_ne!(retry.agent_process_id, next.agent_process_id);
        assert_eq!(store.process(&next.agent_process_id).unwrap(), Some(failed));
        assert_eq!(
            store.process(&first.agent_process_id).unwrap().unwrap().pid,
            recorded.pid
        );
    }

    #[test]
    fn native_spawn_retains_identity_and_fences_effects_after_takeover() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("native", &crate::session_record::new_artifact_key());
        let parent = LfProcessId::new();
        let first = store
            .claim_session_attachment(&session.id, None, &parent, true)
            .unwrap();
        let owner = (store.clone(), session.id.clone(), first.clone());
        let mut command = Command::new("/bin/sh");
        command.env_clear().args(["-c", "exit 42"]);
        let mut child = super::spawn_native(command, &owner).unwrap();
        let recorded = store.process(&first.agent_process_id).unwrap().unwrap();
        assert_eq!(recorded.pid, Some(child.id()));
        assert!(recorded.os_started_at.is_some());
        assert_eq!(child.wait().unwrap().code(), Some(42));
        store
            .record_agent_process_exit(&session.id, &first, true)
            .unwrap();
        let next = store
            .prepare_session_agent_process(&session.id, &first)
            .unwrap();
        let marker = home.path().join("stale-effect");
        let mut stale = Command::new("/bin/sh");
        stale
            .env_clear()
            .args(["-c", "printf effect > \"$1\"", "fixture"])
            .arg(&marker);
        assert!(super::spawn_native(stale, &owner).is_err());
        assert!(!marker.exists());
        assert_eq!(
            store.session_attachment(&session.id).unwrap(),
            Some(next.clone())
        );
        let owner = (store.clone(), session.id.clone(), next.clone());
        let error =
            super::spawn_native(Command::new(home.path().join("absent")), &owner).unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::NotFound
        );
        let failed = store.process(&next.agent_process_id).unwrap().unwrap();
        assert!(failed.completed_at.is_some());
        assert!(failed.outcome.is_none());
        assert_eq!(
            store.process(&first.agent_process_id).unwrap().unwrap().pid,
            recorded.pid
        );
    }

    #[test]
    fn detached_agent_is_reaped_but_duplicate_historical_identity_is_not_signal_authority() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        for (provider, argv) in [
            ("codex", &["codex", "app-server", "--listen", "fixture"][..]),
            ("opencode", &["opencode", "serve"]),
            ("claude", &["claude"]),
        ] {
            reap_detached(provider, argv);
        }
    }

    fn reap_detached(provider: &str, argv: &[&str]) {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let mut child = Command::new("/bin/sh")
            .args(["-c", "sleep 60 & wait; :"])
            .args(argv)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id();
        let start = crate::journal::process_started_at(pid).unwrap().unwrap();
        let parent = LfProcessId::new();
        for name in ["first", "conflict"] {
            let session = store.test_session(name, &crate::session_record::new_artifact_key());
            rusqlite::Connection::open(home.path().join("db"))
                .unwrap()
                .execute(
                    "UPDATE agent_sessions SET provider=?2,interactive=0 WHERE id=?1",
                    [name, provider],
                )
                .unwrap();
            let attached = store
                .claim_session_attachment(&session.id, None, &parent, true)
                .unwrap();
            store
                .record_agent_process_identity(&session.id, &attached, pid, start)
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
        crate::os_process::terminate_process_group(pid);
        let _ = child.wait();
        assert!(report.errors.is_empty(), "{provider}: {:?}", report.errors);
        assert_eq!(report.orphaned, [pid], "{provider}");
        assert_eq!(report.reaped, 1, "{provider}");
        assert!(store.agent_processes().unwrap().is_empty());
    }
}

#[cfg(test)]
mod close_tests {
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};

    struct Group(Child);

    impl Drop for Group {
        fn drop(&mut self) {
            crate::os_process::terminate_process_group(self.0.id());
            let _ = self.0.wait();
        }
    }

    #[test]
    fn exited_leader_does_not_settle_a_surviving_group() {
        let mut group = Group(
            Command::new("/bin/sh")
                .env_clear()
                .args(["-c", "/bin/sleep 60 & read -r done"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()
                .unwrap(),
        );
        let leader = crate::journal::OsProcess::read(group.0.id())
            .unwrap()
            .unwrap();
        // Closing stdin lets the leader exit after it has spawned its helper.
        drop(group.0.stdin.take());
        group.0.wait().unwrap();
        assert!(super::close_agent_process(leader.pid, leader.started_at).is_err());
        assert!(crate::journal::OsProcess::group_is_alive(leader.pid).unwrap());
    }

    #[test]
    fn close_waits_for_helpers_after_the_leader_exits() {
        let mut group = Group(
            Command::new("/bin/sh")
                .env_clear()
                .args([
                    "-c",
                    "trap 'exit 0' TERM; /bin/sh -c 'trap \"\" TERM; printf \"%s\\n\" $$; exec /bin/sleep 60' & wait",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()
                .unwrap(),
        );
        // The helper has installed its TERM handler before close can signal it.
        let mut ready = String::new();
        BufReader::new(group.0.stdout.take().unwrap())
            .read_line(&mut ready)
            .unwrap();
        let helper: u32 = ready.trim().parse().unwrap();
        let leader = crate::journal::OsProcess::read(group.0.id())
            .unwrap()
            .unwrap();
        let child = crate::journal::OsProcess::read(helper).unwrap().unwrap();
        crate::harness::agent_process::close_agent_process(leader.pid, leader.started_at).unwrap();
        assert!(!crate::journal::OsProcess::group_is_alive(leader.pid).unwrap());
        assert_eq!(
            crate::journal::process_identity_evidence(helper, child.started_at),
            crate::journal::ProcessIdentityEvidence::Dead
        );
    }
}
