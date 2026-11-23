//! Diagnose current storage, execution records and the installation.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{anyhow, Result};

use loopflow_execution::output::Colors;
use loopflow_execution::process::diagnosis::{Check, Status};
use loopflow_execution::process::OsProcess;
use loopflow_execution::store::contention::read_contention;

/// Exits non-zero when any check fails, so a caller can gate on it.
/// What `lf doctor --json` prints.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct DoctorReport {
    store: StoreReport,
    checks: Vec<Check>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
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
        print_checks(&report.store, &report.checks);
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

/// The store and installation checks `lf doctor` reports.
pub fn report() -> Result<DoctorReport> {
    let database_path = loopflow_execution::store::database_path_from_env()?;
    let store_report = inspect_store(&database_path);
    let mut checks = vec![match &store_report.migration_error {
        Some(error) => Check::fail("store", error.clone()),
        None => Check::ok("store", "selected database is compatible with this build"),
    }];
    // Binary freshness remains useful when the store cannot open.
    checks.extend(check_installation(&database_path));
    checks.push(check_history(
        &database_path,
        time::OffsetDateTime::now_utc().unix_timestamp(),
    ));
    checks.push(check_running_binaries());
    checks.push(check_strays(&database_path));
    checks.push(check_binary_freshness());
    checks.push(check_contention(
        &loopflow_execution::diagnostic_log::log_path(),
        time::OffsetDateTime::now_utc().unix_timestamp() - CONTENTION_WINDOW_SECONDS,
    ));
    checks.push(loopflow_execution::desktop_exits::check());
    checks.push(check_ptys());
    Ok(DoctorReport {
        store: store_report,
        checks,
    })
}

const RUNNING: &str = "lf-binaries";

/// Name live processes still running a binary the install has moved past.
fn check_running_binaries() -> Check {
    let inventory = match super::binaries::inventory() {
        Ok(inventory) => inventory,
        Err(error) => {
            return Check::warn(RUNNING, format!("cannot list live lf processes: {error:#}"))
        }
    };
    match super::binaries::superseded_summary(&inventory) {
        Some(summary) => Check::warn(
            RUNNING,
            format!("{summary}. Quit a conversation's agent and `lf resume` it to move it"),
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
                .map(super::binaries::RunningBinary::describe)
                .unwrap_or_else(|| "the selected lf".into());
            Check::ok(
                RUNNING,
                format!("{count} live lf process(es) run {selected}"),
            )
        }
    }
}

const PTYS: &str = "ptys";

/// PTYs in use against the Machine's limit. Each Task conversation's held
/// terminal takes one, as does every terminal tab; they run out first.
fn check_ptys() -> Check {
    match loopflow_execution::transport::pty_usage() {
        Some(usage) => pty_check(usage),
        None => Check::warn(PTYS, "cannot read this Machine's PTY limit"),
    }
}

/// Fewer PTYs left than this warns: a reboot starts every Task conversation again.
const PTY_HEADROOM: u64 = 32;

fn pty_check(usage: loopflow_execution::transport::PtyUsage) -> Check {
    use loopflow_execution::transport::PtyUsage;
    let PtyUsage { in_use, max, limit } = usage;
    let left = max.saturating_sub(in_use);
    let detail = format!(
        "{in_use} of {max} PTYs in use ({limit}), {left} left; each Task conversation holds one"
    );
    if left < PTY_HEADROOM.min(max / 10).max(1) {
        Check::warn(
            PTYS,
            format!("{detail}. A conversation that cannot get one says so in `lf task status`"),
        )
    } else {
        Check::ok(PTYS, detail)
    }
}

const STRAYS: &str = "strays";
/// What a Loopflow agent's processes inherit: its Session or the lf they run
/// under, and the home whose store claims them.
const STRAY_VARIABLES: [&str; 3] = ["LF_STEER_SESSION", "LF_PROCESS_ID", "LF_HOME"];

