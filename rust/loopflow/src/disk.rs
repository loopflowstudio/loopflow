//! Loopflow's footprint on this Machine's disk.
//!
//! Home (`~/.lf`) is measured by category. Rotation removes backups, draft store
//! copies, Session captures and logs once they are older than [`RETENTION`],
//! matching Claude Code's default transcript cleanup; there is no size budget.
//! It never removes the database or its sidecars, account homes, the newest
//! backup of each database, anything modified within the grace period, or
//! anything a running process holds open. A store copy's snapshot staging
//! directory goes as soon as no snapshot holds the copies lock, since only a
//! killed snapshot leaves one. Every removal is appended to `logs/disk.log`.

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Rotatable material older than this is removed automatically.
pub const RETENTION: Duration = Duration::from_secs(30 * 24 * 3600);
/// Material modified this recently is never rotated.
pub const GRACE: Duration = Duration::from_secs(24 * 3600);
/// Automatic rotation runs at most this often.
const ROTATION_INTERVAL: Duration = Duration::from_secs(3600);
const ROTATION_STAMP: &str = "runtime/disk-rotation.stamp";
const LOCK: &str = "runtime/disk.lock";
const LOG: &str = "logs/disk.log";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Database,
    Backups,
    Copies,
    Accounts,
    Captures,
    Logs,
    Runtime,
    Other,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::Backups => "backups",
            Self::Copies => "copies",
            Self::Accounts => "accounts",
            Self::Captures => "captures",
            Self::Logs => "logs",
            Self::Runtime => "runtime",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryUsage {
    pub category: Category,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeUsage {
    pub home: PathBuf,
    pub bytes: u64,
    pub categories: Vec<CategoryUsage>,
    /// Process receipt files, dead ones included, counted without reading them.
    pub process_receipts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Removal {
    pub category: Category,
    pub path: PathBuf,
    pub bytes: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationReport {
    pub dry_run: bool,
    pub before: HomeUsage,
    pub removed: Vec<Removal>,
    pub freed_bytes: u64,
    /// Home size after removals (estimated on a dry run).
    pub after_bytes: u64,
    pub errors: Vec<String>,
}

pub fn format_bytes(bytes: u64) -> String {
    let units = ["B", "K", "M", "G", "T"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes}B")
    } else {
        format!("{value:.1}{}", units[unit])
    }
}

// ---------------------------------------------------------------------------
// Measurement

/// Allocated bytes under `path`, counting each inode once and following no symlink.
pub fn allocated_bytes(path: &Path) -> u64 {
    let mut seen = HashSet::new();
    allocated(path, &mut seen)
}

fn allocated(path: &Path, seen: &mut HashSet<(u64, u64)>) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    let mut bytes = if !seen.insert((metadata.dev(), metadata.ino())) {
        0
    } else if metadata.is_file() {
        private_bytes(path).unwrap_or(metadata.blocks() * 512)
    } else {
        metadata.blocks() * 512
    };
    if metadata.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                bytes += allocated(&entry.path(), seen);
            }
        }
    }
    bytes
}

/// Bytes deleting the file at `path` frees now. An APFS clone shares its
/// blocks with the original, so counting its allocated blocks would claim
/// space a removal never returns.
#[cfg(target_os = "macos")]
fn private_bytes(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut request = libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: 0,
        volattr: 0,
        dirattr: 0,
        fileattr: 0,
        forkattr: libc::ATTR_CMNEXT_PRIVATESIZE,
    };
    // The reply is its u32 length followed by the off_t private size.
    let mut reply = [0u8; 12];
    // SAFETY: `path` is NUL-terminated, and `request` and `reply` outlive the
    // call; getattrlist writes at most `reply.len()` bytes and keeps no pointer.
    let status = unsafe {
        libc::getattrlist(
            path.as_ptr(),
            std::ptr::addr_of_mut!(request).cast(),
            reply.as_mut_ptr().cast(),
            reply.len(),
            libc::FSOPT_NOFOLLOW | libc::FSOPT_ATTR_CMN_EXTENDED,
        )
    };
    if status != 0 || u32::from_ne_bytes(reply[..4].try_into().ok()?) < 12 {
        return None;
    }
    u64::try_from(i64::from_ne_bytes(reply[4..].try_into().ok()?)).ok()
}

