use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::git::{absolute_git_dir, current_branch, intervention_state, rev_parse};
use crate::ops::error::{OpsError, OpsResult};

pub(crate) const LF_GIT_OPERATION_ID_ENV: &str = "LF_GIT_OPERATION_ID";

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

    fn parse(value: &str) -> OpsResult<Self> {
        validate_id(value, "Git operation")?;
        Ok(Self(value.to_string()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// The owner of the one sync sequencer a worktree may have in flight.
///
/// `id` is the only authority: an exact match on `LF_GIT_OPERATION_ID`
/// authorizes a recovery child to continue or abort. Everything else is
/// evidence for whoever reads this record during an incident — `root_pid`
/// names the owner in refusals, and the trace ids link it to the journal.
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

    pub(crate) fn scoped_env(&self) -> BTreeMap<String, String> {
        BTreeMap::from([(
            LF_GIT_OPERATION_ID_ENV.to_string(),
            self.owner.id.as_str().to_string(),
        )])
    }

    pub(crate) fn complete(self) -> OpsResult<()> {
        remove_locked_record(&self.path)
    }
}

#[derive(Debug)]
pub(crate) enum OperationAuthorization {
    Borrowed(GitOperationOwner),
    Adopted(SyncOperation),
}

impl OperationAuthorization {
    pub(crate) fn owner(&self) -> &GitOperationOwner {
        match self {
            Self::Borrowed(owner) => owner,
            Self::Adopted(operation) => operation.owner(),
        }
    }

    /// Release ownership after a verified completion or an authorized abort.
    ///
    /// A borrowed operation keeps its record: the waiting parent still holds
    /// that lock, so its visible path must survive until the parent observes
    /// the outcome and releases ownership itself. Unlinking a locked file would
    /// let a foreign operation create a new inode and enter the worktree during
    /// the handoff.
    pub(crate) fn complete(self) -> OpsResult<()> {
        match self {
            Self::Borrowed(_) => Ok(()),
            Self::Adopted(operation) => operation.complete(),
        }
    }
}

pub(crate) fn begin_sync_operation(worktree: &Path, target_ref: &str) -> OpsResult<SyncOperation> {
    refuse_intervention(worktree)?;

    let worktree = canonical(worktree);
    let path = operation_path(&worktree)?;
    let mut file = create_record(&path)?;
    match FileExt::try_lock_exclusive(&file) {
        Ok(()) => {}
        Err(error) if is_contended(&error) => {
            let owner = read_json::<GitOperationOwner>(&mut file).ok();
            return Err(live_operation_error(owner.as_ref()));
        }
        Err(error) => return Err(error.into()),
    }

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

pub(crate) fn authorize_sync_control(
    worktree: &Path,
    adopt_raw: bool,
) -> OpsResult<OperationAuthorization> {
    if intervention_state(worktree)? != Some("merge") {
        return Err(OpsError::Message(
            "no sync is in progress in this worktree".to_string(),
        ));
    }

    let requested = std::env::var(LF_GIT_OPERATION_ID_ENV)
        .ok()
        .map(|value| GitOperationId::parse(&value))
        .transpose()?;
    let path = operation_path(worktree)?;
    if path.exists() {
        let mut file = open_record(&path)?;
        match FileExt::try_lock_exclusive(&file) {
            Err(error) if is_contended(&error) => {
                let owner = read_json::<GitOperationOwner>(&mut file).map_err(|_| {
                    OpsError::Message(
                        "a live sync operation is starting; retry after its owner reports state"
                            .to_string(),
                    )
                })?;
                if requested.as_ref() != Some(&owner.id) {
                    return Err(live_operation_error(Some(&owner)));
                }
                return Ok(OperationAuthorization::Borrowed(owner));
            }
            Err(error) => return Err(error.into()),
            Ok(()) => {
                // The lock is free, so the recorded owner is gone. Adopt the
                // operation it left behind, reusing its pinned branch/target
                // when the record is still readable.
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
        }
    }

    if !adopt_raw {
        return Err(OpsError::Message(
            "this sync has no Loopflow owner; rerun with --adopt to continue or abort it"
                .to_string(),
        ));
    }

    // A raw sequencer with no Loopflow record at all: claim a fresh one.
    let path = operation_path(worktree)?;
    let file = create_record(&path)?;
    FileExt::try_lock_exclusive(&file)?;
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
) -> OpsResult<OperationAuthorization> {
    let mut owner = match stored {
        Some(owner) => owner,
        None => new_owner(worktree, RAW_TARGET_REF)?,
    };
    owner.id = GitOperationId::new();
    owner.root_pid = std::process::id();
    owner.trace_id = std::env::var(crate::journal::LF_TRACE_ID_ENV).ok();
    owner.lf_process_id = std::env::var(crate::journal::LF_PROCESS_ID_ENV).ok();
    write_json(&mut file, &owner)?;
    Ok(OperationAuthorization::Adopted(SyncOperation {
        file,
        path,
        owner,
    }))
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

pub(crate) fn prepare_agent_process(
    worktree: &Path,
    env: &BTreeMap<String, String>,
) -> OpsResult<()> {
    if absolute_git_dir(worktree).is_err() {
        return Ok(());
    }

    let requested_operation = env
        .get(LF_GIT_OPERATION_ID_ENV)
        .cloned()
        .or_else(|| std::env::var(LF_GIT_OPERATION_ID_ENV).ok())
        .map(|value| GitOperationId::parse(&value))
        .transpose()?;
    fence_agent_process(worktree, requested_operation.as_ref())
}

fn fence_agent_process(
    worktree: &Path,
    requested_operation: Option<&GitOperationId>,
) -> OpsResult<()> {
    let path = operation_path(worktree)?;
    if !path.exists() {
        return Ok(());
    }

    let mut file = open_record(&path)?;
    match FileExt::try_lock_exclusive(&file) {
        Err(error) if is_contended(&error) => {
            let owner = read_json::<GitOperationOwner>(&mut file).ok();
            if owner.as_ref().map(|owner| &owner.id) == requested_operation {
                return Ok(());
            }
            Err(live_operation_error(owner.as_ref()))
        }
        Err(error) => Err(error.into()),
        Ok(()) => {
            // An idle merge needs an agent to finish it. Retain its pinned
            // evidence for adoption by sync --continue or --abort; only a live
            // lock excludes an independent agent launch.
            if intervention_state(worktree)?.is_none() {
                fs::remove_file(path)?;
            }
            Ok(())
        }
    }
}

fn refuse_intervention(worktree: &Path) -> OpsResult<()> {
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

fn is_contended(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::WouldBlock
}

fn live_operation_error(owner: Option<&GitOperationOwner>) -> OpsError {
    let label = owner
        .map(owner_label)
        .unwrap_or_else(|| "a live integration operation".to_string());
    OpsError::Message(format!(
        "refusing to mutate Git: {label} owns this worktree; only its authorized recovery child may continue or abort"
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

fn validate_id(value: &str, label: &str) -> OpsResult<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(OpsError::Message(format!("invalid {label} id")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::GitOperationOwner;

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
