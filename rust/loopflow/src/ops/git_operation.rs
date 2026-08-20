use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::git::{absolute_git_dir, current_branch, intervention_state, rev_parse};
use crate::ops::error::{OpsError, OpsResult};

pub(crate) const LF_WORKTREE_WRITER_ID_ENV: &str = "LF_WORKTREE_WRITER_ID";
pub(crate) const LF_GIT_OPERATION_ID_ENV: &str = "LF_GIT_OPERATION_ID";

/// Recorded as the target of a sequencer Loopflow adopted rather than started,
/// where the real rebase target is unknowable after the fact.
const RAW_TARGET_REF: &str = "unknown raw rebase target";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct WorktreeWriterId(String);

impl WorktreeWriterId {
    fn new() -> Self {
        Self(format!("writer_{}", Uuid::new_v4()))
    }

    fn parse(value: &str) -> OpsResult<Self> {
        validate_id(value, "worktree writer")?;
        Ok(Self(value.to_string()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

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

/// The owner of the one rebase sequencer a worktree may have in flight.
///
/// `id` is the only authority: an exact match on `LF_GIT_OPERATION_ID`
/// authorizes a recovery child to continue or abort. Everything else is
/// evidence for a human reading this record during an incident — `root_pid`
/// names the owner in refusals, and the trace ids link it to the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GitOperationOwner {
    id: GitOperationId,
    writer_id: WorktreeWriterId,
    root_pid: u32,
    run_id: Option<String>,
    process_id: Option<String>,
    pub(crate) worktree: PathBuf,
    pub(crate) branch: String,
    pub(crate) head: String,
    pub(crate) target_ref: String,
    pub(crate) target_sha: Option<String>,
}

impl GitOperationOwner {
    pub(crate) fn id(&self) -> &str {
        self.id.as_str()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct WorktreeWriterOwner {
    id: WorktreeWriterId,
    pid: u32,
    run_id: Option<String>,
    process_id: Option<String>,
    #[serde(default)]
    invocation_id: Option<String>,
    worktree: PathBuf,
}

#[derive(Debug)]
struct WorktreeWriterLease {
    path: PathBuf,
    _file: File,
}

impl Drop for WorktreeWriterLease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Debug)]
pub(crate) struct AgentWriterGuard {
    writer_id: WorktreeWriterId,
    _lease: Option<WorktreeWriterLease>,
}

impl AgentWriterGuard {
    pub(crate) fn writer_id(&self) -> &str {
        self.writer_id.as_str()
    }
}

#[derive(Debug)]
pub(crate) struct RebaseOperation {
    file: File,
    path: PathBuf,
    owner: GitOperationOwner,
    _writer_lease: Option<WorktreeWriterLease>,
}

impl RebaseOperation {
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
        BTreeMap::from([
            (
                LF_WORKTREE_WRITER_ID_ENV.to_string(),
                self.owner.writer_id.as_str().to_string(),
            ),
            (
                LF_GIT_OPERATION_ID_ENV.to_string(),
                self.owner.id.as_str().to_string(),
            ),
        ])
    }

    pub(crate) fn complete(self) -> OpsResult<()> {
        remove_locked_record(&self.path)
    }
}

#[derive(Debug)]
pub(crate) enum OperationAuthorization {
    Borrowed(GitOperationOwner),
    Adopted(RebaseOperation),
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

pub(crate) fn begin_rebase_operation(
    worktree: &Path,
    target_ref: &str,
) -> OpsResult<RebaseOperation> {
    refuse_intervention(worktree)?;

    let worktree = canonical(worktree);
    let (writer_id, writer_lease) = establish_writer(&worktree, inherited_writer_id()?)?;
    refuse_other_writers(&worktree, &writer_id)?;

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
            "refusing to rebase from detached HEAD".to_string(),
        ));
    }
    let owner = new_owner(&worktree, writer_id, target_ref)?;
    write_json(&mut file, &owner)?;
    Ok(RebaseOperation {
        file,
        path,
        owner,
        _writer_lease: writer_lease,
    })
}

