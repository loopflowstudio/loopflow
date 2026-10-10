//! Stable conversations joined to exact live process evidence. SQL ownership
//! attributes observations; it never establishes liveness or control authority.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::durable::WorkRef;
use crate::lf::commands::top::AgentProcessObservation;
use crate::store::sqlite::SqliteStore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveSession {
    pub id: String,
    pub title: String,
    pub work: Option<WorkRef>,
    pub processes: Vec<AgentProcessObservation>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DiscoveryState {
    Scanning,
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveSessionsSnapshot {
    pub discovery: DiscoveryState,
    pub home: PathBuf,
    pub observed_at: i64,
    pub task: Option<WorkRef>,
    pub sessions: Vec<ActiveSession>,
    pub gaps: Vec<String>,
}

/// One read of recorded agents and current OS evidence; no discovery cache.
pub fn snapshot(home: &Path, store: &SqliteStore, task: Option<WorkRef>) -> ActiveSessionsSnapshot {
    let mut result = ActiveSessionsSnapshot {
        home: home.to_owned(),
        observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        task,
        discovery: DiscoveryState::Ready,
        sessions: Vec::new(),
        gaps: Vec::new(),
    };
    if let Err(error) = observe(store, &mut result) {
        result.discovery = DiscoveryState::Unavailable;
        result.sessions.clear();
        result.gaps.push(error.to_string());
    }
    result
}

fn observe(store: &SqliteStore, result: &mut ActiveSessionsSnapshot) -> anyhow::Result<()> {
    let agents = store.agent_processes()?;
    let os = crate::journal::OsProcess::sample(result.observed_at);
    if let Err(error) = &os {
        result.discovery = DiscoveryState::Unavailable;
        result.gaps.push(error.to_string());
    }
    let mut sessions = std::collections::BTreeMap::<String, ActiveSession>::new();
    for agent in &agents {
        let Some(process) =
            crate::lf::commands::top::observe_agent_process(os.as_deref().ok(), agent)
        else {
            continue;
        };
        let Some(id) = agent.process.agent_session_id.as_deref() else {
            continue;
        };
        if let Some(active) = sessions.get_mut(id) {
            active.processes.push(process);
            continue;
        }
        let Some(session) = store.session(id)? else {
            continue;
        };
        let work = session
            .task_id
            .map(WorkRef::Task)
            .or_else(|| session.wave_id.map(WorkRef::Wave));
        if result
            .task
            .as_ref()
            .is_some_and(|task| Some(task) != work.as_ref())
        {
            continue;
        }
        sessions.insert(
            id.into(),
            ActiveSession {
                id: id.into(),
                title: session.title,
                work,
                processes: vec![process],
            },
        );
    }
    if agents != store.agent_processes()? {
        result.discovery = DiscoveryState::Scanning;
        result
            .gaps
            .push("AgentProcess inventory changed during observation; reading again".into());
    } else {
        result.sessions = sessions.into_values().collect();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{snapshot, DiscoveryState};
    use crate::durable::{TaskId, WorkRef};
    use crate::id::ProcessLfid;
    use crate::session::TitleSource;
    use crate::session_record::new_artifact_key;
    use crate::store::sqlite::SqliteStore;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Child, Command, Stdio};

    struct Agent(Child);

    impl Agent {
        fn start() -> Self {
            Self(
                Command::new("/bin/cat")
                    .env_clear()
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            )
        }
    }

    impl Drop for Agent {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn snapshots_follow_detached_and_replaced_records_without_capture_files() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _path = crate::test_ambient::EnvGuard::clear(&["PATH"]);
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let mut session = store.test_session("conversation", &new_artifact_key());
        session.provider = Some("codex".into());
        session.artifact_key = new_artifact_key();
        let session = store
            .replace_session_input(session.captured, session)
            .unwrap();
        let parent = ProcessLfid::new();
        let first = store
            .claim_session_attachment(&session.id, None, &parent, true)
            .unwrap();
        let old = Agent::start();
        let pid = old.0.id();
        let start = crate::journal::process_started_at(pid).unwrap().unwrap();
        store
            .record_agent_process_identity(&session.id, &first, pid, start)
            .unwrap();
        let released = store
            .release_session_attachment(&session.id, &first)
            .unwrap();
        let read = || snapshot(home.path(), &store, None);
        assert_eq!(
            read().sessions[0].processes[0].lfid,
            first.agent_process_lfid
        );

        // Next-launch settings and current capture do not reassign the old process.
        let mut next = session.clone();
        next.provider = Some("claude".into());
        next.artifact_key = new_artifact_key();
        store.replace_session_input(session.captured, next).unwrap();
        store
            .rename_session(&session.id, "Renamed", TitleSource::Human)
            .unwrap();
        let second = store
            .claim_session_attachment(&session.id, Some(&released), &parent, true)
            .unwrap();
        let current = Agent::start();
        let pid = current.0.id();
        let start = crate::journal::process_started_at(pid).unwrap().unwrap();
        store
            .record_agent_process_identity(&session.id, &second, pid, start)
            .unwrap();
        let observed = read();
        assert_eq!(observed.discovery, DiscoveryState::Ready);
        assert!(observed.gaps.is_empty());
        assert_eq!(observed.sessions.len(), 1);
        assert_eq!(observed.sessions[0].title, "Renamed");
        assert_eq!(observed.sessions[0].processes.len(), 2);
        assert!(observed.sessions[0]
            .processes
            .iter()
            .any(|p| p.lfid == first.agent_process_lfid && p.provider == "codex"));
        assert!(observed.sessions[0]
            .processes
            .iter()
            .any(|p| p.lfid == second.agent_process_lfid && p.provider == "claude"));
        assert!(
            snapshot(home.path(), &store, Some(WorkRef::Task(TaskId::new())))
                .sessions
                .is_empty()
        );
        assert!(!home.path().join("runs").exists());
        // Sampling failure keeps both records visible as unknown, rather than
        // turning an unavailable observation into an empty Session list.
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let ps = bin.join("ps");
        std::fs::write(&ps, "#!/bin/sh\nexit 2\n").unwrap();
        std::fs::set_permissions(&ps, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::var_os("PATH");
        std::env::set_var("PATH", &bin);
        let unavailable = read();
        match path {
            Some(path) => std::env::set_var("PATH", path),
            None => std::env::remove_var("PATH"),
        }
        assert_eq!(unavailable.discovery, DiscoveryState::Unavailable);
        assert!(!unavailable.gaps.is_empty());
        assert_eq!(unavailable.sessions[0].processes.len(), 2);
        assert!(unavailable.sessions[0]
            .processes
            .iter()
            .all(|process| process.state == crate::lf::commands::top::ActivityState::Unknown));
        drop(old);
        assert_eq!(read().sessions[0].processes.len(), 1);
        assert_eq!(
            read().sessions[0].processes[0].lfid,
            second.agent_process_lfid
        );
        drop(current);
        assert!(read().sessions.is_empty());
        // Observation never claims an attachment or erases unfinished history.
        assert_eq!(store.session_attachment(&session.id).unwrap(), Some(second));
        assert_eq!(store.agent_processes().unwrap().len(), 2);
    }
}