#[cfg(not(target_os = "macos"))]
fn private_bytes(_path: &Path) -> Option<u64> {
    None
}

/// The newest file modification under `path`. A directory's own mtime only
/// counts when it holds nothing, since creating it is not writing to it.
fn newest_modification(path: &Path) -> SystemTime {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return SystemTime::UNIX_EPOCH;
    };
    let own = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    if !metadata.is_dir() {
        return own;
    }
    fs::read_dir(path)
        .ok()
        .and_then(|entries| {
            entries
                .flatten()
                .map(|entry| newest_modification(&entry.path()))
                .max()
        })
        .unwrap_or(own)
}

/// The database a backup file belongs to, e.g. `loopflow.db` for
/// `loopflow.db.backup-0.14.8-…` or `loopflow.db.pre-etude-rename-20260716`.
fn backup_database(name: &str) -> Option<&str> {
    let index = name.find(".db.")?;
    let (database, rest) = name.split_at(index + 3);
    let rest = &rest[1..];
    ["backup", "bak", "pre"]
        .iter()
        .any(|marker| rest.starts_with(marker))
        .then_some(database)
}

fn is_sidecar(name: &str) -> bool {
    name.ends_with("-wal") || name.ends_with("-shm")
}

/// `database` is the selected store's file name when it lives directly in Home.
fn categorize(database: Option<&str>, name: &str) -> Category {
    let base = name
        .strip_suffix("-wal")
        .or_else(|| name.strip_suffix("-shm"))
        .unwrap_or(name);
    if database == Some(base) {
        return Category::Database;
    }
    if backup_database(name).is_some() {
        return Category::Backups;
    }
    match name {
        "accounts" => Category::Accounts,
        "runs" => Category::Captures,
        "logs" | "log" => Category::Logs,
        "runtime" => Category::Runtime,
        "copies" => Category::Copies,
        _ => Category::Other,
    }
}

/// Measure Home by category.
pub fn measure_home(home: &Path) -> Result<HomeUsage> {
    let mut totals: BTreeMap<Category, u64> = BTreeMap::new();
    let mut seen = HashSet::new();
    let store = crate::store::default_db_path();
    let database = (store.parent() == Some(home))
        .then(|| store.file_name().and_then(|name| name.to_str()))
        .flatten();
    for entry in fs::read_dir(home).with_context(|| format!("read {}", home.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        let category = categorize(database, &name.to_string_lossy());
        *totals.entry(category).or_default() += allocated(&entry.path(), &mut seen);
    }
    let categories: Vec<_> = totals
        .into_iter()
        .map(|(category, bytes)| CategoryUsage { category, bytes })
        .collect();
    let process_receipts = match fs::read_dir(home.join(crate::journal::PROCESS_RECEIPT_ROOT)) {
        Ok(entries) => entries.count() as u64,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error).context("read Process receipts"),
    };
    Ok(HomeUsage {
        home: home.to_path_buf(),
        bytes: categories.iter().map(|usage| usage.bytes).sum(),
        categories,
        process_receipts,
    })
}

// ---------------------------------------------------------------------------
// Rotation

/// One removable piece of Home: a capture directory, a log file, a backup with
/// its sidecars, or a draft store copy.
#[derive(Debug)]
struct Unit {
    category: Category,
    paths: Vec<PathBuf>,
    bytes: u64,
    modified: SystemTime,
}

fn unit(category: Category, paths: Vec<PathBuf>) -> Unit {
    let mut seen = HashSet::new();
    Unit {
        category,
        bytes: paths.iter().map(|path| allocated(path, &mut seen)).sum(),
        modified: paths
            .iter()
            .map(|path| newest_modification(path))
            .max()
            .unwrap_or(SystemTime::UNIX_EPOCH),
        paths,
    }
}

fn children(path: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(path)
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    paths.sort();
    paths
}

