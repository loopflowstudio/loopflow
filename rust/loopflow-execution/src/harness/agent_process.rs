//! Launch and settle AgentProcesses in the inventory used by gates and live views.
//! Unknown identity, duplicate PID/birth and unknown attachment life grant no signal authority.
use crate::os_process::terminate_process_group;
use crate::process::attachment;
use crate::process::os_process::{agent_evidence, process_identity_evidence, process_started_at};
use crate::process::{OsProcess, RecordedProcess};
use crate::store::ProcessIdentityEvidence;

use crate::harness::LaunchKind;
use crate::store::{StoreError, StoreResult};
use anyhow::{anyhow, Result};
use loopflow_storage::sqlite::SqliteStore;
use std::collections::{HashMap, HashSet};

use crate::harness::Recording;

/// The pre-exec recorder both launch paths share: the child's PID and birth
/// reach its record under the owner's attachment before the provider runs.
fn record_identity(
    Recording {
        store,
        session,
        attachment,
    }: &Recording,
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
pub fn spawn(
    mut command: tokio::process::Command,
    lifeline: Option<&std::path::Path>,
    owner: &Recording,
    kind: LaunchKind,
) -> Result<tokio::process::Child> {
    let Recording {
        store,
        session,
        attachment,
    } = owner;
    super::dispatch::off_reactor(|| -> Result<tokio::process::Child> {
        context_environment(command.as_std_mut(), owner, kind)?;
        let program = command
            .as_std()
            .get_program()
            .to_string_lossy()
            .into_owned();
        Ok(store.with_session_attachment(session, attachment, || {
            store.record_agent_process_launch(session, attachment, command.as_std(), false)?;
            let spawned = crate::os_process::spawn_agent_process(
                command,
                kind,
                lifeline,
                record_identity(owner),
            );
            if spawned.is_err() {
                store.record_agent_process_exit(session, attachment, false)?;
            }
            spawned.map_err(|error| {
                StoreError::InvalidData(format!("failed to spawn {program}: {error}"))
            })
        })?)
    })
}

/// Native terminals keep their foreground process group. Remote clients do not
/// own the provider; owned native launches use the same admission fence and
/// pre-exec recording as headless launches.
pub(crate) fn spawn_native(
    mut command: std::process::Command,
    owner: &Recording,
    kind: LaunchKind,
) -> Result<std::process::Child> {
    let Recording {
        store,
        session,
        attachment,
    } = owner;
    crate::agent_input_timing::record("provider_spawn_begin", session);
    context_environment(&mut command, owner, kind)?;
    let cwd = command.get_current_dir().map(std::path::Path::to_path_buf);
    let child = store
        .with_session_attachment(session, attachment, || {
            store.record_agent_process_launch(session, attachment, &command, true)?;
            let spawned =
                crate::os_process::spawn_native_agent_process(command, record_identity(owner));
            crate::agent_input_timing::record(
                if spawned.is_ok() {
                    "provider_spawn_returned"
                } else {
                    "provider_spawn_failed"
                },
                session,
            );
            if spawned.is_err() {
                store.record_agent_process_exit(session, attachment, false)?;
            }
            // Preserve the OS error (including NotFound) for the caller's message.
            Ok(spawned)
        })?
        .map_err(anyhow::Error::from)?;
    if let Some(cwd) = cwd {
        crate::transport::report_directory(&cwd);
    }
    Ok(child)
}

/// A managed launch reports turns to the Machine server, so its agent starts
/// once the server welcomes a connection. The selected installation starts
/// a stopped server, as it does when opening a work session.
pub(crate) fn require_server() -> Result<()> {
    use crate::machine::client::Connection;
    if Connection::open().is_ok() {
        return Ok(());
    }
    let setup = crate::installation::server::readiness_command()?.output()?;
    let Err(down) = Connection::open() else {
        return Ok(());
    };
    let setup = String::from_utf8_lossy(&setup.stderr);
    let reason = match setup.trim() {
        "" => format!("{down:#}"),
        setup => setup.to_owned(),
    };
    anyhow::bail!(
        "Loopflow server is not running, and Task and Wave work needs it ({reason}); \
         run `lf install server`, then `lf server status`"
    )
}

/// Capture identity in the launch's delivery file and give the launch its
/// steer capability. Managed hooks use it when reporting to the Machine
/// server. Every launch uses it when reading steers from the store.
fn context_environment(
    command: &mut std::process::Command,
    owner: &Recording,
    kind: LaunchKind,
) -> Result<()> {
    let Recording {
        store,
        session,
        attachment,
    } = owner;
    if let Some(path) = command.get_envs().find_map(|(name, value)| {
        (name == crate::context_block::CONTEXT_DELIVERY_ENV)
            .then_some(value)
            .flatten()
    }) {
        let path = std::path::Path::new(path);
        let mut delivery = crate::context_block::ContextDelivery::load(path)?;
        delivery.identify(store, session)?;
        delivery.save(path)?;
    }
    let capability_env = if kind.is_managed() {
        require_server()?;
        crate::steer::PROVIDER_CONTEXT_ENV
    } else {
        crate::steer::BARE_STEER_ENV
    };
    let capability = store.reserve_steer_adapter(session, attachment)?;
    let home = crate::store::lf_home_dir();
    command.env(capability_env, serde_json::to_string(&capability)?);
    command.env(
        crate::steer::STEER_SEEN_ENV,
        loopflow_storage::steer_markers::seen_marker(&home, &capability.agent_process_id),
    );
    command.env(
        crate::steer::STEER_SESSION_ENV,
        loopflow_storage::steer_markers::session_marker(&home, session),
    );
    // A Session belongs to at most one Task: Task checkouts do not nest.
    if let [task] = store.session_task_ids(session)?.as_slice() {
        command.env(
            crate::steer::STEER_TASK_ENV,
            loopflow_storage::steer_markers::task_marker(&home, task.as_str()),
        );
    }
    Ok(())
}

/// Become the provider: this process is the AgentProcess, recorded under the
/// owner's attachment before the exec. Returns only when the exec failed, with
/// the launch recorded as never spawned.
pub(crate) fn exec_native(
    mut command: std::process::Command,
    owner: &Recording,
    kind: LaunchKind,
    launch_lock: Option<std::fs::File>,
) -> std::io::Error {
    use std::os::unix::process::CommandExt;
    let Recording {
        store,
        session,
        attachment,
    } = owner;
    crate::agent_input_timing::record("provider_exec_begin", session);
    if let Err(error) = context_environment(&mut command, owner, kind) {
        return std::io::Error::other(error.to_string());
    }
    let pid = std::process::id();
    let started_at = match process_started_at(pid) {
        Ok(Some(started_at)) => started_at,
        Ok(None) => return std::io::Error::other("AgentProcess birth unavailable before exec"),
        Err(error) => return error,
    };
    let admitted = store.with_session_attachment(session, attachment, || {
        store.record_agent_process_launch(session, attachment, &command, true)?;
        store.record_agent_process_identity(session, attachment, pid, started_at)
    });
    if let Err(error) = admitted {
        return std::io::Error::other(error.to_string());
    }
    // Identity is visible before another connect can acquire the launch lock.
    drop(launch_lock);
    let error = command.exec();
    crate::agent_input_timing::record("provider_exec_failed", session);
    if let Err(record) = store.with_session_attachment(session, attachment, || {
        store.record_agent_process_exit(session, attachment, false)
    }) {
        tracing::warn!(error = %record, "failed exec was not recorded");
    }
    error
}

/// Become a client of the provider running beside this process. The client
/// takes over holding the provider's lifeline, so the provider outlives lf.
pub(crate) fn exec_client(
    mut command: std::process::Command,
    launch_lock: Option<std::fs::File>,
) -> std::io::Error {
    use std::os::unix::process::CommandExt;
    if let Err(error) = crate::os_process::inherit_lifelines() {
        return error;
    }
    drop(launch_lock);
    command.exec()
}

/// Failed terminal setup still owns an admitted child. Serialize its cleanup
/// with takeover and record only an exact successful wait. A remote client has
/// no provider attachment; ending it must not settle the surviving provider.
pub(crate) fn stop_native(
    child: &mut std::process::Child,
    owner: Option<&Recording>,
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
        if let Some(Recording {
            store,
            session,
            attachment,
        }) = owner
        {
            store.record_agent_process_exit(session, attachment, true)?;
        }
        Ok(())
    };
    match owner {
        Some(Recording {
            store,
            session,
            attachment,
        }) => store.with_session_attachment(session, attachment, stop),
        None => stop(),
    }
    .map_err(Into::into)
}

