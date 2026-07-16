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
use time::{Duration, OffsetDateTime};

use crate::lf::output::Colors;
use crate::receipt::{EvidenceKind, PrIdentity, PrReference, Receipt};
use crate::store::RunEventRow;
use crate::wave::journal::{fold_thread, journal_path, memory_facts, read_events};

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
    database_path: String,
    latest_known_migration: String,
    latest_applied_migration: Option<String>,
    migration_error: Option<String>,
}

pub fn run(json: bool) -> Result<()> {
    let database_path = crate::store::database_path_from_env()?;
    let opened = crate::store::sqlite::SqliteStore::new(&database_path);
    let mut store_report = inspect_store(&database_path);
    let (events, checks) = match opened {
        Ok(store) => {
            let events = store.list_run_events_since(0)?;
            let mut checks = audit(&events);
            checks.push(check_capture(&store, &events)?);
            let (facts, known) = gather_receipt_audit(&store, &events);
            checks.push(check_receipts(&facts, &known));
            (events, checks)
        }
        Err(error) => {
            let detail = error.to_string();
            store_report.migration_error = Some(detail.clone());
            (Vec::new(), vec![Check::fail("store", detail)])
        }
    };
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
                if let Err(error) = crate::store::migrations::validate_sqlite(&connection) {
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
        database_path: path.display().to_string(),
        latest_known_migration: crate::store::migrations::latest_known_version(),
        latest_applied_migration,
        migration_error,
    }
}