fn log_files(path: &Path, units: &mut Vec<Unit>) {
    for child in children(path) {
        let Ok(metadata) = fs::symlink_metadata(&child) else {
            continue;
        };
        if metadata.is_dir() {
            log_files(&child, units);
        } else if metadata.is_file() {
            units.push(unit(Category::Logs, vec![child]));
        }
    }
}

/// Every unit rotation may remove, before liveness and age checks.
fn rotatable_units(home: &Path) -> Vec<Unit> {
    let mut units = Vec::new();

    for shard in children(&home.join("runs")) {
        for capture in children(&shard) {
            if capture.is_dir() {
                units.push(unit(Category::Captures, vec![capture]));
            }
        }
    }
    log_files(&home.join("logs"), &mut units);
    log_files(&home.join("log"), &mut units);

    // Staging directories go by ownership, not age: see `remove_abandoned_snapshots`.
    let selected_copy = crate::store::default_db_path();
    for copy in children(&home.join("copies")) {
        if copy.is_dir()
            && !selected_copy.starts_with(&copy)
            && !crate::store::copy::is_staging(&copy)
        {
            units.push(unit(Category::Copies, vec![copy]));
        }
    }

    // Keep the newest backup of each database; older ones rotate with sidecars.
    let mut backups: BTreeMap<String, Vec<Unit>> = BTreeMap::new();
    for path in children(home) {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(database) = backup_database(name) else {
            continue;
        };
        if name.ends_with(".lock") {
            continue;
        }
        if is_sidecar(name) && home.join(&name[..name.len() - 4]).exists() {
            continue;
        }
        let mut paths = vec![path.clone()];
        for suffix in ["-wal", "-shm"] {
            let sidecar = home.join(format!("{name}{suffix}"));
            if sidecar.exists() {
                paths.push(sidecar);
            }
        }
        backups
            .entry(database.to_owned())
            .or_default()
            .push(unit(Category::Backups, paths));
    }
    for mut generations in backups.into_values() {
        generations.sort_by_key(|unit| unit.modified);
        generations.pop();
        units.extend(generations);
    }
    units
}

/// Remove rotatable units, oldest first. `select` gives the reason a unit goes,
/// or `None` to keep it.
fn rotate(
    home: &Path,
    dry_run: bool,
    select: impl Fn(&Unit) -> Option<String>,
) -> Result<(Vec<Removal>, Vec<String>)> {
    let mut units = rotatable_units(home);
    units.sort_by_key(|unit| unit.modified);
    let cutoff = SystemTime::now() - GRACE;
    let mut open: Option<HashSet<PathBuf>> = None;
    let mut sweep = Sweep {
        dry_run,
        log: (!dry_run).then(|| open_log(home)).transpose()?,
        removed: Vec::new(),
        errors: Vec::new(),
    };
    remove_abandoned_snapshots(home, &mut sweep)?;
    for unit in units {
        if unit.modified > cutoff {
            continue;
        }
        let Some(reason) = select(&unit) else {
            continue;
        };
        if open.is_none() {
            // An unknown process view proves nothing idle: stop rather than guess.
            // Every directory holding an open path, so a unit is checked in one lookup.
            let paths = crate::git::build_cleanup::running_paths()
                .context("cannot prove Home material is idle")?;
            open = Some(
                paths
                    .iter()
                    .flat_map(|path| path.ancestors().map(Path::to_path_buf))
                    .collect(),
            );
        }
        let held = open.as_ref().expect("process paths were just read");
        if unit
            .paths
            .iter()
            .map(|path| fs::canonicalize(path).unwrap_or_else(|_| path.clone()))
            .any(|path| held.contains(&path))
        {
            continue;
        }
        sweep.remove(unit.category, &unit.paths, unit.bytes, reason);
    }
    Ok((sweep.removed, sweep.errors))
}

/// One rotation pass's removals and failures, each removal logged as it happens.
struct Sweep {
    dry_run: bool,
    log: Option<File>,
    removed: Vec<Removal>,
    errors: Vec<String>,
}

