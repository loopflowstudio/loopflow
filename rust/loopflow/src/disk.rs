//! Loopflow's footprint on this Machine's disk.
//!
//! Home (`~/.lf`) is measured by category and kept within `disk_budget` from the
//! Machine's `config.yaml`. Rotation removes the oldest backups, draft store
//! copies, Session captures and logs first. It never removes the database or its
//! sidecars, account homes, the newest backup of each database, anything modified
//! within the grace period, or anything a running process holds open. Every
//! removal is appended to `logs/disk.log`.

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// Home budget when `disk_budget` is unset.
pub const DEFAULT_BUDGET_BYTES: u64 = 20 * GIB;
/// Material modified this recently is never rotated.
pub const GRACE: Duration = Duration::from_secs(24 * 3600);
/// Automatic rotation runs at most this often.
const ROTATION_INTERVAL: Duration = Duration::from_secs(3600);
const ROTATION_STAMP: &str = "runtime/disk-rotation.stamp";
const LOCK: &str = "runtime/disk.lock";
const LOG: &str = "logs/disk.log";
const GIB: u64 = 1024 * 1024 * 1024;

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
    pub budget_bytes: u64,
    pub categories: Vec<CategoryUsage>,
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

// ---------------------------------------------------------------------------
// Budget

/// Parse `20G`, `512M`, `1.5T` or a plain byte count (powers of 1024).
pub fn parse_size(text: &str) -> Result<u64> {
    let text = text.trim();
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: f64 = number
        .parse()
        .with_context(|| format!("invalid size {text:?}"))?;
    let scale = match unit
        .trim()
        .to_ascii_uppercase()
        .trim_end_matches(['B', 'I'])
    {
        "" => 1u64,
        "K" => 1024,
        "M" => 1024 * 1024,
        "G" => GIB,
        "T" => 1024 * GIB,
        _ => bail!("invalid size unit in {text:?}; use K, M, G or T"),
    };
    Ok((number * scale as f64) as u64)
}