fn check_capture(
    store: &crate::store::sqlite::SqliteStore,
    events: &[RunEventRow],
) -> Result<Check> {
    let required_after = store.trace_capture_required_after()?;
    let launches = store.agent_launches_since(0)?;
    let launch_ids = launches
        .iter()
        .map(|launch| launch.id.clone())
        .collect::<Vec<_>>();
    let turns = store.agent_turns_for_launches(&launch_ids)?;
    let turn_ids = turns.iter().map(|turn| turn.id.clone()).collect::<Vec<_>>();
    let assets = store.context_assets_for_turns(&turn_ids)?;

    let launch_processes: HashSet<&str> = launches
        .iter()
        .map(|launch| launch.process_id.as_str())
        .collect();
    let process_started_at = events
        .iter()
        .filter(|event| event.node == "run" && event.event == "started")
        .fold(HashMap::new(), |mut starts, event| {
            starts
                .entry(event.process_id.as_str())
                .and_modify(|started_at: &mut i64| *started_at = (*started_at).min(event.ts))
                .or_insert(event.ts);
            starts
        });
    let uncaptured_spend: BTreeSet<&str> = events
        .iter()
        .filter(|event| {
            process_started_at
                .get(event.process_id.as_str())
                .copied()
                .unwrap_or(event.ts)
                >= required_after
                && event.node == "run"
                && event.event != "started"
                && reports_provider_spend(event)
        })
        .map(|event| event.process_id.as_str())
        .filter(|process| !launch_processes.contains(process))
        .collect();

    let mut failures = Vec::new();
    let mut prompt_only = 0;
    let mut pruned = 0;
    for launch in &launches {
        if launch.capture_status == "pruned" {
            // Tombstoned by `lf runs reconcile`: the artifact is known-absent
            // and the absence is acknowledged. Counted, never a failure — must
            // short-circuit before the file-resolution checks below, which would
            // otherwise flag the known-absent file as a fresh failure.
            pruned += 1;
            continue;
        }
        if crate::trace::resolve_artifact(&launch.artifact_dir).is_err()
            || crate::trace::resolve_artifact(&launch.conversation_path).is_err()
            || launch
                .provider_events_path
                .as_deref()
                .is_some_and(|path| crate::trace::resolve_artifact(path).is_err())
        {
            failures.push(format!("{} has an unsafe artifact path", launch.id));
        }
        if launch.capture_status == "prompt_only" {
            prompt_only += 1;
        }
        if launch.capture_status == "partial" {
            failures.push(format!("{} is partial", launch.id));
        }
        if launch.capture_status == "capturing"
            && events.iter().any(|event| {
                event.process_id == launch.process_id
                    && event.node == "run"
                    && event.event != "started"
            })
        {
            failures.push(format!(
                "{} stayed capturing after its process ended",
                launch.id
            ));
        }
        if launch.capture_status == "complete" {
            let conversation_path = crate::trace::resolve_artifact(&launch.conversation_path);
            let conversation_read = match &conversation_path {
                Ok(path) => {
                    crate::trace::read_conversation_status(path).map_err(|error| error.to_string())
                }
                Err(error) => Err(error.to_string()),
            };
            match conversation_read {
                Ok(read) => {
                    if read.incomplete_tail {
                        failures.push(format!("{} has an unterminated event tail", launch.id));
                    }
                    if read
                        .events
                        .windows(2)
                        .any(|pair| pair[1].seq != pair[0].seq + 1)
                    {
                        failures.push(format!("{} has non-monotonic events", launch.id));
                    }
                    if read.events.len() as i64 != launch.conversation_event_count {
                        failures.push(format!("{} has a stale event count", launch.id));
                    }
                }
                Err(error) => failures.push(format!("{}: {error}", launch.id)),
            }
            if let Ok(path) = conversation_path {
                if std::fs::metadata(path)
                    .is_ok_and(|metadata| metadata.len() as i64 != launch.conversation_bytes)
                {
                    failures.push(format!("{} has a stale byte count", launch.id));
                }
            }
            if !turns.iter().any(|turn| {
                turn.launch_id == launch.id
                    && matches!(turn.status.as_str(), "completed" | "failed" | "interrupted")
            }) {
                failures.push(format!("{} has no terminal turn", launch.id));
            }
        }
    }
    let known_artifacts: HashSet<&str> = launches
        .iter()
        .map(|launch| launch.artifact_dir.as_str())
        .collect();
    for artifact in crate::trace::list_launch_artifact_dirs()? {
        if !known_artifacts.contains(artifact.as_str()) {
            failures.push(format!("orphan trace artifact {artifact}"));
        }
    }

    let mut assets_by_turn: HashMap<&str, Vec<&crate::trace::ContextAssetRow>> = HashMap::new();
    for asset in &assets {
        assets_by_turn
            .entry(asset.turn_id.as_str())
            .or_default()
            .push(asset);
    }
    for turn in &turns {
        if !crate::trace::resolve_artifact(&turn.task_prompt_path).is_ok_and(|path| path.is_file())
            || turn.system_prompt_path.as_deref().is_some_and(|path| {
                !crate::trace::resolve_artifact(path).is_ok_and(|path| path.is_file())
            })
        {
            failures.push(format!("{} is missing a prompt artifact", turn.id));
        }
        if turn.context_coverage == "unknown" {
            continue;
        }
        let Some(turn_assets) = assets_by_turn.get(turn.id.as_str()) else {
            failures.push(format!("{} has no context assets", turn.id));
            continue;
        };
        let system: u64 = turn_assets
            .iter()
            .filter(|row| row.asset.channel == crate::trace::ContextChannel::System)
            .map(|row| row.asset.attributed_tokens)
            .sum();
        let task: u64 = turn_assets
            .iter()
            .filter(|row| row.asset.channel == crate::trace::ContextChannel::Task)
            .map(|row| row.asset.attributed_tokens)
            .sum();
        if system as i64 != turn.system_tokens || task as i64 != turn.task_tokens {
            failures.push(format!("{} has mismatched asset tokens", turn.id));
        }
    }

    for process_id in uncaptured_spend {
        failures.push(format!(
            "process {process_id} reports provider spend but has no launch"
        ));
    }
    let pruned_clause = if pruned > 0 {
        format!(", {pruned} pruned")
    } else {
        String::new()
    };
    if !failures.is_empty() {
        return Ok(Check::fail(
            "capture",
            format!(
                "{} failure(s); {} launches, {} turns, {} bytes{pruned_clause}: {}",
                failures.len(),
                launches.len(),
                turns.len(),
                launches
                    .iter()
                    .map(|launch| launch.conversation_bytes)
                    .sum::<i64>(),
                failures.into_iter().take(4).collect::<Vec<_>>().join("; ")
            ),
        ));
    }
    if prompt_only > 0 {
        return Ok(Check::warn(
            "capture",
            format!(
                "{} launches and {} turns are consistent; {prompt_only} interactive launch(es) are prompt-only{pruned_clause}",
                launches.len(),
                turns.len()
            ),
        ));
    }
    Ok(Check::ok(
        "capture",
        format!(
            "{} launches, {} turns, {} assets, {} bytes{pruned_clause}",
            launches.len(),
            turns.len(),
            assets.len(),
            launches
                .iter()
                .map(|launch| launch.conversation_bytes)
                .sum::<i64>()
        ),
    ))
}

fn reports_provider_spend(event: &RunEventRow) -> bool {
    event.input_tokens.is_some()
        || event.output_tokens.is_some()
        || event.cache_read_tokens.is_some()
        || event.cost_usd.is_some()
}

pub fn audit(events: &[RunEventRow]) -> Vec<Check> {
    if events.is_empty() {
        return vec![Check::warn("continuity", "ledger is empty")];
    }
    let now = OffsetDateTime::now_utc().unix_timestamp();
    vec![
        check_continuity(events, now),
        check_vocabulary(events),
        check_attribution(events),
        check_identity(events),
        check_lineage(events),
        check_coverage(events),
    ]
}

/// Silence longer than this is reported. The 29.2-hour outage is the reason
/// the number exists; a day-granularity check missed it entirely, because the
/// silence began mid-day and ended mid-day and both days held rows.
const MAX_SILENCE_HOURS: f64 = 24.0;

