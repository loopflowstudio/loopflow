//! `lf disk`: Loopflow's footprint on this Machine, and reclaiming it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use clap::Subcommand;
use serde::Serialize;

use crate::disk::{format_bytes, Category, HomeUsage, RotationReport};
use crate::git::build_cleanup::{prune_builds, BuildCleanupPolicy, ROOTS};
use crate::git::worktrees::{list_porcelain, main_repo_root};
use crate::store::sqlite::{read_task_worktree_ownership, TaskWorktreeOwnership};

#[derive(Debug, Subcommand)]
pub enum DiskCommand {
    /// Reclaim finished worktrees and build output, dead receipts, old backups, store copies, captures and logs
    Clean {
        /// Show what would be removed without removing anything
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
        /// Keep captures and logs modified within this many days
        #[arg(long, default_value_t = 7)]
        keep_days: u64,
    },
}

#[derive(Debug, Serialize)]
pub struct WorktreeUsage {
    pub path: PathBuf,
    pub repo: PathBuf,
    /// Owning Task, when a registry records one.
    pub task: Option<String>,
    /// `unfinished`, `completed`, `abandoned` or `untracked`.
    pub state: &'static str,
    pub checkout_bytes: u64,
    pub build_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct Footprint {
    pub home: HomeUsage,
    pub worktrees: Vec<WorktreeUsage>,
    pub total_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct WorktreeRemoval {
    pub path: PathBuf,
    pub reason: String,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct CleanReport {
    pub dry_run: bool,
    pub worktrees: Vec<WorktreeRemoval>,
    pub build_output: Vec<WorktreeRemoval>,
    pub process_receipts: u32,
    pub store: Option<crate::store::sqlite::retention::RetentionReport>,
    pub home: Option<RotationReport>,
    pub freed_bytes: u64,
    pub errors: Vec<String>,
}

pub fn run(json: bool, command: Option<&DiskCommand>) -> Result<()> {
    match command {
        None => footprint(json),
        Some(DiskCommand::Clean {
            dry_run,
            json,
            keep_days,
        }) => clean(*dry_run, *json, *keep_days),
    }
}

// ---------------------------------------------------------------------------
// Footprint

/// Every Task worktree a registry records, first registry winning.
fn task_worktrees() -> BTreeMap<PathBuf, TaskWorktreeOwnership> {
    let mut owners = BTreeMap::new();
    for registry in crate::lf::commands::ops::build_cleanup_registries() {
        for owner in read_task_worktree_ownership(&registry).unwrap_or_default() {
            owners.entry(owner.path.clone()).or_insert(owner);
        }
    }
    owners
}

/// Each Task repository's main checkout, plus the repository at the cwd.
fn repositories(owners: &BTreeMap<PathBuf, TaskWorktreeOwnership>) -> BTreeSet<PathBuf> {
    let current = std::env::current_dir().ok();
    owners
        .keys()
        .chain(current.as_ref())
        .filter_map(|path| main_repo_root(path).ok())
        .collect()
}

fn worktree_usage(
    repositories: &BTreeSet<PathBuf>,
    owners: &BTreeMap<PathBuf, TaskWorktreeOwnership>,
) -> Vec<WorktreeUsage> {
    let worktrees: Vec<(PathBuf, PathBuf)> = repositories
        .iter()
        .flat_map(|repo| {
            list_porcelain(repo)
                .unwrap_or_default()
                .into_iter()
                .filter(move |(path, _)| path != repo)
                .map(|(path, _)| (repo.clone(), path))
        })
        .collect();
    // Walking build output dominates; measure checkouts concurrently.
    std::thread::scope(|scope| {
        let handles: Vec<_> = worktrees
            .iter()
            .map(|(repo, path)| {
                let owner = owners.get(path);
                scope.spawn(move || WorktreeUsage {
                    path: path.clone(),
                    repo: repo.clone(),
                    task: owner.map(|owner| owner.task.clone()),
                    state: match owner {
                        None => "untracked",
                        Some(owner) if owner.unfinished => "unfinished",
                        Some(owner) if owner.completed => "completed",
                        Some(_) => "abandoned",
                    },
                    checkout_bytes: crate::disk::allocated_bytes(path),
                    build_bytes: ROOTS
                        .iter()
                        .map(|root| crate::disk::allocated_bytes(&path.join(root)))
                        .sum(),
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("worktree measurement panicked"))
            .collect()
    })
}

fn footprint(json: bool) -> Result<()> {
    let home = crate::store::lf_home_dir();
    let home = crate::disk::measure_home(&home)?;
    let owners = task_worktrees();
    let mut worktrees = worktree_usage(&repositories(&owners), &owners);
    worktrees.sort_by_key(|worktree| std::cmp::Reverse(worktree.checkout_bytes));
    let total_bytes = home.bytes + worktrees.iter().map(|w| w.checkout_bytes).sum::<u64>();
    let footprint = Footprint {
        home,
        worktrees,
        total_bytes,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&footprint)?);
    } else {
        print!("{}", render_footprint(&footprint));
    }
    Ok(())
}

fn render_footprint(footprint: &Footprint) -> String {
    let home = &footprint.home;
    let days = crate::disk::RETENTION.as_secs() / 86400;
    let mut out = format!(
        "Home {}  {}\n",
        home.home.display(),
        format_bytes(home.bytes)
    );
    let mut categories = home.categories.clone();
    categories.sort_by_key(|usage| std::cmp::Reverse(usage.bytes));
    for usage in &categories {
        out.push_str(&format!(
            "  {:>8}  {}{}\n",
            format_bytes(usage.bytes),
            usage.category.as_str(),
            match usage.category {
                Category::Database | Category::Accounts => "  (never rotated)".to_owned(),
                Category::Backups =>
                    format!("  (newest per database kept; others removed after {days} days)"),
                Category::Copies | Category::Captures | Category::Logs =>
                    format!("  (removed after {days} days)"),
                Category::Runtime => "  (dead receipts cleaned)".to_owned(),
                Category::Other => String::new(),
            }
        ));
    }
    out.push_str(&format!("Process receipts  {}\n", home.process_receipts));
    let checkouts: u64 = footprint.worktrees.iter().map(|w| w.checkout_bytes).sum();
    let builds: u64 = footprint.worktrees.iter().map(|w| w.build_bytes).sum();
    out.push_str(&format!(
        "Worktrees  {} in {} checkouts, {} of it build output\n",
        format_bytes(checkouts),
        footprint.worktrees.len(),
        format_bytes(builds)
    ));
    for worktree in &footprint.worktrees {
        out.push_str(&format!(
            "  {:>8}  {:<10}  {}  (build {})\n",
            format_bytes(worktree.checkout_bytes),
            worktree.state,
            worktree.path.display(),
            format_bytes(worktree.build_bytes)
        ));
    }
    out.push_str(&format!("Total  {}\n", format_bytes(footprint.total_bytes)));
    out
}

// ---------------------------------------------------------------------------
// Clean

fn clean(dry_run: bool, json: bool, keep_days: u64) -> Result<()> {
    let home = crate::store::lf_home_dir();
    let mut report = CleanReport {
        dry_run,
        worktrees: Vec::new(),
        build_output: Vec::new(),
        process_receipts: 0,
        store: None,
        home: None,
        freed_bytes: 0,
        errors: Vec::new(),
    };
    let current = crate::lf::commands::util::find_repo_root().ok();
    let registries = crate::lf::commands::ops::build_cleanup_registries();

    for repo in repositories(&task_worktrees()) {
        let current = current.clone().unwrap_or_else(|| repo.clone());
        // Measure before removal; a removed checkout cannot be measured.
        let worktrees = (|| -> Result<Vec<WorktreeRemoval>> {
            let candidates =
                crate::lf::commands::ops::prune_repo_worktrees(&repo, &current, true)?.candidates;
            let sizes: BTreeMap<PathBuf, u64> = candidates
                .iter()
                .map(|target| {
                    (
                        target.path.clone(),
                        crate::disk::allocated_bytes(&target.path),
                    )
                })
                .collect();
            let pruned = if dry_run {
                candidates
            } else {
                let pruned =
                    crate::lf::commands::ops::prune_repo_worktrees(&repo, &current, false)?;
                for failure in pruned.failed {
                    report.errors.push(format!(
                        "{}: {}",
                        failure.target.path.display(),
                        failure.error
                    ));
                }
                pruned.removed
            };
            Ok(pruned
                .into_iter()
                .map(|target| WorktreeRemoval {
                    bytes: sizes.get(&target.path).copied().unwrap_or(0),
                    reason: target.reason.as_str().to_owned(),
                    path: target.path,
                })
                .collect())
        })();
        match worktrees {
            Ok(worktrees) => report.worktrees.extend(worktrees),
            Err(error) => report
                .errors
                .push(format!("worktrees of {}: {error:#}", repo.display())),
        }

        let policy = BuildCleanupPolicy {
            retention: crate::disk::GRACE,
            maximum_roots: usize::MAX,
            maximum_bytes: u64::MAX,
        };
        match prune_builds(&repo, &current, &registries, policy, dry_run) {
            Ok(builds) => report.build_output.extend(
                builds
                    .entries
                    .into_iter()
                    .filter(|entry| entry.status == "removed" || entry.status == "candidate")
                    .map(|entry| WorktreeRemoval {
                        path: entry.path,
                        reason: entry.reason,
                        bytes: entry.allocated_bytes.unwrap_or(0),
                    }),
            ),
            Err(error) => report
                .errors
                .push(format!("build output of {}: {error:#}", repo.display())),
        }
    }

    match super::top::prune_dead_process_receipts(&home, dry_run) {
        Ok(count) => report.process_receipts = count,
        Err(error) => report.errors.push(format!("process receipts: {error:#}")),
    }

    if !dry_run {
        match crate::disk::prune_store() {
            Ok(store) => report.store = Some(store),
            Err(error) => report.errors.push(format!("store: {error:#}")),
        }
    }

    let keep = Duration::from_secs(keep_days.saturating_mul(86400)).max(crate::disk::GRACE);
    match crate::disk::rotate_home(&home, keep, dry_run) {
        Ok(Some(rotation)) => report.home = Some(rotation),
        Ok(None) => report
            .errors
            .push("another Home rotation is running".to_owned()),
        Err(error) => report.errors.push(format!("Home: {error:#}")),
    }

    report.freed_bytes = report
        .worktrees
        .iter()
        .chain(&report.build_output)
        .map(|removal| removal.bytes)
        .sum::<u64>()
        + report.home.as_ref().map_or(0, |home| home.freed_bytes);
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", render_clean(&report));
    }
    if report.errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "disk clean finished with {} error(s)",
            report.errors.len()
        ))
    }
}

