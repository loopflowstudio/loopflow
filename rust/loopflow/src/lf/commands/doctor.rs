//! Diagnose current storage, execution records and the installation.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::lf::output::Colors;
use crate::process::LfProcess;
use crate::store::contention::read_contention;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}

impl Check {
    pub(crate) fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Ok,
            detail: detail.into(),
        }
    }
    pub(crate) fn warn(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Warn,
            detail: detail.into(),
        }
    }
    fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Fail,
            detail: detail.into(),
        }
    }
}

/// Exits non-zero when any check fails, so a caller can gate on it.
/// What `lf doctor --json` prints.
#[derive(Debug, serde::Serialize)]
pub struct DoctorReport {
    store: StoreReport,
    rows: usize,
    checks: Vec<Check>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct StoreReport {
    build_provenance: crate::build_info::BuildProvenance,
    build_label: Option<String>,
    unmerged_migrations: Vec<&'static str>,
    build_source_identity: String,
    build_source_root: Option<String>,
    build_source_revision: String,
    database_path: String,
    latest_known_migration: String,
    latest_applied_migration: Option<String>,
    migration_error: Option<String>,
}

pub fn run(json: bool) -> Result<()> {
    let report = report()?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print_checks(&report.store, &report.checks, report.rows);
    }
    if report
        .checks
        .iter()
        .any(|check| check.status == Status::Fail)
    {
        return Err(anyhow!("doctor checks failed"));
    }
    Ok(())
}

/// The store, Process evidence and installation checks `lf doctor` reports.
pub fn report() -> Result<DoctorReport> {
    let database_path = crate::store::database_path_from_env()?;
    let store_report = inspect_store(&database_path);
    let mut checks = vec![match &store_report.migration_error {
        Some(error) => Check::fail("store", error.clone()),
        None => Check::ok("store", "selected database is compatible with this build"),
    }];
    // The remedy hint counts exactly what `lf monitor prune` would repair.
    let events = match crate::store::sqlite::SqliteStore::open_processes_read_only(&database_path)
        .and_then(|store| {
            Ok((
                store.processes_since(0)?,
                store.unrecorded_agent_ancestry()?.len(),
            ))
        }) {
        Ok(events) => Some(events),
        Err(error) => {
            checks.push(Check::fail(
                "processes",
                format!("cannot read Process evidence: {error}"),
            ));
            None
        }
    };
    if let Some((events, repairable)) = &events {
        checks.extend([
            check_attribution(events),
            check_identity(events),
            check_lineage(events, *repairable),
        ]);
    }
    let rows = events.as_ref().map_or(0, |(events, _)| events.len());
    // Binary freshness remains useful when the store cannot open.
    checks.extend(check_installation(&database_path));
    checks.push(check_running_binaries());
    checks.push(check_binary_freshness());
    checks.push(check_contention(
        &crate::diagnostic_log::log_path(),
        time::OffsetDateTime::now_utc().unix_timestamp() - CONTENTION_WINDOW_SECONDS,
    ));
    checks.push(crate::desktop_exits::check());
    Ok(DoctorReport {
        store: store_report,
        rows,
        checks,
    })
}

const RUNNING: &str = "lf-binaries";

/// Name live processes still running a binary the install has moved past.
fn check_running_binaries() -> Check {
    let inventory = match super::rollover::inventory() {
        Ok(inventory) => inventory,
        Err(error) => {
            return Check::warn(RUNNING, format!("cannot list live lf processes: {error:#}"))
        }
    };
    match super::rollover::superseded_summary(&inventory) {
        Some(summary) => Check::warn(
            RUNNING,
            format!("{summary}. `lf monitor rollover` moves the resumable ones"),
        ),
        None => {
            let count = inventory
                .binaries
                .iter()
                .map(|binary| binary.processes.len())
                .sum::<usize>();
            let selected = inventory
                .binaries
                .first()
                .map(super::rollover::RunningBinary::describe)
                .unwrap_or_else(|| "the selected lf".into());
            Check::ok(
                RUNNING,
                format!("{count} live lf process(es) run {selected}"),
            )
        }
    }
}