pub(crate) fn authorize_rebase_control(
    worktree: &Path,
    adopt_raw: bool,
) -> OpsResult<OperationAuthorization> {
    if intervention_state(worktree)? != Some("rebase") {
        return Err(OpsError::Message(
            "no rebase is in progress in this worktree".to_string(),
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
                        "a live rebase operation is starting; retry after its owner reports state"
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
                            "the stale rebase owner record is unreadable; rerun with --adopt to claim the raw sequencer"
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
            "this rebase has no Loopflow owner; rerun with --adopt to continue or abort it"
                .to_string(),
        ));
    }

    // A raw sequencer with no Loopflow record at all: claim a fresh one.
    let path = operation_path(worktree)?;
    let file = create_record(&path)?;
    FileExt::try_lock_exclusive(&file)?;
    adopt_operation(worktree, path, file, None)
}

/// Take ownership of a rebase whose previous owner released its lock.
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
    let (writer_id, writer_lease) = establish_writer(worktree, inherited_writer_id()?)?;
    refuse_other_writers(worktree, &writer_id)?;

    let mut owner = match stored {
        Some(owner) => owner,
        None => new_owner(worktree, writer_id.clone(), RAW_TARGET_REF)?,
    };
    owner.id = GitOperationId::new();
    owner.writer_id = writer_id;
    owner.root_pid = std::process::id();
    owner.run_id = std::env::var(crate::journal::LF_TRACE_ID_ENV).ok();
    owner.process_id = std::env::var(crate::journal::LF_PROCESS_ID_ENV).ok();
    write_json(&mut file, &owner)?;
    Ok(OperationAuthorization::Adopted(RebaseOperation {
        file,
        path,
        owner,
        _writer_lease: writer_lease,
    }))
}

fn new_owner(
    worktree: &Path,
    writer_id: WorktreeWriterId,
    target_ref: &str,
) -> OpsResult<GitOperationOwner> {
    Ok(GitOperationOwner {
        id: GitOperationId::new(),
        writer_id,
        root_pid: std::process::id(),
        run_id: std::env::var(crate::journal::LF_TRACE_ID_ENV).ok(),
        process_id: std::env::var(crate::journal::LF_PROCESS_ID_ENV).ok(),
        worktree: canonical(worktree),
        branch: current_branch(worktree)?.unwrap_or_else(|| "HEAD".to_string()),
        head: rev_parse(worktree, "HEAD")?,
        target_ref: target_ref.to_string(),
        target_sha: None,
    })
}

pub(crate) fn prepare_agent_writer(
    worktree: &Path,
    env: &BTreeMap<String, String>,
) -> OpsResult<Option<AgentWriterGuard>> {
    if absolute_git_dir(worktree).is_err() {
        return Ok(None);
    }

    let inherited_writer = env
        .get(LF_WORKTREE_WRITER_ID_ENV)
        .cloned()
        .or_else(|| std::env::var(LF_WORKTREE_WRITER_ID_ENV).ok())
        .map(|value| WorktreeWriterId::parse(&value))
        .transpose()?;
    let (writer_id, lease) = establish_writer(worktree, inherited_writer)?;
    // Establish the writer first. A rebase racing this preflight will then see
    // the live writer and refuse, instead of creating an operation in the gap
    // between an agent's operation check and its writer claim.
    let requested_operation = env
        .get(LF_GIT_OPERATION_ID_ENV)
        .cloned()
        .or_else(|| std::env::var(LF_GIT_OPERATION_ID_ENV).ok())
        .map(|value| GitOperationId::parse(&value))
        .transpose()?;
    fence_agent_launch(worktree, requested_operation.as_ref())?;
    Ok(Some(AgentWriterGuard {
        writer_id,
        _lease: lease,
    }))
}

fn fence_agent_launch(
    worktree: &Path,
    requested_operation: Option<&GitOperationId>,
) -> OpsResult<()> {
    let path = operation_path(worktree)?;
    if !path.exists() {
        if let Some(state) = intervention_state(worktree)? {
            return Err(OpsError::Message(format!(
                "refusing to launch an agent while an unowned {state} operation is in progress"
            )));
        }
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
            let owner = read_json::<GitOperationOwner>(&mut file).ok();
            if intervention_state(worktree)?.is_some() {
                return Err(OpsError::Message(format!(
                    "refusing to launch an agent while stale {} owns a recoverable Git operation",
                    owner
                        .as_ref()
                        .map(owner_label)
                        .unwrap_or_else(|| "operation metadata".to_string())
                )));
            }
            fs::remove_file(path)?;
            Ok(())
        }
    }
}