/// A day the ledger recorded nothing is a day it may not have been listening —
/// but so is a long silence inside two busy days. Measure both.
fn check_continuity(events: &[RunEventRow], now: i64) -> Check {
    let days: BTreeSet<_> = events.iter().filter_map(|e| day_of(e.ts)).collect();
    let (Some(first), Some(last_event_day)) = (days.first(), days.last()) else {
        return Check::warn("continuity", "no timestamps");
    };
    let last = day_of(now)
        .map(|today| today.max(*last_event_day))
        .unwrap_or(*last_event_day);

    let mut gaps = Vec::new();
    let mut cursor = *first;
    while cursor < last {
        cursor += Duration::days(1);
        if cursor < last && !days.contains(&cursor) {
            gaps.push(cursor.to_string());
        }
    }

    let span = format!("{first} → {last}");
    if !gaps.is_empty() {
        return Check::fail(
            "continuity",
            format!("{} gap-day(s) in {span}: {}", gaps.len(), gaps.join(", ")),
        );
    }

    let silence = longest_silence_hours(events, now);
    if silence > MAX_SILENCE_HOURS {
        return Check::warn(
            "continuity",
            format!(
                "no gap-days ({span}), but {silence:.1}h of silence — was the ledger listening?"
            ),
        );
    }
    Check::ok(
        "continuity",
        format!("no gap-days ({span}); longest silence {silence:.1}h"),
    )
}

/// The largest interval between consecutive recorded events or between the
/// latest event and now, in hours. The tail matters most during an active
/// outage: until a later write succeeds, there is no second event to expose it.
fn longest_silence_hours(events: &[RunEventRow], now: i64) -> f64 {
    let mut stamps: Vec<i64> = events.iter().map(|event| event.ts).collect();
    stamps.push(now);
    stamps.sort_unstable();
    stamps
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .max()
        .unwrap_or(0) as f64
        / 3600.0
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

/// A parent must resolve to a process recorded in the same trace. The two ways
/// that fails have different owners — an absent parent is a writer that
/// exported identity it never recorded, a foreign one is a trace boundary
/// crossed — so the check counts them apart and names them. Reporting the sum
/// under one sentence once cost hours of archaeology to learn that all seven
/// were the same kind.
fn check_lineage(events: &[RunEventRow]) -> Check {
    let processes: HashMap<&str, &str> = events
        .iter()
        .map(|event| (event.process_id.as_str(), event.run_id.as_str()))
        .collect();
    let mut absent: BTreeSet<&str> = BTreeSet::new();
    let mut foreign: BTreeSet<&str> = BTreeSet::new();
    for event in events {
        let Some(parent) = event.parent_process_id.as_deref() else {
            continue;
        };
        match processes.get(parent).copied() {
            Some(run_id) if run_id == event.run_id => {}
            Some(_) => {
                foreign.insert(parent);
            }
            None => {
                absent.insert(parent);
            }
        }
    }
    if absent.is_empty() && foreign.is_empty() {
        return Check::ok("lineage", "every parent process resolves");
    }

    let mut parts = Vec::new();
    if !absent.is_empty() {
        parts.push(format!(
            "{} parent(s) absent from the ledger ({})",
            absent.len(),
            name_processes(&absent)
        ));
    }
    if !foreign.is_empty() {
        parts.push(format!(
            "{} parent(s) belong to another trace ({})",
            foreign.len(),
            name_processes(&foreign)
        ));
    }
    Check::fail("lineage", parts.join("; "))
}

/// Name the offenders so the failure is a starting point, not a count. Enough
/// ids to query; a cap so a systemic break stays one readable line.
fn name_processes(processes: &BTreeSet<&str>) -> String {
    const NAMED: usize = 3;
    let mut named = processes
        .iter()
        .take(NAMED)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if processes.len() > NAMED {
        named.push_str(&format!(", +{} more", processes.len() - NAMED));
    }
    named
}

/// A run that launched an agent and recorded no tokens is a run whose cost is
/// lost. Skill rows prove an agent ran; the terminal row should carry the spend.
fn check_coverage(events: &[RunEventRow]) -> Check {
    let agent_processes: HashSet<&str> = events
        .iter()
        .filter(|event| event.node == "skill" || event.provider.is_some())
        .map(|event| event.process_id.as_str())
        .collect();
    if agent_processes.is_empty() {
        return Check::ok("coverage", "no agent-bearing runs recorded");
    }

    let with_tokens: HashSet<&str> = events
        .iter()
        .filter(|e| e.node == "run" && e.event != "started" && e.input_tokens.is_some())
        .map(|event| event.process_id.as_str())
        .collect();
    let covered = agent_processes.intersection(&with_tokens).count();
    let total = agent_processes.len();
    let pct = (covered as f64 / total as f64) * 100.0;

    let detail = format!("{covered}/{total} agent-bearing runs carry tokens ({pct:.0}%)");
    if covered == total {
        Check::ok("coverage", detail)
    } else {
        Check::warn("coverage", detail)
    }
}

// ── Receipt sweep ───────────────────────────────────────────────────

/// Known ids for each evidence kind — the resolution surface the receipt
/// sweep checks against. Gathered from the store and wave journals; the check
/// itself is pure over these sets.
///
/// `unavailable` records kinds whose backing store or journal could not be read
/// this run. A receipt of an unavailable kind can't be judged resolved-or-orphaned
/// — the sweep reports it as *inaccessible* rather than silently calling it
/// orphaned, so a dropped read never masquerades as a missing record.
#[derive(Debug, Default)]
pub struct ReceiptKnownIds {
    pub chat_turn_ids: HashSet<String>,
    pub run_ids: HashSet<String>,
    pub trace_turn_ids: HashSet<String>,
    pub pm_ids: HashSet<String>,
    pub prs: Vec<PrIdentity>,
    pub unavailable: HashSet<EvidenceKind>,
    pub errors: Vec<String>,
}

impl ReceiptKnownIds {
    /// Record that an evidence kind's backing source could not be read, so its
    /// receipts are reported inaccessible (not orphaned) and the reason is
    /// surfaced in the check detail.
    fn mark_unavailable(&mut self, kind: EvidenceKind, error: impl Into<String>) {
        self.unavailable.insert(kind);
        self.errors.push(error.into());
    }
}

/// One memory fact paired with its claim wave, for the receipt sweep.
#[derive(Debug, Clone)]
pub struct ReceiptFact {
    pub wave: String,
    pub receipts: Vec<Receipt>,
}

/// Audit curated memory facts for receipt health: missing (zero receipts),
/// orphaned (reference resolves to no known record), and cross-wave (receipt
/// wave ≠ claim wave). Pure over the facts and known-id sets — testable
/// without a store.
///
/// During the post-contract grace window all findings are `warn`, not `fail`,
/// so pre-contract claims and cross-machine references don't break the gate.
/// Cross-wave is always `warn` — legitimate for child→parent citation.
/// How one receipt fared against the known-id sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resolution {
    /// The reference resolves to a known record.
    Resolved,
    /// The reference resolves to no known record (a real dangling pointer).
    Orphaned,
    /// The reference's evidence kind could not be read this run, so we can't
    /// judge it — surfaced, never silently counted as orphaned.
    Inaccessible,
}