impl Sweep {
    fn remove(&mut self, category: Category, paths: &[PathBuf], bytes: u64, reason: String) {
        if !self.dry_run {
            if let Err(error) = remove_paths(paths) {
                self.errors
                    .push(format!("{}: {error:#}", paths[0].display()));
                return;
            }
        }
        let removal = Removal {
            category,
            path: paths[0].clone(),
            bytes,
            reason,
        };
        if let Some(log) = self.log.as_mut() {
            log_removal(log, &removal);
        }
        self.removed.push(removal);
    }
}

/// Remove snapshot staging directories whose process died mid-snapshot,
/// however recent: holding the copies lock proves no snapshot is filling one.
fn remove_abandoned_snapshots(home: &Path, sweep: &mut Sweep) -> Result<()> {
    let copies = home.join("copies");
    let Some(_idle) = crate::store::copy::idle_lock(&copies)? else {
        return Ok(());
    };
    for path in crate::store::copy::abandoned_snapshots(&copies) {
        let bytes = allocated_bytes(&path);
        sweep.remove(
            Category::Copies,
            std::slice::from_ref(&path),
            bytes,
            "abandoned snapshot".to_owned(),
        );
    }
    Ok(())
}

fn remove_paths(paths: &[PathBuf]) -> Result<()> {
    for path in paths {
        let result = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
            Ok(_) => fs::remove_file(path),
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).with_context(|| format!("remove {}", path.display())),
        }
    }
    Ok(())
}

fn open_log(home: &Path) -> Result<File> {
    let path = home.join(LOG);
    fs::create_dir_all(path.parent().expect("log has a directory"))?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))
}

fn log_removal(log: &mut File, removal: &Removal) {
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let _ = writeln!(
        log,
        "{now} removed {} {} {} ({})",
        removal.category.as_str(),
        format_bytes(removal.bytes),
        removal.path.display(),
        removal.reason
    );
}

/// Hold the Home rotation lock, or `None` when another rotation holds it.
fn try_lock(home: &Path) -> Result<Option<File>> {
    let path = home.join(LOCK);
    fs::create_dir_all(path.parent().expect("lock has a directory"))?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)?;
    // SAFETY: the descriptor is owned by `file` for the duration of the call.
    let locked = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    Ok(locked.then_some(file))
}

/// Remove everything rotatable older than `older_than`. Returns `None` when
/// another rotation is running.
pub fn rotate_home(
    home: &Path,
    older_than: Duration,
    dry_run: bool,
) -> Result<Option<RotationReport>> {
    let Some(_lock) = try_lock(home)? else {
        return Ok(None);
    };
    let before = measure_home(home)?;
    let expired = SystemTime::now() - older_than;
    let reason = format!("older than {} days", older_than.as_secs() / 86400);
    let (removed, errors) = rotate(home, dry_run, |unit| {
        (unit.modified < expired).then(|| reason.clone())
    })?;
    let freed_bytes = removed.iter().map(|removal| removal.bytes).sum();
    let after_bytes = if dry_run {
        before.bytes.saturating_sub(freed_bytes)
    } else {
        measure_home(home)?.bytes
    };
    Ok(Some(RotationReport {
        dry_run,
        before,
        removed,
        freed_bytes,
        after_bytes,
        errors,
    }))
}

/// Rotate Home on a background thread when the last rotation began over an hour
/// ago. Called as Loopflow produces new captures, so expired material leaves
/// without anyone running a cleanup. Rotation runs in this process rather than a
/// spawned `lf`, so it adds no Process history; one cut short by exit resumes on
/// the next due capture.
pub(crate) fn rotate_when_due(home: &Path) {
    // Unit tests capture into scratch homes; rotation must not follow them.
    if cfg!(test) {
        return;
    }
    let stamp = home.join(ROTATION_STAMP);
    if let Ok(age) = fs::metadata(&stamp)
        .and_then(|metadata| metadata.modified())
        .map(|modified| modified.elapsed().unwrap_or_default())
    {
        if age < ROTATION_INTERVAL {
            return;
        }
    }
    let home = home.to_path_buf();
    std::thread::spawn(move || {
        let rotated = (|| -> Result<()> {
            fs::create_dir_all(stamp.parent().expect("stamp has a directory"))?;
            File::create(&stamp)?;
            rotate_home(&home, RETENTION, false)?;
            Ok(())
        })();
        if let Err(error) = rotated {
            tracing::warn!(error = %format!("{error:#}"), "automatic Home rotation failed");
        }
        // Receipts are tiny but unbounded in number; a burst of unrecorded runs
        // must not leave them for `lf disk clean` to parse forever.
        if let Err(error) = crate::lf::commands::top::prune_dead_process_receipts(&home, false) {
            tracing::warn!(error = %format!("{error:#}"), "automatic Process receipt prune failed");
        }
        if let Err(error) = prune_store() {
            tracing::warn!(error = %format!("{error:#}"), "automatic store prune failed");
        }
    });
}