fn establish_writer(
    worktree: &Path,
    inherited: Option<WorktreeWriterId>,
) -> OpsResult<(WorktreeWriterId, Option<WorktreeWriterLease>)> {
    if let Some(id) = inherited {
        let path = writer_path(worktree, &id)?;
        if !path.exists() {
            return Err(revoked_writer_error(&id));
        }
        let mut file = open_existing_record(&path)?;
        return match FileExt::try_lock_exclusive(&file) {
            Err(error) if is_contended(&error) => {
                let owner = read_json::<WorktreeWriterOwner>(&mut file)
                    .map_err(|_| revoked_writer_error(&id))?;
                if owner.id != id || !writer_owner_is_authoritative(&owner)? {
                    Err(revoked_writer_error(&id))
                } else {
                    Ok((id, None))
                }
            }
            Err(error) => Err(error.into()),
            Ok(()) => Err(revoked_writer_error(&id)),
        };
    }

    let id = WorktreeWriterId::new();
    let path = writer_path(worktree, &id)?;
    let mut file = create_record(&path)?;
    match FileExt::try_lock_exclusive(&file) {
        Err(error) if is_contended(&error) => Ok((id, None)),
        Err(error) => Err(error.into()),
        Ok(()) => {
            let owner = WorktreeWriterOwner {
                id: id.clone(),
                pid: std::process::id(),
                run_id: std::env::var(crate::journal::LF_TRACE_ID_ENV).ok(),
                process_id: std::env::var(crate::journal::LF_PROCESS_ID_ENV).ok(),
                invocation_id: std::env::var(crate::durable::AGENT_INVOCATION_ENV).ok(),
                worktree: worktree.to_path_buf(),
            };
            write_json(&mut file, &owner)?;
            Ok((id.clone(), Some(WorktreeWriterLease { path, _file: file })))
        }
    }
}

