//! Bounded files in a Loopflow home: diagnostic log rotation and pruning of
//! old process logs, context deliveries, migration backups, and lock files.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use loopflow_storage::lock_file;
use loopflow_storage::migration_runner;
use loopflow_storage::sqlite::ActiveFileOwners;
use sha2::{Digest, Sha256};

use crate::store::StoreResult;

pub const LF_LOG_MAX_BYTES: u64 = 32 * 1024 * 1024;
/// Active `lf.log` plus `lf.log.1..=lf.log.3` (4 files total).
pub const LF_LOG_KEEP_FILES: usize = 4;
pub const PROCESS_LOG_MAX_AGE_SECS: i64 = 30 * 24 * 60 * 60;
pub const DELIVERY_MAX_AGE_SECS: i64 = 7 * 24 * 60 * 60;
pub const BACKUP_FRONTIER_AGE_SECS: i64 = 7 * 24 * 60 * 60;
/// Grace window so brand-new transient locks whose owner row has not committed
/// yet are left untouched even if momentarily unlocked.
const TRANSIENT_LOCK_GRACE_SECS: i64 = 5 * 60;
/// Grace margin when comparing a delivery's mtime against a live agent's start.
const LIVE_DELIVERY_GRACE_SECS: i64 = 60;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct FileRetentionReport {
    pub log_rotated: bool,
    pub process_logs_removed: usize,
    pub deliveries_removed: usize,
    pub backups_removed: usize,
    pub lock_files_removed: usize,
}

impl FileRetentionReport {
    pub fn any(&self) -> bool {
        self.log_rotated
            || self.process_logs_removed > 0
            || self.deliveries_removed > 0
            || self.backups_removed > 0
            || self.lock_files_removed > 0
    }
}

/// Rotate `logs/lf.log` and prune stale files for the home owning `database`.
pub fn prune(database: &Path, now: i64) -> StoreResult<FileRetentionReport> {
    let Some(home) = database.parent() else {
        return Ok(FileRetentionReport::default());
    };
    let mut report = FileRetentionReport::default();
    match rotate_diagnostic_log(home, LF_LOG_MAX_BYTES, LF_LOG_KEEP_FILES) {
        Ok(rotated) => report.log_rotated = rotated,
        Err(error) => tracing::warn!(%error, "diagnostic log rotation failed"),
    }
    if !database.exists() {
        return Ok(report);
    }
    let store = crate::store::opening::open_read_only_validated(database)?;
    let owners = store.active_file_owners()?;
    drop(store);

    report.process_logs_removed = prune_process_logs(home, &owners, now);
    report.deliveries_removed = prune_context_deliveries(home, &owners, now);
    report.backups_removed = prune_backups(database, &owners, now);
    report.lock_files_removed = prune_lock_files(database, home, &owners, now);
    Ok(report)
}