/// OS processes carrying a Loopflow identity that no live Session or run
/// claims. A descendant of a claimed process is that process's. Reports,
/// never kills.
fn check_strays(database: &Path) -> Check {
    match strays(database) {
        Ok(strays) if strays.is_empty() => Check::ok(
            STRAYS,
            "every process carrying a Loopflow identity belongs to a live Session or run",
        ),
        Ok(strays) => Check::warn(
            STRAYS,
            format!(
                "{} process(es) carry a Loopflow identity no live Session or run claims: {}",
                strays.len(),
                strays
                    .iter()
                    .take(10)
                    // The program alone: arguments can hold a prompt.
                    .map(|process| {
                        let program = process.command.split_whitespace().next().unwrap_or("");
                        let name = Path::new(program)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or(program);
                        format!("{} ({name})", process.pid)
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
        Err(error) => Check::warn(STRAYS, format!("cannot scan for strays: {error:#}")),
    }
}

fn strays(database: &Path) -> Result<Vec<OsProcess>> {
    let processes = OsProcess::sample(time::OffsetDateTime::now_utc().unix_timestamp())?;
    let parents: HashMap<u32, u32> = processes
        .iter()
        .map(|process| (process.pid, process.ppid))
        .collect();
    let ancestry = |pid: u32| {
        std::iter::successors(Some(pid), |pid| {
            parents
                .get(pid)
                .copied()
                .filter(|parent| *parent > 1 && parent != pid)
        })
    };
    // This doctor and whatever runs it are not strays; their other
    // descendants still can be.
    let exempt: HashSet<u32> = ancestry(std::process::id()).collect();
    let mut claimed = HashSet::new();
    if database.exists() {
        let store = loopflow_storage::sqlite::SqliteStore::open_read_only(
            database,
            loopflow_execution::store::token_key::encryption_key,
        )?;
        claimed.extend(store.recorded_lfs()?.into_iter().map(|lf| lf.pid));
        claimed.extend(
            store
                .agent_processes()?
                .into_iter()
                .filter_map(|agent| agent.pid),
        );
    }
    // A process works for the Machine its `LF_HOME` names, the account's own
    // when unset; another home's work is that home's to claim.
    let resolve = |home: &Path| std::fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
    let this_home = resolve(database.parent().unwrap_or(database));
    let account_home = loopflow_execution::store::production_database_path()
        .parent()
        .map(Path::to_path_buf)
        .expect("the account's database lives in its home");
    Ok(processes
        .into_iter()
        .filter(|process| !exempt.contains(&process.pid))
        .filter(|process| !ancestry(process.pid).any(|pid| claimed.contains(&pid)))
        .filter(|process| {
            let Some(found) = OsProcess::environment(process.pid, &STRAY_VARIABLES)
                .ok()
                .flatten()
            else {
                return false;
            };
            let mut home = account_home.clone();
            let mut identified = false;
            for (name, value) in found {
                if name != "LF_HOME" {
                    identified = true;
                } else if !value.is_empty() {
                    home = value.into();
                }
            }
            identified && resolve(&home) == this_home
        })
        .collect())
}

const CONTENTION_WINDOW_SECONDS: i64 = 86_400;

/// `history.db` grows with use: its size and oldest rows. A history row past
/// retention means no prune has run on this Machine.
fn check_history(database: &Path, now: i64) -> Check {
    const NAME: &str = "history";
    let path = loopflow_storage::history::history_path(database);
    let Ok(metadata) = std::fs::metadata(&path) else {
        return Check::ok(NAME, "history.db absent");
    };
    let retained = now - (loopflow_storage::sqlite::retention::HISTORY_DAYS + 2) * 86_400;
    let conn =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY);
    let oldest = |query: &str| -> Option<i64> {
        conn.as_ref()
            .ok()
            .and_then(|conn| conn.query_row(query, [], |row| row.get(0)).ok())
            .flatten()
    };
    let since = |oldest: Option<i64>| {
        oldest
            .and_then(|at| time::OffsetDateTime::from_unix_timestamp(at).ok())
            .map_or("empty".into(), |at| format!("oldest {}", at.date()))
    };
    let history = oldest("SELECT MIN(observed_at) FROM session_history");
    let turns = oldest("SELECT MIN(COALESCE(started_at,ended_at)) FROM turns");
    let stale = history.is_some_and(|at| at < retained);
    let detail = format!(
        "history.db {:.1} MB; Session history {}; turns {}",
        metadata.len() as f64 / 1_000_000.0,
        since(history),
        since(turns)
    );
    if stale {
        Check::warn(
            NAME,
            format!("{detail}; history is past retention: is the Machine server running? `lf machine prune` applies it now"),
        )
    } else {
        Check::ok(NAME, detail)
    }
}

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
        state => Check::warn(FRESHNESS, format!("cannot classify the running build: {state:?}")),
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
                match loopflow_execution::store::migrations::latest_applied_version_sqlite(
                    &connection,
                ) {
                    Ok(version) => latest_applied_migration = version,
                    Err(error) => migration_error = Some(error.to_string()),
                }
                // A private home migrates itself on its next open.
                let custom = loopflow_execution::store::custom_home_selected();
                let validation = loopflow_execution::store::migrations::validate_sqlite(&connection).and_then(|()| {
                    match loopflow_execution::store::migrations::pending_shared_migration(&connection)? {
                        Some(pending) if !custom => Err(loopflow_execution::store::StoreError::InvalidData(format!(
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
        unmerged_migrations: loopflow_execution::store::migrations::unmerged_migrations(),
        build_source_identity: crate::build_info::source_identity(),
        build_source_root: crate::build_info::source_root().map(|root| root.display().to_string()),
        build_source_revision: crate::build_info::source_revision().to_string(),
        database_path: path.display().to_string(),
        latest_known_migration: loopflow_execution::store::migrations::latest_known_version(),
        latest_applied_migration,
        migration_error,
    }
}

fn check_installation(database_path: &Path) -> Vec<Check> {
    let root = match loopflow_execution::installation::root() {
        Ok(root) => root,
        Err(error) => {
            return vec![Check::fail(
                "install-selection",
                format!("cannot resolve installation authority: {error}"),
            )]
        }
    };
    match loopflow_execution::installation::read_state(&root) {
        Ok(loopflow_execution::installation::InstallationState::Legacy) => vec![
            Check::warn(
                "install-selection",
                "no versioned installation receipt; the next promotion will initialize it",
            ),
            Check::warn(
                "install-fallback",
                "published fallback has not been retained by the installation path yet",
            ),
        ],
        Ok(loopflow_execution::installation::InstallationState::Switching(receipt)) => vec![Check::fail(
            "install-switch",
            format!(
                "install switch {} is unsettled at {:?}; ordinary startup falls back to the prior install — recover or rerun the promotion to finish it",
                receipt.id, receipt.phase
            ),
        )],
        Ok(loopflow_execution::installation::InstallationState::Settled(active)) => {
            let selected = loopflow_execution::installation::selection_for_current_executable();
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
            } else if loopflow_execution::store::custom_home_selected() {
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
                loopflow_execution::installation::ArtifactRole::Cli,

            ];
            if active
                .selection
                .artifact_set
                .artifact(&loopflow_execution::installation::ArtifactRole::App)
                .is_some()
            {
                fallback_roles.extend([
                    loopflow_execution::installation::ArtifactRole::App,
                    loopflow_execution::installation::ArtifactRole::AppHelper("lf".to_string()),

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
        Ok(state) => vec![Check::fail("install-selection", format!("unsupported installation state: {state:?}"))],
        Err(error) => vec![Check::fail("install-selection", error.to_string())],
    }
}

fn print_checks(store: &StoreReport, checks: &[Check]) {
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
    println!();
    for check in checks {
        let (mark, color) = match check.status {
            Status::Ok => ("ok  ", colors.green),
            Status::Warn => ("warn", colors.yellow),
            Status::Fail => ("FAIL", colors.red),
            _ => ("warn", colors.yellow),
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
    use super::{check_contention, inspect_store, pty_check, strays, Status};
    use loopflow_execution::process::OsProcess;
    use loopflow_execution::transport::PtyUsage;

    const STRAY_CHILD: &str = "LOOPFLOW_DOCTOR_STRAY_CHILD";

    /// Waits as a child of the stray test; does nothing in an ordinary run.
    #[test]
    fn stray_child() {
        if std::env::var_os(STRAY_CHILD).is_some() {
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    }

    /// Headroom is the room left under the limit; little of it warns.
    #[test]
    fn pty_headroom_counts_what_is_left_and_warns_when_little_is() {
        let usage = |in_use| PtyUsage {
            in_use,
            max: 511,
            limit: "kern.tty.ptmx_max",
        };
        let roomy = pty_check(usage(144));
        assert_eq!(roomy.status, Status::Ok);
        assert_eq!(
            roomy.detail,
            "144 of 511 PTYs in use (kern.tty.ptmx_max), 367 left; each Task conversation holds one"
        );
        assert_eq!(pty_check(usage(490)).status, Status::Warn);
        assert_eq!(pty_check(usage(511)).status, Status::Warn);
    }

    #[test]
    fn store_report_admits_a_newer_migration_tail() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        loopflow_execution::store::migrations::apply_sqlite(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at)
                 VALUES ('9.0.001_newer', unixepoch() + 1)",
                [],
            )
            .unwrap();
        drop(connection);

        let report = inspect_store(&path);

        assert_eq!(
            report.latest_applied_migration.as_deref(),
            Some("9.0.001_newer")
        );
        assert_eq!(report.migration_error, None);
    }

    #[test]
    fn a_process_carrying_a_session_that_nothing_claims_is_a_stray() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        // This test binary waits as the child: macOS hides the environment
        // of platform binaries such as /bin/sleep.
        let spawn = |session: Option<&str>, lf_home: &std::path::Path| {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "lf::commands::doctor::tests::stray_child",
                    "--nocapture",
                ])
                .env(STRAY_CHILD, "1")
                .env_remove("LF_STEER_SESSION")
                .env_remove("LF_PROCESS_ID")
                .env("LF_HOME", lf_home)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            if let Some(session) = session {
                command.env("LF_STEER_SESSION", session);
            }
            let child = command.spawn().unwrap();
            // The harness has started once its own arguments are readable.
            while OsProcess::arguments(child.id())
                .ok()
                .flatten()
                .is_none_or(|args| !args.iter().any(|arg| arg == "--exact"))
            {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            child
        };
        let mut stray = spawn(Some("session_gone"), home.path());
        let mut plain = spawn(None, home.path());
        let mut other_home = spawn(Some("session_gone"), elsewhere.path());
        let found: Vec<u32> = strays(&home.path().join("loopflow.db"))
            .unwrap()
            .iter()
            .map(|process| process.pid)
            .collect();
        for child in [&mut stray, &mut plain, &mut other_home] {
            child.kill().unwrap();
            child.wait().unwrap();
        }
        assert!(found.contains(&stray.id()), "{found:?}");
        assert!(!found.contains(&plain.id()), "{found:?}");
        assert!(!found.contains(&other_home.id()), "{found:?}");
    }

    #[test]
    fn contention_names_the_holding_binary_and_a_quiet_log_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("lf.log");
        assert_eq!(check_contention(&log, 0).status, Status::Ok);
        std::fs::write(
            &log,
            "2026-10-27T04:19:02.000000Z  WARN loopflow_execution::store::contention: held the store write lock \
             held_seconds=9.12 call_site=loopflow_execution::store::sqlite::retention::retain_history_in \
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
}