const CONTENTION_WINDOW_SECONDS: i64 = 86_400;

/// Who waited on the store's write lock and who held it, by binary, from lf.log.
fn check_contention(log: &Path, since: i64) -> Check {
    const NAME: &str = "contention";
    let binaries = match read_contention(log, since) {
        Ok(binaries) => binaries,
        Err(error) => return Check::warn(NAME, format!("cannot read {}: {error}", log.display())),
    };
    if binaries.is_empty() {
        return Check::ok(
            NAME,
            "no store lock failures or slow writers in the last 24h",
        );
    }
    let failures: usize = binaries.values().map(|binary| binary.lock_failures).sum();
    let slow: usize = binaries.values().map(|binary| binary.slow_writes).sum();
    let mut detail = format!("last 24h: {failures} lock failures, {slow} slow writers");
    for (binary, contention) in &binaries {
        let mut parts = Vec::new();
        if contention.slow_writes > 0 {
            parts.push(format!(
                "held the lock {} times, up to {:.1} s, in {}",
                contention.slow_writes,
                contention.longest_hold_seconds,
                contention
                    .call_sites
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if contention.lock_failures > 0 {
            parts.push(format!("{} lock failures", contention.lock_failures));
        }
        if !contention.pids.is_empty() {
            let pids: Vec<String> = contention.pids.iter().map(u32::to_string).collect();
            parts.push(format!("pids {}", pids.join(" ")));
        }
        detail.push_str(&format!("\n{:20}{binary}: {}", "", parts.join("; ")));
    }
    Check::warn(NAME, detail)
}

const FRESHNESS: &str = "binary-freshness";
const UPSTREAM: &str = "origin/main";

/// Report whether the running binary predates merged upstream work.
fn check_binary_freshness() -> Check {
    let revision = crate::build_info::source_revision();
    let Some(repo) = freshness_repo() else {
        return Check::warn(
            FRESHNESS,
            format!(
                "cannot compare build revision {}: no git checkout at this binary's source root \
                 or working directory. Run `lf doctor` from a loopflow checkout to learn whether \
                 the running binary is current",
                crate::build_info::short_revision(revision)
            ),
        );
    };

    match crate::build_info::classify_revision(revision, &repo, UPSTREAM) {
        crate::build_info::BuildFreshness::Current { revision } => Check::ok(
            FRESHNESS,
            format!(
                "running lf is built from {}, current with cached {UPSTREAM} (not refreshed)",
                crate::build_info::short_revision(&revision)
            ),
        ),
        crate::build_info::BuildFreshness::Behind { revision, missing } => {
            let commits = missing
                .iter()
                .rev()
                .take(3)
                .map(|commit| {
                    format!(
                        "{} {}",
                        crate::build_info::short_revision(&commit.revision),
                        commit.subject
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            Check::warn(
                FRESHNESS,
                format!(
                    "running lf is built from {} and is {} merged commit(s) behind cached {UPSTREAM} (not refreshed); \
                     latest merged changes: {commits}. Run `lf install` to install the stable release",
                    crate::build_info::short_revision(&revision),
                    missing.len(),
                ),
            )
        }
        crate::build_info::BuildFreshness::OffMain { revision } => Check::warn(
            FRESHNESS,
            format!(
                "running lf is built from {}, which is not on cached {UPSTREAM}; release freshness is unproven",
                crate::build_info::short_revision(&revision)
            ),
        ),
        crate::build_info::BuildFreshness::Unprovable { reason } => Check::warn(
            FRESHNESS,
            format!("cannot prove whether the running lf is current: {reason}"),
        ),
    }
}

/// Prefer the build checkout, then walk up from the working directory.
fn freshness_repo() -> Option<std::path::PathBuf> {
    if let Some(root) = crate::build_info::source_root() {
        if root.join(".git").exists() {
            return Some(root.to_path_buf());
        }
    }
    let cwd = std::env::current_dir().ok()?;
    cwd.ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf)
}

fn inspect_store(path: &Path) -> StoreReport {
    let mut latest_applied_migration = None;
    let mut migration_error = None;
    if path.exists() {
        match rusqlite::Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(connection) => {
                match crate::store::migrations::latest_applied_version_sqlite(&connection) {
                    Ok(version) => latest_applied_migration = version,
                    Err(error) => migration_error = Some(error.to_string()),
                }
                // A private home migrates itself on its next open.
                let custom = crate::store::custom_home_selected();
                let validation = crate::store::migrations::validate_sqlite(&connection).and_then(|()| {
                    match crate::store::migrations::pending_shared_migration(&connection)? {
                        Some(pending) if !custom => Err(crate::store::StoreError::InvalidData(format!(
                            "selected database is missing {pending}; run `lf install` to finish installing this build"
                        ))),
                        _ => Ok(()),
                    }
                });
                if let Err(error) = validation {
                    migration_error.get_or_insert_with(|| error.to_string());
                }
            }
            Err(error) => migration_error = Some(error.to_string()),
        }
    }
    if !path.exists() {
        migration_error = Some(format!("selected database {} does not exist; run `lf self install` to initialize an installation", path.display()));
    }
    StoreReport {
        build_provenance: crate::build_info::provenance(),
        build_label: crate::build_info::label().map(str::to_string),
        unmerged_migrations: crate::store::migrations::unmerged_migrations(),
        build_source_identity: crate::build_info::source_identity(),
        build_source_root: crate::build_info::source_root().map(|root| root.display().to_string()),
        build_source_revision: crate::build_info::source_revision().to_string(),
        database_path: path.display().to_string(),
        latest_known_migration: crate::store::migrations::latest_known_version(),
        latest_applied_migration,
        migration_error,
    }
}

fn check_installation(database_path: &Path) -> Vec<Check> {
    let root = match crate::installation::root() {
        Ok(root) => root,
        Err(error) => {
            return vec![Check::fail(
                "install-selection",
                format!("cannot resolve installation authority: {error}"),
            )]
        }
    };
    match crate::installation::read_state(&root) {
        Ok(crate::installation::InstallationState::Legacy) => vec![
            Check::warn(
                "install-selection",
                "no versioned installation receipt; the next promotion will initialize it",
            ),
            Check::warn(
                "install-fallback",
                "published fallback has not been retained by the installation path yet",
            ),
        ],
        Ok(crate::installation::InstallationState::Switching(receipt)) => vec![Check::fail(
            "install-switch",
            format!(
                "install switch {} is unsettled at {:?}; ordinary startup falls back to the prior install — recover or rerun the promotion to finish it",
                receipt.id, receipt.phase
            ),
        )],
        Ok(crate::installation::InstallationState::Settled(active)) => {
            let selected = crate::installation::selection_for_current_executable();
            let selection = match &selected {
                Ok(Some(selection)) => Check::ok(
                    "install-selection",
                    format!(
                        "{} installation {} selects {}",
                        selection
                            .artifact_set
                            .build_label
                            .clone()
                            .unwrap_or_else(|| format!("{:?}", selection.source)),
                        selection.installation_id,
                        selection.store.display()
                    ),
                ),
                Ok(None) => Check::ok(
                    "install-selection",
                    "running source artifact is outside the pinned installation",
                ),
                Err(error) => Check::fail("install-selection", error.to_string()),
            };
            let store = if active.selection.store == database_path {
                Check::ok(
                    "install-store",
                    format!("running store matches {}", database_path.display()),
                )
            } else if crate::store::custom_home_selected() {
                Check::ok(
                    "install-store",
                    "explicit disposable Machine; no upgrade or recovery contract",
                )
            } else {
                Check::fail(
                    "install-store",
                    format!(
                        "receipt selects {}, running process opened {}",
                        active.selection.store.display(),
                        database_path.display()
                    ),
                )
            };
            let mut fallback_roles = vec![
                crate::installation::ArtifactRole::Cli,

            ];
            if active
                .selection
                .artifact_set
                .artifact(&crate::installation::ArtifactRole::App)
                .is_some()
            {
                fallback_roles.extend([
                    crate::installation::ArtifactRole::App,
                    crate::installation::ArtifactRole::AppHelper("lf".to_string()),

                ]);
            }
            let fallback = match active.published_fallback.verify(&fallback_roles) {
                Ok(()) => Check::ok(
                    "install-fallback",
                    format!(
                        "published fallback {} is complete and verified",
                        active.published_fallback.id
                    ),
                ),
                Err(error) => Check::fail("install-fallback", error.to_string()),
            };
            vec![selection, store, fallback]
        }
        Err(error) => vec![Check::fail("install-selection", error.to_string())],
    }
}

/// A process may name only one command, and its terminal row names that work.
fn check_attribution(events: &[LfProcess]) -> Check {
    let unnamed = events
        .iter()
        .filter(|process| process.completed_at.is_some() && process.command.is_none())
        .count();
    if unnamed == 0 {
        Check::ok("attribution", "every completed Process names its command")
    } else {
        Check::fail(
            "attribution",
            format!("{unnamed} completed Processes name no command"),
        )
    }
}

/// Processes may be machine-scoped; recorded repositories must be absolute.
fn check_identity(events: &[LfProcess]) -> Check {
    let repos: HashSet<&str> = events
        .iter()
        .filter_map(|event| event.repo.as_deref())
        .collect();
    let unscoped = events.iter().filter(|event| event.repo.is_none()).count();
    let invalid = repos
        .iter()
        .filter(|repo| !Path::new(repo).is_absolute())
        .count();
    if invalid == 0 {
        return Check::ok(
            "identity",
            format!(
                "{} repo value(s), all absolute; {unscoped} Process(s) without repository scope",
                repos.len()
            ),
        );
    }
    Check::fail(
        "identity",
        format!("{invalid}/{} repo value(s) are not absolute", repos.len()),
    )
}

pub(crate) fn check_lineage(events: &[LfProcess], repairable: usize) -> Check {
    let processes: HashMap<&str, &str> = events
        .iter()
        .map(|event| (event.id.as_str(), event.trace_id.as_str()))
        .collect();
    let dangling: HashSet<&str> = events
        .iter()
        .filter_map(|event| {
            let parent = event.parent_lf_process_id.as_ref()?.as_str();
            (processes.get(parent).copied() != Some(event.trace_id.as_str())).then_some(parent)
        })
        .collect();
    if dangling.is_empty() {
        return Check::ok("lineage", "every parent process resolves");
    }
    let mut detail = format!(
        "{} parent process id(s) are missing or belong to another trace",
        dangling.len()
    );
    if repairable > 0 {
        detail.push_str(&format!(
            "; {repairable} AgentProcess parent(s) were never recorded, and `lf monitor prune` repairs them"
        ));
    }
    Check::fail("lineage", detail)
}

fn print_checks(store: &StoreReport, checks: &[Check], rows: usize) {
    let colors = Colors::default();
    let build = store
        .build_label
        .clone()
        .unwrap_or_else(|| store.build_provenance.to_string());
    println!("build: {build} ({})", store.build_source_identity);
    if !store.unmerged_migrations.is_empty() {
        println!(
            "unmerged migrations: {}",
            store.unmerged_migrations.join(", ")
        );
    }
    println!("revision: {}", store.build_source_revision);
    if let Some(root) = &store.build_source_root {
        println!("source: {root}");
    }
    println!("database: {}", store.database_path);
    println!(
        "migrations: applied {} / known {}",
        store.latest_applied_migration.as_deref().unwrap_or("none"),
        store.latest_known_migration
    );
    if let Some(error) = &store.migration_error {
        println!("migration error: {error}");
    }
    println!("ledger: {rows} Processes\n");
    for check in checks {
        let (mark, color) = match check.status {
            Status::Ok => ("ok  ", colors.green),
            Status::Warn => ("warn", colors.yellow),
            Status::Fail => ("FAIL", colors.red),
        };
        println!(
            "{color}{mark}{reset}  {bold}{name:<13}{reset} {detail}",
            color = color,
            reset = colors.reset,
            bold = colors.bold,
            mark = mark,
            name = check.name,
            detail = check.detail,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        check_attribution, check_contention, check_identity, check_lineage, inspect_store, Status,
    };
    use crate::process::LfProcess;

    const DAY: i64 = 86_400;

    #[test]
    fn store_report_exposes_unknown_applied_migration() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        crate::store::migrations::apply_sqlite(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at)
                 VALUES ('9.0.001_divergent', unixepoch() + 1)",
                [],
            )
            .unwrap();
        drop(connection);

        let report = inspect_store(&path);

        assert_eq!(
            report.latest_applied_migration.as_deref(),
            Some("9.0.001_divergent")
        );
        let error = report.migration_error.unwrap();
        assert!(error.contains("9.0.001_divergent"), "{error}");
        assert!(error.contains("latest known"), "{error}");
    }

    fn row(ts: i64, event: &str) -> LfProcess {
        LfProcess {
            kind: crate::process::ProcessKind::Lf,
            agent_session_id: None,
            os_started_at: None,
            id: crate::id::LfProcessId::new(),
            pid: None,
            trace_id: crate::id::TraceId::new(),
            parent_lf_process_id: None,
            via_agent: Some(false),
            caller_session_id: None,
            caller_agent_process_id: None,
            command: None,
            repo: Some("/src/loopflow".into()),
            cwd: None,
            started_at: ts,
            completed_at: (event != "started").then_some(ts),
            outcome: (event != "started").then(|| "succeeded".into()),
            exit_code: None,
            signal: None,
            error: None,
        }
    }

    fn named(mut row: LfProcess, command: &str) -> LfProcess {
        row.command = Some(command.to_string());
        row
    }

    fn status_of(rows: &[LfProcess], name: &str) -> Status {
        [
            check_attribution(rows),
            check_identity(rows),
            check_lineage(rows, 0),
        ]
        .into_iter()
        .find(|check| check.name == name)
        .expect("check exists")
        .status
    }

    #[test]
    fn contention_names_the_holding_binary_and_a_quiet_log_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("lf.log");
        assert_eq!(check_contention(&log, 0).status, Status::Ok);
        std::fs::write(
            &log,
            "2026-10-27T04:19:02.000000Z  WARN loopflow::store::contention: held the store write lock \
             held_seconds=9.12 call_site=loopflow::store::sqlite::retention::retain_history_in \
             pid=7 lf=0.14.16 rev=d49c57c12\n",
        )
        .unwrap();
        let check = check_contention(&log, 0);
        assert_eq!(check.status, Status::Warn);
        assert!(
            check.detail.contains("lf 0.14.16 (d49c57c12)"),
            "{}",
            check.detail
        );
        assert!(
            check.detail.contains("retain_history_in"),
            "{}",
            check.detail
        );
    }

    #[test]
    fn a_terminal_row_that_names_its_work_attributes_cleanly() {
        let mut terminal = row(DAY, "completed");
        terminal.command = Some(r#"["lf","code"]"#.to_string());
        let rows = [named(row(DAY, "started"), r#"["lf","code"]"#), terminal];
        assert_eq!(status_of(&rows, "attribution"), Status::Ok);
    }

    #[test]
    fn machine_scoped_processes_have_valid_identity() {
        let mut event = named(row(DAY, "completed"), "lf help");
        event.repo = None;
        assert_eq!(status_of(&[event], "identity"), Status::Ok);
    }

    #[test]
    fn a_repo_basename_fails_identity() {
        let mut event = row(DAY, "completed");
        event.repo = Some("loopflow".to_string());
        assert_eq!(status_of(&[event], "identity"), Status::Fail);
    }
}