/// Explicit Move stops the recorded provider under the same fence as takeover.
/// A native provider shares the terminal's group, so signal only its exact PID.
/// Headless providers retain the existing descendant/group stop policy.
/// `departure` says where the conversation goes, for the terminal it leaves.
pub(crate) fn stop_session_agent(
    store: &SqliteStore,
    session: &str,
    departure: crate::process::Departure,
) -> Result<()> {
    let Some(attachment) = store.session_attachment(session)? else {
        return Ok(());
    };
    let agent = store
        .agent_processes()?
        .into_iter()
        .find(|agent| agent.id == attachment.agent_process_id);
    let Some(agent) = agent else {
        return Ok(());
    };
    // Already-dead providers have no observed interruption outcome. Preserve that
    // uncertainty instead of attributing their death to this Move request.
    // When its own exit settled it first, stop whatever the Session holds now.
    if agent_evidence(store, &agent.id) == ProcessIdentityEvidence::Dead {
        let settled = attachment::settle_agent_process(store, &agent, || {
            if agent_evidence(store, &agent.id) != ProcessIdentityEvidence::Dead {
                return Err(StoreError::InvalidAuthority(
                    "AgentProcess death changed".into(),
                ));
            }
            Ok(())
        })?;
        if !settled {
            return stop_session_agent(store, session, departure);
        }
        return Ok(());
    }
    let stop = || -> Result<()> {
        let (pid, birth) = agent.pid.zip(agent.os_started_at).ok_or_else(|| {
            anyhow::anyhow!("AgentProcess identity is unknown; resume was not started")
        })?;
        match process_identity_evidence(pid, birth) {
            ProcessIdentityEvidence::Dead => return Ok(()),
            ProcessIdentityEvidence::Unknown => {
                anyhow::bail!("AgentProcess liveness is unknown; resume was not started")
            }
            ProcessIdentityEvidence::Live => {}
        }
        if !agent.interactive {
            return terminate_agent_process(pid, birth, agent.provider.as_deref());
        }
        for signal in [libc::SIGTERM, libc::SIGKILL] {
            match process_identity_evidence(pid, birth) {
                ProcessIdentityEvidence::Dead => return Ok(()),
                ProcessIdentityEvidence::Unknown => {
                    anyhow::bail!("AgentProcess identity became unknown")
                }
                ProcessIdentityEvidence::Live => {}
            }
            anyhow::ensure!(pid > 1, "invalid AgentProcess PID");
            let pid = libc::pid_t::try_from(pid)?;
            // SAFETY: a positive PID whose recorded birth was just checked;
            // never target the shared foreground process group.
            if unsafe { libc::kill(pid, signal) } != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error.into());
                }
            }
            for _ in 0..20 {
                if process_identity_evidence(pid as u32, birth) == ProcessIdentityEvidence::Dead {
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        anyhow::bail!("AgentProcess did not exit; resume was not started")
    };
    attachment::finish_session_attachment(
        store,
        session,
        &attachment,
        "interrupted",
        Some(&departure),
        || {
            stop().map_err(|error| StoreError::InvalidAuthority(error.to_string()))?;
            Ok(true)
        },
    )?;
    Ok(())
}