fn check_receipts(facts: &[ReceiptFact], known: &ReceiptKnownIds) -> Check {
    if facts.is_empty() && known.errors.is_empty() {
        return Check::ok("receipts", "no memory facts to audit");
    }

    let mut missing = 0;
    let mut orphaned = 0;
    let mut inaccessible = 0;
    let mut cross_wave = 0;
    let mut total_receipts = 0;

    for fact in facts {
        if fact.receipts.is_empty() {
            missing += 1;
            continue;
        }
        for receipt in &fact.receipts {
            total_receipts += 1;
            if receipt.wave != fact.wave {
                cross_wave += 1;
            }
            match resolve_receipt(receipt, known) {
                Resolution::Resolved => {}
                Resolution::Orphaned => orphaned += 1,
                Resolution::Inaccessible => inaccessible += 1,
            }
        }
    }

    let total_facts = facts.len();
    let mut detail = format!(
        "{total_facts} fact(s) with {total_receipts} receipt(s): \
         {missing} missing, {orphaned} orphaned, {inaccessible} inaccessible, {cross_wave} cross-wave"
    );
    if !known.errors.is_empty() {
        detail.push_str(&format!(
            "; unreadable evidence: {}",
            known.errors.join("; ")
        ));
    }

    if missing > 0 || orphaned > 0 || inaccessible > 0 || cross_wave > 0 || !known.errors.is_empty()
    {
        Check::warn("receipts", detail)
    } else {
        Check::ok("receipts", detail)
    }
}

fn resolve_receipt(receipt: &Receipt, known: &ReceiptKnownIds) -> Resolution {
    if known.unavailable.contains(&receipt.kind) {
        return Resolution::Inaccessible;
    }
    let resolved = match receipt.kind {
        EvidenceKind::ChatTurn => known.chat_turn_ids.contains(&receipt.reference),
        EvidenceKind::WorkerReport => known.run_ids.contains(&receipt.reference),
        EvidenceKind::Trace => known.trace_turn_ids.contains(&receipt.reference),
        EvidenceKind::Pm => known.pm_ids.contains(&receipt.reference),
        EvidenceKind::Pr => PrReference::parse(&receipt.reference)
            .is_some_and(|target| known.prs.iter().any(|id| target.matches(id))),
    };
    if resolved {
        Resolution::Resolved
    } else {
        Resolution::Orphaned
    }
}

