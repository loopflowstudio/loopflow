//! Current capture intervals joined to verified, Home-local provider ownership.
//! An interval establishes attribution, never liveness by itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod events;
mod reader;
pub(crate) use reader::ActiveRunReader;

use crate::durable::{RunId, TaskWorkerOwner, WorkRef};
use crate::lf::commands::top::{live_exec_providers, LiveExecProviders, LiveProviderProcess};
use crate::run_record::{
    read_manifest, record_dir, write_private_exclusive, RunManifest, SubjectAttribution,
};
use crate::store::SharedStore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RunBinding {
    run_id: RunId,
    owner: Option<TaskWorkerOwner>,
}

/// Its lifetime is one capture, not the containing Exec.
#[derive(Debug)]
pub(crate) struct RunBindingGuard {
    path: PathBuf,
}

impl RunBindingGuard {
    pub(crate) fn publish(dir: &Path) -> std::io::Result<Self> {
        let owner = crate::journal::current_process_identity();
        let manifest = read_manifest(dir)?;
        let home = dir
            .ancestors()
            .nth(3)
            .ok_or_else(|| std::io::Error::other("Run directory has no Home"))?;
        Self::publish_at(
            home,
            RunBinding {
                run_id: manifest.run_id,
                owner,
            },
        )
    }

    fn publish_at(home: &Path, binding: RunBinding) -> std::io::Result<Self> {
        let root = home.join("run-bindings");
        fs::create_dir_all(&root)?;
        let id = Uuid::new_v4();
        let path = root.join(format!("{id}.json"));
        let staging = root.join(format!(".{id}.staging"));
        write_private_exclusive(&staging, &serde_json::to_vec(&binding)?)?;
        fs::rename(staging, &path)?;
        Ok(Self { path })
    }
}