/// The Machine's Home budget: `disk_budget` in `config.yaml`, else the default.
pub fn budget_bytes() -> Result<u64> {
    let config = crate::config::load_global_config().map_err(|error| anyhow::anyhow!("{error}"))?;
    match config.and_then(|config| config.disk_budget) {
        Some(text) => parse_size(&text).context("disk_budget in config.yaml"),
        None => Ok(DEFAULT_BUDGET_BYTES),
    }
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
    let mut bytes = if seen.insert((metadata.dev(), metadata.ino())) {
        metadata.blocks() * 512
    } else {
        0
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

fn categorize(home: &Path, name: &str) -> Category {
    let database = crate::store::default_db_path();
    let database_name = database
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("loopflow.db");
    let base = name
        .strip_suffix("-wal")
        .or_else(|| name.strip_suffix("-shm"))
        .unwrap_or(name);
    if database.parent() == Some(home) && base == database_name {
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
pub fn measure_home(home: &Path, budget_bytes: u64) -> Result<HomeUsage> {
    let mut totals: BTreeMap<Category, u64> = BTreeMap::new();
    let mut seen = HashSet::new();
    for entry in fs::read_dir(home).with_context(|| format!("read {}", home.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        let category = categorize(home, &name.to_string_lossy());
        *totals.entry(category).or_default() += allocated(&entry.path(), &mut seen);
    }
    let categories: Vec<_> = totals
        .into_iter()
        .map(|(category, bytes)| CategoryUsage { category, bytes })
        .collect();
    Ok(HomeUsage {
        home: home.to_path_buf(),
        bytes: categories.iter().map(|usage| usage.bytes).sum(),
        budget_bytes,
        categories,
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

    let selected_copy = crate::store::default_db_path();
    for copy in children(&home.join("copies")) {
        if copy.is_dir() && !selected_copy.starts_with(&copy) {
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

/// Remove rotatable units, oldest first. `select` decides whether a unit is
/// still wanted given the bytes freed so far.
fn rotate(
    home: &Path,
    dry_run: bool,
    mut select: impl FnMut(&Unit, u64) -> Option<String>,
) -> Result<(Vec<Removal>, Vec<String>)> {
    let mut units = rotatable_units(home);
    units.sort_by_key(|unit| unit.modified);
    let cutoff = SystemTime::now() - GRACE;
    let mut open: Option<HashSet<PathBuf>> = None;
    let mut removed = Vec::new();
    let mut errors = Vec::new();
    let mut freed = 0;
    let mut log = (!dry_run).then(|| open_log(home)).transpose()?;
    for unit in units {
        if unit.modified > cutoff {
            continue;
        }
        let Some(reason) = select(&unit, freed) else {
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
        if !dry_run {
            if let Err(error) = remove_paths(&unit.paths) {
                errors.push(format!("{}: {error:#}", unit.paths[0].display()));
                continue;
            }
        }
        freed += unit.bytes;
        let removal = Removal {
            category: unit.category,
            path: unit.paths[0].clone(),
            bytes: unit.bytes,
            reason,
        };
        if let Some(log) = log.as_mut() {
            log_removal(log, &removal);
        }
        removed.push(removal);
    }
    Ok((removed, errors))
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

/// Rotate Home until it fits `budget_bytes`, or remove everything rotatable older
/// than `older_than` when set. Returns `None` when another rotation is running.
pub fn rotate_home(
    home: &Path,
    budget_bytes: u64,
    older_than: Option<Duration>,
    dry_run: bool,
) -> Result<Option<RotationReport>> {
    let Some(_lock) = try_lock(home)? else {
        return Ok(None);
    };
    let before = measure_home(home, budget_bytes)?;
    let excess = before.bytes.saturating_sub(budget_bytes);
    let expired = older_than.map(|age| SystemTime::now() - age);
    let (removed, errors) = rotate(home, dry_run, |unit, freed| {
        if expired.is_some_and(|expired| unit.modified < expired) {
            Some(format!(
                "older than {} days",
                older_than.expect("expiry has an age").as_secs() / 86400
            ))
        } else if freed < excess {
            Some(format!(
                "Home over its {} budget",
                format_bytes(budget_bytes)
            ))
        } else {
            None
        }
    })?;
    let freed_bytes = removed.iter().map(|removal| removal.bytes).sum();
    let after_bytes = if dry_run {
        before.bytes.saturating_sub(freed_bytes)
    } else {
        measure_home(home, budget_bytes)?.bytes
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

/// Start a detached `lf disk rotate` when the last one began over an hour ago.
/// Called as Loopflow produces new captures, so Home stays within budget
/// without anyone running a cleanup.
pub(crate) fn rotate_when_due(home: &Path) {
    // Unit tests capture into scratch homes; a detached lf must not follow them.
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
    let started = (|| -> Result<()> {
        fs::create_dir_all(stamp.parent().expect("stamp has a directory"))?;
        File::create(&stamp)?;
        let mut command = Command::new(crate::os_process::resolve_lf_binary()?);
        command
            .args(["disk", "rotate", "--quiet"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let mut child = command.spawn()?;
        std::thread::spawn(move || child.wait());
        Ok(())
    })();
    if let Err(error) = started {
        tracing::warn!(error = %format!("{error:#}"), "automatic Home rotation did not start");
    }
}

#[cfg(test)]
mod tests {
    use super::{backup_database, parse_size, rotate_home, Category, GIB};
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
    fn sizes_parse_with_binary_units() {
        assert_eq!(parse_size("20G").unwrap(), 20 * GIB);
        assert_eq!(parse_size("512M").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_size("1.5GiB").unwrap(), 3 * GIB / 2);
        assert_eq!(parse_size("4096").unwrap(), 4096);
        assert!(parse_size("lots").is_err());
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
    fn rotation_removes_oldest_material_until_home_fits_and_keeps_the_store() {
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

        let total = super::measure_home(home, 0).unwrap().bytes;
        // Room for everything except roughly the oldest capture and old backup.
        let budget = total - 250_000;
        let report = rotate_home(home, budget, None, false).unwrap().unwrap();

        assert!(report.after_bytes <= budget, "{report:#?}");
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

        let report = rotate_home(home, u64::MAX, Some(Duration::from_secs(7 * 86400)), false)
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
}
