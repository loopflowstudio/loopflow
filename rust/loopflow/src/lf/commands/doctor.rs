//! `lf doctor` — the ledger reports on itself.
//!
//! Every wave question is a query against the run ledger, so a ledger that is
//! wrong, deaf, or ambiguous makes every downstream answer confidently wrong.
//! These checks exist because each one failed silently at least once: a schema
//! drift dropped 29 hours of writes while `debug!` swallowed the error, a
//! column rename left `node='step'` and `node='skill'` meaning the same thing,
//! and the old process-grained run view once spliced one process's label onto
//! another's cost.
//!
//! Checks are pure functions of the rows, so they are tested without a store.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::{anyhow, Result};
use chrono::{Datelike, Local, LocalResult, NaiveDate, TimeZone, Utc};
use time::{Duration, OffsetDateTime};

use crate::lf::output::Colors;
use crate::ops::{CronObligation, CronSource};
use crate::store::RunEventRow;

/// A node value the current binary understands. `step` is the pre-054 spelling
/// of `skill`; rows carrying it are history the readers silently drop.
const NODES: [&str; 3] = ["run", "flow", "skill"];
const EVENTS: [&str; 4] = ["started", "completed", "errored", "escalated"];

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
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Ok,
            detail: detail.into(),
        }
    }
    fn warn(name: &'static str, detail: impl Into<String>) -> Self {
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

/// Exits non-zero when any check fails, so a cron can gate on it.
#[derive(Debug, serde::Serialize)]
struct DoctorReport<'a> {
    store: StoreReport,
    rows: usize,
    checks: &'a [Check],
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct StoreReport {
    build_provenance: crate::build_info::BuildProvenance,
    migration_authority: String,
    build_source_identity: String,
    build_source_root: Option<String>,
    build_source_revision: String,
    database_path: String,
    latest_known_migration: String,
    latest_applied_migration: Option<String>,
    migration_error: Option<String>,
}

pub fn run(json: bool) -> Result<()> {
    let database_path = crate::store::database_path_from_env()?;
    let opened = crate::store::sqlite::SqliteStore::new(&database_path);
    let mut store_report = inspect_store(&database_path);
    let (events, mut checks) = match opened {
        Ok(store) => {
            let events = store.list_run_events_since(0)?;
            let now = OffsetDateTime::now_utc().unix_timestamp();
            let checks = match crate::ops::default_launch_agents_dir()
                .and_then(|directory| crate::ops::list_cron_obligations(&directory))
            {
                Ok(obligations) => audit_at(&events, &obligations, now),
                Err(error) => {
                    let mut checks = audit_at(&events, &[], now);
                    let continuity = Check::fail(
                        "continuity",
                        format!("cannot read durable scheduler obligations: {error}"),
                    );
                    if let Some(existing) =
                        checks.iter_mut().find(|check| check.name == "continuity")
                    {
                        *existing = continuity;
                    } else {
                        checks.insert(0, continuity);
                    }
                    checks
                }
            };
            (events, checks)
        }
        Err(error) => {
            let detail = error.to_string();
            store_report.migration_error = Some(detail.clone());
            (Vec::new(), vec![Check::fail("store", detail)])
        }
    };
    // Binary freshness remains useful when the store cannot open.
    checks.extend(check_machine_install(&database_path));
    checks.push(check_binary_freshness());
    if json {
        println!(
            "{}",
            serde_json::to_string(&DoctorReport {
                store: store_report,
                rows: events.len(),
                checks: &checks,
            })?
        );
    } else {
        print_checks(&store_report, &checks, events.len());
    }

    if checks.iter().any(|check| check.status == Status::Fail) {
        return Err(anyhow!("run ledger audit failed"));
    }
    Ok(())
}

const FRESHNESS: &str = "binary-freshness";
const UPSTREAM: &str = "origin/main";
const UPSTREAM_REFSPEC: &str = "+refs/heads/main:refs/remotes/origin/main";

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

    if let Err(error) = crate::engine::git::fetch(&repo, "origin", UPSTREAM_REFSPEC) {
        return Check::warn(
            FRESHNESS,
            format!("cannot prove whether the running lf is current: could not refresh {UPSTREAM}: {error}"),
        );
    }

    match crate::build_info::classify_revision(revision, &repo, UPSTREAM) {
        crate::build_info::BuildFreshness::Current { revision } => Check::ok(
            FRESHNESS,
            format!(
                "running lf is built from {}, current with {UPSTREAM}",
                crate::build_info::short_revision(&revision)
            ),
        ),
        crate::build_info::BuildFreshness::Behind { revision, missing } => {
            let commits = missing
                .iter()
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
                    "running lf is built from {} and is {} merged commit(s) behind {UPSTREAM}, so \
                     these fixes are not running: {commits}. Rebuilding is an operator action; \
                     this check installs nothing",
                    crate::build_info::short_revision(&revision),
                    missing.len(),
                ),
            )
        }
        crate::build_info::BuildFreshness::OffMain { revision } => Check::ok(
            FRESHNESS,
            format!(
                "running lf is built from {}, which is not on {UPSTREAM}; nothing to compare",
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
                let validation = match crate::machine_install::selection_for_current_executable() {
                    Ok(Some(selection))
                        if selection.source
                            == crate::machine_install::InstallSource::Development =>
                    {
                        crate::store::migrations::validate_installed_development_sqlite(
                            &connection,
                            crate::build_info::migration_draft_manifest(),
                        )
                    }
                    _ => crate::store::migrations::validate_sqlite(&connection),
                };
                if let Err(error) = validation {
                    migration_error.get_or_insert_with(|| error.to_string());
                }
            }
            Err(error) => migration_error = Some(error.to_string()),
        }
    }
    StoreReport {
        build_provenance: crate::build_info::provenance(),
        migration_authority: match crate::build_info::migration_authority() {
            crate::build_info::MigrationAuthority::Published => "published",
            crate::build_info::MigrationAuthority::ValidationOnly => "validation-only",
        }
        .to_string(),
        build_source_identity: crate::build_info::source_identity(),
        build_source_root: crate::build_info::source_root().map(|root| root.display().to_string()),
        build_source_revision: crate::build_info::source_revision().to_string(),
        database_path: path.display().to_string(),
        latest_known_migration: crate::store::migrations::latest_known_version(),
        latest_applied_migration,
        migration_error,
    }
}