fn verb(dry_run: bool) -> &'static str {
    if dry_run {
        "would remove"
    } else {
        "removed"
    }
}

/// One removal, in the same shape `logs/disk.log` records it.
fn removal_line(dry_run: bool, kind: &str, bytes: u64, path: &Path, reason: &str) -> String {
    format!(
        "{} {kind} {} {} ({reason})\n",
        verb(dry_run),
        format_bytes(bytes),
        path.display()
    )
}

fn render_clean(report: &CleanReport) -> String {
    let mut out = String::new();
    for (kind, removals) in [
        ("worktree", &report.worktrees),
        ("build", &report.build_output),
    ] {
        for removal in removals {
            out.push_str(&removal_line(
                report.dry_run,
                kind,
                removal.bytes,
                &removal.path,
                &removal.reason,
            ));
        }
    }
    if report.process_receipts > 0 {
        out.push_str(&format!(
            "{} {} dead process receipts\n",
            verb(report.dry_run),
            report.process_receipts
        ));
    }
    if let Some(store) = report.store.as_ref().filter(|store| {
        store.captures + store.usage_rows + store.closed_processes + store.processes > 0
    }) {
        out.push_str(&format!(
            "pruned {} stream rows from {} captures, {} usage rows and {} inspection Processes from the store; closed {} dead Processes{}\n",
            store.stream_rows,
            store.captures,
            store.usage_rows,
            store.processes,
            store.closed_processes,
            if store.vacuumed { "; vacuumed" } else { "" }
        ));
    }
    if let Some(home) = &report.home {
        out.push_str(&render_rotation(home));
    }
    for error in &report.errors {
        out.push_str(&format!("error: {error}\n"));
    }
    out.push_str(&format!(
        "{} {}\n",
        if report.dry_run {
            "Would free"
        } else {
            "Freed"
        },
        format_bytes(report.freed_bytes)
    ));
    out
}

fn render_rotation(report: &RotationReport) -> String {
    let mut out = String::new();
    for removal in &report.removed {
        out.push_str(&removal_line(
            report.dry_run,
            removal.category.as_str(),
            removal.bytes,
            &removal.path,
            &removal.reason,
        ));
    }
    for error in &report.errors {
        out.push_str(&format!("error: {error}\n"));
    }
    out.push_str(&format!(
        "Home {} -> {}\n",
        format_bytes(report.before.bytes),
        format_bytes(report.after_bytes)
    ));
    out
}