impl Drop for RunBindingGuard {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path) {
            tracing::warn!(%error, path = %self.path.display(), "failed to clear Run capture interval");
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveRun {
    pub id: RunId,
    pub work: Option<WorkRef>,
    pub subjects: Vec<SubjectAttribution>,
    pub label: String,
    pub harness: String,
    pub model: Option<String>,
    pub repo: Option<PathBuf>,
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
pub struct ActiveRunsSnapshot {
    pub discovery: DiscoveryState,
    pub home: PathBuf,
    pub observed_at: i64,
    pub task: Option<WorkRef>,
    pub runs: Vec<ActiveRun>,
    pub gaps: Vec<String>,
}

#[cfg(test)]
fn read_bindings(home: &Path) -> std::io::Result<BTreeMap<PathBuf, RunBinding>> {
    let entries = match fs::read_dir(home.join("run-bindings")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(error),
    };
    let mut bindings = BTreeMap::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        bindings.insert(path, serde_json::from_slice(&bytes)?);
    }
    Ok(bindings)
}

fn join_bindings(
    bindings: &BTreeMap<PathBuf, RunBinding>,
    execs: &[LiveExecProviders],
    gaps: &mut Vec<String>,
) -> BTreeMap<String, Vec<LiveProviderProcess>> {
    let mut runs: BTreeMap<String, Vec<LiveProviderProcess>> = BTreeMap::new();
    for exec in execs.iter().filter(|exec| !exec.providers.is_empty()) {
        let ids = bindings
            .values()
            .filter(|binding| {
                let Some(owner) = &binding.owner else {
                    return false;
                };
                owner.exec_id.as_str() == exec.receipt.exec_id
                    && owner.trace_id.as_str() == exec.receipt.trace_id
                    && owner.pid == exec.receipt.pid
                    && owner.started_at == exec.receipt.started_at
            })
            .map(|binding| binding.run_id.to_string())
            .collect::<BTreeSet<_>>();
        if ids.len() != 1 {
            gaps.push(format!(
                "Exec {} has live providers but {} current Run identities",
                exec.receipt.exec_id,
                ids.len()
            ));
            continue;
        }
        let id = ids.into_iter().next().expect("exactly one Run identity");
        runs.entry(id)
            .or_default()
            .extend(exec.providers.iter().cloned());
    }
    for processes in runs.values_mut() {
        processes.sort_by_key(|process| process.pid);
        processes.dedup_by_key(|process| process.pid);
    }
    runs
}

pub async fn snapshot(
    home: &Path,
    store: &SharedStore,
    task: Option<WorkRef>,
) -> ActiveRunsSnapshot {
    match ActiveRunReader::start(home, false, tokio_util::sync::CancellationToken::new()) {
        Ok(mut reader) => reader.observe(store, task).await,
        Err(error) => ActiveRunsSnapshot {
            home: home.to_owned(),
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            task,
            discovery: DiscoveryState::Unavailable,
            runs: Vec::new(),
            gaps: vec![error.to_string()],
        },
    }
}

async fn project(
    home: &Path,
    store: &SharedStore,
    bindings: &BTreeMap<PathBuf, RunBinding>,
    processes: &crate::lf::commands::top::ProcessSnapshot,
    clients: &[(RunId, crate::run_record::ProviderClientRef, String)],
    snapshot: &mut ActiveRunsSnapshot,
    cost: &mut reader::DiscoveryCost,
) {
    let owners = bindings
        .values()
        .filter_map(|binding| binding.owner.clone())
        .collect::<Vec<_>>();
    let execs = live_exec_providers(processes, &owners, clients);
    let unowned = bindings
        .values()
        .filter(|binding| {
            binding.owner.is_none() && !clients.iter().any(|(id, _, _)| id == &binding.run_id)
        })
        .map(|binding| binding.run_id.to_string())
        .collect::<BTreeSet<_>>();
    for id in unowned {
        snapshot.gaps.push(format!(
            "Run {id}: capture has no Exec or native-client ownership evidence"
        ));
    }
    let mut runs = join_bindings(bindings, &execs.execs, &mut snapshot.gaps);
    snapshot.gaps.extend(execs.gaps);
    for (id, process) in execs.clients {
        runs.entry(id.to_string()).or_default().push(process);
    }
    for (id, mut processes) in runs {
        processes.sort_by_key(|process| process.pid);
        processes.dedup_by_key(|process| process.pid);
        let manifest = RunId::parse(&id)
            .ok()
            .and_then(|id| record_dir(home, &id))
            .ok_or_else(|| std::io::Error::other("invalid Run ID"))
            .and_then(|dir| cost.manifest(&dir));
        let manifest = match manifest {
            Ok(manifest) => manifest,
            Err(error) => {
                snapshot.gaps.push(format!("Run {id}: {error}"));
                continue;
            }
        };
        let work = crate::run_record::attributed_work(store, &manifest).await;
        if work.is_none() && !manifest.subjects.is_empty() {
            snapshot
                .gaps
                .push(format!("Run {id}: Work attribution unavailable"));
        }
        if snapshot
            .task
            .as_ref()
            .is_some_and(|task| work.as_ref() != Some(task))
        {
            continue;
        }
        snapshot.runs.push(active_run(manifest, work, processes));
    }
}

fn active_run(
    manifest: RunManifest,
    work: Option<WorkRef>,
    processes: Vec<LiveProviderProcess>,
) -> ActiveRun {
    ActiveRun {
        id: manifest.run_id,
        work,
        subjects: manifest.subjects,
        label: manifest.skill.unwrap_or_else(|| manifest.harness.clone()),
        harness: manifest.harness,
        model: manifest.model,
        repo: manifest.repo,
        processes,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use crate::durable::{RunId, TaskWorkerOwner};
    use crate::id::{ExecId, TraceId};
    use crate::journal::ExecProcessReceipt;
    use crate::lf::commands::top::{ActivityState, LiveExecProviders, LiveProviderProcess};
    use crate::run_record::active::{join_bindings, read_bindings, RunBinding, RunBindingGuard};

    #[derive(Debug)]
    struct OwnedClient(std::process::Child);

    impl Drop for OwnedClient {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[tokio::test]
    async fn native_client_receipts_do_not_require_a_new_capture_binding() {
        let home = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::store::open_store(&crate::store::StorageConfig::sqlite(
                home.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let id = crate::run_record::CaptureHandle::prepare_at(
            home.path(),
            crate::run_record::RunSpec {
                harness: "cat".into(),
                model: None,
                surface: "tui".into(),
                cwd: home.path().to_owned(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
            },
            None,
        )
        .unwrap();
        let (dir, _) = crate::run_record::resolve_manifest(home.path(), id.as_str()).unwrap();
        let client = OwnedClient(
            std::process::Command::new("/bin/cat")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        crate::run_record::write_provider_client(&dir, client.0.id()).unwrap();
        assert!(!home.path().join("run-bindings").exists());
        let snapshot = super::snapshot(home.path(), &store, None).await;
        assert!(snapshot.gaps.is_empty(), "{:?}", snapshot.gaps);
        assert_eq!(
            snapshot.runs.iter().map(|run| &run.id).collect::<Vec<_>>(),
            [&id]
        );
        drop(client);
        let snapshot = super::snapshot(home.path(), &store, None).await;
        assert!(snapshot.runs.is_empty());
        assert!(snapshot.gaps.is_empty());
    }

    #[tokio::test]
    async fn old_waiting_runs_are_task_exact_in_one_checkout_and_dead_clients_disappear() {
        use crate::durable::WorkRef;
        use crate::run_record::{
            read_manifest, write_provider_client, CaptureHandle, RunSpec, SubjectAttribution,
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
        let mut captures = Vec::new();
        let mut clients = Vec::new();
        for subject in [&task, &other_task] {
            let capture = CaptureHandle::begin_at(
                home.path(),
                RunSpec {
                    harness: "cat".into(),
                    model: None,
                    surface: "tui".into(),
                    cwd: home.path().to_owned(),
                    repo: Some(home.path().to_owned()),
                    worktree: None,
                    skill: None,
                    subjects: vec![SubjectAttribution::declared(format!("task:{subject}"))],
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
        let snapshot = crate::run_record::active::snapshot(
            home.path(),
            &store,
            Some(WorkRef::Task(task.clone())),
        )
        .await;
        assert!(snapshot.gaps.is_empty(), "{:?}", snapshot.gaps);
        assert_eq!(snapshot.runs.len(), 1);
        assert_eq!(snapshot.runs[0].id, captures[0].run_id());
        assert_eq!(snapshot.runs[0].work, Some(WorkRef::Task(task)));
        assert_eq!(snapshot.runs[0].processes[0].state, ActivityState::Waiting);
        let snapshot = crate::run_record::active::snapshot(home.path(), &store, None).await;
        assert_eq!(snapshot.runs.len(), 2);
        drop(clients);
        // Captures and client receipts remain unfinished, but neither process lives.
        let snapshot = crate::run_record::active::snapshot(home.path(), &store, None).await;
        assert!(snapshot.runs.is_empty());
        assert!(snapshot.gaps.is_empty());
        std::fs::write(home.path().join("run-bindings/broken.json"), b"{").unwrap();
        let unavailable = crate::run_record::active::snapshot(home.path(), &store, None).await;
        assert!(unavailable.runs.is_empty());
        assert!(!unavailable.gaps.is_empty());
    }

    fn owner(pid: u32) -> TaskWorkerOwner {
        TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid,
            started_at: 100,
        }
    }

    fn exec(owner: &TaskWorkerOwner, pids: &[u32]) -> LiveExecProviders {
        LiveExecProviders {
            receipt: ExecProcessReceipt {
                schema_version: 1,
                trace_id: owner.trace_id.to_string(),
                exec_id: owner.exec_id.to_string(),
                pid: owner.pid,
                started_at: owner.started_at,
            },
            providers: pids
                .iter()
                .map(|pid| LiveProviderProcess {
                    pid: *pid,
                    provider: "claude".into(),
                    state: ActivityState::Waiting,
                })
                .collect(),
        }
    }

    #[test]
    fn capture_intervals_do_not_keep_prior_runs_alive_in_a_reused_exec() {
        let home = tempfile::tempdir().unwrap();
        let owner = owner(50);
        let first = RunId::new();
        let second = RunId::new();
        let first_guard = RunBindingGuard::publish_at(
            home.path(),
            RunBinding {
                run_id: first.clone(),
                owner: Some(owner.clone()),
            },
        )
        .unwrap();
        let mut gaps = Vec::new();
        assert!(join_bindings(
            &read_bindings(home.path()).unwrap(),
            &[exec(&owner, &[51])],
            &mut gaps
        )
        .contains_key(first.as_str()));
        drop(first_guard);
        let _second_guard = RunBindingGuard::publish_at(
            home.path(),
            RunBinding {
                run_id: second.clone(),
                owner: Some(owner.clone()),
            },
        )
        .unwrap();
        let runs = join_bindings(
            &read_bindings(home.path()).unwrap(),
            &[exec(&owner, &[51])],
            &mut gaps,
        );
        assert_eq!(runs.keys().collect::<Vec<_>>(), vec![second.as_str()]);
        assert!(gaps.is_empty());
    }

    #[test]
    fn exact_owners_deduplicate_one_run_and_keep_other_runs_separate() {
        let first = owner(50);
        let second = owner(60);
        let third = owner(70);
        let run = RunId::new();
        let other = RunId::new();
        let bindings = BTreeMap::from([
            (
                PathBuf::from("capture"),
                RunBinding {
                    run_id: run.clone(),
                    owner: Some(first.clone()),
                },
            ),
            (
                PathBuf::from("resume"),
                RunBinding {
                    run_id: run.clone(),
                    owner: Some(second.clone()),
                },
            ),
            (
                PathBuf::from("other-task"),
                RunBinding {
                    run_id: other.clone(),
                    owner: Some(third.clone()),
                },
            ),
        ]);
        let mut gaps = Vec::new();
        let runs = join_bindings(
            &bindings,
            &[
                exec(&first, &[51, 52]),
                exec(&second, &[61]),
                exec(&third, &[71]),
            ],
            &mut gaps,
        );
        assert_eq!(runs.len(), 2);
        assert_eq!(
            runs[run.as_str()]
                .iter()
                .map(|process| process.pid)
                .collect::<Vec<_>>(),
            [51, 52, 61]
        );
        assert_eq!(runs[other.as_str()][0].pid, 71);
        assert!(gaps.is_empty());
    }

    #[test]
    fn dead_missing_reused_and_ambiguous_owners_never_claim_a_live_run() {
        let owner = owner(50);
        let run = RunId::new();
        let mut bindings = BTreeMap::from([(
            PathBuf::from("first"),
            RunBinding {
                run_id: run,
                owner: Some(owner.clone()),
            },
        )]);
        let mut gaps = Vec::new();
        assert!(join_bindings(&bindings, &[], &mut gaps).is_empty());
        assert!(gaps.is_empty());
        // A live containment without a provider is not an active Run.
        assert!(join_bindings(&bindings, &[exec(&owner, &[])], &mut gaps).is_empty());
        let mut reused = owner.clone();
        reused.started_at += 100;
        assert!(join_bindings(&bindings, &[exec(&reused, &[51])], &mut gaps).is_empty());
        assert_eq!(gaps.len(), 1);
        bindings.insert(
            PathBuf::from("second"),
            RunBinding {
                run_id: RunId::new(),
                owner: Some(owner.clone()),
            },
        );
        assert!(join_bindings(&bindings, &[exec(&owner, &[51])], &mut gaps).is_empty());
        assert_eq!(gaps.len(), 2);
    }
}
