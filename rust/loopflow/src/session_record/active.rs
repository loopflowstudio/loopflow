//! Stable conversations joined to exact live process evidence. SQL ownership
//! attributes observations; it never establishes liveness or control authority.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

mod events;
mod reader;
pub(crate) use reader::ActiveSessionReader;

use crate::durable::WorkRef;
use crate::lf::commands::top::{
    exact_provider_process, live_process_providers, LiveProviderProcess,
};
use crate::process::SessionProcessOwnership;
use crate::store::SharedStore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveSession {
    pub id: String,
    pub title: String,
    pub work: Option<WorkRef>,
    pub processes: Vec<LiveProviderProcess>,
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

pub async fn snapshot(
    home: &Path,
    store: &SharedStore,
    task: Option<WorkRef>,
) -> ActiveSessionsSnapshot {
    match ActiveSessionReader::start(home, false, tokio_util::sync::CancellationToken::new()) {
        Ok(mut reader) => reader.observe(store, task).await,
        Err(error) => ActiveSessionsSnapshot {
            home: home.to_owned(),
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            task,
            discovery: DiscoveryState::Unavailable,
            sessions: Vec::new(),
            gaps: vec![error.to_string()],
        },
    }
}

fn project(
    ownership: &SessionProcessOwnership,
    processes: &crate::lf::commands::top::ProcessSnapshot,
    clients: &[(String, crate::session_record::ProviderClientRef)],
    snapshot: &mut ActiveSessionsSnapshot,
) {
    let live = live_process_providers(processes, clients);
    snapshot.gaps.extend(live.gaps);
    let mut drivers = BTreeMap::new();
    for session in &ownership.sessions {
        let Some(driver) = &session.driver_process_lfid else {
            continue;
        };
        let receipts = live
            .processes
            .iter()
            .filter(|process| process.receipt.process_lfid == driver.as_str())
            .collect::<Vec<_>>();
        if receipts.len() == 1
            && session.driver_trace_id.as_deref() == Some(receipts[0].receipt.trace_id.as_str())
        {
            drivers.insert(session.id.as_str(), driver.as_str());
        } else {
            snapshot.gaps.push(format!(
                "Session {}: driver {} lacks one exact live Process receipt; liveness is unresolved",
                session.id, driver,
            ));
        }
    }
    let mut attributed: BTreeMap<String, Vec<LiveProviderProcess>> = BTreeMap::new();
    // A receipt names its original immutable input, whose retained link identifies
    // the conversation even after input replacement or driver release.
    let mut native: BTreeMap<u32, (LiveProviderProcess, BTreeSet<String>)> = BTreeMap::new();
    for (input, process) in live.clients {
        let Some(session) = ownership.inputs.get(input.as_str()) else {
            snapshot.gaps.push(format!(
                "Native client {}: input {} has no known Session",
                process.pid, input
            ));
            continue;
        };
        native
            .entry(process.pid)
            .or_insert_with(|| (process, BTreeSet::new()))
            .1
            .insert(session.clone());
    }
    let native_pids = native.keys().copied().collect::<BTreeSet<_>>();
    for (_, (process, sessions)) in native {
        attribute(process, sessions, &mut attributed, &mut snapshot.gaps);
    }
    // Known retained engines cannot be borrowed by the next conversation in a
    // reused Process. A shared engine without exact conversation evidence stays a gap.
    let mut engines = BTreeMap::new();
    for session in &ownership.sessions {
        let (Some(pid), Some(start)) = (session.provider_pid, session.provider_started_at) else {
            continue;
        };
        if native_pids.contains(&pid) {
            continue;
        }
        if let Some(process) = exact_provider_process(processes, pid, start) {
            engines
                .entry(pid)
                .or_insert_with(|| (process, BTreeSet::new()));
            if drivers.contains_key(session.id.as_str()) {
                engines
                    .get_mut(&pid)
                    .expect("engine inserted above")
                    .1
                    .insert(session.id.clone());
            }
        } else if drivers.contains_key(session.id.as_str()) {
            snapshot.gaps.push(format!("Session {}: recorded engine {} has no matching live PID/start evidence; liveness is unresolved", session.id, pid));
        }
    }
    let known_pids = engines.keys().copied().collect::<BTreeSet<_>>();
    for (_, (process, sessions)) in engines {
        // A retained idle engine alone is not an active conversation.
        if !sessions.is_empty() {
            attribute(process, sessions, &mut attributed, &mut snapshot.gaps);
        }
    }
    // Providers without a recorded engine identity use nearest-Process containment.
    // Never assign its entire process set to every conversation sharing that Process.
    for owner in live.processes {
        for process in owner.providers {
            if known_pids.contains(&process.pid) {
                continue;
            }
            // A released conversation with no exact engine identity may still
            // own this process. Its historical origin blocks guessing, not work.
            if ownership.sessions.iter().any(|session| {
                session
                    .provider_process_lfid
                    .as_ref()
                    .is_some_and(|origin| origin.as_str() == owner.receipt.process_lfid)
                    && session.provider_pid.is_none()
                    && !drivers.contains_key(session.id.as_str())
            }) {
                snapshot.gaps.push(format!("Live process {}: an earlier Session in Process {} has no exact engine identity; attribution is unresolved", process.pid, owner.receipt.process_lfid));
                continue;
            }
            let sessions = ownership
                .sessions
                .iter()
                .filter(|session| {
                    drivers.get(session.id.as_str()).is_some_and(|driver|
                        *driver == owner.receipt.process_lfid.as_str())
                        // Exact identity already rejected this Session's old process.
                        && session.provider_pid != Some(process.pid)
                })
                .map(|session| session.id.clone())
                .collect();
            attribute(process, sessions, &mut attributed, &mut snapshot.gaps);
        }
    }
    for session in &ownership.sessions {
        let Some(mut processes) = attributed.remove(&session.id) else {
            continue;
        };
        if snapshot
            .task
            .as_ref()
            .is_some_and(|task| session.work.as_ref() != Some(task))
        {
            continue;
        }
        processes.sort_by_key(|process| process.pid);
        processes.dedup_by_key(|process| process.pid);
        snapshot.sessions.push(ActiveSession {
            id: session.id.clone(),
            title: session.title.clone(),
            work: session.work.clone(),
            processes,
        });
    }
    snapshot.gaps.sort();
    snapshot.gaps.dedup();
}

fn attribute(
    process: LiveProviderProcess,
    sessions: BTreeSet<String>,
    attributed: &mut BTreeMap<String, Vec<LiveProviderProcess>>,
    gaps: &mut Vec<String>,
) {
    if sessions.len() != 1 {
        gaps.push(format!(
            "Live process {} has {} possible current Sessions; attribution is unresolved",
            process.pid,
            sessions.len()
        ));
        return;
    }
    let session = sessions.into_iter().next().expect("one Session identity");
    attributed.entry(session).or_default().push(process);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::{project, ActiveSessionsSnapshot, DiscoveryState};

    use crate::id::ProcessLfid;
    use crate::journal::ProcessReceipt;
    use crate::lf::commands::top::{test_process, ActivityState, ProcessSnapshot};
    use crate::process::{SessionProcessObservation, SessionProcessOwnership};

    #[derive(Debug)]
    struct OwnedClient(std::process::Child);

    impl Drop for OwnedClient {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Isolate capture admission from ambient storage.
    async fn native_client_survives_input_replacement_without_retired_payload() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::store::open_store(&crate::store::StorageConfig::sqlite(
                home.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let capture = crate::session_record::CaptureHandle::begin_at(
            home.path(),
            crate::session_record::SessionCaptureSpec {
                harness: "cat".into(),
                model: None,
                surface: "tui".into(),
                cwd: home.path().to_owned(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                flow: crate::session_record::SessionFlowMembership::Independent,
                work: None,
            },
        )
        .unwrap();
        let id = capture.artifact_key();
        let session_id = store.sqlite.session_for_artifact(&id).unwrap().unwrap().id;
        let (dir, _) = crate::session_record::resolve_manifest(home.path(), id.as_str()).unwrap();
        let client = OwnedClient(
            std::process::Command::new("/bin/cat")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        crate::session_record::write_provider_client(&dir, client.0.id()).unwrap();
        assert!(!home.path().join("run-bindings").exists());
        let snapshot = super::snapshot(home.path(), &store, None).await;
        assert!(snapshot.gaps.is_empty(), "{:?}", snapshot.gaps);
        assert_eq!(
            snapshot
                .sessions
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            [session_id.as_str()]
        );
        let mut replacement = store.sqlite.session_for_artifact(&id).unwrap().unwrap();
        replacement.artifact_key = crate::session_record::new_artifact_key();
        replacement.provider = Some("different-provider".into());
        store
            .sqlite
            .replace_session_input(replacement.captured, replacement)
            .unwrap();
        store
            .sqlite
            .rename_session(
                &session_id,
                "Retained conversation",
                crate::session::TitleSource::Human,
            )
            .unwrap();
        std::fs::remove_file(dir.join("manifest.json")).unwrap();
        std::fs::write(dir.join("events.jsonl"), b"broken retired payload").unwrap();
        let snapshot = super::snapshot(home.path(), &store, None).await;
        assert_eq!(snapshot.sessions[0].id, session_id);
        assert_eq!(snapshot.sessions[0].title, "Retained conversation");
        assert_eq!(snapshot.sessions[0].processes[0].provider, "cat");
        assert!(snapshot.gaps.is_empty(), "{snapshot:?}");
        drop(client);
        let snapshot = super::snapshot(home.path(), &store, None).await;
        assert!(snapshot.sessions.is_empty());
        assert!(snapshot.gaps.is_empty());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Isolate capture admission from ambient storage.
    async fn old_waiting_sessions_are_task_exact_in_one_checkout_and_dead_clients_disappear() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        use crate::durable::WorkRef;
        use crate::session_record::{
            read_manifest, write_provider_client, CaptureHandle, SessionCaptureSpec,
            SubjectAttribution,
        };
        use crate::store::{open_store, StorageConfig};
        use crate::work::task::TaskId;
        use std::process::{Command, Stdio};
        use std::sync::Arc;

        let home = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let task = TaskId::new();
        let other_task = TaskId::new();
        let untouched = TaskId::new();
        // Each live conversation names a registered Task.
        let wave = crate::id::WaveId::new();
        {
            let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
            conn.execute_batch(&format!(
                "INSERT INTO waves(id,name,repo,created_at) VALUES('{wave}','live','/repo',1);
                 INSERT INTO projects(id,wave_id,external_project_id,created_at)
                 VALUES('project','{wave}','project',1);
                 INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,issue_title,issue_description,pm_snapshot_synced_at,worktree,workspace_slug,created_at,updated_at)
                 VALUES('{task}','project','{task}','{task}','Task','',1,'/repo.one','one',1,1),
                       ('{other_task}','project','{other_task}','{other_task}','Other Task','',1,'/repo.two','two',1,1),
                       ('{untouched}','project','{untouched}','{untouched}','Untouched Task','',1,'/repo.three','three',1,1);"
            ))
            .unwrap();
        }
        let mut captures = Vec::new();
        let mut clients = Vec::new();
        for subject in [&task, &other_task] {
            let capture = CaptureHandle::begin_at(
                home.path(),
                SessionCaptureSpec {
                    harness: "cat".into(),
                    model: None,
                    surface: "tui".into(),
                    cwd: home.path().to_owned(),
                    repo: Some(home.path().to_owned()),
                    worktree: None,
                    skill: None,
                    subjects: vec![SubjectAttribution::declared(format!("task:{subject}"))],
                    flow: crate::session_record::SessionFlowMembership::Independent,
                    work: Some(crate::session::SessionWork {
                        task_id: Some(subject.clone()),
                        wave_id: None,
                        source: crate::session::WorkSource::Declared,
                    }),
                },
            )
            .unwrap();
            let dir = capture.artifact_dir();
            let mut manifest = read_manifest(&dir).unwrap();
            manifest.created_at = time::OffsetDateTime::UNIX_EPOCH;
            std::fs::write(
                dir.join("manifest.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            let client = OwnedClient(
                Command::new("/bin/cat")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            write_provider_client(&dir, client.0.id()).unwrap();
            capture.mark_spawn_requested();
            captures.push(capture);
            clients.push(client);
        }
        store.sqlite.assert_no_historical_runs();
        let inputs = captures
            .iter()
            .map(|capture| capture.artifact_key())
            .collect::<Vec<_>>();
        let before = store
            .sqlite
            .session_process_ownership(&inputs, &[], &[])
            .unwrap();
        let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let started: Vec<(String, Option<i64>)> = conn
            .prepare("SELECT id,started_at FROM tasks ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(started
            .iter()
            .any(|(id, at)| id == untouched.as_str() && at.is_none()));
        let snapshot = crate::session_record::active::snapshot(
            home.path(),
            &store,
            Some(WorkRef::Task(task.clone())),
        )
        .await;
        assert!(snapshot.gaps.is_empty(), "{:?}", snapshot.gaps);
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(
            snapshot.sessions[0].id,
            store
                .sqlite
                .session_for_artifact(&captures[0].artifact_key())
                .unwrap()
                .unwrap()
                .id
        );
        assert_eq!(snapshot.sessions[0].work, Some(WorkRef::Task(task)));
        assert_eq!(
            snapshot.sessions[0].processes[0].state,
            ActivityState::Waiting
        );
        let snapshot = crate::session_record::active::snapshot(home.path(), &store, None).await;
        assert_eq!(snapshot.sessions.len(), 2);
        drop(clients);
        // Captures and client receipts remain unfinished, but neither process lives.
        let snapshot = crate::session_record::active::snapshot(home.path(), &store, None).await;
        assert!(snapshot.sessions.is_empty());
        assert!(snapshot.gaps.is_empty());
        std::fs::create_dir_all(home.path().join("run-bindings")).unwrap();
        std::fs::write(home.path().join("run-bindings/broken.json"), b"{").unwrap();
        let unavailable = crate::session_record::active::snapshot(home.path(), &store, None).await;
        assert!(unavailable.sessions.is_empty());
        assert!(
            unavailable.gaps.is_empty(),
            "retired files are not ownership"
        );
        assert_eq!(
            before,
            store
                .sqlite
                .session_process_ownership(&inputs, &[], &[])
                .unwrap()
        );
        let after: Vec<(String, Option<i64>)> = conn
            .prepare("SELECT id,started_at FROM tasks ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(started, after);
    }

    fn session(id: &str, process: &ProcessLfid) -> SessionProcessObservation {
        SessionProcessObservation {
            id: id.into(),
            title: format!("Conversation {id}"),
            work: None,
            driver_process_lfid: Some(process.clone()),
            driver_trace_id: Some("trace".into()),
            driver_generation: 1,
            provider_process_lfid: Some(process.clone()),
            provider_pid: None,
            provider_started_at: None,
        }
    }

    fn receipt(process: &ProcessLfid, pid: u32) -> ProcessReceipt {
        ProcessReceipt {
            schema_version: 1,
            trace_id: "trace".into(),
            process_lfid: process.to_string(),
            pid,
            started_at: 100,
        }
    }

    fn observe(
        owners: Vec<SessionProcessObservation>,
        processes: &ProcessSnapshot,
    ) -> ActiveSessionsSnapshot {
        let ownership = SessionProcessOwnership {
            sessions: owners,
            inputs: BTreeMap::new(),
        };
        let mut result = ActiveSessionsSnapshot {
            discovery: DiscoveryState::Ready,
            home: PathBuf::from("/fixture"),
            observed_at: 100,
            task: None,
            sessions: vec![],
            gaps: vec![],
        };
        project(&ownership, processes, &[], &mut result);
        result
    }

    #[test]
    fn sequential_sessions_do_not_borrow_a_retained_engine() {
        let process = ProcessLfid::new();
        let mut first = session("first", &process);
        first.driver_process_lfid = None;
        first.driver_trace_id = None;
        first.provider_pid = Some(51);
        first.provider_started_at = Some(100);
        let second = session("second", &process);
        let processes = ProcessSnapshot {
            processes: vec![
                test_process(50, 1, 100, "lf"),
                test_process(51, 50, 100, "codex app-server"),
                test_process(52, 50, 100, "claude --print"),
            ],
            receipts: vec![receipt(&process, 50)],
            opencode_servers: vec![],
        };
        let snapshot = observe(vec![first.clone(), second.clone()], &processes);
        assert!(snapshot.gaps.is_empty(), "{snapshot:?}");
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].id, "second");
        assert_eq!(
            snapshot.sessions[0]
                .processes
                .iter()
                .map(|p| p.pid)
                .collect::<Vec<_>>(),
            [52]
        );
        // The old integer PID cannot hide a new provider beneath the current driver.
        let mut old_pid = first.clone();
        old_pid.provider_started_at = Some(1);
        let reused = observe(vec![old_pid, second.clone()], &processes);
        assert_eq!(reused.sessions.len(), 1);
        assert_eq!(reused.sessions[0].id, "second");
        assert_eq!(
            reused.sessions[0]
                .processes
                .iter()
                .map(|p| p.pid)
                .collect::<Vec<_>>(),
            [51, 52]
        );
        // Two unresolved current drivers cannot both borrow the Claude process.
        let mut unknown = first.clone();
        unknown.provider_pid = None;
        unknown.provider_started_at = None;
        let unresolved = observe(vec![unknown, second.clone()], &processes);
        assert!(unresolved.sessions.is_empty());
        assert!(unresolved
            .gaps
            .iter()
            .any(|gap| gap.contains("earlier Session")));
        first.driver_process_lfid = Some(process);
        first.driver_trace_id = Some("trace".into());
        let mut reused_pid = first.clone();
        reused_pid.provider_started_at = Some(1);
        let rejected = observe(vec![reused_pid], &processes);
        assert!(rejected
            .sessions
            .iter()
            .all(|session| session.processes.iter().all(|process| process.pid != 51)));
        assert!(rejected.gaps.iter().any(|gap| gap.contains("PID/start")));
        let ambiguous = observe(vec![first, second], &processes);
        assert_eq!(ambiguous.sessions[0].id, "first"); // its exact engine, not Claude
        assert_eq!(ambiguous.sessions[0].processes[0].pid, 51);
        assert!(ambiguous
            .gaps
            .iter()
            .any(|g| g.contains("52") && g.contains("2 possible")));
    }

    #[test]
    fn child_process_is_distinct_and_pid_or_trace_mismatch_cannot_claim_work() {
        let root = ProcessLfid::new();
        let child = ProcessLfid::new();
        let first = session("root", &root);
        let second = session("child", &child);
        let mut processes = ProcessSnapshot {
            processes: vec![
                test_process(50, 1, 100, "lf"),
                test_process(51, 50, 100, "claude"),
                test_process(60, 50, 100, "lf"),
                test_process(61, 60, 100, "codex app-server"),
            ],
            receipts: vec![receipt(&root, 50), receipt(&child, 60)],
            opencode_servers: vec![],
        };
        let snapshot = observe(vec![first.clone(), second.clone()], &processes);
        assert!(snapshot.gaps.is_empty(), "{snapshot:?}");
        assert_eq!(snapshot.sessions[0].processes[0].pid, 51);
        assert_eq!(snapshot.sessions[1].processes[0].pid, 61);
        processes.receipts[0].started_at = 1;
        processes.receipts[1].trace_id = "different-trace".into();
        let snapshot = observe(vec![first, second], &processes);
        assert!(snapshot.sessions.is_empty(), "{snapshot:?}");
        assert!(!snapshot.gaps.is_empty());
    }

    #[test]
    fn shared_engine_is_not_broadcast_and_native_clients_survive_driver_exit() {
        let process = ProcessLfid::new();
        let mut first = session("first", &process);
        first.provider_pid = Some(51);
        first.provider_started_at = Some(100);
        let mut second = first.clone();
        second.id = "second".into();
        let mut processes = ProcessSnapshot {
            processes: vec![
                test_process(50, 1, 100, "lf"),
                test_process(51, 1, 100, "codex app-server"),
                test_process(70, 1, 100, "codex"),
                test_process(71, 1, 100, "codex"),
            ],
            receipts: vec![receipt(&process, 50)],
            opencode_servers: vec![],
        };
        let snapshot = observe(vec![first.clone(), second.clone()], &processes);
        assert!(snapshot.sessions.is_empty());
        assert!(snapshot.gaps.iter().any(|g| g.contains("2 possible")));
        first.driver_process_lfid = None;
        first.driver_trace_id = None;
        second.driver_process_lfid = None;
        second.driver_trace_id = None;
        processes.receipts.clear();
        processes.processes.remove(0);
        let old = crate::session_record::new_artifact_key();
        let sibling = crate::session_record::new_artifact_key();
        let ownership = SessionProcessOwnership {
            sessions: vec![first, second],
            inputs: BTreeMap::from([
                (old.to_string(), "first".into()),
                (sibling.to_string(), "second".into()),
            ]),
        };
        let clients = [(old, 70), (sibling, 71)].map(|(input, pid)| {
            (
                input,
                crate::session_record::ProviderClientRef {
                    schema_version: 1,
                    pid,
                    terminal_id: None,
                    started_at: time::OffsetDateTime::from_unix_timestamp(100).unwrap(),
                },
            )
        });
        let mut snapshot = observe(vec![], &processes);
        project(&ownership, &processes, &clients, &mut snapshot);
        assert_eq!(snapshot.sessions.len(), 2, "{snapshot:?}");
        assert_eq!(snapshot.sessions[0].processes[0].pid, 70);
        assert_eq!(snapshot.sessions[1].processes[0].pid, 71);
        assert!(snapshot.gaps.is_empty(), "{snapshot:?}");
    }
}
