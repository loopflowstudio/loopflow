use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::git::{absolute_git_dir, current_branch, intervention_state, rev_parse};
use crate::ops::error::{OpsError, OpsResult};

/// Recorded as the target of a sequencer Loopflow adopted rather than started,
/// where the real sync target is unknowable after the fact.
const RAW_TARGET_REF: &str = "unknown raw sync target";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct GitOperationId(String);

impl GitOperationId {
    fn new() -> Self {
        Self(format!("gitop_{}", Uuid::new_v4()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// The owner of the one sync sequencer a worktree may have in flight.
///
/// The record's flock is the authority; these fields are evidence for whoever
/// reads it during an incident — `id` and `root_pid` name the owner in
/// refusals, and the trace ids link it to the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GitOperationOwner {
    id: GitOperationId,
    root_pid: u32,
    // Released sequencer receipts retain these field names on disk.
    #[serde(rename = "run_id")]
    trace_id: Option<String>,
    #[serde(rename = "process_id")]
    lf_process_id: Option<String>,
    pub(crate) worktree: PathBuf,
    pub(crate) branch: String,
    pub(crate) head: String,
    pub(crate) target_ref: String,
    pub(crate) target_sha: Option<String>,
}

/// The history-rewrite fence, held from the merge through publication, and
/// while an abort or continue runs; fetching and planning happen before it is
/// taken. A sync that stops on conflicts or errors drops it, and so does
/// one handing its conflicts to a resolver; Git's merge state stays for
/// whoever continues, and other writers are admitted meanwhile.
#[derive(Debug)]
pub(crate) struct SyncOperation {
    file: File,
    path: PathBuf,
    owner: GitOperationOwner,
}

impl SyncOperation {
    pub(crate) fn owner(&self) -> &GitOperationOwner {
        &self.owner
    }

    /// Pin the resolved target commit once, after the single fetch. Every later
    /// postcondition is proved against this immutable SHA rather than a moving
    /// remote ref, so a concurrent fetch cannot change the proof midway.
    pub(crate) fn pin_target(&mut self, target_sha: String) -> OpsResult<()> {
        self.owner.target_sha = Some(target_sha);
        write_json(&mut self.file, &self.owner)
    }

    /// Let go of the fence and keep the record, so a resolver can continue
    /// the merge while other writers are admitted.
    pub(crate) fn release(self) -> GitOperationOwner {
        self.owner
    }

    pub(crate) fn complete(self) -> OpsResult<()> {
        remove_locked_record(&self.path)
    }
}

pub(crate) fn begin_sync_operation(worktree: &Path, target_ref: &str) -> OpsResult<SyncOperation> {
    refuse_intervention(worktree)?;

    let worktree = canonical(worktree);
    let path = operation_path(&worktree)?;
    let mut file = create_record(&path)?;
    lock_record(&mut file)?;

    refuse_intervention(&worktree)?;
    if current_branch(&worktree)?.is_none() {
        return Err(OpsError::Message(
            "refusing to sync from detached HEAD".to_string(),
        ));
    }
    let owner = new_owner(&worktree, target_ref)?;
    write_json(&mut file, &owner)?;
    Ok(SyncOperation { file, path, owner })
}

/// Take the fence back after a resolver ran, to verify and publish the sync.
pub(crate) fn reclaim_sync_operation(owner: GitOperationOwner) -> OpsResult<SyncOperation> {
    let path = operation_path(&owner.worktree)?;
    let mut file = create_record(&path)?;
    lock_record(&mut file)?;
    write_json(&mut file, &owner)?;
    Ok(SyncOperation { file, path, owner })
}

pub(crate) fn authorize_sync_control(worktree: &Path, adopt_raw: bool) -> OpsResult<SyncOperation> {
    if intervention_state(worktree)? != Some("merge") {
        return Err(OpsError::Message(
            "no sync is in progress in this worktree".to_string(),
        ));
    }

    let path = operation_path(worktree)?;
    if path.exists() {
        let mut file = open_record(&path)?;
        lock_record(&mut file)?;
        // The lock is free, so the recorded owner is gone. Adopt the operation
        // it left behind, reusing its pinned branch/target when the record is
        // still readable.
        let stored = match read_json::<GitOperationOwner>(&mut file) {
            Ok(owner) => Some(owner),
            Err(_) if adopt_raw => None,
            Err(_) => {
                return Err(OpsError::Message(
                    "the stale sync owner record is unreadable; rerun with --adopt to claim the raw sequencer"
                        .to_string(),
                ));
            }
        };
        return adopt_operation(worktree, path, file, stored);
    }

    if !adopt_raw {
        return Err(OpsError::Message(
            "this sync has no Loopflow owner; rerun with --adopt to continue or abort it"
                .to_string(),
        ));
    }

    // A raw sequencer with no Loopflow record at all: claim a fresh one.
    let mut file = create_record(&path)?;
    lock_record(&mut file)?;
    adopt_operation(worktree, path, file, None)
}

/// Take ownership of a sync whose previous owner released its lock.
///
/// Reuses the stale record's pinned branch, HEAD, and target when one survived;
/// otherwise describes the raw sequencer found in the worktree. Either way the
/// identity fields are replaced, so the old operation id can no longer
/// authorize anything.
fn adopt_operation(
    worktree: &Path,
    path: PathBuf,
    mut file: File,
    stored: Option<GitOperationOwner>,
) -> OpsResult<SyncOperation> {
    let mut owner = match stored {
        Some(owner) => owner,
        None => new_owner(worktree, RAW_TARGET_REF)?,
    };
    owner.id = GitOperationId::new();
    owner.root_pid = std::process::id();
    owner.trace_id = std::env::var(crate::journal::LF_TRACE_ID_ENV).ok();
    owner.lf_process_id = std::env::var(crate::journal::LF_PROCESS_ID_ENV).ok();
    write_json(&mut file, &owner)?;
    Ok(SyncOperation { file, path, owner })
}

fn new_owner(worktree: &Path, target_ref: &str) -> OpsResult<GitOperationOwner> {
    Ok(GitOperationOwner {
        id: GitOperationId::new(),
        root_pid: std::process::id(),
        trace_id: std::env::var(crate::journal::LF_TRACE_ID_ENV).ok(),
        lf_process_id: std::env::var(crate::journal::LF_PROCESS_ID_ENV).ok(),
        worktree: canonical(worktree),
        branch: current_branch(worktree)?.unwrap_or_else(|| "HEAD".to_string()),
        head: rev_parse(worktree, "HEAD")?,
        target_ref: target_ref.to_string(),
        target_sha: None,
    })
}

/// Admit an agent to a worktree unless a sync is mid-merge in it right now.
pub(crate) fn prepare_agent_process(worktree: &Path) -> OpsResult<()> {
    if absolute_git_dir(worktree).is_err() {
        return Ok(());
    }
    fence_agent_process(worktree)
}

fn fence_agent_process(worktree: &Path) -> OpsResult<()> {
    let path = operation_path(worktree)?;
    if !path.exists() {
        return Ok(());
    }

    let mut file = open_record(&path)?;
    lock_record(&mut file)?;
    // An idle merge needs an agent to finish it. Retain its pinned evidence for
    // adoption by sync --continue or --abort; only a live lock excludes an
    // independent agent launch.
    if intervention_state(worktree)?.is_none() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub(crate) fn refuse_intervention(worktree: &Path) -> OpsResult<()> {
    if let Some(state) = intervention_state(worktree)? {
        return Err(OpsError::Message(format!(
            "refusing to sync: a {state} operation already exists; Loopflow did not start or abort it"
        )));
    }
    Ok(())
}

fn operation_path(worktree: &Path) -> OpsResult<PathBuf> {
    // Keep the durable lock location: an older executable may still own it.
    // Renaming the command must not introduce a second Git operation authority.
    Ok(absolute_git_dir(worktree)?
        .join("loopflow")
        .join("rebase-owner.json"))
}

fn open_record(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
}

fn create_record(path: &Path) -> OpsResult<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(open_record(path)?)
}

fn canonical(worktree: &Path) -> PathBuf {
    worktree
        .canonicalize()
        .unwrap_or_else(|_| worktree.to_path_buf())
}

fn write_json<T: Serialize>(file: &mut File, value: &T) -> OpsResult<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    serde_json::to_writer_pretty(&mut *file, value)
        .map_err(|error| OpsError::Parse(error.to_string()))?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(file: &mut File) -> OpsResult<T> {
    file.seek(SeekFrom::Start(0))?;
    let mut value = String::new();
    file.read_to_string(&mut value)?;
    serde_json::from_str(&value).map_err(|error| OpsError::Parse(error.to_string()))
}

fn remove_locked_record(path: &Path) -> OpsResult<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Take the record's flock, or refuse naming the live sync that holds it.
fn lock_record(file: &mut File) -> OpsResult<()> {
    match FileExt::try_lock_exclusive(file) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            let owner = read_json::<GitOperationOwner>(file).ok();
            Err(live_operation_error(owner.as_ref()))
        }
        Err(error) => Err(error.into()),
    }
}

fn live_operation_error(owner: Option<&GitOperationOwner>) -> OpsError {
    let label = owner
        .map(owner_label)
        .unwrap_or_else(|| "a live integration operation".to_string());
    OpsError::Message(format!(
        "refusing to mutate Git: {label} is running in this worktree; retry when it stops"
    ))
}

fn owner_label(owner: &GitOperationOwner) -> String {
    format!(
        "sync {} (pid {}, branch {})",
        owner.id.as_str(),
        owner.root_pid,
        owner.branch
    )
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    use loopflow_test_support::TestRepo;

    use super::{begin_sync_operation, fence_agent_process, operation_path, GitOperationOwner};
    use crate::ops::{
        continue_sync_for_resolution, recover_sync, sync_with_recovery, NullProgress, OpsError,
        SyncOptions,
    };

    fn conflicting_feature() -> TestRepo {
        let fixture = TestRepo::new();
        fixture.create_branch("feature");
        fixture.create_file("README.md", "feature");
        fixture.stage_all();
        fixture.commit("feature");
        fixture.checkout("main");
        fixture.create_file("README.md", "main");
        fixture.stage_all();
        fixture.commit("main");
        fixture.push();
        fixture.checkout("feature");
        fixture
    }

    #[test]
    fn a_running_sync_fences_agents_and_a_stopped_one_admits_them() {
        let fixture = conflicting_feature();
        let repo = fixture.path();

        let operation = begin_sync_operation(repo, "main").unwrap();
        assert!(fence_agent_process(repo).is_err());

        // The sync stops on conflicts and its process lets go of the fence.
        let merge = Command::new("git")
            .args(["merge", "--no-commit", "main"])
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(!merge.status.success());
        drop(operation);
        assert!(fence_agent_process(repo).is_ok());
        assert!(operation_path(repo).unwrap().exists());
    }

    #[test]
    fn a_resolving_sync_admits_other_agents_until_it_publishes() {
        let fixture = conflicting_feature();
        let repo = fixture.path();
        let options = SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        };
        let recovery = match sync_with_recovery(repo, &options, &NullProgress) {
            Err(OpsError::SyncConflict {
                recovery: Some(recovery),
                ..
            }) => *recovery,
            other => panic!("expected an owned conflict, got {other:?}"),
        };

        recover_sync(recovery, || {
            // A wedged resolver would sit here; other writers still get in.
            assert!(fence_agent_process(repo).is_ok());
            fixture.create_file("README.md", "both");
            continue_sync_for_resolution(repo, false)
        })
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(repo.join("README.md")).unwrap(),
            "both"
        );
        assert!(!operation_path(repo).unwrap().exists());
    }

    #[test]
    fn a_stalled_fetch_admits_other_agents() {
        let fixture = conflicting_feature();
        let repo = fixture.path().to_path_buf();
        let gate = tempfile::tempdir().unwrap();
        let started = gate.path().join("started");
        let release = gate.path().join("release");
        let upload_pack = gate.path().join("upload-pack");
        std::fs::write(
            &upload_pack,
            format!(
                "#!/bin/sh\ntouch '{}'\ni=0\nwhile [ ! -e '{}' ] && [ $i -lt 300 ]; do sleep 0.1; i=$((i+1)); done\nexec git upload-pack \"$@\"\n",
                started.display(),
                release.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&upload_pack, std::fs::Permissions::from_mode(0o755)).unwrap();
        let config = Command::new("git")
            .args(["config", "remote.origin.uploadpack"])
            .arg(&upload_pack)
            .current_dir(&repo)
            .status()
            .unwrap();
        assert!(config.success());

        let sync = std::thread::spawn({
            let repo = repo.clone();
            move || {
                let options = SyncOptions {
                    onto: "origin/main".to_string(),
                    push: false,
                    fork_base: None,
                };
                sync_with_recovery(&repo, &options, &NullProgress)
            }
        });
        while !started.exists() {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        assert!(fence_agent_process(&repo).is_ok());
        std::fs::write(&release, "").unwrap();
        assert!(matches!(
            sync.join().unwrap(),
            Err(OpsError::SyncConflict { .. })
        ));
    }

    #[test]
    fn sequencer_receipt_preserves_released_process_reference() {
        let receipt = serde_json::json!({
            "id": "gitop_retained",
            "root_pid": 4242,
            "run_id": "retained-trace",
            "process_id": "retained-process",
            "worktree": "/repo/task",
            "branch": "feature",
            "head": "retained-head",
            "target_ref": "origin/main",
            "target_sha": null,
        });
        let owner: GitOperationOwner = serde_json::from_value(receipt.clone()).unwrap();
        assert_eq!(owner.lf_process_id.as_deref(), Some("retained-process"));
        assert_eq!(serde_json::to_value(owner).unwrap(), receipt);
    }
}