fn check_machine_install(database_path: &Path) -> Vec<Check> {
    let root = match crate::machine_install::root() {
        Ok(root) => root,
        Err(error) => {
            return vec![Check::fail(
                "install-selection",
                format!("cannot resolve machine install authority: {error}"),
            )]
        }
    };
    match crate::machine_install::read_state(&root) {
        Ok(crate::machine_install::MachineInstallState::Legacy) => vec![
            Check::warn(
                "install-selection",
                "no versioned machine install receipt; the next promotion will initialize it",
            ),
            Check::warn(
                "install-fallback",
                "published fallback has not been retained by the machine install path yet",
            ),
        ],
        Ok(crate::machine_install::MachineInstallState::Switching(receipt)) => vec![Check::fail(
            "install-switch",
            format!(
                "install switch {} is unsettled at {:?}; ordinary startup falls back to the prior install — recover or rerun the promotion to finish it",
                receipt.id, receipt.phase
            ),
        )],
        Ok(crate::machine_install::MachineInstallState::Settled(active)) => {
            let selected = crate::machine_install::selection_for_current_executable();
            let source_artifact = matches!(&selected, Ok(None));
            let selection = match &selected {
                Ok(Some(selection)) => Check::ok(
                    "install-selection",
                    format!(
                        "{:?} installation {} selects {}",
                        selection.source,
                        selection.installation_id,
                        selection.store.display()
                    ),
                ),
                Ok(None) => Check::ok(
                    "install-selection",
                    "running source artifact is outside the pinned machine installation",
                ),
                Err(error) => Check::fail("install-selection", error.to_string()),
            };
            let store = if active.selection.store == database_path {
                Check::ok(
                    "install-store",
                    format!("running store matches {}", database_path.display()),
                )
            } else if source_artifact {
                Check::ok(
                    "install-store",
                    "source artifact keeps its own isolated store",
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
                crate::machine_install::ArtifactRole::Cli,
                crate::machine_install::ArtifactRole::Daemon,
            ];
            if active
                .selection
                .artifact_set
                .artifact(&crate::machine_install::ArtifactRole::App)
                .is_some()
            {
                fallback_roles.extend([
                    crate::machine_install::ArtifactRole::App,
                    crate::machine_install::ArtifactRole::AppHelper("lf".to_string()),
                    crate::machine_install::ArtifactRole::AppHelper("lfd".to_string()),
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

pub fn audit(events: &[RunEventRow]) -> Vec<Check> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    audit_at(events, &[], now)
}

fn audit_at(events: &[RunEventRow], obligations: &[CronObligation], now: i64) -> Vec<Check> {
    if events.is_empty() && obligations.is_empty() {
        return vec![Check::warn("continuity", "ledger is empty")];
    }
    if events.is_empty() {
        return vec![check_continuity(events, obligations, now)];
    }
    vec![
        check_continuity(events, obligations, now),
        check_vocabulary(events),
        check_attribution(events),
        check_identity(events),
        check_lineage(events),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExpectedInterval {
    start: i64,
    end: i64,
}

fn check_continuity(events: &[RunEventRow], obligations: &[CronObligation], now: i64) -> Check {
    let gaps = ledger_gap_days(events, now);
    if obligations.is_empty() {
        return Check::ok(
            "continuity",
            format!(
                "no installed cron obligations; {}",
                format_gap_summary(&gaps, obligations)
            ),
        );
    }

    let mut due = 0usize;
    let mut satisfied = 0usize;
    let mut missing = Vec::new();
    for obligation in obligations {
        let Some(interval) = latest_due_interval(obligation, now) else {
            continue;
        };
        due += 1;
        let has_receipt = obligation.receipts.iter().any(|receipt| {
            receipt.source == CronSource::Scheduled
                && receipt.wave == obligation.wave
                && receipt.flow == obligation.flow
                && receipt.target_kind == obligation.target_kind
                && receipt.home_id == obligation.home_id
                && receipt.schedule == obligation.schedule.expression()
                && receipt.started_at >= interval.start
                && receipt.started_at < interval.end
        });
        if has_receipt {
            satisfied += 1;
            continue;
        }
        missing.push(format!(
            "{}/{} on Home {} expected interval {} ({}) has no scheduled receipt; inspect `lf cron history --wave {} --flow {} --days 2`",
            obligation.wave,
            obligation.flow,
            obligation.home_id,
            format_interval(interval),
            obligation.schedule.expression(),
            obligation.wave,
            obligation.flow,
        ));
    }

    let history = format_gap_summary(&gaps, obligations);
    if !missing.is_empty() {
        return Check::fail(
            "continuity",
            format!(
                "{} current scheduled obligation(s) missing: {}; {history}",
                missing.len(),
                missing.join("; ")
            ),
        );
    }
    Check::ok(
        "continuity",
        format!("{satisfied}/{due} current scheduled obligation(s) have receipts; {history}"),
    )
}

fn ledger_gap_days(events: &[RunEventRow], now: i64) -> Vec<time::Date> {
    let days: BTreeSet<_> = events.iter().filter_map(|e| day_of(e.ts)).collect();
    let (Some(first), Some(last_event_day)) = (days.first(), days.last()) else {
        return Vec::new();
    };
    let last = day_of(now)
        .map(|today| today.max(*last_event_day))
        .unwrap_or(*last_event_day);

    let mut gaps = Vec::new();
    let mut cursor = *first;
    while cursor < last {
        cursor += Duration::days(1);
        if cursor < last && !days.contains(&cursor) {
            gaps.push(cursor);
        }
    }
    gaps
}

fn format_gap_summary(gaps: &[time::Date], obligations: &[CronObligation]) -> String {
    if gaps.is_empty() {
        return "no historical ledger gap-days".to_string();
    }
    let first_activation_day = obligations
        .iter()
        .filter_map(|obligation| day_of(obligation.activated_at))
        .min();

    let mut summaries = Vec::new();
    let historical = if let Some(activation) = first_activation_day {
        let (before_activation, after_activation): (Vec<_>, Vec<_>) =
            gaps.iter().partition(|gap| **gap < activation);
        if !before_activation.is_empty() {
            summaries.push(format!(
                "{} historical ledger gap-day(s) predate first cron activation {activation}: {}",
                before_activation.len(),
                format_gap_dates(&before_activation),
            ));
        }
        after_activation
    } else {
        gaps.iter().collect()
    };
    if !historical.is_empty() {
        summaries.push(format!(
            "{} historical ledger gap-day(s) retained outside the current scheduled-receipt window: {}",
            historical.len(),
            format_gap_dates(&historical),
        ));
    }
    summaries.join("; ")
}

fn format_gap_dates(gaps: &[&time::Date]) -> String {
    gaps.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn latest_due_interval(obligation: &CronObligation, now: i64) -> Option<ExpectedInterval> {
    let start = scheduled_at_or_before(
        now,
        obligation.schedule.hour(),
        obligation.schedule.minute(),
    )?;
    if start < obligation.activated_at {
        return None;
    }
    let end = scheduled_after(
        start,
        obligation.schedule.hour(),
        obligation.schedule.minute(),
    )?;
    Some(ExpectedInterval { start, end })
}

fn scheduled_at_or_before(now: i64, hour: u32, minute: u32) -> Option<i64> {
    let mut date = Local.timestamp_opt(now, 0).single()?.date_naive();
    for _ in 0..370 {
        if let Some(timestamp) = scheduled_on(date, hour, minute) {
            if timestamp <= now {
                return Some(timestamp);
            }
        }
        date = date.pred_opt()?;
    }
    None
}

fn scheduled_after(timestamp: i64, hour: u32, minute: u32) -> Option<i64> {
    let mut date = Local.timestamp_opt(timestamp, 0).single()?.date_naive();
    for _ in 0..370 {
        date = date.succ_opt()?;
        if let Some(next) = scheduled_on(date, hour, minute) {
            if next > timestamp {
                return Some(next);
            }
        }
    }
    None
}

fn scheduled_on(date: NaiveDate, hour: u32, minute: u32) -> Option<i64> {
    match Local.with_ymd_and_hms(date.year(), date.month(), date.day(), hour, minute, 0) {
        LocalResult::Single(value) => Some(value.timestamp()),
        LocalResult::Ambiguous(first, _) => Some(first.timestamp()),
        LocalResult::None => None,
    }
}

fn format_interval(interval: ExpectedInterval) -> String {
    format!(
        "[{}, {})",
        format_local_timestamp(interval.start),
        format_local_timestamp(interval.end)
    )
}

fn format_local_timestamp(timestamp: i64) -> String {
    chrono::DateTime::<Utc>::from_timestamp(timestamp, 0)
        .map(|value| value.with_timezone(&Local).to_rfc3339())
        .unwrap_or_else(|| timestamp.to_string())
}

/// A half-landed rename leaves two spellings of one concept, and every query
/// grouping on it silently drops history.
fn check_vocabulary(events: &[RunEventRow]) -> Check {
    let mut unknown: HashMap<String, usize> = HashMap::new();
    for event in events {
        if !NODES.contains(&event.node.as_str()) {
            *unknown.entry(format!("node={}", event.node)).or_default() += 1;
        }
        if !EVENTS.contains(&event.event.as_str()) {
            *unknown.entry(format!("event={}", event.event)).or_default() += 1;
        }
    }
    if unknown.is_empty() {
        return Check::ok("vocabulary", "node and event values are all known");
    }
    let mut parts: Vec<_> = unknown
        .into_iter()
        .map(|(value, count)| format!("{value} ({count} rows)"))
        .collect();
    parts.sort();
    Check::fail(
        "vocabulary",
        format!("values outside the closed set: {}", parts.join(", ")),
    )
}

/// A process may name only one command, and its terminal row names that work.
fn check_attribution(events: &[RunEventRow]) -> Check {
    let mut commands: HashMap<&str, HashSet<&str>> = HashMap::new();
    let mut terminal = 0usize;
    let mut terminal_unnamed = 0usize;

    for event in events {
        if let Some(command) = event.command.as_deref() {
            commands
                .entry(&event.process_id)
                .or_default()
                .insert(command);
        }
        if event.node == "run" && event.event != "started" {
            terminal += 1;
            if event.command.is_none() && event.flow.is_none() && event.skill.is_none() {
                terminal_unnamed += 1;
            }
        }
    }

    let ambiguous = commands.values().filter(|set| set.len() > 1).count();
    if ambiguous == 0 && terminal_unnamed == 0 {
        return Check::ok("attribution", "every terminal row names its work");
    }
    Check::fail(
        "attribution",
        format!(
            "{ambiguous} process_id(s) carry >1 command; {terminal_unnamed}/{terminal} terminal rows name no command, flow, or skill"
        ),
    )
}

/// Repo identity is the absolute main-repo root, never a basename.
fn check_identity(events: &[RunEventRow]) -> Check {
    let repos: HashSet<Option<&str>> = events.iter().map(|event| event.repo.as_deref()).collect();
    let invalid = repos
        .iter()
        .filter(|repo| repo.is_none_or(|repo| !Path::new(repo).is_absolute()))
        .count();
    if invalid == 0 {
        return Check::ok(
            "identity",
            format!("{} repo value(s), all absolute", repos.len()),
        );
    }
    Check::fail(
        "identity",
        format!(
            "{invalid}/{} repo value(s) are missing or not absolute",
            repos.len()
        ),
    )
}

fn check_lineage(events: &[RunEventRow]) -> Check {
    let processes: HashMap<&str, &str> = events
        .iter()
        .map(|event| (event.process_id.as_str(), event.run_id.as_str()))
        .collect();
    let dangling: HashSet<&str> = events
        .iter()
        .filter_map(|event| {
            let parent = event.parent_process_id.as_deref()?;
            (processes.get(parent).copied() != Some(event.run_id.as_str())).then_some(parent)
        })
        .collect();
    if dangling.is_empty() {
        return Check::ok("lineage", "every parent process resolves");
    }
    Check::fail(
        "lineage",
        format!(
            "{} parent process id(s) are missing or belong to another trace",
            dangling.len()
        ),
    )
}

fn day_of(ts: i64) -> Option<time::Date> {
    OffsetDateTime::from_unix_timestamp(ts)
        .ok()
        .map(|dt| dt.date())
}

fn print_checks(store: &StoreReport, checks: &[Check], rows: usize) {
    let colors = Colors::default();
    println!(
        "build: {} ({}) · migrations {}",
        store.build_provenance, store.build_source_identity, store.migration_authority
    );
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
    println!("ledger: {rows} run events\n");
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
    use std::path::PathBuf;

    use super::{audit, check_continuity, inspect_store, latest_due_interval, Status};
    use crate::durable::{CronReceiptId, HomeId};
    use crate::ops::{
        parse_schedule, CronObligation, CronOutcome, CronReceipt, CronSource, CronTargetKind,
    };
    use crate::store::RunEventRow;

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

    fn row(run_id: &str, ts: i64, node: &str, event: &str) -> RunEventRow {
        RunEventRow {
            run_id: run_id.to_string(),
            process_id: run_id.to_string(),
            parent_process_id: None,
            seq: 0,
            ts,
            repo: Some("/src/loopflow".to_string()),
            worktree: None,
            wave: None,
            node: node.to_string(),
            event: event.to_string(),
            command: None,
            flow: None,
            skill: None,
            step_index: None,
            error: None,
        }
    }

    fn named(mut row: RunEventRow, command: &str) -> RunEventRow {
        row.command = Some(command.to_string());
        row
    }

    fn status_of(rows: &[RunEventRow], name: &str) -> Status {
        audit(rows)
            .into_iter()
            .find(|check| check.name == name)
            .expect("check exists")
            .status
    }

    fn timestamp(value: &str) -> i64 {
        time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
            .unwrap()
            .unix_timestamp()
    }

    fn obligation(activated_at: i64) -> CronObligation {
        CronObligation {
            wave: "infrastructure".to_string(),
            flow: "telemetry-daily".to_string(),
            target_kind: CronTargetKind::Flow,
            schedule: parse_schedule("0 0 9 * * *").unwrap(),
            home_id: HomeId::parse("home_11111111111111111111111111111111").unwrap(),
            activated_at,
            receipts: Vec::new(),
        }
    }

    fn receipt(obligation: &CronObligation, started_at: i64, source: CronSource) -> CronReceipt {
        CronReceipt {
            schema_version: 1,
            id: CronReceiptId::new(),
            runner_pid: 123,
            home_id: obligation.home_id.clone(),
            wave: obligation.wave.clone(),
            flow: obligation.flow.clone(),
            target_kind: obligation.target_kind,
            source,
            schedule: obligation.schedule.expression().to_string(),
            repo: PathBuf::from("/src/loopflow"),
            lf_path: PathBuf::from("/usr/local/bin/lf"),
            log_path: PathBuf::from("/src/loopflow/.lf/logs/cron.log"),
            started_at,
            finished_at: Some(started_at + 60),
            outcome: CronOutcome::Succeeded,
            exit_code: Some(0),
            error: None,
        }
    }

    #[test]
    fn august_ledger_gaps_predate_the_durable_cron_obligation() {
        let mut rows = vec![row(
            "august-03",
            timestamp("2026-08-03T12:00:00Z"),
            "run",
            "completed",
        )];
        for day in 12..=23 {
            rows.push(row(
                &format!("august-{day}"),
                timestamp(&format!("2026-08-{day:02}T12:00:00Z")),
                "run",
                "completed",
            ));
        }
        rows.push(row(
            "august-25",
            timestamp("2026-08-25T12:00:00Z"),
            "run",
            "completed",
        ));
        let now = timestamp("2026-08-25T23:00:00Z");
        let mut cron = obligation(timestamp("2026-08-22T16:00:00Z"));
        let interval = latest_due_interval(&cron, now).unwrap();
        cron.receipts
            .push(receipt(&cron, interval.start + 60, CronSource::Scheduled));

        let check = check_continuity(&rows, &[cron], now);

        assert_eq!(check.status, Status::Ok, "{}", check.detail);
        assert!(
            check
                .detail
                .contains("8 historical ledger gap-day(s) predate first cron activation"),
            "{}",
            check.detail
        );
        for day in 4..=11 {
            assert!(
                check.detail.contains(&format!("2026-08-{day:02}")),
                "{}",
                check.detail
            );
        }
        assert!(
            check.detail.contains(
                "1 historical ledger gap-day(s) retained outside the current scheduled-receipt window: 2026-08-24"
            ),
            "{}",
            check.detail
        );
    }

    #[test]
    fn a_missing_due_receipt_names_the_cron_home_interval_and_action() {
        let now = timestamp("2026-08-23T23:00:00Z");
        let cron = obligation(timestamp("2026-08-20T00:00:00Z"));

        let check = check_continuity(&[], &[cron], now);

        assert_eq!(check.status, Status::Fail, "{}", check.detail);
        for expected in [
            "infrastructure/telemetry-daily",
            "Home home_11111111111111111111111111111111",
            "expected interval [",
            "0 0 9 * * *",
            "has no scheduled receipt",
            "lf cron history --wave infrastructure --flow telemetry-daily --days 2",
        ] {
            assert!(
                check.detail.contains(expected),
                "missing {expected}: {}",
                check.detail
            );
        }
    }

    #[test]
    fn a_pre_activation_day_has_no_due_interval() {
        let now = timestamp("2026-08-23T23:00:00Z");
        let cron = obligation(now);

        let check = check_continuity(&[], &[cron], now);

        assert_eq!(check.status, Status::Ok, "{}", check.detail);
        assert!(
            check
                .detail
                .contains("0/0 current scheduled obligation(s) have receipts"),
            "{}",
            check.detail
        );
    }

    #[test]
    fn a_scheduled_failure_counts_but_a_manual_trigger_does_not() {
        let now = timestamp("2026-08-23T23:00:00Z");
        let mut cron = obligation(timestamp("2026-08-20T00:00:00Z"));
        let interval = latest_due_interval(&cron, now).unwrap();
        cron.receipts
            .push(receipt(&cron, interval.start + 60, CronSource::Manual));

        assert_eq!(
            check_continuity(&[], &[cron.clone()], now).status,
            Status::Fail
        );

        let mut scheduled = receipt(&cron, interval.start + 120, CronSource::Scheduled);
        scheduled.outcome = CronOutcome::Failed;
        scheduled.exit_code = Some(1);
        scheduled.error = Some("target failed".to_string());
        cron.receipts.push(scheduled);

        assert_eq!(check_continuity(&[], &[cron], now).status, Status::Ok);
    }

    #[test]
    fn a_later_receipt_restores_the_current_window_without_rewriting_history() {
        let now = timestamp("2026-08-23T23:00:00Z");
        let rows = vec![
            row(
                "before-gap",
                timestamp("2026-08-03T12:00:00Z"),
                "run",
                "completed",
            ),
            row(
                "after-gap",
                timestamp("2026-08-12T12:00:00Z"),
                "run",
                "completed",
            ),
        ];
        let original_rows = rows.clone();
        let mut cron = obligation(timestamp("2026-08-20T00:00:00Z"));
        let interval = latest_due_interval(&cron, now).unwrap();
        assert_eq!(
            check_continuity(&rows, &[cron.clone()], now).status,
            Status::Fail
        );

        cron.receipts
            .push(receipt(&cron, interval.start + 60, CronSource::Scheduled));
        let check = check_continuity(&rows, &[cron], now);

        assert_eq!(check.status, Status::Ok, "{}", check.detail);
        assert_eq!(rows, original_rows);
        assert!(
            check
                .detail
                .contains("historical ledger gap-day(s) retained"),
            "{}",
            check.detail
        );
    }

    #[test]
    fn a_half_landed_rename_is_caught() {
        // `step` is the pre-054 spelling of `skill`. Both in one ledger means
        // every query grouping on node silently drops history.
        let rows = [
            row("a", DAY, "run", "completed"),
            row("a", DAY, "step", "completed"),
        ];
        assert_eq!(status_of(&rows, "vocabulary"), Status::Fail);
    }

    #[test]
    fn one_process_carrying_two_commands_is_unattributable() {
        let rows = [
            named(row("shared", DAY, "run", "started"), r#"["lf","wave"]"#),
            named(row("shared", DAY, "run", "started"), r#"["lf","op","pm"]"#),
            row("shared", DAY, "run", "completed"),
        ];
        let check = audit(&rows)
            .into_iter()
            .find(|c| c.name == "attribution")
            .unwrap();
        assert_eq!(check.status, Status::Fail);
        assert!(check.detail.contains("1 process_id"), "{}", check.detail);
    }

    #[test]
    fn a_terminal_row_that_names_its_work_attributes_cleanly() {
        let mut terminal = row("a", DAY, "run", "completed");
        terminal.command = Some(r#"["lf","code"]"#.to_string());
        let rows = [
            named(row("a", DAY, "run", "started"), r#"["lf","code"]"#),
            terminal,
        ];
        assert_eq!(status_of(&rows, "attribution"), Status::Ok);
    }

    #[test]
    fn two_processes_in_one_trace_are_attributable() {
        let mut parent = named(row("shared", DAY, "run", "completed"), r#"["lf","wave"]"#);
        parent.process_id = "parent".to_string();
        let mut child = named(row("shared", DAY, "run", "completed"), r#"["lf","pm"]"#);
        child.process_id = "child".to_string();
        child.parent_process_id = Some("parent".to_string());
        assert_eq!(status_of(&[parent, child], "attribution"), Status::Ok);
    }

    #[test]
    fn a_repo_basename_fails_identity() {
        let mut event = row("a", DAY, "run", "completed");
        event.repo = Some("loopflow".to_string());
        assert_eq!(status_of(&[event], "identity"), Status::Fail);
    }

    #[test]
    fn a_dangling_parent_process_id_fails_the_doctor() {
        let mut event = row("a", DAY, "run", "completed");
        event.parent_process_id = Some("missing".to_string());
        assert_eq!(status_of(&[event], "lineage"), Status::Fail);
    }

    #[test]
    fn a_parent_from_another_trace_fails_lineage() {
        let mut parent = row("trace-a", DAY, "run", "completed");
        parent.process_id = "parent".to_string();
        let mut child = row("trace-b", DAY, "run", "completed");
        child.process_id = "child".to_string();
        child.parent_process_id = Some("parent".to_string());
        assert_eq!(status_of(&[parent, child], "lineage"), Status::Fail);
    }
}
