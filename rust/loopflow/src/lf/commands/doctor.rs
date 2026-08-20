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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::{anyhow, Result};
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

use crate::lf::output::Colors;
use crate::store::RunEventRow;
use crate::trace::AgentInvocationRow;

/// A node value the current binary understands. `step` is the pre-054 spelling
/// of `skill`; rows carrying it are history the readers silently drop.
const NODES: [&str; 3] = ["run", "flow", "skill"];
const EVENTS: [&str; 4] = ["started", "completed", "errored", "escalated"];
const CAPTURE_RECOVERY_WINDOW_HOURS: i64 = 48;
const ORPHAN_PUBLICATION_GRACE_SECONDS: i64 = 5 * 60;
const MAX_CAPTURE_LOSS_DETAILS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
enum CaptureLossKind {
    Partial,
    Orphan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaptureLoss {
    kind: CaptureLossKind,
    at: i64,
    id: String,
    owner: String,
    provider: Option<String>,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CaptureEpisode {
    Healthy,
    ActiveLoss {
        latest_loss_at: i64,
    },
    Recovering {
        latest_loss_at: i64,
        recovery_started_at: i64,
    },
    Recovered {
        latest_loss_at: i64,
        recovery_started_at: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaptureStorage {
    available_bytes: Option<u64>,
    home_bytes: Option<u64>,
    trace_bytes: Option<u64>,
}

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
            let mut checks = audit(&events);
            checks.push(check_capture(&store, &events)?);
            checks.push(check_usage_coverage(&store)?);
            (events, checks)
        }
        Err(error) => {
            let detail = error.to_string();
            store_report.migration_error = Some(detail.clone());
            (Vec::new(), vec![Check::fail("store", detail)])
        }
    };
    // Binary freshness remains useful when the store cannot open.
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
        build_source_revision: crate::build_info::source_revision().to_string(),
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
    check_capture_at(
        store,
        events,
        OffsetDateTime::now_utc().unix_timestamp(),
        &_capture_storage,
    )
}

fn check_capture_at(
    store: &crate::store::sqlite::SqliteStore,
    events: &[RunEventRow],
    now: i64,
    storage: &dyn Fn() -> CaptureStorage,
) -> Result<Check> {
    let invocations = store.agent_invocations_since(0)?;
    let invocation_ids = invocations
        .iter()
        .map(|invocation| invocation.id.clone())
        .collect::<Vec<_>>();
    let turns = store.agent_turns_for_invocations(&invocation_ids)?;
    let turn_ids = turns.iter().map(|turn| turn.id.clone()).collect::<Vec<_>>();
    let assets = store.context_assets_for_turns(&turn_ids)?;

    let mut failures = Vec::new();
    let mut losses = Vec::new();
    let mut prompt_only = 0;
    let mut pruned = 0;
    let mut interrupted = 0;
    let mut lost = 0;
    for invocation in &invocations {
        if invocation.capture_status == "prompt_only" {
            // A durable supervised Invocation is reserved before its provider
            // starts. It deliberately owns no trace paths yet, so empty paths
            // are part of this state rather than unsafe capture evidence.
            prompt_only += 1;
            continue;
        }
        if invocation.capture_status == "pruned" {
            // Tombstoned by `lf runs reconcile`: the artifact is known-absent
            // and the absence is acknowledged. Counted, never a failure — must
            // short-circuit before the file-resolution checks below, which would
            // otherwise flag the known-absent file as a fresh failure.
            if invocation
                .incomplete_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
            {
                failures.push(format!("{} is pruned without a reason", invocation.id));
            }
            pruned += 1;
            continue;
        }
        let artifact_dir = crate::trace::resolve_artifact(&invocation.artifact_dir);
        let conversation_path = crate::trace::resolve_artifact(&invocation.conversation_path);
        let provider_events_path = invocation
            .provider_events_path
            .as_deref()
            .map(crate::trace::resolve_artifact)
            .transpose();
        if artifact_dir.is_err() || conversation_path.is_err() || provider_events_path.is_err() {
            failures.push(format!("{} has an unsafe artifact path", invocation.id));
        }
        if invocation.capture_status == "partial" {
            let reason = invocation
                .incomplete_reason
                .as_deref()
                .unwrap_or("reason unknown");
            if reason.trim().is_empty() {
                failures.push(format!("{} is partial without a reason", invocation.id));
            }
            losses.push(CaptureLoss {
                kind: CaptureLossKind::Partial,
                at: invocation.ended_at.unwrap_or(invocation.started_at),
                id: invocation.id.clone(),
                owner: _capture_owner(invocation),
                provider: Some(invocation.provider.clone()),
                reason: reason.to_string(),
            });
        }
        if matches!(invocation.capture_status.as_str(), "interrupted" | "lost") {
            if invocation.capture_status == "interrupted" {
                interrupted += 1;
                if invocation.outcome != "interrupted" {
                    failures.push(format!(
                        "{} is capture-interrupted but its invocation outcome is {}",
                        invocation.id, invocation.outcome
                    ));
                }
            } else {
                lost += 1;
            }
            if invocation
                .incomplete_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
            {
                failures.push(format!(
                    "{} is {} without a reason",
                    invocation.id, invocation.capture_status
                ));
            }
            if !artifact_dir.as_ref().is_ok_and(|path| path.is_dir())
                || !conversation_path.as_ref().is_ok_and(|path| path.is_file())
                || !provider_events_path
                    .as_ref()
                    .is_ok_and(|path| path.as_ref().is_none_or(|path| path.is_file()))
            {
                failures.push(format!(
                    "{} is {} but its retained capture is missing",
                    invocation.id, invocation.capture_status
                ));
            }
            if turns
                .iter()
                .any(|turn| turn.invocation_id == invocation.id && turn.status == "running")
            {
                failures.push(format!(
                    "{} is {} over a running turn",
                    invocation.id, invocation.capture_status
                ));
            }
        }
        if invocation.capture_status == "capturing"
            && events.iter().any(|event| {
                event.process_id == invocation.process_id
                    && event.node == "run"
                    && event.event != "started"
            })
        {
            failures.push(format!(
                "{} stayed capturing after its process ended",
                invocation.id
            ));
        }
        if invocation.capture_status == "complete" {
            let conversation_path = crate::trace::resolve_artifact(&invocation.conversation_path);
            let conversation_read = match &conversation_path {
                Ok(path) => {
                    crate::trace::read_conversation_status(path).map_err(|error| error.to_string())
                }
                Err(error) => Err(error.to_string()),
            };
            match conversation_read {
                Ok(read) => {
                    if read.incomplete_tail {
                        failures.push(format!("{} has an unterminated event tail", invocation.id));
                    }
                    if read
                        .events
                        .windows(2)
                        .any(|pair| pair[1].seq != pair[0].seq + 1)
                    {
                        failures.push(format!("{} has non-monotonic events", invocation.id));
                    }
                    if read.events.len() as i64 != invocation.conversation_event_count {
                        failures.push(format!("{} has a stale event count", invocation.id));
                    }
                }
                Err(error) => failures.push(format!("{}: {error}", invocation.id)),
            }
            if let Ok(path) = conversation_path {
                if std::fs::metadata(path)
                    .is_ok_and(|metadata| metadata.len() as i64 != invocation.conversation_bytes)
                {
                    failures.push(format!("{} has a stale byte count", invocation.id));
                }
            }
            if !turns.iter().any(|turn| {
                turn.invocation_id == invocation.id
                    && matches!(turn.status.as_str(), "completed" | "failed" | "interrupted")
            }) {
                failures.push(format!("{} has no terminal turn", invocation.id));
            }
        }
    }
    let known_artifacts: HashSet<&str> = invocations
        .iter()
        .map(|invocation| invocation.artifact_dir.as_str())
        .collect();
    let orphan_guard = now - ORPHAN_PUBLICATION_GRACE_SECONDS;
    for artifact in crate::trace::list_invocation_artifact_dirs()? {
        if !known_artifacts.contains(artifact.as_str()) {
            let path = crate::trace::resolve_artifact(&artifact)?;
            let (_, modified) = super::runs::directory_size_and_mtime(&path);
            let modified = _artifact_modified_at(&path, modified);
            if modified < orphan_guard {
                losses.push(CaptureLoss {
                    kind: CaptureLossKind::Orphan,
                    at: modified,
                    id: artifact.clone(),
                    owner: _orphan_owner(&artifact, events),
                    provider: None,
                    reason: "unclaimed trace artifact".to_string(),
                });
            }
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

    let mut terminal_counts = Vec::new();
    if pruned > 0 {
        terminal_counts.push(format!("{pruned} pruned"));
    }
    if interrupted > 0 {
        terminal_counts.push(format!("{interrupted} interrupted"));
    }
    if lost > 0 {
        terminal_counts.push(format!("{lost} lost"));
    }
    let terminal_clause = if terminal_counts.is_empty() {
        String::new()
    } else {
        format!(", {}", terminal_counts.join(", "))
    };
    let metrics = format!(
        "{} invocations, {} turns, {} assets, {} bytes{terminal_clause}",
        invocations.len(),
        turns.len(),
        assets.len(),
        invocations
            .iter()
            .map(|invocation| invocation.conversation_bytes)
            .sum::<i64>()
    );
    let episode = _classify_capture_episode(&losses, &invocations, now);
    if !failures.is_empty() {
        let mut detail = format!(
            "{} integrity failure(s): {}",
            failures.len(),
            failures
                .into_iter()
                .take(MAX_CAPTURE_LOSS_DETAILS)
                .collect::<Vec<_>>()
                .join("; ")
        );
        if episode != CaptureEpisode::Healthy {
            detail.push_str("; ");
            detail.push_str(&_format_capture_episode(
                &episode,
                &losses,
                matches!(
                    episode,
                    CaptureEpisode::ActiveLoss { .. } | CaptureEpisode::Recovering { .. }
                )
                .then(storage),
            ));
        }
        detail.push_str(&format!("; {metrics}"));
        return Ok(Check::fail("capture", detail));
    }
    if matches!(
        episode,
        CaptureEpisode::ActiveLoss { .. } | CaptureEpisode::Recovering { .. }
    ) {
        return Ok(Check::fail(
            "capture",
            format!(
                "{}; {metrics}",
                _format_capture_episode(&episode, &losses, Some(storage()))
            ),
        ));
    }
    if let CaptureEpisode::Recovered { .. } = episode {
        let detail = format!(
            "{}; {metrics}",
            _format_capture_episode(&episode, &losses, None)
        );
        return if prompt_only > 0 {
            Ok(Check::warn(
                "capture",
                format!("{detail}; {prompt_only} invocation(s) are prompt-only"),
            ))
        } else {
            Ok(Check::ok("capture", detail))
        };
    }
    if prompt_only > 0 {
        return Ok(Check::warn(
            "capture",
            format!("{metrics}; {prompt_only} invocation(s) are prompt-only"),
        ));
    }
    Ok(Check::ok("capture", metrics))
}

fn _classify_capture_episode(
    losses: &[CaptureLoss],
    invocations: &[AgentInvocationRow],
    now: i64,
) -> CaptureEpisode {
    let Some(latest_loss_at) = losses.iter().map(|loss| loss.at).max() else {
        return CaptureEpisode::Healthy;
    };
    let recovery_started_at = invocations
        .iter()
        .filter(|invocation| invocation.capture_status == "complete")
        .filter_map(|invocation| invocation.ended_at)
        .filter(|ended_at| *ended_at > latest_loss_at)
        .min();
    let Some(recovery_started_at) = recovery_started_at else {
        return CaptureEpisode::ActiveLoss { latest_loss_at };
    };
    if now < recovery_started_at + CAPTURE_RECOVERY_WINDOW_HOURS * 3600 {
        return CaptureEpisode::Recovering {
            latest_loss_at,
            recovery_started_at,
        };
    }
    CaptureEpisode::Recovered {
        latest_loss_at,
        recovery_started_at,
    }
}

fn _format_capture_episode(
    episode: &CaptureEpisode,
    losses: &[CaptureLoss],
    storage: Option<CaptureStorage>,
) -> String {
    let partial_count = losses
        .iter()
        .filter(|loss| loss.kind == CaptureLossKind::Partial)
        .count();
    let orphan_count = losses
        .iter()
        .filter(|loss| loss.kind == CaptureLossKind::Orphan)
        .count();
    let counts = format!(
        "{partial_count} partial capture(s), {orphan_count} unclaimed artifact(s) retained"
    );
    let mut detail = match episode {
        CaptureEpisode::Healthy => "capture healthy".to_string(),
        CaptureEpisode::ActiveLoss { latest_loss_at } => format!(
            "capture active loss: {counts}; latest {}; no complete capture after it",
            _format_timestamp(*latest_loss_at)
        ),
        CaptureEpisode::Recovering {
            latest_loss_at,
            recovery_started_at,
        } => format!(
            "capture recovering: {counts}; latest loss {}; complete capture {}; requires loss-free through {}",
            _format_timestamp(*latest_loss_at),
            _format_timestamp(*recovery_started_at),
            _format_timestamp(
                *recovery_started_at + CAPTURE_RECOVERY_WINDOW_HOURS * 3600
            )
        ),
        CaptureEpisode::Recovered {
            latest_loss_at,
            recovery_started_at,
        } => format!(
            "capture recovered: {counts}; latest loss {}; complete capture {}; {CAPTURE_RECOVERY_WINDOW_HOURS}h loss-free",
            _format_timestamp(*latest_loss_at),
            _format_timestamp(*recovery_started_at)
        ),
    };
    if matches!(
        episode,
        CaptureEpisode::ActiveLoss { .. } | CaptureEpisode::Recovering { .. }
    ) {
        let mut newest = losses.iter().collect::<Vec<_>>();
        newest.sort_by(|left, right| (right.at, &right.id).cmp(&(left.at, &left.id)));
        let observations = newest
            .into_iter()
            .take(MAX_CAPTURE_LOSS_DETAILS)
            .map(_format_capture_loss)
            .collect::<Vec<_>>()
            .join("; ");
        if !observations.is_empty() {
            detail.push_str(&format!("; {observations}"));
        }
    }
    if let Some(storage) = storage {
        detail.push_str(&format!("; {}", _format_capture_storage(&storage)));
    }
    detail
}

fn _format_capture_loss(loss: &CaptureLoss) -> String {
    let provider = loss
        .provider
        .as_deref()
        .map(|provider| format!(" via {provider}"))
        .unwrap_or_default();
    format!(
        "{} {} {}{provider}: {}",
        _format_timestamp(loss.at),
        loss.owner,
        loss.id,
        loss.reason
    )
}

fn _capture_owner(invocation: &AgentInvocationRow) -> String {
    if let Some(task) = invocation.task.as_deref() {
        return format!("task {task}");
    }
    if let Some(project) = invocation.project.as_deref() {
        return format!("project {project}");
    }
    if let Some(wave) = invocation.wave.as_deref() {
        return format!("wave {wave}");
    }
    if !invocation.worktree.is_empty() {
        return format!("worktree {}", invocation.worktree);
    }
    format!("repo {}", invocation.repo)
}

fn _orphan_owner(artifact: &str, events: &[RunEventRow]) -> String {
    let Some(run_id) =
        Path::new(artifact)
            .components()
            .next()
            .and_then(|component| match component {
                std::path::Component::Normal(value) => value.to_str(),
                _ => None,
            })
    else {
        return "owner unknown".to_string();
    };
    if let Some(wave) = events
        .iter()
        .filter(|event| event.run_id == run_id)
        .find_map(|event| event.wave.as_deref())
    {
        return format!("wave {wave}");
    }
    if let Some(worktree) = events
        .iter()
        .filter(|event| event.run_id == run_id)
        .find_map(|event| event.worktree.as_deref())
    {
        return format!("worktree {worktree}");
    }
    if let Some(repo) = events
        .iter()
        .filter(|event| event.run_id == run_id)
        .find_map(|event| event.repo.as_deref())
    {
        return format!("repo {repo}");
    }
    format!("run {run_id} owner unknown")
}

fn _artifact_modified_at(path: &Path, newest_file: i64) -> i64 {
    if newest_file > 0 {
        return newest_file;
    }
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_secs() as i64)
}

fn _capture_storage() -> CaptureStorage {
    let home = crate::store::lf_home_dir();
    let traces = crate::trace::trace_root();
    CaptureStorage {
        available_bytes: fs2::available_space(&home).ok(),
        home_bytes: _capture_directory_bytes(&home),
        trace_bytes: _capture_directory_bytes(&traces),
    }
}

fn _capture_directory_bytes(path: &Path) -> Option<u64> {
    std::fs::read_dir(path).ok()?;
    Some(super::runs::directory_size_and_mtime(path).0)
}

fn _format_capture_storage(storage: &CaptureStorage) -> String {
    let available = _format_optional_bytes(storage.available_bytes);
    format!(
        "storage {available} available, .lf {}, .lf/traces {}",
        _format_optional_bytes(storage.home_bytes),
        _format_optional_bytes(storage.trace_bytes)
    )
}

fn _format_optional_bytes(bytes: Option<u64>) -> String {
    bytes
        .map(_format_bytes)
        .unwrap_or_else(|| "unknown".to_string())
}

fn _format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn _format_timestamp(timestamp: i64) -> String {
    OffsetDateTime::from_unix_timestamp(timestamp)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_else(|| timestamp.to_string())
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
    ]
}

/// A invocation that finished capturing and recorded no provider measurement is a
/// invocation whose cost is lost. Spend lives only on turns now, so absent usage is
/// correctly `None` rather than a fictitious zero — which makes it invisible
/// unless something counts it. This is that count.
///
/// Scoped to `complete` invocations: `capturing` has not finished, `prompt_only`
/// never streamed, and `partial`/`pruned` already report themselves through
/// `check_capture`. The breakdown names providers because that is the actionable
/// unit — a provider reporting nothing is a mapping gap, not a quiet week.
fn check_usage_coverage(store: &crate::store::sqlite::SqliteStore) -> Result<Check> {
    let invocations: Vec<_> = store
        .agent_invocations_since(0)?
        .into_iter()
        .filter(|invocation| invocation.capture_status == "complete")
        .collect();
    if invocations.is_empty() {
        return Ok(Check::ok("usage", "no completed invocations recorded"));
    }
    let invocation_ids = invocations
        .iter()
        .map(|invocation| invocation.id.clone())
        .collect::<Vec<_>>();
    let measured: HashSet<String> = store
        .agent_turns_for_invocations(&invocation_ids)?
        .into_iter()
        .filter(|turn| turn.usage.is_some())
        .map(|turn| turn.invocation_id)
        .collect();

    let mut missing_by_provider: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for invocation in &invocations {
        let entry = missing_by_provider
            .entry(invocation.provider.as_str())
            .or_default();
        entry.1 += 1;
        if !measured.contains(&invocation.id) {
            entry.0 += 1;
        }
    }
    let covered = invocations
        .iter()
        .filter(|invocation| measured.contains(&invocation.id))
        .count();
    let total = invocations.len();
    if covered == total {
        return Ok(Check::ok(
            "usage",
            format!("{total} completed invocations all report provider usage"),
        ));
    }
    let breakdown = missing_by_provider
        .iter()
        .filter(|(_, (missing, _))| *missing > 0)
        .map(|(provider, (missing, seen))| format!("{provider} {missing}/{seen}"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(Check::warn(
        "usage",
        format!(
            "{covered}/{total} completed invocations report provider usage; missing: {breakdown}"
        ),
    ))
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
    use super::{
        _artifact_modified_at, _classify_capture_episode, _format_capture_storage, audit,
        check_capture, check_capture_at, check_continuity, check_usage_coverage, inspect_store,
        CaptureEpisode, CaptureLoss, CaptureLossKind, CaptureStorage, Status,
    };
    use crate::store::RunEventRow;
    use crate::trace::AgentInvocationRow;

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

    /// Drive a real capture to `complete`, returning its invocation id and the
    /// conversation artifact on disk. Uses the production write path so the
    /// tombstone tests act on genuinely-shaped rows.
    fn captured_invocation(
        guard: &crate::journal::TestLedgerGuard,
        skill: &str,
    ) -> (String, std::path::PathBuf) {
        captured_invocation_with_outcome(guard, skill, "completed")
    }

    fn captured_invocation_with_outcome(
        guard: &crate::journal::TestLedgerGuard,
        skill: &str,
        outcome: &str,
    ) -> (String, std::path::PathBuf) {
        let capture = crate::trace::CaptureHandle::begin(
            crate::trace::TraceCaptureContext {
                run_id: crate::id::TraceId::new(),
                process_id: crate::id::ExecId::new(),
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
                basis: None,
                supervision: None,
            },
        )
        .unwrap();
        capture.begin_turn("message", "follow up").unwrap();
        capture.finish(outcome, false).unwrap();

        let store = crate::journal::open_ledger().unwrap();
        let invocation = store
            .agent_invocations_since(0)
            .unwrap()
            .into_iter()
            .find(|invocation| invocation.skill.as_deref() == Some(skill))
            .expect("the capture we just drove must be in the ledger");
        assert_eq!(invocation.capture_status, "complete");
        let conversation = crate::trace::resolve_artifact(&invocation.conversation_path).unwrap();
        assert!(conversation.is_file());
        (invocation.id, conversation)
    }

    fn capturing_invocation(guard: &crate::journal::TestLedgerGuard, skill: &str) -> String {
        let capture = crate::trace::CaptureHandle::begin(
            crate::trace::TraceCaptureContext {
                run_id: crate::id::TraceId::new(),
                process_id: crate::id::ExecId::new(),
                repo: guard.home().to_path_buf(),
                worktree: guard.home().to_path_buf(),
                wave: Some("infrastructure".to_string()),
                project: None,
                task: Some("ENG-117".to_string()),
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
                basis: None,
                supervision: None,
            },
        )
        .unwrap();
        let invocation_id = capture.invocation_id().to_string();
        drop(capture);
        invocation_id
    }

    /// The same production path, but with the provider reporting what it
    /// measured — the shape a covered invocation has.
    fn captured_invocation_with_usage(guard: &crate::journal::TestLedgerGuard, skill: &str) {
        let capture = crate::trace::CaptureHandle::begin(
            crate::trace::TraceCaptureContext {
                run_id: crate::id::TraceId::new(),
                process_id: crate::id::ExecId::new(),
                repo: guard.home().to_path_buf(),
                worktree: guard.home().to_path_buf(),
                wave: Some("infrastructure".to_string()),
                project: None,
                task: None,
                flow: None,
                skill: Some(skill.to_string()),
            },
            crate::trace::PreparedTurnContext::from_prompts("system", "task"),
            crate::trace::CaptureStart {
                provider: "claude".to_string(),
                model: Some("opus".to_string()),
                surface: "headless".to_string(),
                input_op: "initial".to_string(),
                gather_ms: 1,
                render_ms: 2,
                raw_provider: true,
                basis: None,
                supervision: None,
            },
        )
        .unwrap();
        capture.record_conversation(crate::chat::types::ConversationEvent::UsageCheckpoint {
            turn_id: "turn-1".to_string(),
            usage: crate::chat::types::TurnUsage {
                input_tokens: Some(40),
                output_tokens: Some(5_197),
                ..Default::default()
            },
            final_receipt: true,
        });
        capture.finish("completed", false).unwrap();
    }

    fn invocation_at(id: &str, capture_status: &str, ended_at: Option<i64>) -> AgentInvocationRow {
        AgentInvocationRow {
            id: id.to_string(),
            run_id: format!("run-{id}"),
            answer_ask_id: None,
            process_id: format!("process-{id}"),
            started_at: ended_at.unwrap_or(0).saturating_sub(1),
            ended_at,
            repo: "/src/loopflow".to_string(),
            worktree: "/src/loopflow.task".to_string(),
            wave: Some("infrastructure".to_string()),
            flow: None,
            skill: Some("implement".to_string()),
            project: Some("stability-security".to_string()),
            task: Some("LOO-219".to_string()),
            provider: "codex".to_string(),
            model: Some("gpt-5".to_string()),
            surface: "headless".to_string(),
            capture_status: capture_status.to_string(),
            incomplete_reason: None,
            outcome: "completed".to_string(),
            artifact_dir: format!("run-{id}/process-{id}/{id}"),
            conversation_path: format!("run-{id}/process-{id}/{id}/conversation.jsonl"),
            provider_events_path: None,
            provider_session_id: None,
            provider_session_path: None,
            conversation_event_count: 0,
            conversation_bytes: 0,
            supervision: None,
        }
    }

    fn partial_loss(id: &str, at: i64) -> CaptureLoss {
        CaptureLoss {
            kind: CaptureLossKind::Partial,
            at,
            id: id.to_string(),
            owner: "task LOO-219".to_string(),
            provider: Some("codex".to_string()),
            reason: "No space left on device".to_string(),
        }
    }

    fn fixed_storage() -> CaptureStorage {
        CaptureStorage {
            available_bytes: Some(170 * 1024 * 1024 * 1024),
            home_bytes: Some(23 * 1024 * 1024 * 1024),
            trace_bytes: Some(11 * 1024 * 1024 * 1024),
        }
    }

    #[test]
    fn missing_storage_context_is_unknown_not_zero() {
        let detail = _format_capture_storage(&CaptureStorage {
            available_bytes: None,
            home_bytes: None,
            trace_bytes: None,
        });

        assert_eq!(
            detail,
            "storage unknown available, .lf unknown, .lf/traces unknown"
        );
    }

    #[test]
    fn recovery_requires_a_later_complete_capture_and_the_full_window() {
        let losses = vec![partial_loss("loss", 100)];

        assert_eq!(
            _classify_capture_episode(&losses, &[], 100 + 10 * DAY),
            CaptureEpisode::ActiveLoss {
                latest_loss_at: 100
            }
        );

        let invocations = vec![invocation_at("recovery", "complete", Some(200))];
        assert_eq!(
            _classify_capture_episode(&losses, &invocations, 200 + 48 * 3600 - 1),
            CaptureEpisode::Recovering {
                latest_loss_at: 100,
                recovery_started_at: 200,
            }
        );
        assert_eq!(
            _classify_capture_episode(&losses, &invocations, 200 + 48 * 3600),
            CaptureEpisode::Recovered {
                latest_loss_at: 100,
                recovery_started_at: 200,
            }
        );
    }

    #[test]
    fn historical_partial_is_immutable_and_a_recurrence_is_actionable() {
        let guard = crate::journal::TestLedgerGuard::new();
        let (historical_id, historical_file) = captured_invocation(&guard, "kickoff");
        let (recovery_id, _) = captured_invocation(&guard, "implement");
        let connection = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
        connection
            .execute(
                "UPDATE agent_invocations
                 SET started_at = 90, ended_at = 100, capture_status = 'partial',
                     incomplete_reason = 'No space left on device'
                 WHERE id = ?1",
                [&historical_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE agent_invocations SET started_at = 190, ended_at = 200 WHERE id = ?1",
                [&recovery_id],
            )
            .unwrap();
        drop(connection);
        let store = crate::journal::open_ledger().unwrap();
        let before = store
            .agent_invocations_since(0)
            .unwrap()
            .into_iter()
            .find(|invocation| invocation.id == historical_id)
            .unwrap();
        let artifact_before = std::fs::read(&historical_file).unwrap();

        let recovering =
            check_capture_at(&store, &[], 200 + 48 * 3600 - 1, &fixed_storage).unwrap();
        assert_eq!(recovering.status, Status::Fail, "{}", recovering.detail);
        assert!(recovering.detail.contains("capture recovering"));

        let recovered = check_capture_at(&store, &[], 200 + 48 * 3600, &fixed_storage).unwrap();
        assert_eq!(recovered.status, Status::Ok, "{}", recovered.detail);
        assert!(recovered.detail.contains("capture recovered"));
        assert!(recovered.detail.contains("1 partial capture(s)"));
        let after = store
            .agent_invocations_since(0)
            .unwrap()
            .into_iter()
            .find(|invocation| invocation.id == historical_id)
            .unwrap();
        assert_eq!(after, before);
        assert_eq!(std::fs::read(&historical_file).unwrap(), artifact_before);

        let (recurrence_id, _) = captured_invocation(&guard, "review");
        let connection = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
        connection
            .execute(
                "UPDATE agent_invocations
                 SET started_at = 499990, ended_at = 500000, capture_status = 'partial',
                     incomplete_reason = 'No space left on device'
                 WHERE id = ?1",
                [&recurrence_id],
            )
            .unwrap();
        drop(connection);

        let recurring = check_capture_at(&store, &[], 500010, &fixed_storage).unwrap();
        assert_eq!(recurring.status, Status::Fail, "{}", recurring.detail);
        for expected in [
            "capture active loss",
            "1970-01-06T18:53:20Z",
            "task W2-235",
            recurrence_id.as_str(),
            "via codex",
            "No space left on device",
            "170.0 GiB available",
            ".lf 23.0 GiB",
            "traces 11.0 GiB",
        ] {
            assert!(
                recurring.detail.contains(expected),
                "missing {expected:?}: {}",
                recurring.detail
            );
        }
    }

    #[test]
    fn prompt_only_reservation_does_not_claim_trace_paths() {
        let guard = crate::journal::TestLedgerGuard::new();
        let (invocation_id, _) = captured_invocation(&guard, "kickoff");
        let connection = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
        connection
            .execute(
                "UPDATE agent_invocations
                 SET capture_status = 'prompt_only', artifact_dir = '',
                     conversation_path = '', provider_events_path = NULL
                 WHERE id = ?1",
                [&invocation_id],
            )
            .unwrap();
        drop(connection);
        let store = crate::journal::open_ledger().unwrap();

        let check = check_capture_at(&store, &[], 1, &fixed_storage).unwrap();

        assert_eq!(check.status, Status::Warn, "{}", check.detail);
        assert!(check.detail.contains("1 invocation(s) are prompt-only"));
        assert!(!check.detail.contains("unsafe artifact path"));
    }

    #[test]
    fn an_unclaimed_artifact_has_only_a_bounded_publication_grace() {
        let guard = crate::journal::TestLedgerGuard::new();
        let artifact = guard.home().join("traces/run/process/invocation");
        std::fs::create_dir_all(&artifact).unwrap();
        std::fs::write(artifact.join("conversation.jsonl"), "{}\n").unwrap();
        let modified = _artifact_modified_at(
            &artifact,
            super::super::runs::directory_size_and_mtime(&artifact).1,
        );
        let store = crate::journal::open_ledger().unwrap();

        let publishing = check_capture_at(&store, &[], modified + 299, &fixed_storage).unwrap();
        assert_eq!(publishing.status, Status::Ok, "{}", publishing.detail);

        let orphaned = check_capture_at(&store, &[], modified + 301, &fixed_storage).unwrap();
        assert_eq!(orphaned.status, Status::Fail, "{}", orphaned.detail);
        assert!(orphaned.detail.contains("unclaimed trace artifact"));
        assert!(orphaned.detail.contains("run run owner unknown"));
    }

    /// Spend lives only on turns now, so a invocation that reported nothing is
    /// correctly `None` everywhere — invisible unless this check counts it. The
    /// provider breakdown is the actionable part: it is how a harness that stops
    /// reporting usage announces itself instead of silently costing nothing.
    #[test]
    fn a_completed_invocation_that_reported_no_usage_is_a_coverage_warning() {
        let guard = crate::journal::TestLedgerGuard::new();
        captured_invocation(&guard, "kickoff");
        let store = crate::journal::open_ledger().unwrap();

        let check = check_usage_coverage(&store).unwrap();

        assert_eq!(check.status, Status::Warn, "{}", check.detail);
        assert!(
            check.detail.contains("0/1") && check.detail.contains("codex 1/1"),
            "the warning must name the provider that reported nothing: {}",
            check.detail
        );
    }

    #[test]
    fn a_invocation_whose_provider_measured_the_turn_is_covered() {
        let guard = crate::journal::TestLedgerGuard::new();
        captured_invocation_with_usage(&guard, "implement");
        let store = crate::journal::open_ledger().unwrap();

        let check = check_usage_coverage(&store).unwrap();

        assert_eq!(check.status, Status::Ok, "{}", check.detail);
    }

    #[test]
    fn normal_terminal_outcomes_create_complete_resolvable_captures() {
        let guard = crate::journal::TestLedgerGuard::new();
        for outcome in ["completed", "failed", "interrupted"] {
            captured_invocation_with_outcome(&guard, outcome, outcome);
        }
        let store = crate::journal::open_ledger().unwrap();

        let invocations = store.agent_invocations_since(0).unwrap();
        assert_eq!(invocations.len(), 3);
        assert!(invocations
            .iter()
            .all(|invocation| invocation.capture_status == "complete"));
        assert_eq!(
            invocations
                .iter()
                .map(|invocation| invocation.outcome.as_str())
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from(["completed", "failed", "interrupted"])
        );
        let check = check_capture(&store, &[]).unwrap();
        assert_eq!(check.status, Status::Ok, "{}", check.detail);
    }

    #[test]
    fn a_tombstoned_capture_is_counted_while_fresh_loss_still_fails() {
        // The whole point of W2-235: acknowledged historical loss goes green
        // and stays visible as a count, but the surface must remain sensitive
        // to a capture that goes missing afterwards.
        let guard = crate::journal::TestLedgerGuard::new();
        let (historical, historical_file) = captured_invocation(&guard, "kickoff");
        let store = crate::journal::open_ledger().unwrap();
        assert_eq!(check_capture(&store, &[]).unwrap().status, Status::Ok);

        // The artifact vanishes out of band — the disk-reclaim shape that
        // produced the 235 dangling references on the release host.
        std::fs::remove_file(&historical_file).unwrap();
        let check = check_capture(&store, &[]).unwrap();
        assert_eq!(check.status, Status::Fail, "{}", check.detail);
        assert!(
            check.detail.contains("1 integrity failure(s)"),
            "{}",
            check.detail
        );

        // Acknowledge it the way `lf runs reconcile --apply` does.
        store
            .prune_invocation_capture(
                &historical,
                "conversation artifact absent at reconcile",
                500,
            )
            .unwrap();
        let check = check_capture(&store, &[]).unwrap();
        assert_eq!(check.status, Status::Ok, "{}", check.detail);
        assert!(check.detail.contains("1 pruned"), "{}", check.detail);

        // A *new* capture loss must still be a failure — un-acknowledged loss
        // is the actionable signal a red capture check is supposed to carry.
        let (_, fresh_file) = captured_invocation(&guard, "implement");
        std::fs::remove_file(&fresh_file).unwrap();
        let check = check_capture(&store, &[]).unwrap();
        assert_eq!(check.status, Status::Fail, "{}", check.detail);
        assert!(
            check.detail.contains("1 integrity failure(s)"),
            "{}",
            check.detail
        );
        assert!(check.detail.contains("1 pruned"), "{}", check.detail);
    }

    #[test]
    fn interrupting_an_intact_capture_closes_its_invocation_and_turn_atomically() {
        let guard = crate::journal::TestLedgerGuard::new();
        let invocation_id = capturing_invocation(&guard, "implement");
        let store = crate::journal::open_ledger().unwrap();

        store
            .interrupt_invocation_capture(
                &invocation_id,
                "capture interrupted; process ended without finalizing",
                500,
            )
            .unwrap();

        let invocation = store
            .agent_invocations_since(0)
            .unwrap()
            .into_iter()
            .find(|invocation| invocation.id == invocation_id)
            .unwrap();
        assert_eq!(invocation.capture_status, "interrupted");
        assert_eq!(invocation.outcome, "interrupted");
        assert_eq!(invocation.ended_at, Some(500));
        assert!(invocation
            .incomplete_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("process ended")));
        let turns = store
            .agent_turns_for_invocations(std::slice::from_ref(&invocation_id))
            .unwrap();
        assert!(!turns.is_empty());
        assert!(turns
            .iter()
            .all(|turn| { turn.status == "interrupted" && turn.ended_at == Some(500) }));

        let check = check_capture(&store, &[]).unwrap();
        assert_eq!(check.status, Status::Ok, "{}", check.detail);
        assert!(check.detail.contains("1 interrupted"), "{}", check.detail);
    }

    #[test]
    fn acknowledged_write_loss_stays_distinct_and_validates_retained_evidence() {
        let guard = crate::journal::TestLedgerGuard::new();
        let (invocation_id, conversation) = captured_invocation(&guard, "implement");
        let store = crate::journal::open_ledger().unwrap();
        let connection = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
        connection
            .execute(
                "UPDATE agent_invocations
                 SET capture_status = 'partial', incomplete_reason = 'ENOSPC while syncing'
                 WHERE id = ?1",
                [&invocation_id],
            )
            .unwrap();
        drop(connection);

        let check = check_capture(&store, &[]).unwrap();
        assert_eq!(check.status, Status::Fail, "{}", check.detail);
        assert!(
            check.detail.contains("1 partial capture(s)"),
            "{}",
            check.detail
        );
        assert!(
            check.detail.contains("ENOSPC while syncing"),
            "{}",
            check.detail
        );

        store.lose_invocation_capture(&invocation_id, 500).unwrap();
        let invocation = store
            .agent_invocations_since(0)
            .unwrap()
            .into_iter()
            .find(|invocation| invocation.id == invocation_id)
            .unwrap();
        assert_eq!(invocation.capture_status, "lost");
        assert_eq!(invocation.outcome, "completed");
        assert_eq!(
            invocation.incomplete_reason.as_deref(),
            Some("ENOSPC while syncing")
        );
        let check = check_capture(&store, &[]).unwrap();
        assert_eq!(check.status, Status::Ok, "{}", check.detail);
        assert!(check.detail.contains("1 lost"), "{}", check.detail);

        std::fs::remove_file(conversation).unwrap();
        let check = check_capture(&store, &[]).unwrap();
        assert_eq!(check.status, Status::Fail, "{}", check.detail);
        assert!(
            check.detail.contains("retained capture is missing"),
            "{}",
            check.detail
        );
    }

    #[test]
    fn a_fresh_directory_before_its_launch_row_is_not_an_orphan_failure() {
        let guard = crate::journal::TestLedgerGuard::new();
        let directory = guard
            .home()
            .join("traces/run-fresh/process-fresh/launch-fresh");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("conversation.jsonl"), "{\"seq\":0}\n").unwrap();
        let store = crate::journal::open_ledger().unwrap();

        let check = check_capture(&store, &[]).unwrap();

        assert_eq!(check.status, Status::Ok, "{}", check.detail);
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
}