/// Stop a harness's provider without settling its invocation. Group death and
/// the exit receipt remain under the same frozen attachment fence.
pub fn stop(
    Recording {
        store,
        session,
        attachment,
    }: &Recording,
) -> Result<()> {
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
/// Foreground providers may be observed dead, but never use headless group control.
pub(crate) fn close_session_agent_process(store: &SqliteStore, session: &str) -> StoreResult<bool> {
    let Some(attachment) = store.session_attachment(session)? else {
        return Ok(false);
    };
    let agents = store.agent_processes()?;
    let Some(agent) = agents
        .iter()
        .find(|agent| agent.id == attachment.agent_process_id)
    else {
        return Ok(false);
    };
    let Some((pid, started)) = agent.pid.zip(agent.os_started_at) else {
        return Ok(false);
    };
    if agent.interactive {
        return Ok(process_identity_evidence(pid, started) == ProcessIdentityEvidence::Dead);
    }
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
    if agents.iter().any(|other| {
        other.id != agent.id && other.pid == Some(pid) && other.os_started_at == Some(started)
    }) {
        return Err(StoreError::InvalidAuthority(
            "AgentProcess OS identity has multiple owners".into(),
        ));
    }
    if let Some(kind) = agent
        .provider
        .as_deref()
        .and_then(loopflow_harness::HarnessKind::parse)
    {
        crate::harness::launcher(kind).validate_agent_process_close(store, session, &agent.id)?;
    }
    close_agent_process(pid, started)
        .map_err(|error| StoreError::InvalidAuthority(error.to_string()))?;
    Ok(true)
}

/// Stop an exactly recorded headless process group. The caller holds its Session
/// attachment fence and has refused ambiguous ownership before reaching here.
fn close_agent_process(pid: u32, started: i64) -> Result<()> {
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
    // A worker owns its OS session, including jobs that outlived their
    // immediate parent. Other providers own only their recorded group.
    match OsProcess::session_id(pid)? {
        Some(session) if session == pid => crate::os_process::terminate_worker_jobs(pid, started),
        Some(_) => terminate_process_group(pid)
            .map_err(|error| anyhow!("AgentProcess group {pid} death is unresolved: {error}")),
        None => confirmed_group_death(pid),
    }
}

// Leader death alone neither proves group death nor authorizes signaling helpers.
fn confirmed_group_death(pid: u32) -> Result<()> {
    crate::os_process::release_agent_lifeline(pid);
    let mut survivors = OsProcess::group(pid)?;
    // Recovery in another process holds no lifeline writer, but can still
    // observe helpers finishing the previous owner's teardown.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !survivors.is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(25));
        survivors = OsProcess::group(pid)?;
    }
    if !survivors.is_empty() {
        return Err(anyhow!(
            "AgentProcess group {pid} survives its recorded leader: {}",
            OsProcess::describe(&survivors)
        ));
    }
    Ok(())
}

/// What one reaping pass did: agents it settled (their end written), headless
/// providers it stopped, and the failures it left for the next pass.
#[derive(Debug, Default)]
pub struct AgentProcessReapReport {
    pub settled: u32,
    pub reaped: Vec<u32>,
    pub errors: Vec<String>,
}

/// Settle dead agents and stop headless providers no live lf holds.
pub fn reap_agent_processes(database: &std::path::Path) -> Result<AgentProcessReapReport> {
    let store = crate::store::open_sqlite(database)?;
    Ok(reap_in(&store)?)
}

/// A Session agent's holder is dead when no lf holds its attachment, or the
/// lf holding it no longer runs.
fn holder_dead(agent: &crate::process::AgentProcessState) -> bool {
    agent.holder.as_ref().is_none_or(|holder| {
        RecordedProcess::from(holder).evidence() == ProcessIdentityEvidence::Dead
    })
}