fn refuse_other_writers(worktree: &Path, own_id: &WorktreeWriterId) -> OpsResult<()> {
    let dir = writers_dir(worktree)?;
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        if path.file_stem().and_then(|value| value.to_str()) == Some(own_id.as_str()) {
            continue;
        }
        let mut file = open_record(&path)?;
        match FileExt::try_lock_exclusive(&file) {
            Err(error) if is_contended(&error) => {
                let owner = read_json::<WorktreeWriterOwner>(&mut file).map_err(|_| {
                    OpsError::Message(format!(
                        "refusing to reclaim unreadable writer authority at {}",
                        path.display()
                    ))
                })?;
                if writer_owner_is_authoritative(&owner)? {
                    return Err(OpsError::Message(format!(
                        "refusing to rebase while independent writer {} (pid {}) has live durable authority in this worktree",
                        owner.id.as_str(), owner.pid
                    )));
                }
                fs::remove_file(&path)?;
            }
            Err(error) => return Err(error.into()),
            Ok(()) => {
                fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}

fn writer_owner_is_authoritative(owner: &WorktreeWriterOwner) -> OpsResult<bool> {
    if let Some(invocation_id) = owner.invocation_id.as_deref() {
        let db_path = crate::store::observability_database_path().map_err(|error| {
            OpsError::Message(format!(
                "cannot resolve durable writer authority for {invocation_id}: {error}"
            ))
        })?;
        return crate::store::writer_invocation_is_authoritative(&db_path, invocation_id).map_err(
            |error| {
                OpsError::Message(format!(
                    "cannot read durable writer authority for {invocation_id}: {error}"
                ))
            },
        );
    }
    // A Loopflow agent writer created before invocation identity was recorded
    // has provenance but no durable owner. Its surviving PID is evidence only.
    // A direct host Git operation has neither and retains its ordinary file lock.
    Ok(owner.run_id.is_none() && owner.process_id.is_none())
}

fn revoked_writer_error(id: &WorktreeWriterId) -> OpsError {
    OpsError::Message(format!(
        "worktree writer {} was revoked because its durable Run, Turn, or Ask no longer owns mutation authority; start a fresh authorized operation",
        id.as_str()
    ))
}

fn refuse_intervention(worktree: &Path) -> OpsResult<()> {
    if let Some(state) = intervention_state(worktree)? {
        return Err(OpsError::Message(format!(
            "refusing to rebase: a {state} operation already exists; Loopflow did not start or abort it"
        )));
    }
    Ok(())
}

fn operation_path(worktree: &Path) -> OpsResult<PathBuf> {
    Ok(absolute_git_dir(worktree)?
        .join("loopflow")
        .join("rebase-owner.json"))
}

fn writers_dir(worktree: &Path) -> OpsResult<PathBuf> {
    Ok(absolute_git_dir(worktree)?.join("loopflow").join("writers"))
}

fn writer_path(worktree: &Path, id: &WorktreeWriterId) -> OpsResult<PathBuf> {
    Ok(writers_dir(worktree)?.join(format!("{}.json", id.as_str())))
}

fn open_record(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
}

fn open_existing_record(path: &Path) -> io::Result<File> {
    OpenOptions::new().read(true).write(true).open(path)
}

fn create_record(path: &Path) -> OpsResult<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(open_record(path)?)
}

fn inherited_writer_id() -> OpsResult<Option<WorktreeWriterId>> {
    std::env::var(LF_WORKTREE_WRITER_ID_ENV)
        .ok()
        .map(|value| WorktreeWriterId::parse(&value))
        .transpose()
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
        "rebase {} (pid {}, branch {})",
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
    use super::{begin_rebase_operation, establish_writer, prepare_agent_writer, writer_path};
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::path::Path;
    use std::process::Command;

    struct EnvRestore(Vec<(&'static str, Option<OsString>)>);

    impl EnvRestore {
        fn capture(names: &[&'static str]) -> Self {
            Self(
                names
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (name, value) in &self.0 {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn settled_ask_writer_is_reclaimed_and_its_token_stays_revoked() {
        let _env_lock = crate::journal::test_env_lock();
        let _restore = EnvRestore::capture(&[
            "LF_CONTROL_DB_PATH",
            "LF_AGENT_INVOCATION_ID",
            "LF_TRACE_ID",
            "LF_PROCESS_ID",
            "LF_WORKTREE_WRITER_ID",
        ]);
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-b", "main"]);
        git(&repo, &["config", "user.email", "test@example.com"]);
        git(&repo, &["config", "user.name", "Loopflow Test"]);
        std::fs::write(repo.join("README.md"), "proof\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "proof"]);

        let database = directory.path().join("authority.db");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE runs (id TEXT PRIMARY KEY, state TEXT NOT NULL);
                 CREATE TABLE agent_invocations (
                    id TEXT PRIMARY KEY,
                    ended_at INTEGER,
                    supervising_run_id TEXT,
                    answer_ask_id TEXT
                 );
                 CREATE TABLE agent_turns (
                    id TEXT PRIMARY KEY,
                    invocation_id TEXT NOT NULL,
                    status TEXT NOT NULL
                 );
                 CREATE TABLE ask_exchanges (
                    id TEXT PRIMARY KEY,
                    state TEXT NOT NULL,
                    active_invocation_id TEXT
                 );
                 INSERT INTO runs VALUES ('run_stale', 'active');
                 INSERT INTO agent_invocations
                    VALUES ('invocation_stale', NULL, 'run_stale', 'ask_stale');
                 INSERT INTO agent_turns
                    VALUES ('turn_stale', 'invocation_stale', 'running');
                 INSERT INTO ask_exchanges
                    VALUES ('ask_stale', 'claimed', 'invocation_stale');",
            )
            .unwrap();
        std::env::set_var("LF_CONTROL_DB_PATH", &database);
        std::env::set_var("LF_AGENT_INVOCATION_ID", "invocation_stale");
        std::env::set_var("LF_TRACE_ID", "trace_stale");
        std::env::set_var("LF_PROCESS_ID", "process_stale");
        let guard = prepare_agent_writer(&repo, &BTreeMap::new())
            .unwrap()
            .expect("Git repo gets a writer");
        let stale_id = guard.writer_id.clone();
        let stale_path = writer_path(&repo, &stale_id).unwrap();
        assert!(stale_path.exists());

        connection
            .execute_batch(
                "UPDATE agent_turns SET status='completed' WHERE id='turn_stale';
                 UPDATE ask_exchanges
                 SET state='resolved', active_invocation_id=NULL
                 WHERE id='ask_stale';",
            )
            .unwrap();
        drop(connection);
        std::env::remove_var("LF_AGENT_INVOCATION_ID");
        std::env::remove_var("LF_TRACE_ID");
        std::env::remove_var("LF_PROCESS_ID");

        let operation = begin_rebase_operation(&repo, "main").unwrap();
        assert!(
            !stale_path.exists(),
            "terminal owner is reclaimed even while its PID lives"
        );
        let error = establish_writer(&repo, Some(stale_id))
            .expect_err("a stale process cannot recreate its revoked writer token");
        assert!(error.to_string().contains("was revoked"));
        operation.complete().unwrap();
        drop(guard);
    }
}