/// Run the store's retention prune on the selected database.
pub(crate) fn prune_store() -> Result<crate::store::sqlite::retention::RetentionReport> {
    let database = crate::store::default_db_path();
    if !database.exists() {
        return Ok(Default::default());
    }
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    Ok(crate::store::sqlite::retention::prune(&database, now)?)
}

#[cfg(test)]
mod tests {
    use super::{backup_database, rotate_home, Category};
    use std::fs::{self, File, FileTimes};
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    fn write(path: &Path, bytes: usize, age_days: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![7u8; bytes]).unwrap();
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(
                FileTimes::new()
                    .set_modified(SystemTime::now() - Duration::from_secs(age_days * 86400)),
            )
            .unwrap();
    }

    #[test]
    fn footprint_counts_process_receipts_without_reading_them() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        assert_eq!(super::measure_home(home).unwrap().process_receipts, 0);
        let receipts = home.join(crate::journal::PROCESS_RECEIPT_ROOT);
        fs::create_dir_all(&receipts).unwrap();
        for name in ["a.json", "b.json", "not-json-at-all.json"] {
            fs::write(receipts.join(name), b"{").unwrap();
        }
        let usage = super::measure_home(home).unwrap();
        assert_eq!(usage.process_receipts, 3);
        assert_eq!(serde_json::to_value(&usage).unwrap()["process_receipts"], 3);
    }

    #[test]
    fn backups_are_recognized_by_database() {
        assert_eq!(
            backup_database("loopflow.db.backup-0.14.8-0123456789abcdef"),
            Some("loopflow.db")
        );
        assert_eq!(
            backup_database("loopflow.db.pre-etude-rename-20260716"),
            Some("loopflow.db")
        );
        assert_eq!(backup_database("lfd.db.bak-20260704"), Some("lfd.db"));
        assert_eq!(backup_database("loopflow.db-wal"), None);
        assert_eq!(backup_database("loopflow.db"), None);
    }

    #[test]
    fn rotation_removes_expired_material_and_keeps_the_store() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        std::env::set_var("LF_HOME", home);
        write(&home.join("loopflow.db"), 400_000, 30);
        write(&home.join("loopflow.db-wal"), 100_000, 30);
        write(
            &home.join("accounts/codex/a/sessions/old.jsonl"),
            300_000,
            60,
        );
        write(
            &home.join("loopflow.db.backup-0.14.7-0123456789abcdef"),
            200_000,
            20,
        );
        write(
            &home.join("loopflow.db.backup-0.14.7-0123456789abcdef-shm"),
            4096,
            20,
        );
        write(
            &home.join("loopflow.db.backup-0.14.8-fedcba9876543210"),
            200_000,
            10,
        );
        write(&home.join("runs/ab/oldest/events.jsonl"), 100_000, 40);
        write(&home.join("runs/cd/older/events.jsonl"), 100_000, 15);
        write(&home.join("runs/ef/fresh/events.jsonl"), 100_000, 0);
        write(&home.join("logs/old.log"), 50_000, 5);

        let report = rotate_home(home, Duration::from_secs(18 * 86400), false)
            .unwrap()
            .unwrap();

        assert_eq!(
            report
                .removed
                .iter()
                .map(|removal| removal.category)
                .collect::<Vec<_>>(),
            vec![Category::Captures, Category::Backups]
        );
        assert!(!home.join("runs/ab/oldest").exists());
        assert!(!home
            .join("loopflow.db.backup-0.14.7-0123456789abcdef")
            .exists());
        assert!(!home
            .join("loopflow.db.backup-0.14.7-0123456789abcdef-shm")
            .exists());
        for kept in [
            "loopflow.db",
            "loopflow.db-wal",
            "accounts/codex/a/sessions/old.jsonl",
            "loopflow.db.backup-0.14.8-fedcba9876543210",
            "runs/cd/older/events.jsonl",
            "runs/ef/fresh/events.jsonl",
            "logs/old.log",
        ] {
            assert!(home.join(kept).exists(), "{kept} was removed");
        }
        let log = fs::read_to_string(home.join("logs/disk.log")).unwrap();
        assert!(log.contains("removed captures"), "{log}");
    }

    #[test]
    fn expiry_spares_recent_open_and_newest_backup_material() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        std::env::set_var("LF_HOME", home);
        write(&home.join("loopflow.db"), 4096, 30);
        write(
            &home.join("loopflow.db.backup-0.14.8-fedcba9876543210"),
            4096,
            30,
        );
        write(&home.join("runs/ab/old/events.jsonl"), 4096, 30);
        write(&home.join("runs/cd/fresh/events.jsonl"), 4096, 0);
        write(&home.join("logs/old.log"), 4096, 30);
        write(&home.join("logs/held.log"), 4096, 30);
        let _writer = File::options()
            .append(true)
            .open(home.join("logs/held.log"))
            .unwrap();

        let report = rotate_home(home, Duration::from_secs(7 * 86400), false)
            .unwrap()
            .unwrap();

        assert_eq!(report.removed.len(), 2, "{report:#?}");
        assert!(home.join("logs/held.log").exists());
        assert!(!home.join("runs/ab/old").exists());
        assert!(!home.join("logs/old.log").exists());
        assert!(home.join("runs/cd/fresh").exists());
        assert!(home
            .join("loopflow.db.backup-0.14.8-fedcba9876543210")
            .exists());
    }

    #[test]
    fn a_killed_snapshot_staging_directory_goes_once_no_snapshot_holds_the_lock() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        std::env::set_var("LF_HOME", home);
        write(&home.join("loopflow.db"), 4096, 0);
        write(&home.join("copies/.tmpAbC123/loopflow.db"), 400_000, 0);
        write(&home.join("copies/0123456789abcdef/loopflow.db"), 4096, 0);

        let report = rotate_home(home, Duration::from_secs(7 * 86400), false)
            .unwrap()
            .unwrap();
        assert_eq!(report.removed.len(), 1, "{report:#?}");
        assert_eq!(report.removed[0].reason, "abandoned snapshot");
        assert!(report.freed_bytes >= 400_000, "{report:#?}");
        assert!(!home.join("copies/.tmpAbC123").exists());
        assert!(home.join("copies/0123456789abcdef/loopflow.db").exists());

        write(&home.join("copies/.tmpDeF456/loopflow.db"), 4096, 0);
        let _snapshotting = crate::store::copy::idle_lock(&home.join("copies"))
            .unwrap()
            .unwrap();
        let mut sweep = super::Sweep {
            dry_run: false,
            log: None,
            removed: Vec::new(),
            errors: Vec::new(),
        };
        super::remove_abandoned_snapshots(home, &mut sweep).unwrap();
        assert!(sweep.removed.is_empty() && sweep.errors.is_empty());
        assert!(home.join("copies/.tmpDeF456").exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_clone_measures_only_the_blocks_its_removal_frees() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original");
        let clone = dir.path().join("clone");
        write(&original, 4 << 20, 0);
        let from = CString::new(original.as_os_str().as_bytes()).unwrap();
        let to = CString::new(clone.as_os_str().as_bytes()).unwrap();
        // SAFETY: both paths are NUL-terminated and outlive the call.
        assert_eq!(unsafe { libc::clonefile(from.as_ptr(), to.as_ptr(), 0) }, 0);

        assert!(super::allocated_bytes(&clone) < 1 << 20);
    }
}