fn reap_in(store: &SqliteStore) -> StoreResult<AgentProcessReapReport> {
    let agents = store.agent_processes()?;
    let mut counts = HashMap::new();
    for agent in &agents {
        if let Some(identity) = agent.pid.zip(agent.os_started_at) {
            *counts.entry(identity).or_insert(0usize) += 1;
        }
    }
    let mut report = AgentProcessReapReport::default();
    for agent in &agents {
        let Some((pid, start)) = agent.pid.zip(agent.os_started_at) else {
            // A reservation never requested a launch. Serialize with launch/takeover,
            // and require the lf holding it to be dead before retiring it.
            if agent.spawn_reserved && holder_dead(agent) {
                match attachment::settle_agent_process(store, agent, || {
                    if !holder_dead(agent) {
                        return Err(StoreError::InvalidAuthority(
                            "reservation owner death changed".into(),
                        ));
                    }
                    Ok(())
                }) {
                    Ok(true) => report.settled += 1,
                    Ok(false) => {}
                    Err(error) => report
                        .errors
                        .push(format!("AgentProcess {}: {error}", agent.id)),
                }
            }
            continue;
        };
        // PID absence is not exit evidence when the OS cannot be read.
        let observed = match OsProcess::read(pid) {
            Ok(process) => process,
            Err(error) => {
                report.errors.push(format!(
                    "AgentProcess {}: cannot observe PID {pid}: {error}",
                    agent.id
                ));
                continue;
            }
        };
        let dead = observed
            .as_ref()
            .is_none_or(|process| process.evidence(start) == ProcessIdentityEvidence::Dead);
        let orphaned = counts[&(pid, start)] == 1
            && !agent.interactive
            && holder_dead(agent)
            && observed
                .as_ref()
                .is_some_and(|process| is_agent_process(process, start, agent.provider.as_deref()));
        if !dead && !orphaned {
            continue;
        }
        let result = attachment::settle_agent_process(store, agent, || {
            if dead {
                if process_identity_evidence(pid, start) != ProcessIdentityEvidence::Dead {
                    return Err(StoreError::InvalidAuthority(
                        "AgentProcess death changed".into(),
                    ));
                }
            } else {
                // Attachment life is sampled again beneath the transfer lock.
                if !holder_dead(agent) {
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
            Ok(true) if orphaned => report.reaped.push(pid),
            Ok(true) => report.settled += 1,
            Ok(false) => continue,
            Err(error) => {
                report
                    .errors
                    .push(format!("AgentProcess {}: {error}", agent.id));
                continue;
            }
        }
        let session = agent.session_id.as_str();
        let Some(attachment) = store.session_attachment(session)? else {
            continue;
        };
        if attachment.agent_process_id != agent.id
            || attachment.lf_process_id.is_none()
            || !holder_dead(agent)
        {
            continue;
        }
        match attachment::finish_session_attachment(
            store,
            session,
            &attachment,
            "interrupted",
            None,
            || Ok(false),
        ) {
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
    provider
        .and_then(loopflow_harness::HarnessKind::parse)
        .is_some_and(|kind| kind.harness().recognizes_driver(&process.command))
}

/// Reap a verified provider and its jobs. Providers without a private OS
/// session can still have attached helpers leading their own groups.
fn terminate_agent_process(pid: u32, started_at: i64, provider: Option<&str>) -> Result<()> {
    let refused = |reason: &str| anyhow::anyhow!("AgentProcess {pid} {reason}");
    let descendants = if OsProcess::session_id(pid)? == Some(pid) {
        Vec::new()
    } else {
        descendant_group_leaders(pid, &OsProcess::sample_names()?)
    };
    if !OsProcess::read(pid)?
        .as_ref()
        .is_some_and(|process| is_agent_process(process, started_at, provider))
    {
        return Err(refused("is no longer the recorded process"));
    }
    close_agent_process(pid, started_at)?;
    let mut failures = Vec::new();
    for (leader, birth) in descendants {
        let result = match OsProcess::read(leader) {
            Ok(Some(process)) if process.pgid == leader && process.matches_start(leader, birth) => {
                terminate_process_group(leader)
            }
            Ok(_) => continue,
            Err(error) => Err(error.into()),
        };
        if let Err(error) = result {
            failures.push(format!("group {leader}: {error}"));
        }
    }
    anyhow::ensure!(
        failures.is_empty(),
        "AgentProcess {pid} has unresolved descendants: {}",
        failures.join(", ")
    );
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
    use crate::process::attachment;

    use crate::harness::LaunchKind;
    use loopflow_model::id::LfProcessId;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    #[test]
    fn move_after_agent_death_preserves_unknown_outcome() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("dead", &crate::session_record::new_artifact_key());
        let attachment = attachment::claim_session_attachment(
            &store,
            &session.id,
            None,
            &store.test_lf_process(),
        )
        .unwrap();
        let mut command = Command::new("/usr/bin/true");
        command.env_clear();
        let mut child = super::spawn_native(
            command,
            &crate::harness::Recording {
                store: store.clone(),
                session: session.id.to_string(),
                attachment: attachment.clone(),
            },
            LaunchKind::Batch,
        )
        .unwrap();
        child.wait().unwrap();
        super::stop_session_agent(
            &store,
            &session.id,
            crate::process::Departure::Terminal { to: None },
        )
        .unwrap();
        let process = store
            .agent_process(&attachment.agent_process_id)
            .unwrap()
            .unwrap();
        assert!(process.completed_at.is_some());
        assert_eq!(
            store.session_attachment(&session.id).unwrap(),
            Some(attachment)
        );
    }

    #[test]
    fn a_reservation_under_a_killed_holder_is_not_live_and_the_next_pass_ends_it() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("reserved", &crate::session_record::new_artifact_key());
        let mut holder = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let pid = holder.id();
        let birth = crate::process::os_process::process_started_at(pid)
            .unwrap()
            .unwrap();
        let launcher = LfProcessId::new();
        let start = crate::process::AttachingLf {
            lf_process_id: launcher.clone(),
            holder: crate::process::AttachmentHolder {
                pid: Some(pid),
                os_started_at: Some(birth),
            },
            terminal: None,
        };
        let nobody = loopflow_storage::sqlite::SessionCaller {
            agent: None,
            capture_key: None,
        };
        let attachment = store
            .attach_session(&session.id, None, &launcher, Some(&start), &nobody)
            .unwrap();
        let live = || {
            crate::store::session_projection::session_summary(
                &store,
                &session.id,
                loopflow_storage::rows::now_unix(),
            )
            .unwrap()
            .unwrap()
            .agent_process_evidence
        };
        assert_eq!(live(), crate::store::ProcessIdentityEvidence::Unknown);

        holder.kill().unwrap();
        holder.wait().unwrap();
        assert_eq!(live(), crate::store::ProcessIdentityEvidence::Dead);
        assert_eq!(
            crate::process::os_process::agent_evidence(&store, &attachment.agent_process_id),
            crate::store::ProcessIdentityEvidence::Dead
        );

        let report = super::reap_in(&store).unwrap();
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let agent = store
            .agent_state(&attachment.agent_process_id)
            .unwrap()
            .unwrap();
        assert!(agent.ended_at.is_some());
        assert!(store.agent_processes().unwrap().is_empty());
    }

    #[test]
    fn a_stop_records_where_the_conversation_went_for_the_attachment_it_ended() {
        use crate::process::Departure;
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("db");
        let store = crate::store::opening::open_ephemeral(&database).unwrap();
        let session = store.test_session("moved", &crate::session_record::new_artifact_key());
        let sql = rusqlite::Connection::open(&database).unwrap();
        sql.execute(
            "UPDATE agent_sessions SET interactive=1 WHERE id=?1",
            [&session.id],
        )
        .unwrap();
        let departures = [
            Departure::Terminal {
                to: Some("Desktop".into()),
            },
            Departure::Machine {
                machine: loopflow_model::durable::MachineId::new(),
                task: Some(loopflow_model::durable::TaskId::new()),
            },
        ];
        for departure in departures {
            let parent = LfProcessId::new();
            sql.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
                [&parent],
            )
            .unwrap();
            let current = store.session_attachment(&session.id).unwrap();
            let attachment = attachment::claim_session_attachment(
                &store,
                &session.id,
                current.as_ref(),
                &parent,
            )
            .unwrap();
            let mut command = Command::new("/bin/sleep");
            command.env_clear().arg("60");
            let mut child = super::spawn_native(
                command,
                &crate::harness::Recording {
                    store: store.clone(),
                    session: session.id.to_string(),
                    attachment: attachment.clone(),
                },
                LaunchKind::Batch,
            )
            .unwrap();
            let stopped = super::stop_session_agent(&store, &session.id, departure.clone());
            let _ = child.kill();
            child.wait().unwrap();
            stopped.unwrap();
            let retained = store.session(&session.id).unwrap().unwrap();
            assert_eq!(retained.completed_at, None, "a move continues the Session");
            assert_eq!(
                store
                    .attachment_departure(&session.id, &attachment)
                    .unwrap(),
                Some(departure)
            );
            let after = store.session_attachment(&session.id).unwrap().unwrap();
            assert_eq!(
                store.attachment_departure(&session.id, &after).unwrap(),
                None
            );
        }
    }

    #[test]
    fn native_move_uses_process_identity_and_clears_the_terminal() {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("db");
        let store = crate::store::opening::open_ephemeral(&database).unwrap();
        let session = store.test_session("move", &crate::session_record::new_artifact_key());
        let sql = rusqlite::Connection::open(&database).unwrap();
        sql.execute(
            "UPDATE agent_sessions SET interactive=1 WHERE id=?1",
            [&session.id],
        )
        .unwrap();
        let parent = LfProcessId::new();
        // A launcher that may still be running; an ended launcher would make
        // its unidentified agent dead instead.
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,strftime('%s','now'))",
            [&parent],
        )
        .unwrap();
        let attachment =
            attachment::claim_session_attachment(&store, &session.id, None, &parent).unwrap();
        // Missing birth cannot become permission to signal or replace.
        assert!(super::stop_session_agent(
            &store,
            &session.id,
            crate::process::Departure::Terminal { to: None }
        )
        .is_err());
        assert_eq!(
            store.session_attachment(&session.id).unwrap(),
            Some(attachment.clone())
        );
        let mut command = Command::new("/bin/sleep");
        command.env_clear().arg("60");
        let mut child = super::spawn_native(
            command,
            &crate::harness::Recording {
                store: store.clone(),
                session: session.id.to_string(),
                attachment: attachment.clone(),
            },
            LaunchKind::Batch,
        )
        .unwrap();
        store
            .record_session_terminal(&session.id, &attachment, Some("first-pane"))
            .unwrap();
        let summary = crate::store::session_projection::session_summary(&store, &session.id, 1)
            .unwrap()
            .unwrap();
        assert_eq!(summary.terminal_id.as_deref(), Some("first-pane"));
        assert_eq!(
            summary.agent_process_evidence,
            crate::store::ProcessIdentityEvidence::Live
        );
        let stopped = super::stop_session_agent(
            &store,
            &session.id,
            crate::process::Departure::Terminal { to: None },
        );
        // Always reap only this fixture's child, including when the assertion fails.
        let _ = child.kill();
        child.wait().unwrap();
        stopped.unwrap();
        let summary = crate::store::session_projection::session_summary(&store, &session.id, 1)
            .unwrap()
            .unwrap();
        assert_eq!(
            summary.agent_process_evidence,
            crate::store::ProcessIdentityEvidence::Dead
        );
        assert!(summary.terminal_id.is_none());
        assert!(!summary.available);
        assert!(store
            .agent_process(&attachment.agent_process_id)
            .unwrap()
            .unwrap()
            .completed_at
            .is_some());
    }

    #[test]
    fn reaping_retires_only_unlaunched_reservations_with_dead_owners() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let database = ledger.home().join("loopflow.db");
        let store = crate::store::opening::open_ephemeral(&database).unwrap();
        let sql = rusqlite::Connection::open(database).unwrap();
        let mut expected = Vec::new();
        for (name, state, ended) in [
            ("retired", Some("reserved"), true),
            ("unknown", None, true),
            ("requested", Some("spawn_requested"), true),
            ("unresolved-owner", Some("reserved"), false),
        ] {
            let session = store.test_session(name, &crate::session_record::new_artifact_key());
            let attachment = attachment::claim_session_attachment(
                &store,
                &session.id,
                None,
                &LfProcessId::new(),
            )
            .unwrap();
            // This test's own process holds each attachment; one recorded
            // with another start time is an lf that has exited.
            let birth = crate::process::os_process::process_started_at(std::process::id())
                .unwrap()
                .unwrap();
            store.test_set_holder(
                &session.id,
                std::process::id(),
                if ended { birth - 3600 } else { birth },
            );
            sql.execute(
                "UPDATE agent_sessions SET agent_spawn_state=?2 WHERE agent_process_id=?1",
                rusqlite::params![attachment.agent_process_id, state],
            )
            .unwrap();
            expected.push((attachment.agent_process_id, name == "retired"));
        }
        let applied = super::reap_in(&store).unwrap();
        assert!(applied.errors.is_empty());
        assert_eq!(applied.settled, 1);
        for (id, retired) in &expected {
            let process = store.agent_process(id).unwrap().unwrap();
            assert_eq!(process.completed_at.is_some(), *retired);
            assert!(process.pid.is_none());
        }
        let retry = super::reap_in(&store).unwrap();
        assert_eq!(retry.settled, 0);
        assert!(store.session("retired").unwrap().is_some());
    }

    #[test]
    fn observed_agent_death_settles_only_a_provably_dead_attachment() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let database = ledger.home().join("loopflow.db");
        let store = crate::store::opening::open_ephemeral(&database).unwrap();
        for ended in [false, true] {
            let session = store.test_session(
                if ended { "ended" } else { "unknown" },
                &crate::session_record::new_artifact_key(),
            );
            let attachment = attachment::claim_session_attachment(
                &store,
                &session.id,
                None,
                &LfProcessId::new(),
            )
            .unwrap();
            // A holder recorded with another start time has exited.
            let birth = crate::process::os_process::process_started_at(std::process::id())
                .unwrap()
                .unwrap();
            store.test_set_holder(
                &session.id,
                std::process::id(),
                if ended { birth - 3600 } else { birth },
            );
            let mut command = Command::new("/usr/bin/true");
            command.env_clear();
            let mut child = super::spawn_native(
                command,
                &crate::harness::Recording {
                    store: store.clone(),
                    session: session.id.to_string(),
                    attachment: attachment.clone(),
                },
                LaunchKind::Batch,
            )
            .unwrap();
            child.wait().unwrap();
            // No launcher exit receipt: the reaper must observe the OS death.
            let report = super::reap_in(&store).unwrap();
            assert!(report.errors.is_empty());
            assert!(report.reaped.is_empty());
            let process = store
                .agent_process(&attachment.agent_process_id)
                .unwrap()
                .unwrap();
            assert!(process.completed_at.is_some());
            let current = store.session_attachment(&session.id).unwrap().unwrap();
            assert_eq!(current.lf_process_id.is_none(), ended);
            // An exit with nowhere to go leaves no history.
            let history = store.session_history(&session.id, 0, 0).unwrap();
            assert!(!history
                .iter()
                .any(|event| event.payload["type"] == "attachment_exit"));
            if !ended {
                assert_eq!(current, attachment);
            }
        }
    }

    #[test]
    fn native_cleanup_records_wait_without_touching_a_replacement_or_remote_provider() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("cleanup", &crate::session_record::new_artifact_key());
        let parent = store.test_lf_process();
        let first =
            attachment::claim_session_attachment(&store, &session.id, None, &parent).unwrap();
        let first_owner = crate::harness::Recording {
            store: store.clone(),
            session: session.id.to_string(),
            attachment: first.clone(),
        };
        let command = || {
            let mut command = Command::new("/bin/sleep");
            command.env_clear().arg("60");
            command
        };
        let mut child = super::spawn_native(command(), &first_owner, LaunchKind::Batch).unwrap();
        let pid = child.id();
        super::stop_native(&mut child, Some(&first_owner)).unwrap();
        let exited = store
            .agent_process(&first.agent_process_id)
            .unwrap()
            .unwrap();
        assert_eq!(exited.pid, Some(pid));
        assert!(exited.os_started_at.is_some());
        assert!(exited.completed_at.is_some());

        let next = attachment::prepare_session_agent_process(&store, &session.id, &first).unwrap();
        let next_owner = crate::harness::Recording {
            store: store.clone(),
            session: session.id.to_string(),
            attachment: next.clone(),
        };
        let mut provider = super::spawn_native(command(), &next_owner, LaunchKind::Batch).unwrap();
        let recorded = store
            .agent_process(&next.agent_process_id)
            .unwrap()
            .unwrap();
        // A late wait receipt from the previous provider cannot settle this one.
        assert!(store
            .record_agent_process_exit(&session.id, &first, true)
            .is_err());
        assert_eq!(
            store.agent_process(&next.agent_process_id).unwrap(),
            Some(recorded.clone())
        );
        // Reusing the current attachment is not permission for a second spawn.
        // Reject before fork, without recording failure on the running process.
        let duplicate = super::spawn_native(command(), &next_owner, LaunchKind::Batch);
        assert!(duplicate.is_err());
        assert_eq!(
            store.agent_process(&next.agent_process_id).unwrap(),
            Some(recorded)
        );
        // A delayed cleanup has no authority over a replacement, even if it
        // accidentally receives the replacement's child handle.
        assert!(super::stop_native(&mut provider, Some(&first_owner)).is_err());
        let provider_alive = provider.try_wait().unwrap().is_none();
        let mut client = command().spawn().unwrap();
        super::stop_native(&mut client, None).unwrap();
        let remote_unchanged = store
            .agent_process(&next.agent_process_id)
            .unwrap()
            .unwrap();
        let remote_alive = provider.try_wait().unwrap().is_none();
        super::stop_native(&mut provider, Some(&next_owner)).unwrap();
        assert!(provider_alive && remote_alive);
        assert!(remote_unchanged.completed_at.is_none());
        assert_eq!(
            store.agent_process(&first.agent_process_id).unwrap(),
            Some(exited)
        );
    }

    #[test]
    fn native_cleanup_failed_wait_retains_unknown_exit() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("failed-wait", &crate::session_record::new_artifact_key());
        let attachment = attachment::claim_session_attachment(
            &store,
            &session.id,
            None,
            &store.test_lf_process(),
        )
        .unwrap();
        let owner = crate::harness::Recording {
            store: store.clone(),
            session: session.id.to_string(),
            attachment: attachment.clone(),
        };
        let mut command = Command::new("/bin/sh");
        command.env_clear().args(["-c", "exit 42"]);
        let mut child = super::spawn_native(command, &owner, LaunchKind::Batch).unwrap();
        // Reap only this throwaway child outside its handle, forcing ECHILD.
        // SAFETY: waitpid observes the exact fixture child; it signals nothing.
        assert_eq!(
            unsafe { libc::waitpid(child.id() as i32, std::ptr::null_mut(), 0) },
            child.id() as i32
        );
        assert!(super::stop_native(&mut child, Some(&owner)).is_err());
        let retained = store
            .agent_process(&attachment.agent_process_id)
            .unwrap()
            .unwrap();
        assert_eq!(retained.pid, Some(child.id()));
        assert!(retained.completed_at.is_none());
    }

    #[tokio::test]
    async fn headless_spawn_retains_attempts_and_refuses_stale_attachments() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("spawn", &crate::session_record::new_artifact_key());
        let parent = store.test_lf_process();
        let first =
            attachment::claim_session_attachment(&store, &session.id, None, &parent).unwrap();
        let owner = |attachment| crate::harness::Recording {
            store: store.clone(),
            session: session.id.to_string(),
            attachment,
        };
        let mut command = tokio::process::Command::new("/bin/sh");
        command.env_clear().args(["-c", "exit 42"]);
        let mut child =
            super::spawn(command, None, &owner(first.clone()), LaunchKind::Batch).unwrap();
        let recorded = store
            .agent_process(&first.agent_process_id)
            .unwrap()
            .unwrap();
        assert_eq!(recorded.pid, child.id());
        assert!(recorded.os_started_at.is_some());
        assert_eq!(child.wait().await.unwrap().code(), Some(42));
        // Spawn success alone must not manufacture exit evidence.
        assert!(recorded.completed_at.is_none());
        store
            .record_agent_process_exit(&session.id, &first, true)
            .unwrap();
        let next = attachment::prepare_session_agent_process(&store, &session.id, &first).unwrap();

        let marker = home.path().join("stale-effect");
        let mut stale = tokio::process::Command::new("/bin/sh");
        stale
            .env_clear()
            .args(["-c", "printf effect > \"$1\"", "fixture"])
            .arg(&marker);
        assert!(super::spawn(stale, None, &owner(first.clone()), LaunchKind::Batch).is_err());
        assert!(!marker.exists());
        assert_eq!(
            store.session_attachment(&session.id).unwrap(),
            Some(next.clone())
        );
        assert!(store
            .agent_process(&next.agent_process_id)
            .unwrap()
            .unwrap()
            .pid
            .is_none());

        let missing = tokio::process::Command::new(home.path().join("absent-provider"));
        assert!(super::spawn(missing, None, &owner(next.clone()), LaunchKind::Batch).is_err());
        let failed = store
            .agent_process(&next.agent_process_id)
            .unwrap()
            .unwrap();
        assert!(failed.completed_at.is_some());
        let retry = attachment::prepare_session_agent_process(&store, &session.id, &next).unwrap();
        assert_ne!(retry.agent_process_id, next.agent_process_id);
        assert_eq!(
            store.agent_process(&next.agent_process_id).unwrap(),
            Some(failed)
        );
        assert_eq!(
            store
                .agent_process(&first.agent_process_id)
                .unwrap()
                .unwrap()
                .pid,
            recorded.pid
        );
    }

    /// A bare conversation reads steers without reporting to a server.
    #[test]
    fn a_bare_launch_carries_only_a_store_steer_capability() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let capability = |kind: crate::harness::LaunchKind| {
            let id = format!("{kind:?}");
            let session = store.test_session(&id, &crate::session_record::new_artifact_key());
            let attachment = attachment::claim_session_attachment(
                &store,
                &session.id,
                None,
                &store.test_lf_process(),
            )
            .unwrap();
            let seen = home.path().join(&id);
            let mut command = Command::new("/bin/sh");
            command.env_clear().args([
                "-c",
                &format!(
                    "printf '%s\\n%s' \"${{{}:-}}\" \"${{{}:-}}\" > {}",
                    crate::steer::PROVIDER_CONTEXT_ENV,
                    crate::steer::BARE_STEER_ENV,
                    seen.display()
                ),
            ]);
            let owner = crate::harness::Recording {
                store: store.clone(),
                session: session.id.to_string(),
                attachment,
            };
            super::spawn_native(command, &owner, kind)
                .unwrap()
                .wait()
                .unwrap();
            std::fs::read_to_string(seen).unwrap()
        };
        let output = capability(LaunchKind::Batch);
        let (server, bare) = output.split_once('\n').unwrap();
        assert_eq!(server, "");
        let adapter: crate::steer::SteerAdapter = serde_json::from_str(bare).unwrap();
        assert_eq!(adapter.session, "Batch");
    }

    #[test]
    fn native_spawn_retains_identity_and_fences_effects_after_takeover() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("native", &crate::session_record::new_artifact_key());
        let parent = store.test_lf_process();
        let first =
            attachment::claim_session_attachment(&store, &session.id, None, &parent).unwrap();
        let owner = crate::harness::Recording {
            store: store.clone(),
            session: session.id.to_string(),
            attachment: first.clone(),
        };
        let mut command = Command::new("/bin/sh");
        command.env_clear().args(["-c", "exit 42"]);
        let mut child = super::spawn_native(command, &owner, LaunchKind::Batch).unwrap();
        let recorded = store
            .agent_process(&first.agent_process_id)
            .unwrap()
            .unwrap();
        assert_eq!(recorded.pid, Some(child.id()));
        assert!(recorded.os_started_at.is_some());
        assert_eq!(child.wait().unwrap().code(), Some(42));
        store
            .record_agent_process_exit(&session.id, &first, true)
            .unwrap();
        let next = attachment::prepare_session_agent_process(&store, &session.id, &first).unwrap();
        let marker = home.path().join("stale-effect");
        let mut stale = Command::new("/bin/sh");
        stale
            .env_clear()
            .args(["-c", "printf effect > \"$1\"", "fixture"])
            .arg(&marker);
        assert!(super::spawn_native(stale, &owner, LaunchKind::Batch).is_err());
        assert!(!marker.exists());
        assert_eq!(
            store.session_attachment(&session.id).unwrap(),
            Some(next.clone())
        );
        let owner = crate::harness::Recording {
            store: store.clone(),
            session: session.id.to_string(),
            attachment: next.clone(),
        };
        let error = super::spawn_native(
            Command::new(home.path().join("absent")),
            &owner,
            LaunchKind::Batch,
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::NotFound
        );
        let failed = store
            .agent_process(&next.agent_process_id)
            .unwrap()
            .unwrap();
        assert!(failed.completed_at.is_some());
        assert_eq!(
            store
                .agent_process(&first.agent_process_id)
                .unwrap()
                .unwrap()
                .pid,
            recorded.pid
        );
    }

    #[test]
    fn detached_agent_is_reaped_but_duplicate_historical_identity_is_not_signal_authority() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        for (provider, argv) in [
            ("codex", &["codex", "app-server", "--listen", "fixture"][..]),
            ("opencode", &["opencode", "run"]),
            ("claude", &["claude"]),
        ] {
            reap_detached(provider, argv);
        }
    }

    /// Only the portable reader invokes ps; macOS observes the kernel directly.
    /// A failed reading, of the leader or only of its descendants, leaves the
    /// orphan running and its records untouched.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn failed_sampling_reaps_nothing_and_reports_why() {
        use std::os::unix::fs::PermissionsExt;

        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
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
        let start = crate::process::os_process::process_started_at(pid)
            .unwrap()
            .unwrap();
        let session = store.test_session("orphan", &crate::session_record::new_artifact_key());
        rusqlite::Connection::open(home.path().join("db"))
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider='codex',interactive=0 WHERE id=?1",
                [session.id.as_str()],
            )
            .unwrap();
        let attached = attachment::claim_session_attachment(
            &store,
            &session.id,
            None,
            &store.test_lf_process(),
        )
        .unwrap();
        store
            .record_agent_process_identity(&session.id, &attached, pid, start)
            .unwrap();
        store
            .release_session_attachment(&session.id, &attached)
            .unwrap();
        let before = store.agent_process(&attached.agent_process_id).unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let ps = bin.join("ps");
        let previous = std::env::var_os("PATH");
        // Exact leader observation succeeds in the second reading, but
        // descendant inventory fails: the reaper refuses before signaling.
        for forward in ["", "if [ \"$1\" = -p ]; then exec /bin/ps \"$@\"; fi\n"] {
            std::fs::write(
                &ps,
                format!("#!/bin/sh\n{forward}printf 'fixture sampling unavailable' >&2\nexit 2\n"),
            )
            .unwrap();
            std::fs::set_permissions(&ps, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::env::set_var("PATH", format!("{}:/usr/bin:/bin", bin.display()));
            let report = super::reap_in(&store);
            match &previous {
                Some(path) => std::env::set_var("PATH", path),
                None => std::env::remove_var("PATH"),
            }
            let report = report.unwrap();
            assert!(
                report
                    .errors
                    .iter()
                    .any(|error| error.contains("fixture sampling unavailable")),
                "{report:?}"
            );
            assert!(report.reaped.is_empty(), "{report:?}");
            assert!(child.try_wait().unwrap().is_none());
            assert_eq!(
                store.agent_process(&attached.agent_process_id).unwrap(),
                before
            );
        }
        let _ = crate::os_process::terminate_process_group(pid);
        let _ = child.wait();
    }

    fn reap_detached(provider: &str, argv: &[&str]) {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
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
        let start = crate::process::os_process::process_started_at(pid)
            .unwrap()
            .unwrap();
        let parent = store.test_lf_process();
        for name in ["first", "conflict"] {
            let session = store.test_session(name, &crate::session_record::new_artifact_key());
            rusqlite::Connection::open(home.path().join("db"))
                .unwrap()
                .execute(
                    "UPDATE agent_sessions SET provider=?2,interactive=0 WHERE id=?1",
                    [name, provider],
                )
                .unwrap();
            let attached =
                attachment::claim_session_attachment(&store, &session.id, None, &parent).unwrap();
            store
                .record_agent_process_identity(&session.id, &attached, pid, start)
                .unwrap();
            store
                .release_session_attachment(&session.id, &attached)
                .unwrap();
        }
        let conflict = super::reap_in(&store).unwrap();
        assert!(conflict.errors.is_empty());
        assert!(conflict.reaped.is_empty());
        assert!(child.try_wait().unwrap().is_none());
        // Retire only the fixture's duplicate observation, not its OS process.
        rusqlite::Connection::open(home.path().join("db"))
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET agent_ended_at=1, agent_pid=NULL, agent_os_started_at=NULL WHERE id='conflict'",
                [],
            )
            .unwrap();
        let report = super::reap_in(&store).unwrap();
        // Always clean up this fixture's own group before assertions.
        let _ = crate::os_process::terminate_process_group(pid);
        let _ = child.wait();
        assert!(report.errors.is_empty(), "{provider}: {:?}", report.errors);
        assert_eq!(report.reaped, [pid], "{provider}");
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
            let _ = crate::os_process::terminate_process_group(self.0.id());
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
        let leader = crate::process::OsProcess::read(group.0.id())
            .unwrap()
            .unwrap();
        // Closing stdin lets the leader exit after it has spawned its helper.
        drop(group.0.stdin.take());
        group.0.wait().unwrap();
        let survivors = crate::process::OsProcess::group(leader.pid).unwrap();
        let [helper] = survivors.as_slice() else {
            panic!("expected one helper, found {survivors:?}");
        };
        let error = super::close_agent_process(leader.pid, leader.started_at).unwrap_err();
        assert!(
            error.to_string().contains(&format!("pid {} (", helper.pid)),
            "{error}"
        );
        assert!(!crate::process::OsProcess::group(leader.pid)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn close_settles_a_group_with_a_lingering_zombie() {
        let group = Group(
            Command::new("/bin/sleep")
                .env_clear()
                .arg("60")
                .process_group(0)
                .spawn()
                .unwrap(),
        );
        // A second member this test never waits stays a zombie in the group.
        let mut zombie = Command::new("/usr/bin/true")
            .env_clear()
            .process_group(group.0.id() as i32)
            .spawn()
            .unwrap();
        // SAFETY: waitid inspects only this fixture's child; WNOWAIT leaves
        // the zombie unreaped.
        let result = unsafe {
            let mut info = std::mem::zeroed::<libc::siginfo_t>();
            libc::waitid(
                libc::P_PID,
                zombie.id(),
                &mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        assert_eq!(result, 0, "{}", std::io::Error::last_os_error());
        let leader = crate::process::OsProcess::read(group.0.id())
            .unwrap()
            .unwrap();
        super::close_agent_process(leader.pid, leader.started_at).unwrap();
        assert!(crate::process::OsProcess::read(zombie.id())
            .unwrap()
            .unwrap()
            .has_exited());
        zombie.wait().unwrap();
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
        let leader = crate::process::OsProcess::read(group.0.id())
            .unwrap()
            .unwrap();
        let child = crate::process::OsProcess::read(helper).unwrap().unwrap();
        super::close_agent_process(leader.pid, leader.started_at).unwrap();
        assert!(crate::process::OsProcess::group(leader.pid)
            .unwrap()
            .is_empty());
        assert_eq!(
            crate::process::os_process::process_identity_evidence(helper, child.started_at),
            crate::store::ProcessIdentityEvidence::Dead
        );
    }
}
