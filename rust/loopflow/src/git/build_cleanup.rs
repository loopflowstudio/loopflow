//! Generated output only; completion is read from registries without migrations.
//! The worktree lease coordinates Loopflow operations. Direct package managers
//! newly racing cleanup in a completed checkout do not participate in that lease.
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::git::{acquire_worktree_lease, worktrees::is_persistent_worktree};
use crate::os_process::bounded_output;
use crate::store::sqlite::{read_task_worktree_ownership, SqliteStore};

#[cfg(test)]
thread_local! {
    static AFTER_SELECTION: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

pub(crate) const ROOTS: [&str; 6] = [
    "target",
    ".build",
    "swift/.build",
    "node_modules",
    "website/node_modules",
    ".venv",
];
/// lsof across a busy Machine and a walk of a large `target/` each take tens of seconds.
const INSPECTION_DEADLINE: Duration = Duration::from_secs(180);

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct BuildCleanupEntry {
    pub worktree: PathBuf,
    pub path: PathBuf,
    pub allocated_bytes: Option<u64>,
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct BuildCleanupReport {
    pub dry_run: bool,
    pub entries: Vec<BuildCleanupEntry>,
    pub removed_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BuildCleanupPolicy {
    pub retention: Duration,
    pub maximum_roots: usize,
    pub maximum_bytes: u64,
}

fn output(command: &mut Command) -> Result<Vec<u8>> {
    let output = bounded_output(command, INSPECTION_DEADLINE)
        .context("inspection failed or exceeded its deadline")?;
    if !output.status.success() {
        bail!("inspection failed");
    }
    if !output.stderr.is_empty() {
        bail!("inspection reported incomplete access");
    }
    Ok(output.stdout)
}

/// Include cwd, mapped executables/libraries and every open output file.
/// A failed or partial lsof observation is unknown, never an empty activity set.
pub(crate) fn running_paths() -> Result<HashSet<PathBuf>> {
    let bytes = output(Command::new("lsof").args(["-nP", "-Fn0"]))?;
    let text = std::str::from_utf8(&bytes).context("invalid process paths")?;
    let paths = text
        .split(['\0', '\n'])
        .filter_map(|field| field.strip_prefix('n'))
        .filter(|path| path.starts_with('/'))
        .map(|path| normalize(Path::new(path)))
        .collect::<HashSet<_>>();
    if paths.is_empty() {
        bail!("process inspection returned no paths");
    }
    Ok(paths)
}

fn normalize(path: &Path) -> PathBuf {
    // lsof uses /private/tmp on macOS while fixtures commonly use /tmp.
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn ownership(worktree: &Path, registries: &[PathBuf]) -> Result<()> {
    let mut completed = false;
    for registry in registries {
        let owners = read_task_worktree_ownership(registry)
            .with_context(|| format!("ownership unavailable from {}", registry.display()))?;
        let associated: Vec<_> = owners
            .into_iter()
            .filter(|owner| normalize(&owner.path).starts_with(worktree))
            .collect();
        if associated.iter().any(|owner| owner.unfinished) {
            bail!("unfinished Task owns this checkout");
        }
        if associated.is_empty() {
            continue;
        }
        let store = SqliteStore::open_processes_read_only(registry)
            .context("execution inspection unavailable")?;
        for owner in associated {
            let task = crate::durable::TaskId::parse(&owner.task)?;
            let blockers = crate::ops::task_execution::task_execution_blockers(&store, &task)
                .context("execution inspection unavailable")?;
            if !blockers.is_empty() {
                bail!("live or unresolved Task execution");
            }
            completed |= owner.completed && normalize(&owner.path) == worktree;
        }
    }
    if !completed {
        bail!("no positive completed local Task ownership");
    }
    Ok(())
}

fn inspect_tree(
    path: &Path,
    root: &Path,
    cutoff: SystemTime,
    deadline: Instant,
    seen: &mut HashSet<(u64, u64)>,
) -> Result<u64> {
    if Instant::now() >= deadline {
        bail!("output inspection timed out");
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.modified()? > cutoff {
        bail!("output is within the retention grace period");
    }
    if metadata.file_type().is_symlink() {
        if !fs::canonicalize(path)?.starts_with(root) {
            bail!("output contains a symlink escape");
        }
    } else if !metadata.is_dir() && !metadata.is_file() {
        bail!("output contains a special file");
    }
    // A real store/history placed under generated output is still durable state.
    if matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("loopflow.db" | "agent_sessions" | "runs" | ".lf" | ".git")
    ) {
        bail!("output contains retained Loopflow state");
    }
    let mut bytes = if seen.insert((metadata.dev(), metadata.ino())) {
        metadata.blocks() * 512
    } else {
        0
    };
    if metadata.is_dir() {
        for child in fs::read_dir(path)? {
            bytes = bytes
                .checked_add(inspect_tree(&child?.path(), root, cutoff, deadline, seen)?)
                .context("output allocation overflow")?;
        }
    }
    Ok(bytes)
}

fn inspect_candidate(
    repo: &Path,
    current: &Path,
    worktree: &Path,
    path: &Path,
    registries: &[PathBuf],
    policy: BuildCleanupPolicy,
) -> Result<u64> {
    if worktree == normalize(repo) || worktree == normalize(current) {
        bail!("primary or current checkout");
    }
    if is_persistent_worktree(worktree)? {
        bail!("persistent checkout");
    }
    ownership(worktree, registries)?;
    if fs::symlink_metadata(path)?.file_type().is_symlink() || normalize(path) != path {
        bail!("generated root is a symlink or escapes the checkout");
    }
    let relative = path.strip_prefix(worktree)?;
    let tracked = output(
        Command::new("git")
            .arg("-C")
            .arg(worktree)
            .args(["ls-files", "-z", "--"])
            .arg(relative),
    )?;
    if !tracked.is_empty() {
        bail!("output contains tracked files");
    }
    output(
        Command::new("git")
            .arg("-C")
            .arg(worktree)
            .args(["check-ignore", "--quiet", "--"])
            .arg(relative),
    )
    .context("output is not ignored generated content")?;
    for home in [
        crate::store::lf_home_dir(),
        crate::store::production_database_path()
            .parent()
            .context("production database has no Home")?
            .to_path_buf(),
    ] {
        if normalize(&home).starts_with(normalize(path)) {
            bail!("output contains a selected or installed Home");
        }
    }
    if running_paths()?
        .iter()
        .any(|active| active.starts_with(worktree))
    {
        bail!("completed candidate has a process cwd, executable mapping or open file; verify Task selection");
    }
    let cutoff = SystemTime::now()
        .checked_sub(policy.retention)
        .context("invalid retention period")?;
    inspect_tree(
        path,
        path,
        cutoff,
        Instant::now() + INSPECTION_DEADLINE,
        &mut HashSet::new(),
    )
}

pub(crate) fn prune_builds(
    repo: &Path,
    current: &Path,
    registries: &[PathBuf],
    policy: BuildCleanupPolicy,
    dry_run: bool,
) -> Result<BuildCleanupReport> {
    let mut report = BuildCleanupReport {
        dry_run,
        entries: Vec::new(),
        removed_bytes: 0,
    };
    // Read only Git registration: no remote lookup, main refresh or metadata prune.
    let registered = output(Command::new("git").arg("-C").arg(repo).args([
        "worktree",
        "list",
        "--porcelain",
        "-z",
    ]))?;
    let text = std::str::from_utf8(&registered)?;
    let mut roots_used = 0;
    let mut bytes_used: u64 = 0;
    for worktree in text
        .split('\0')
        .filter_map(|field| field.strip_prefix("worktree "))
    {
        let worktree = normalize(Path::new(worktree));
        for relative in ROOTS {
            let path = worktree.join(relative);
            match fs::symlink_metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                _ => {}
            }
            let mut entry = BuildCleanupEntry {
                worktree: worktree.clone(),
                path: path.clone(),
                allocated_bytes: None,
                status: "retained".into(),
                reason: String::new(),
            };
            let result = (|| -> Result<()> {
                let _lease = acquire_worktree_lease(repo, &worktree, "build-output cleanup")?;
                let mut bytes =
                    inspect_candidate(repo, current, &worktree, &path, registries, policy)?;
                entry.allocated_bytes = Some(bytes);
                if roots_used >= policy.maximum_roots
                    || bytes > policy.maximum_bytes.saturating_sub(bytes_used)
                {
                    bail!("cleanup budget exhausted");
                }
                entry.status = "candidate".into();
                entry.reason = "completed, idle, ignored output past retention grace".into();
                if !dry_run {
                    #[cfg(test)]
                    AFTER_SELECTION.with(|hook| {
                        if let Some(hook) = hook.borrow_mut().take() {
                            hook();
                        }
                    });
                    // Re-read completion, execution, content and process paths under
                    // the same lease immediately before the destructive operation.
                    bytes = inspect_candidate(repo, current, &worktree, &path, registries, policy)?;
                    entry.allocated_bytes = Some(bytes);
                    if bytes > policy.maximum_bytes.saturating_sub(bytes_used) {
                        bail!("cleanup budget exhausted");
                    }
                }
                roots_used += 1;
                bytes_used += bytes;
                if !dry_run {
                    if let Err(error) = fs::remove_dir_all(&path) {
                        entry.status = "failed".into();
                        return Err(error).context("failed to remove generated output");
                    }
                    report.removed_bytes += bytes;
                    entry.status = "removed".into();
                }
                Ok(())
            })();
            if let Err(error) = result {
                if entry.status != "failed" {
                    entry.status = "retained".into();
                }
                entry.reason = format!("{error:#}");
            }
            report.entries.push(entry);
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::{prune_builds, BuildCleanupPolicy, BuildCleanupReport, AFTER_SELECTION};
    use crate::git::{worktree_add, worktree_lease_path, WorktreeBranch};
    use crate::store::sqlite::SqliteStore;
    use loopflow_test_support::TestRepo;
    use rusqlite::{params, Connection};
    use std::fs::{self, File, FileTimes};
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, SystemTime};

    #[test]
    fn build_cleanup_receipt_has_required_fields_in_both_consumers() {
        let json = include_str!("../../../../tests/fixtures/dto/build_cleanup_report.json");
        let report: BuildCleanupReport = serde_json::from_str(json).unwrap();
        assert_eq!(
            serde_json::to_value(report).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }

    #[test]
    fn build_cleanup_retains_unreadable_ownership_without_changing_the_registry() {
        let repo = TestRepo::new();
        let temp = tempfile::tempdir().unwrap();
        let checkout = temp.path().canonicalize().unwrap().join("finished");
        worktree_add(
            repo.path(),
            &checkout,
            "finished",
            WorktreeBranch::New {
                start_point: "HEAD",
            },
        )
        .unwrap();
        fs::write(checkout.join(".gitignore"), "target/\n").unwrap();
        fs::create_dir(checkout.join("target")).unwrap();
        fs::write(checkout.join("target/artifact"), "retain output").unwrap();
        old(&checkout.join("target"));
        for name in ["missing", "incompatible"] {
            let database = temp.path().join(name);
            if name == "incompatible" {
                fs::write(&database, "retain unknown store bytes").unwrap();
            }
            let report = prune_builds(
                repo.path(),
                repo.path(),
                std::slice::from_ref(&database),
                BuildCleanupPolicy {
                    retention: Duration::from_secs(24 * 3600),
                    maximum_roots: 8,
                    maximum_bytes: 1024 * 1024,
                },
                false,
            )
            .unwrap();
            assert_eq!(report.removed_bytes, 0);
            assert_eq!(report.entries[0].status, "retained");
            assert!(report.entries[0].reason.contains("ownership unavailable"));
            assert_eq!(report.entries[0].allocated_bytes, None);
            assert_eq!(
                fs::read_to_string(checkout.join("target/artifact")).unwrap(),
                "retain output"
            );
            if name == "incompatible" {
                assert_eq!(
                    fs::read_to_string(database).unwrap(),
                    "retain unknown store bytes"
                );
            }
        }
    }

    fn old(path: &Path) {
        if path.is_dir() {
            for child in fs::read_dir(path).unwrap() {
                old(&child.unwrap().path());
            }
        }
        File::open(path)
            .unwrap()
            .set_times(
                FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(48 * 3600)),
            )
            .unwrap();
    }

    #[test]
    fn build_cleanup_rechecks_ownership_and_excludes_writer_after_selection() {
        let repo = TestRepo::new();
        let temp = tempfile::tempdir().unwrap();
        let checkout = temp.path().canonicalize().unwrap().join("finished");
        worktree_add(
            repo.path(),
            &checkout,
            "finished",
            WorktreeBranch::New {
                start_point: "HEAD",
            },
        )
        .unwrap();
        fs::write(checkout.join(".gitignore"), "target/\n").unwrap();
        fs::create_dir(checkout.join("target")).unwrap();
        fs::write(checkout.join("target/artifact"), "old output").unwrap();
        old(&checkout.join("target"));
        let db = temp.path().join("db");
        let store = SqliteStore::open_ephemeral(&db).unwrap();
        let task = crate::durable::TaskId::new();
        let wave = crate::id::WaveId::new();
        let project = crate::work::project::ProjectId::new();
        let conn = Connection::open(&db).unwrap();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof',?2,1)",
            params![wave, repo.path().to_str().unwrap()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO projects(id,wave_id,created_at) VALUES(?1,?2,1)",
            params![project.as_str(), wave],
        )
        .unwrap();
        conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
        conn.execute("INSERT INTO tasks(repository_id,id,project_id,issue_identifier,worktree,created_at,checkout_machine_id,planning_completed) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'PROOF-1',?3,1,(SELECT id FROM machines WHERE route='local'),1)", params![task.as_str(),project.as_str(),checkout.to_str().unwrap()]).unwrap();
        drop(conn);
        drop(store);
        let lock = worktree_lease_path(repo.path(), &checkout).unwrap();
        let marker = temp.path().join("writer-ran");
        let database = db.clone();
        let task_id = task.to_string();
        let writer_marker = marker.clone();
        AFTER_SELECTION.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move || {
                let code = r#"
import fcntl
import sys
from pathlib import Path

with open(sys.argv[1], "a") as lock:
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        sys.exit(23)
    Path(sys.argv[2]).write_text("writer acquired")
"#;
                let status = Command::new("uv")
                    .args(["run", "--no-sync", "python", "-c", code])
                    .arg(&lock)
                    .arg(&writer_marker)
                    .stdin(Stdio::null())
                    .status()
                    .unwrap();
                assert_eq!(status.code(), Some(23));
                Connection::open(database)
                    .unwrap()
                    .execute(
                        "UPDATE tasks SET planning_completed=0 WHERE id=?1",
                        [task_id],
                    )
                    .unwrap();
            }))
        });
        let report = prune_builds(
            repo.path(),
            repo.path(),
            &[db],
            BuildCleanupPolicy {
                retention: Duration::from_secs(24 * 3600),
                maximum_roots: 8,
                maximum_bytes: 1024 * 1024,
            },
            false,
        )
        .unwrap();
        assert_eq!(report.removed_bytes, 0);
        assert!(
            report.entries[0].reason.contains("unfinished"),
            "{:?}",
            report.entries
        );
        assert!(checkout.join("target/artifact").exists());
        assert!(!marker.exists());
    }
}