/// Rotate `<home>/logs/lf.log` when it reaches `max_bytes`, keeping at most
/// `keep_files` total files (`lf.log` plus `lf.log.1..lf.log.<keep_files-1>`).
///
/// Uses atomic renames (never truncating in place): writers that open per event
/// create a fresh `lf.log` on their next line, and any writer holding an open
/// descriptor finishes writing cleanly into `lf.log.1`.
pub fn rotate_diagnostic_log(
    home: &Path,
    max_bytes: u64,
    keep_files: usize,
) -> std::io::Result<bool> {
    let logs = home.join("logs");
    let active = logs.join("lf.log");
    let size = match fs::metadata(&active) {
        Ok(meta) => meta.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if size < max_bytes {
        return Ok(false);
    }
    let max_rotated = keep_files.saturating_sub(1);
    if max_rotated == 0 {
        let _ = fs::remove_file(&active);
        return Ok(true);
    }
    // Remove any excess rotated generations first.
    if let Ok(entries) = fs::read_dir(&logs) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            if let Some(index) = name
                .to_str()
                .and_then(|s| s.strip_prefix("lf.log."))
                .and_then(|n| n.parse::<usize>().ok())
            {
                if index >= max_rotated {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
    for idx in (1..max_rotated).rev() {
        let from = logs.join(format!("lf.log.{idx}"));
        let to = logs.join(format!("lf.log.{}", idx + 1));
        if from.exists() {
            fs::rename(&from, &to)?;
        }
    }
    fs::rename(&active, logs.join("lf.log.1"))?;
    Ok(true)
}

fn modified_unix(path: &Path) -> Option<i64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_secs()).ok()
}

fn process_id_active(stem: &str, active: &HashSet<String>) -> bool {
    if active.contains(stem) {
        return true;
    }
    // Graph logs can carry a prefix before the UUID, e.g. `repository-release-<uuid>`.
    // Check the ASCII `-` byte before slicing so non-ASCII filenames cannot split a UTF-8 char.
    if stem.len() > 36 && stem.as_bytes()[stem.len() - 37] == b'-' {
        let tail = &stem[stem.len() - 36..];
        if active.contains(tail) {
            return true;
        }
    }
    false
}

fn prune_process_logs(home: &Path, owners: &ActiveFileOwners, now: i64) -> usize {
    let logs = home.join("logs");
    let Ok(entries) = fs::read_dir(&logs) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(stem) = name
            .strip_prefix("terminal-")
            .or_else(|| name.strip_prefix("graph-"))
            .and_then(|rest| rest.strip_suffix(".log"))
        else {
            continue;
        };
        let path = entry.path();
        let Some(mtime) = modified_unix(&path) else {
            continue;
        };
        if now.saturating_sub(mtime) < PROCESS_LOG_MAX_AGE_SECS {
            continue;
        }
        if process_id_active(stem, &owners.active_process_ids) {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[derive(serde::Deserialize)]
struct DeliveryIdentityOnly {
    identity: Option<DeliverySessionOnly>,
}

#[derive(serde::Deserialize)]
struct DeliverySessionOnly {
    session: String,
}

fn prune_context_deliveries(home: &Path, owners: &ActiveFileOwners, now: i64) -> usize {
    let dir = home.join("context-deliveries");
    let Ok(entries) = fs::read_dir(&dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        // `.thread` and `.store-gap` markers carry state and stay.
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(mtime) = modified_unix(&path) else {
            continue;
        };
        if now.saturating_sub(mtime) < DELIVERY_MAX_AGE_SECS {
            continue;
        }
        let launch_active = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<DeliveryIdentityOnly>(&bytes).ok())
            .and_then(|delivery| delivery.identity)
            .is_some_and(|identity| {
                owners
                    .live_session_agents
                    .get(&identity.session)
                    .is_some_and(|started_at| {
                        started_at.is_none_or(|started| {
                            mtime >= started.saturating_sub(LIVE_DELIVERY_GRACE_SECS)
                        })
                    })
            });
        if launch_active {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

fn prune_backups(database: &Path, owners: &ActiveFileOwners, now: i64) -> usize {
    let before = migration_runner::migration_backups(database);
    if before.is_empty() {
        return 0;
    }
    let Some(applied_at) = owners.latest_migration_applied_at else {
        return 0;
    };
    let kept = if now.saturating_sub(applied_at) >= BACKUP_FRONTIER_AGE_SECS {
        0
    } else if before.len() > 1 {
        1
    } else {
        return 0;
    };
    migration_runner::prune_migration_backups(database, kept);
    let after = migration_runner::migration_backups(database);
    before.len().saturating_sub(after.len())
}

fn prune_lock_files(database: &Path, home: &Path, owners: &ActiveFileOwners, now: i64) -> usize {
    let mut removed = 0;
    let canonical_db = database
        .canonicalize()
        .unwrap_or_else(|_| database.to_path_buf());

    // 1. `<db>.session-locks/<sha256(fence_locator)>`
    let active_fences: HashSet<String> = owners
        .active_fence_locators
        .iter()
        .map(|locator| hex::encode(Sha256::digest(locator.as_bytes())))
        .collect();
    removed += prune_lock_dir(
        &canonical_db.with_extension("session-locks"),
        &active_fences,
        None,
        now,
    );

    // 2. `<home>/human-sessions/.<sha256(session_id)[..16]>.launch.lock`
    let active_human_locks: HashSet<String> = owners
        .active_session_ids
        .iter()
        .map(|id| {
            let digest = hex::encode(&Sha256::digest(id.as_bytes())[..16]);
            format!(".{digest}.launch.lock")
        })
        .collect();
    removed += prune_lock_dir_matching(
        &home.join("human-sessions"),
        &active_human_locks,
        |name| name.starts_with('.') && name.ends_with(".launch.lock"),
        None,
        now,
    );

    // 3. `<home>/creation-locks/<sha256(request_id)>`
    let active_creations: HashSet<String> = owners
        .active_creation_requests
        .iter()
        .map(|id| hex::encode(Sha256::digest(id.as_bytes())))
        .collect();
    removed += prune_lock_dir(
        &home.join("creation-locks"),
        &active_creations,
        Some(TRANSIENT_LOCK_GRACE_SECS),
        now,
    );

    // 4. `<db>.admission/<sha256(checkout_or_ancestor)>`
    let mut active_admissions = HashSet::new();
    for checkout in &owners.active_checkouts {
        let canonical = crate::store::canonicalize_with_missing_tail(checkout)
            .unwrap_or_else(|_| checkout.to_path_buf());
        for path in canonical.ancestors() {
            active_admissions.insert(hex::encode(Sha256::digest(
                path.as_os_str().as_encoded_bytes(),
            )));
        }
    }
    removed += prune_lock_dir(
        &canonical_db.with_extension("admission"),
        &active_admissions,
        Some(TRANSIENT_LOCK_GRACE_SECS),
        now,
    );

    removed
}

fn prune_lock_dir(
    dir: &Path,
    active_names: &HashSet<String>,
    grace_secs: Option<i64>,
    now: i64,
) -> usize {
    prune_lock_dir_matching(dir, active_names, |_| true, grace_secs, now)
}

fn prune_lock_dir_matching(
    dir: &Path,
    active_names: &HashSet<String>,
    matches_name: impl Fn(&str) -> bool,
    grace_secs: Option<i64>,
    now: i64,
) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !matches_name(name) || active_names.contains(name) {
            continue;
        }
        let path = entry.path();
        if let Some(grace) = grace_secs {
            if modified_unix(&path).is_some_and(|mtime| now.saturating_sub(mtime) < grace) {
                continue;
            }
        }
        if matches!(lock_file::try_remove_if_unlocked(&path), Ok(true)) {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
fn set_mtime(path: &Path, unix_secs: i64) {
    let secs = u64::try_from(unix_secs.max(0)).unwrap();
    let time = UNIX_EPOCH + std::time::Duration::from_secs(secs);
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;

    use loopflow_model::id::{AgentProcessId, LfProcessId};
    use loopflow_storage::graph_process::ExecutionStart;
    use loopflow_storage::lock_file;
    use sha2::{Digest, Sha256};

    use super::{
        prune, rotate_diagnostic_log, set_mtime, BACKUP_FRONTIER_AGE_SECS, DELIVERY_MAX_AGE_SECS,
        PROCESS_LOG_MAX_AGE_SECS,
    };
    use crate::context_block::{ContextDelivery, SessionIdentity};

    #[test]
    fn diagnostic_log_rotates_keeps_cap_and_open_writers_finish_cleanly() {
        let home = tempfile::tempdir().unwrap();
        let logs = home.path().join("logs");
        fs::create_dir_all(&logs).unwrap();
        let active = logs.join("lf.log");

        // Hold an open O_APPEND descriptor across rotation to prove it stays writable
        // and its bytes land in `lf.log.1`, while new appends start a fresh `lf.log`.
        fs::write(&active, "first-generation-payload\n").unwrap();
        let mut open_writer = fs::OpenOptions::new().append(true).open(&active).unwrap();

        assert!(!rotate_diagnostic_log(home.path(), 1024, 4).unwrap());
        assert!(rotate_diagnostic_log(home.path(), 10, 4).unwrap());

        writeln!(open_writer, "late-line-on-old-fd").unwrap();
        drop(open_writer);

        let rotated1 = fs::read_to_string(logs.join("lf.log.1")).unwrap();
        assert!(rotated1.contains("first-generation-payload"));
        assert!(rotated1.contains("late-line-on-old-fd"));
        assert!(!active.exists());

        // New writers create a fresh lf.log; rotating 4 more times keeps only 3 rotated + active (4 files).
        for gen in 2..=6 {
            fs::write(&active, format!("generation-{gen}-payload\n")).unwrap();
            assert!(rotate_diagnostic_log(home.path(), 10, 4).unwrap());
        }
        fs::write(&active, "current-active\n").unwrap();

        assert!(logs.join("lf.log").exists());
        assert!(logs.join("lf.log.1").exists());
        assert!(logs.join("lf.log.2").exists());
        assert!(logs.join("lf.log.3").exists());
        assert!(!logs.join("lf.log.4").exists());
        assert_eq!(
            fs::read_to_string(logs.join("lf.log.1")).unwrap(),
            "generation-6-payload\n"
        );
        assert_eq!(
            fs::read_to_string(logs.join("lf.log.3")).unwrap(),
            "generation-4-payload\n"
        );
    }

    #[test]
    fn prune_removes_ended_logs_deliveries_backups_and_locks_while_preserving_active_ones() {
        let home = tempfile::tempdir().unwrap();
        let db = home.path().join("loopflow.db");
        let store = crate::store::opening::open_ephemeral(&db).unwrap();
        let now = 2_000_000_000_i64;

        // Seed one active Graph and one ended Graph, one active Session with live agent and one ended Session.
        let active_graph = LfProcessId::new();
        let ended_graph = LfProcessId::new();
        let compiled = loopflow_execution_compiled();
        store
            .record_graph(
                &active_graph,
                &compiled,
                None,
                &ExecutionStart::reserved(now - 100),
                None,
                Some("/active/checkout"),
                None,
            )
            .unwrap();
        store
            .record_graph(
                &ended_graph,
                &compiled,
                None,
                &ExecutionStart::reserved(now - 500),
                None,
                Some("/ended/checkout"),
                None,
            )
            .unwrap();
        store
            .finish_run(
                loopflow_storage::graph_process::RunOwner::GraphRun,
                &ended_graph,
                now - 400,
                Some(0),
                None,
            )
            .unwrap();

        let active_session = "session_00000000000000000000000000000001";
        let ended_session = "session_00000000000000000000000000000002";
        let active_agent = AgentProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO agent_sessions(id, title, title_source, fence_locator, created_at, input_published, cwd, completed_at, agent_process_id, agent_started_at, agent_ended_at)
                 VALUES (?1, 'Active', 'human', 'fence-active', ?2, 0, '/active/checkout', NULL, ?3, ?4, NULL)",
                rusqlite::params![active_session, now - 1000, active_agent.as_str(), now - DELIVERY_MAX_AGE_SECS - 100],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO agent_sessions(id, title, title_source, fence_locator, created_at, input_published, cwd, completed_at, agent_process_id, agent_started_at, agent_ended_at)
                 VALUES (?1, 'Ended', 'human', 'fence-ended', ?2, 0, '/ended/checkout', ?3, NULL, NULL, ?3)",
                rusqlite::params![ended_session, now - 2000, now - 1000],
            )
            .unwrap();
            // Mark migrations as applied 8 days ago so the backup is eligible for removal.
            conn.execute(
                "UPDATE schema_migrations SET applied_at = ?1",
                [now - BACKUP_FRONTIER_AGE_SECS - 3600],
            )
            .unwrap();
        }
        drop(store);

        // 1. Process logs (30-day threshold + active-owner guard).
        let logs = home.path().join("logs");
        fs::create_dir_all(&logs).unwrap();
        let old_ended_log = logs.join(format!("graph-{ended_graph}.log"));
        let old_active_log = logs.join(format!("graph-{active_graph}.log"));
        let recent_ended_log = logs.join(format!("terminal-{ended_graph}.log"));
        let old_prefixed_active_log =
            logs.join(format!("graph-repository-release-{active_graph}.log"));
        let old_non_ascii_log = logs.join("graph-日本語ログファイル名テスト用の長い名前.log");
        for p in [
            &old_ended_log,
            &old_active_log,
            &recent_ended_log,
            &old_prefixed_active_log,
            &old_non_ascii_log,
        ] {
            fs::write(p, "log\n").unwrap();
        }
        set_mtime(&old_ended_log, now - PROCESS_LOG_MAX_AGE_SECS - 60);
        set_mtime(&old_active_log, now - PROCESS_LOG_MAX_AGE_SECS - 60);
        set_mtime(
            &old_prefixed_active_log,
            now - PROCESS_LOG_MAX_AGE_SECS - 60,
        );
        set_mtime(&old_non_ascii_log, now - PROCESS_LOG_MAX_AGE_SECS - 60);
        set_mtime(&recent_ended_log, now - 60);

        // 2. Context deliveries (7-day threshold + live launch guard + keep .thread / .store-gap).
        let deliveries = home.path().join("context-deliveries");
        fs::create_dir_all(&deliveries).unwrap();
        let write_delivery = |name: &str, session: Option<&str>, mtime: i64| {
            let path = deliveries.join(name);
            let delivery = ContextDelivery {
                identity: session.map(|s| SessionIdentity {
                    session: s.to_owned(),
                    task: None,
                }),
                repository: None,
                repo: PathBuf::from("/tmp/repo"),
                wave: None,
                skill_file: None,
                references: Vec::new(),
                submitted_docs: None,
                home: home.path().to_path_buf(),
            };
            delivery.save(&path).unwrap();
            set_mtime(&path, mtime);
            path
        };
        let old_ended_delivery = write_delivery(
            "ended.json",
            Some(ended_session),
            now - DELIVERY_MAX_AGE_SECS - 60,
        );
        let old_active_delivery = write_delivery(
            "active.json",
            Some(active_session),
            now - DELIVERY_MAX_AGE_SECS - 60,
        );
        let thread_marker = deliveries.join("ended.thread");
        let gap_marker = deliveries.join("ended.store-gap");
        fs::write(&thread_marker, "thread-1").unwrap();
        fs::write(&gap_marker, "gap").unwrap();
        set_mtime(&thread_marker, now - DELIVERY_MAX_AGE_SECS - 60);
        set_mtime(&gap_marker, now - DELIVERY_MAX_AGE_SECS - 60);

        // 3. Migration backup older than 7-day frontier age.
        let backup = home
            .path()
            .join("loopflow.db.backup-0.15.0-0123456789abcdef");
        fs::write(&backup, "backup-bytes").unwrap();

        // 4. Lock files: ended unlocked goes, active stays, ended-but-held stays.
        let canonical_db = db.canonicalize().unwrap();
        let session_locks = canonical_db.with_extension("session-locks");
        fs::create_dir_all(&session_locks).unwrap();
        let active_fence_file = session_locks.join(hex::encode(Sha256::digest(b"fence-active")));
        let ended_fence_file = session_locks.join(hex::encode(Sha256::digest(b"fence-ended")));
        let busy_ended_file = session_locks.join(hex::encode(Sha256::digest(b"fence-busy-ended")));
        fs::write(&active_fence_file, "").unwrap();
        fs::write(&ended_fence_file, "").unwrap();
        let _busy_guard = lock_file::lock_exclusive_verified(&busy_ended_file).unwrap();

        let human_locks = home.path().join("human-sessions");
        fs::create_dir_all(&human_locks).unwrap();
        let ended_human = human_locks.join(format!(
            ".{}.launch.lock",
            hex::encode(&Sha256::digest(ended_session.as_bytes())[..16])
        ));
        let active_human = human_locks.join(format!(
            ".{}.launch.lock",
            hex::encode(&Sha256::digest(active_session.as_bytes())[..16])
        ));
        fs::write(&ended_human, "").unwrap();
        fs::write(&active_human, "").unwrap();

        let creation_locks = home.path().join("creation-locks");
        fs::create_dir_all(&creation_locks).unwrap();
        let stale_creation = creation_locks.join(hex::encode(Sha256::digest(b"finished-req")));
        fs::write(&stale_creation, "").unwrap();
        set_mtime(&stale_creation, now - 600);

        let admission_locks = canonical_db.with_extension("admission");
        fs::create_dir_all(&admission_locks).unwrap();
        let stale_admission = admission_locks.join(hex::encode(Sha256::digest(b"/ended/checkout")));
        let active_admission =
            admission_locks.join(hex::encode(Sha256::digest(b"/active/checkout")));
        fs::write(&stale_admission, "").unwrap();
        fs::write(&active_admission, "").unwrap();
        set_mtime(&stale_admission, now - 600);
        set_mtime(&active_admission, now - 600);

        let report = prune(&db, now).unwrap();
        assert_eq!(report.process_logs_removed, 2);
        assert_eq!(report.deliveries_removed, 1);
        assert_eq!(report.backups_removed, 1);
        assert_eq!(report.lock_files_removed, 4);

        assert!(!old_ended_log.exists());
        assert!(!old_non_ascii_log.exists());
        assert!(old_active_log.exists());
        assert!(old_prefixed_active_log.exists());
        assert!(recent_ended_log.exists());

        assert!(!old_ended_delivery.exists());
        assert!(old_active_delivery.exists());
        assert!(thread_marker.exists());
        assert!(gap_marker.exists());

        assert!(!backup.exists());

        assert!(active_fence_file.exists());
        assert!(!ended_fence_file.exists());
        assert!(busy_ended_file.exists());
        assert!(active_human.exists());
        assert!(!ended_human.exists());
        assert!(!stale_creation.exists());
        assert!(!stale_admission.exists());
        assert!(active_admission.exists());
    }

    fn loopflow_execution_compiled() -> loopflow_storage::graph::CompiledGraph {
        crate::graph::compiled::compile("probe", &[])
    }
}