/// Gather memory facts (with their claim wave) and the known-id sets for each
/// evidence kind, from the store and every wave journal on this machine.
fn gather_receipt_audit(
    store: &crate::store::sqlite::SqliteStore,
    events: &[RunEventRow],
) -> (Vec<ReceiptFact>, ReceiptKnownIds) {
    let mut known = ReceiptKnownIds::default();
    let mut facts = Vec::new();

    known.run_ids = events.iter().map(|e| e.run_id.clone()).collect();

    // Trace turns: a failure to read launches or their turns leaves the set
    // unknown, so mark the whole kind inaccessible rather than judging every
    // trace receipt orphaned against an empty set.
    match store.agent_launches_since(0) {
        Ok(launches) => {
            let launch_ids: Vec<String> = launches.iter().map(|l| l.id.clone()).collect();
            match store.agent_turns_for_launches(&launch_ids) {
                Ok(turns) => known.trace_turn_ids = turns.iter().map(|t| t.id.clone()).collect(),
                Err(err) => {
                    known.mark_unavailable(EvidenceKind::Trace, format!("trace turns: {err}"))
                }
            }
        }
        Err(err) => known.mark_unavailable(EvidenceKind::Trace, format!("agent launches: {err}")),
    }

    match store.all_task_prs() {
        Ok(prs) => known.prs = prs.iter().filter_map(|pr| pr.pr_identity()).collect(),
        Err(err) => known.mark_unavailable(EvidenceKind::Pr, format!("task PRs: {err}")),
    }

    let waves = match store.list_waves(None) {
        Ok(waves) => waves,
        Err(err) => {
            // Without the wave list we can read neither journals (chat turns,
            // facts) nor PM snapshots — the sweep is blind for those kinds.
            known.mark_unavailable(EvidenceKind::ChatTurn, format!("wave list: {err}"));
            known.mark_unavailable(EvidenceKind::Pm, format!("wave list: {err}"));
            return (facts, known);
        }
    };

    for wave in &waves {
        let repo_root = Path::new(wave.repo());
        let journal = journal_path(repo_root, wave.name());
        if journal.exists() {
            let journal_events = read_events(&journal);
            for fact in memory_facts(&journal_events) {
                facts.push(ReceiptFact {
                    wave: wave.name().to_string(),
                    receipts: fact.receipts,
                });
            }
            let fold = fold_thread(&journal_events);
            known
                .chat_turn_ids
                .extend(fold.turns.iter().map(|t| t.id.clone()));
            known
                .chat_turn_ids
                .extend(fold.open.iter().map(|t| t.id.clone()));
        }

        match store.pm_snapshot(wave.repo(), wave.name()) {
            Ok(Some(snapshot)) => {
                match serde_json::from_str::<serde_json::Value>(&snapshot.payload) {
                    Ok(payload) => {
                        for key in ["items", "projects"] {
                            if let Some(arr) = payload.get(key).and_then(|v| v.as_array()) {
                                for entry in arr {
                                    if let Some(id) = entry.get("id").and_then(|v| v.as_str()) {
                                        known.pm_ids.insert(id.to_string());
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => known.mark_unavailable(
                        EvidenceKind::Pm,
                        format!("PM snapshot for wave '{}': {err}", wave.name()),
                    ),
                }
            }
            Ok(None) => {}
            Err(err) => known.mark_unavailable(
                EvidenceKind::Pm,
                format!("PM snapshot for wave '{}': {err}", wave.name()),
            ),
        }
    }

    (facts, known)
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
    use super::{audit, check_capture, check_continuity, inspect_store, Status};
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

    /// Drive a real capture to `complete`, returning its launch id and the
    /// conversation artifact on disk. Uses the production write path so the
    /// tombstone tests act on genuinely-shaped rows.
    fn captured_launch(
        guard: &crate::journal::TestLedgerGuard,
        skill: &str,
    ) -> (String, std::path::PathBuf) {
        let capture = crate::trace::CaptureHandle::begin(
            crate::trace::TraceCaptureContext {
                run_id: crate::id::RunId::new(),
                process_id: crate::id::ProcessId::new(),
                repo: guard.home().to_path_buf(),
                worktree: guard.home().to_path_buf(),
                wave: Some("infrastructure".to_string()),
                project: None,
                task: Some("W2-235".to_string()),
                flow: None,
                skill: Some(skill.to_string()),
            },
            crate::trace::PreparedTurnContext::from_prompts("system", "task"),
            crate::trace::CaptureStart {
                provider: "codex".to_string(),
                model: Some("gpt-5".to_string()),
                surface: "headless".to_string(),
                input_op: "initial".to_string(),
                gather_ms: 1,
                render_ms: 2,
                raw_provider: true,
            },
        )
        .unwrap();
        capture.begin_turn("message", "follow up").unwrap();
        capture.finish("completed", false).unwrap();

        let store = crate::journal::open_ledger().unwrap();
        let launch = store
            .agent_launches_since(0)
            .unwrap()
            .into_iter()
            .find(|launch| launch.skill.as_deref() == Some(skill))
            .expect("the capture we just drove must be in the ledger");
        assert_eq!(launch.capture_status, "complete");
        let conversation = crate::trace::resolve_artifact(&launch.conversation_path).unwrap();
        assert!(conversation.is_file());
        (launch.id, conversation)
    }

    #[test]
    fn a_tombstoned_capture_is_counted_while_fresh_loss_still_fails() {
        // The whole point of W2-235: acknowledged historical loss goes green
        // and stays visible as a count, but the surface must remain sensitive
        // to a capture that goes missing afterwards.
        let guard = crate::journal::TestLedgerGuard::new();
        let (historical, historical_file) = captured_launch(&guard, "kickoff");
        let store = crate::journal::open_ledger().unwrap();
        assert_eq!(check_capture(&store, &[]).unwrap().status, Status::Ok);

        // The artifact vanishes out of band — the disk-reclaim shape that
        // produced the 235 dangling references on the release host.
        std::fs::remove_file(&historical_file).unwrap();
        let check = check_capture(&store, &[]).unwrap();
        assert_eq!(check.status, Status::Fail, "{}", check.detail);
        assert!(check.detail.contains("1 failure(s)"), "{}", check.detail);

        // Acknowledge it the way `lf runs reconcile --apply` does.
        store
            .reconcile_launch_capture(
                &historical,
                "pruned",
                Some("conversation artifact absent at reconcile"),
                500,
            )
            .unwrap();
        let check = check_capture(&store, &[]).unwrap();
        assert_eq!(check.status, Status::Ok, "{}", check.detail);
        assert!(check.detail.contains("1 pruned"), "{}", check.detail);

        // A *new* capture loss must still be a failure — un-acknowledged loss
        // is the actionable signal a red capture check is supposed to carry.
        let (_, fresh_file) = captured_launch(&guard, "implement");
        std::fs::remove_file(&fresh_file).unwrap();
        let check = check_capture(&store, &[]).unwrap();
        assert_eq!(check.status, Status::Fail, "{}", check.detail);
        assert!(check.detail.contains("1 failure(s)"), "{}", check.detail);
        assert!(check.detail.contains("1 pruned"), "{}", check.detail);
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
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cost_usd: None,
            duration_secs: None,
            provider: None,
            model: None,
        }
    }

    #[test]
    fn capture_check_rejects_post_epoch_provider_spend_without_a_launch() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::open_ledger().unwrap();
        let mut event = row(
            "missing-capture",
            store.trace_capture_required_after().unwrap(),
            "run",
            "completed",
        );
        event.provider = Some("codex".to_string());
        event.input_tokens = Some(10);

        let check = check_capture(&store, &[event]).unwrap();
        assert_eq!(check.status, Status::Fail);
        assert!(
            check
                .detail
                .contains("process missing-capture reports provider spend but has no launch"),
            "{}",
            check.detail
        );
    }

    #[test]
    fn capture_check_ignores_ungated_orchestrators_and_external_hosts() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::open_ledger().unwrap();
        let required_after = store.trace_capture_required_after().unwrap();
        let orchestrator = row("orchestrator", required_after, "skill", "completed");
        let mut external_host = row("external", required_after, "run", "completed");
        external_host.provider = Some("codex".to_string());

        let check = check_capture(&store, &[orchestrator, external_host]).unwrap();
        assert_eq!(check.status, Status::Ok, "{}", check.detail);
    }

    #[test]
    fn capture_check_ignores_a_pre_epoch_process_that_finishes_after_activation() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::open_ledger().unwrap();
        let required_after = store.trace_capture_required_after().unwrap();
        let started = row("old-process", required_after - 1, "run", "started");
        let mut completed = row("old-process", required_after + 1, "run", "completed");
        completed.input_tokens = Some(10);

        let check = check_capture(&store, &[started, completed]).unwrap();
        assert_eq!(check.status, Status::Ok, "{}", check.detail);
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

    #[test]
    fn a_missing_day_is_a_failure() {
        // The 29-hour outage looked exactly like this: writes, silence, writes.
        let rows = [
            row("a", DAY, "run", "completed"),
            row("b", DAY * 3, "run", "completed"),
        ];
        let check = check_continuity(&rows, DAY * 3);
        assert_eq!(check.status, Status::Fail);
        assert!(check.detail.contains("1 gap-day"), "{}", check.detail);
    }

    #[test]
    fn a_long_silence_inside_two_busy_days_is_still_caught() {
        // The real 29.2h outage: rows early on day 1, rows late on day 2, no
        // missing day at all. A gap-day check calls this healthy. It is not.
        let rows = [
            row("a", DAY, "run", "completed"),              // day 1, 00:00
            row("b", DAY + 3600, "run", "completed"),       // day 1, 01:00
            row("c", DAY * 2 + 79_200, "run", "completed"), // day 2, 22:00
        ];
        let check = check_continuity(&rows, DAY * 2 + 79_200);
        assert_eq!(check.status, Status::Warn, "{}", check.detail);
        assert!(check.detail.contains("silence"), "{}", check.detail);
    }

    #[test]
    fn consecutive_days_have_no_gap() {
        let rows = [
            row("a", DAY, "run", "completed"),
            row("b", DAY + 3600, "run", "completed"),
            row("c", DAY * 2, "run", "completed"),
        ];
        assert_eq!(check_continuity(&rows, DAY * 2).status, Status::Ok);
    }

    #[test]
    fn an_active_silence_after_the_last_event_is_caught() {
        let rows = [
            row("a", DAY, "run", "completed"),
            row("b", DAY + 3600, "run", "completed"),
        ];

        let check = check_continuity(&rows, DAY + 26 * 3600);
        assert_eq!(check.status, Status::Warn, "{}", check.detail);
        assert!(
            check.detail.contains("25.0h of silence"),
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

    /// The seven production failures read as one sentence covering two very
    /// different owners. Say which one, and name it: the detail is where the
    /// next person starts.
    #[test]
    fn lineage_separates_an_absent_parent_from_a_foreign_one_and_names_both() {
        let mut elsewhere = row("trace-a", DAY, "run", "completed");
        elsewhere.process_id = "in-another-trace".to_string();
        let mut foreign_child = row("trace-b", DAY, "run", "completed");
        foreign_child.process_id = "child-b".to_string();
        foreign_child.parent_process_id = Some("in-another-trace".to_string());
        let mut orphan = row("trace-c", DAY, "run", "completed");
        orphan.process_id = "child-c".to_string();
        orphan.parent_process_id = Some("never-recorded".to_string());

        let checks = audit(&[elsewhere, foreign_child, orphan]);
        let lineage = checks
            .iter()
            .find(|check| check.name == "lineage")
            .expect("lineage check");

        assert_eq!(lineage.status, Status::Fail);
        assert!(
            lineage
                .detail
                .contains("1 parent(s) absent from the ledger")
                && lineage.detail.contains("never-recorded"),
            "an absent parent is named as absent: {}",
            lineage.detail
        );
        assert!(
            lineage
                .detail
                .contains("1 parent(s) belong to another trace")
                && lineage.detail.contains("in-another-trace"),
            "a foreign parent is named as foreign: {}",
            lineage.detail
        );
    }

    #[test]
    fn a_resolved_parent_inside_one_trace_passes_lineage() {
        let mut parent = row("trace-a", DAY, "run", "completed");
        parent.process_id = "parent".to_string();
        let mut child = row("trace-a", DAY, "run", "completed");
        child.process_id = "child".to_string();
        child.parent_process_id = Some("parent".to_string());
        assert_eq!(status_of(&[parent, child], "lineage"), Status::Ok);
    }

    #[test]
    fn an_agent_run_without_tokens_is_a_coverage_warning() {
        let rows = [
            row("a", DAY, "skill", "completed"),
            row("a", DAY, "run", "completed"),
        ];
        assert_eq!(status_of(&rows, "coverage"), Status::Warn);
    }

    #[test]
    fn an_inline_agent_with_provider_and_tokens_is_covered() {
        let mut terminal = row("a", DAY, "run", "completed");
        terminal.provider = Some("claude".to_string());
        terminal.input_tokens = Some(100);
        assert_eq!(status_of(&[terminal], "coverage"), Status::Ok);
    }

    // ── receipt sweep tests (pure over facts + known-id sets) ─────────

    use super::{check_receipts, ReceiptFact, ReceiptKnownIds};
    use crate::receipt::{EvidenceKind, PrIdentity, Receipt};

    fn receipt(kind: EvidenceKind, reference: &str, wave: &str) -> Receipt {
        Receipt::new(kind, reference, wave)
    }

    fn known(turn_ids: &[&str], run_ids: &[&str]) -> ReceiptKnownIds {
        ReceiptKnownIds {
            chat_turn_ids: turn_ids.iter().map(|s| s.to_string()).collect(),
            run_ids: run_ids.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn no_facts_is_ok() {
        let check = check_receipts(&[], &ReceiptKnownIds::default());
        assert_eq!(check.status, Status::Ok);
    }

    #[test]
    fn facts_with_resolving_receipts_are_ok() {
        let facts = vec![ReceiptFact {
            wave: "ship".to_string(),
            receipts: vec![
                receipt(EvidenceKind::ChatTurn, "turn-3", "ship"),
                receipt(EvidenceKind::WorkerReport, "run-9", "ship"),
            ],
        }];
        let known = known(&["turn-3"], &["run-9"]);
        let check = check_receipts(&facts, &known);
        assert_eq!(check.status, Status::Ok);
        assert!(check.detail.contains("1 fact(s)"), "{}", check.detail);
    }

    #[test]
    fn facts_with_zero_receipts_warn_during_grace() {
        let facts = vec![ReceiptFact {
            wave: "ship".to_string(),
            receipts: vec![],
        }];
        let check = check_receipts(&facts, &ReceiptKnownIds::default());
        assert_eq!(check.status, Status::Warn);
        assert!(check.detail.contains("1 missing"), "{}", check.detail);
    }

    #[test]
    fn orphaned_receipts_warn() {
        let facts = vec![ReceiptFact {
            wave: "ship".to_string(),
            receipts: vec![receipt(EvidenceKind::ChatTurn, "turn-99", "ship")],
        }];
        let known = known(&["turn-3"], &[]);
        let check = check_receipts(&facts, &known);
        assert_eq!(check.status, Status::Warn);
        assert!(check.detail.contains("1 orphaned"), "{}", check.detail);
    }

    #[test]
    fn cross_wave_receipts_warn() {
        let facts = vec![ReceiptFact {
            wave: "ship".to_string(),
            receipts: vec![receipt(EvidenceKind::ChatTurn, "turn-3", "product")],
        }];
        let known = known(&["turn-3"], &[]);
        let check = check_receipts(&facts, &known);
        assert_eq!(check.status, Status::Warn);
        assert!(check.detail.contains("1 cross-wave"), "{}", check.detail);
    }

    fn known_pr(repo: &str, number: u32, shas: &[&str]) -> ReceiptKnownIds {
        ReceiptKnownIds {
            prs: vec![PrIdentity {
                repo: repo.to_string(),
                number,
                shas: shas.iter().map(|s| s.to_string()).collect(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn pr_receipt_resolves_by_repo_and_number() {
        let facts = vec![ReceiptFact {
            wave: "ship".to_string(),
            receipts: vec![receipt(
                EvidenceKind::Pr,
                "loopflow/loopflow#912@abc123",
                "ship",
            )],
        }];
        let known = known_pr("loopflow/loopflow", 912, &["abc123"]);
        let check = check_receipts(&facts, &known);
        assert_eq!(check.status, Status::Ok);
    }

    #[test]
    fn pr_receipt_with_matching_number_but_wrong_repo_is_orphaned() {
        // A bare number scan would resolve this; matching repo + number rejects it.
        let facts = vec![ReceiptFact {
            wave: "ship".to_string(),
            receipts: vec![receipt(EvidenceKind::Pr, "other/repo#912", "ship")],
        }];
        let known = known_pr("loopflow/loopflow", 912, &[]);
        let check = check_receipts(&facts, &known);
        assert_eq!(check.status, Status::Warn);
        assert!(check.detail.contains("1 orphaned"), "{}", check.detail);
    }

    #[test]
    fn pr_receipt_with_unknown_number_is_orphaned() {
        let facts = vec![ReceiptFact {
            wave: "ship".to_string(),
            receipts: vec![receipt(EvidenceKind::Pr, "loopflow/loopflow#999", "ship")],
        }];
        let known = known_pr("loopflow/loopflow", 912, &[]);
        let check = check_receipts(&facts, &known);
        assert_eq!(check.status, Status::Warn);
        assert!(check.detail.contains("1 orphaned"), "{}", check.detail);
    }

    #[test]
    fn inaccessible_evidence_is_surfaced_not_orphaned() {
        // The store couldn't read task PRs, so a pr receipt is inaccessible —
        // reported and explained, never silently counted orphaned.
        let facts = vec![ReceiptFact {
            wave: "ship".to_string(),
            receipts: vec![receipt(EvidenceKind::Pr, "loopflow/loopflow#912", "ship")],
        }];
        let mut known = ReceiptKnownIds::default();
        known.mark_unavailable(EvidenceKind::Pr, "task PRs: database is locked");
        let check = check_receipts(&facts, &known);
        assert_eq!(check.status, Status::Warn);
        assert!(check.detail.contains("1 inaccessible"), "{}", check.detail);
        assert!(check.detail.contains("0 orphaned"), "{}", check.detail);
        assert!(
            check.detail.contains("database is locked"),
            "{}",
            check.detail
        );
    }

    #[test]
    fn unreadable_evidence_warns_even_with_no_facts() {
        // A wave-list failure yields zero facts; the sweep must still warn and
        // surface the error rather than reporting a clean "nothing to audit".
        let mut known = ReceiptKnownIds::default();
        known.mark_unavailable(EvidenceKind::ChatTurn, "wave list: connection refused");
        let check = check_receipts(&[], &known);
        assert_eq!(check.status, Status::Warn);
        assert!(
            check.detail.contains("connection refused"),
            "{}",
            check.detail
        );
    }
}
